//! Gemini CLI adapter.
//!
//! Files under `~/.gemini`:
//!
//! * `.env` – Gemini CLI loads it on start-up. Key fields of a previous
//!   provider (`GOOGLE_*`, `GEMINI_API_KEY`, `GEMINI_MODEL`, auth-mechanism
//!   switches, see [`crate::floor::gemini_floor_env`]) are cleared, then
//!   `GEMINI_API_KEY`, `GOOGLE_GEMINI_BASE_URL` and `GEMINI_MODEL` are
//!   written. Other lines and comments are preserved in order.
//! * `settings.json` – `security.auth.selectedType = "gemini-api-key"` so the
//!   CLI does not prompt for Google login, and `model.name`.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::{detect_tool, home_dir, Adapter, ClientId, ConnectionSpec, CurrentConfig, Detection, PlannedFile, Validation};
use crate::error::Result;
use crate::floor;
use crate::fsutil;
use crate::patch::dotenv::{self, DotenvPatch};
use crate::patch::json::{self as jsonp, JsonPatch};
use crate::patch::{KeyPath, LivePatch};

const KEY_TOKEN: &str = "GEMINI_API_KEY";
const KEY_BASE: &str = "GOOGLE_GEMINI_BASE_URL";
const KEY_MODEL: &str = "GEMINI_MODEL";
const AUTH_TYPE: &str = "gemini-api-key";

pub struct GeminiAdapter {
    config_dir: PathBuf,
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new(home_dir().join(".gemini"))
    }
}

impl GeminiAdapter {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self { config_dir: config_dir.into() }
    }

    fn env_path(&self) -> PathBuf {
        self.config_dir.join(".env")
    }
    fn settings_path(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }

    fn read_env(&self) -> Result<(Option<Vec<u8>>, String)> {
        let pre = fsutil::read_optional(&self.env_path())?;
        let text = pre.as_deref().map(|b| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
        Ok((pre, text))
    }

    fn read_settings(&self) -> Result<(Option<Vec<u8>>, Value)> {
        let path = self.settings_path();
        let pre = fsutil::read_optional(&path)?;
        let (doc, _) = jsonp::parse(&path, pre.as_deref())?;
        Ok((pre, doc))
    }

    fn settings_patch(spec: &ConnectionSpec, doc: &Value) -> JsonPatch {
        let mut set = vec![
            (KeyPath::new(&["security", "auth", "selectedType"]), json!(AUTH_TYPE)),
            (KeyPath::new(&["model", "name"]), json!(spec.model)),
        ];
        // Pre-v0.3 key: keep it consistent only if the user still has it.
        if doc.get("selectedAuthType").is_some() {
            set.push((KeyPath::new(&["selectedAuthType"]), json!(AUTH_TYPE)));
        }
        JsonPatch { set, ..JsonPatch::default() }
    }
}

impl Adapter for GeminiAdapter {
    fn id(&self) -> ClientId {
        ClientId::Gemini
    }
    fn display_name(&self) -> &'static str {
        "Gemini CLI"
    }
    fn protocol(&self) -> &'static str {
        "gemini"
    }

    fn detect(&self) -> Detection {
        detect_tool("gemini", &[], &self.config_dir)
    }

    fn managed_paths(&self) -> Vec<PathBuf> {
        vec![self.env_path(), self.settings_path()]
    }

    fn read_config(&self) -> CurrentConfig {
        let env = self.read_env().map(|(_, t)| t).unwrap_or_default();
        let non_empty = |k: &str| dotenv::get(&env, k).filter(|v| !v.trim().is_empty());
        CurrentConfig {
            base_url: non_empty(KEY_BASE),
            model: non_empty(KEY_MODEL),
            has_credential: non_empty(KEY_TOKEN).is_some() || non_empty("GOOGLE_API_KEY").is_some(),
        }
    }

    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
        let (env_pre, _) = self.read_env()?;
        let env_patch = DotenvPatch {
            clear: Some(floor::gemini_floor_env),
            set: vec![
                (KEY_TOKEN.into(), spec.token.clone()),
                (KEY_BASE.into(), spec.base_url.clone()),
                (KEY_MODEL.into(), spec.model.clone()),
            ],
            ..DotenvPatch::default()
        };
        let env_content = env_patch.apply(&self.env_path(), env_pre.as_deref())?;

        let (settings_pre, doc) = self.read_settings()?;
        let settings_content = Self::settings_patch(spec, &doc).apply(&self.settings_path(), settings_pre.as_deref())?;

        Ok(vec![
            PlannedFile { path: self.env_path(), pre: env_pre, content: env_content, secret: true },
            PlannedFile { path: self.settings_path(), pre: settings_pre, content: settings_content, secret: false },
        ])
    }

    fn validate_config(&self, base_url: &str) -> Validation {
        let mut problems = Vec::new();
        match self.read_env() {
            Err(e) => problems.push(e.to_string()),
            Ok((_, env)) => {
                let url = dotenv::get(&env, KEY_BASE);
                if url.as_deref() != Some(base_url) {
                    problems.push(format!("{KEY_BASE} is {url:?}, expected {base_url:?}"));
                }
                if dotenv::get(&env, KEY_TOKEN).is_none_or(|t| t.trim().is_empty()) {
                    problems.push(format!("{KEY_TOKEN} is not set"));
                }
                if dotenv::get(&env, "GOOGLE_API_KEY").is_some() {
                    problems.push("GOOGLE_API_KEY is also set and would take precedence".into());
                }
            }
        }
        match self.read_settings() {
            Err(e) => problems.push(e.to_string()),
            Ok((_, doc)) => {
                let t = jsonp::value_at(&doc, &KeyPath::new(&["security", "auth", "selectedType"])).and_then(Value::as_str);
                if t != Some(AUTH_TYPE) {
                    problems.push(format!("security.auth.selectedType is {t:?}, expected {AUTH_TYPE:?}"));
                }
            }
        }
        Validation::fail(problems)
    }

    fn env_conflict_prefixes(&self) -> &'static [&'static str] {
        &["GEMINI_API_KEY", "GEMINI_MODEL", "GOOGLE_GEMINI_BASE_URL", "GOOGLE_API_KEY", "GOOGLE_CLOUD_"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testutil;

    #[test]
    fn closed_loop_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".gemini");
        let a = GeminiAdapter::new(&dir);
        testutil::closed_loop(
            &a,
            &[
                (&dir.join(".env"), "GEMINI_API_KEY=old\nGOOGLE_CLOUD_PROJECT=p\nGEMINI_SANDBOX=docker\n"),
                (&dir.join("settings.json"), r#"{"theme":"Default","selectedAuthType":"oauth-personal"}"#),
            ],
        );
    }

    #[test]
    fn closed_loop_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        testutil::closed_loop(&GeminiAdapter::new(tmp.path().join(".gemini")), &[]);
    }

    #[test]
    fn plan_clears_floor_and_sets_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(".env"), "GEMINI_API_KEY=old\nGOOGLE_CLOUD_PROJECT=p\nGEMINI_SANDBOX=docker\n").unwrap();
        std::fs::write(dir.join("settings.json"), "{\n  \"theme\": \"Default\",\n  \"selectedAuthType\": \"oauth-personal\"\n}\n").unwrap();
        let files = GeminiAdapter::new(&dir).plan(&testutil::spec()).unwrap();

        let env = String::from_utf8(files[0].content.clone()).unwrap();
        assert_eq!(
            env,
            "GEMINI_API_KEY=tk_test_1234567890abcdef\nGEMINI_SANDBOX=docker\nGOOGLE_GEMINI_BASE_URL=https://api.tokease.test/v1\nGEMINI_MODEL=code-best\n"
        );
        let settings = String::from_utf8(files[1].content.clone()).unwrap();
        let v: Value = serde_json::from_str(&settings).unwrap();
        assert_eq!(v["theme"], "Default");
        assert_eq!(v["selectedAuthType"], AUTH_TYPE);
        assert_eq!(v["security"]["auth"]["selectedType"], AUTH_TYPE);
        assert_eq!(v["model"]["name"], "code-best");
        assert!(settings.starts_with("{\n  \"theme\""), "indent and order kept:\n{settings}");
    }
}

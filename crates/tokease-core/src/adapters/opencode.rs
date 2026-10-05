//! OpenCode adapter.
//!
//! File: `$XDG_CONFIG_HOME/opencode/opencode.json` (default
//! `~/.config/opencode/opencode.json`). An existing `opencode.jsonc` is used
//! when it is the only file present; comments in that file make the parse
//! fail and the write is refused.
//!
//! Enabling adds a `provider.tokease` block (`@ai-sdk/openai-compatible`) and
//! sets `model` / `small_model` to the best and cheap logical models. Other
//! providers and the rest of the config stay.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    detect_tool, home_dir, Adapter, ClientId, ConnectionSpec, CurrentConfig, Detection,
    PlannedFile, Validation,
};
use crate::error::Result;
use crate::fsutil;
use crate::patch::json::{self as jsonp, JsonPatch};
use crate::patch::{KeyPath, LivePatch};
use crate::PROVIDER_ID;

pub struct OpenCodeAdapter {
    config_dir: PathBuf,
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home_dir().join(".config"));
        Self::new(config.join("opencode"))
    }
}

impl OpenCodeAdapter {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// `opencode.json`, or `opencode.jsonc` when that is the only one present.
    fn config_path(&self) -> PathBuf {
        let json = self.config_dir.join("opencode.json");
        let jsonc = self.config_dir.join("opencode.jsonc");
        if !json.exists() && jsonc.exists() {
            jsonc
        } else {
            json
        }
    }

    fn read_doc(&self) -> Result<(Option<Vec<u8>>, Value)> {
        let path = self.config_path();
        let pre = fsutil::read_optional(&path)?;
        let (doc, _) = jsonp::parse(&path, pre.as_deref())?;
        Ok((pre, doc))
    }

    fn str_at(doc: &Value, path: &[&str]) -> Option<String> {
        jsonp::value_at(doc, &KeyPath::new(path))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn patch(spec: &ConnectionSpec) -> JsonPatch {
        let set = |segments: &[&str], value: Value| (KeyPath::new(segments), value);
        JsonPatch {
            set: vec![
                set(&["model"], json!(format!("{PROVIDER_ID}/{}", spec.model))),
                set(
                    &["small_model"],
                    json!(format!("{PROVIDER_ID}/{}", spec.cheap())),
                ),
                set(
                    &["provider", PROVIDER_ID, "npm"],
                    json!("@ai-sdk/openai-compatible"),
                ),
                set(&["provider", PROVIDER_ID, "name"], json!("Tokease")),
                set(
                    &["provider", PROVIDER_ID, "options", "baseURL"],
                    json!(spec.base_url),
                ),
                set(
                    &["provider", PROVIDER_ID, "options", "apiKey"],
                    json!(spec.token),
                ),
                set(
                    &["provider", PROVIDER_ID, "models", spec.model.as_str(), "name"],
                    json!(spec.model),
                ),
                set(
                    &["provider", PROVIDER_ID, "models", spec.fast(), "name"],
                    json!(spec.fast()),
                ),
                set(
                    &["provider", PROVIDER_ID, "models", spec.cheap(), "name"],
                    json!(spec.cheap()),
                ),
            ],
            ..JsonPatch::default()
        }
    }
}

impl Adapter for OpenCodeAdapter {
    fn id(&self) -> ClientId {
        ClientId::OpenCode
    }
    fn display_name(&self) -> &'static str {
        "OpenCode"
    }
    fn protocol(&self) -> &'static str {
        "openai"
    }

    fn detect(&self) -> Detection {
        detect_tool("opencode", &[], &self.config_dir)
    }

    fn managed_paths(&self) -> Vec<PathBuf> {
        vec![self.config_path()]
    }

    fn read_config(&self) -> CurrentConfig {
        let Ok((_, doc)) = self.read_doc() else {
            return CurrentConfig::default();
        };
        CurrentConfig {
            base_url: Self::str_at(&doc, &["provider", PROVIDER_ID, "options", "baseURL"]),
            model: Self::str_at(&doc, &["model"]),
            has_credential: Self::str_at(&doc, &["provider", PROVIDER_ID, "options", "apiKey"])
                .is_some(),
        }
    }

    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
        let path = self.config_path();
        let pre = fsutil::read_optional(&path)?;
        let content = Self::patch(spec).apply(&path, pre.as_deref())?;
        Ok(vec![PlannedFile {
            path,
            pre,
            content,
            secret: true,
        }])
    }

    fn validate_config(&self, base_url: &str) -> Validation {
        let (_, doc) = match self.read_doc() {
            Ok(d) => d,
            Err(e) => return Validation::fail(vec![e.to_string()]),
        };
        let mut problems = Vec::new();
        let url = Self::str_at(&doc, &["provider", PROVIDER_ID, "options", "baseURL"]);
        if url.as_deref() != Some(base_url) {
            problems.push(format!(
                "provider.{PROVIDER_ID}.options.baseURL is {url:?}, expected {base_url:?}"
            ));
        }
        if Self::str_at(&doc, &["provider", PROVIDER_ID, "options", "apiKey"]).is_none() {
            problems.push(format!(
                "provider.{PROVIDER_ID}.options.apiKey is not set"
            ));
        }
        let model = Self::str_at(&doc, &["model"]);
        if !model
            .as_deref()
            .is_some_and(|m| m.starts_with(&format!("{PROVIDER_ID}/")))
        {
            problems.push(format!("model is {model:?}, expected {PROVIDER_ID}/…"));
        }
        Validation::fail(problems)
    }

    fn env_conflict_prefixes(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testutil;

    #[test]
    fn closed_loop_keeps_other_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("opencode");
        let path = dir.join("opencode.json");
        testutil::closed_loop(
            &OpenCodeAdapter::new(&dir),
            &[(
                path.as_path(),
                "{\n  \"theme\": \"system\",\n  \"provider\": {\n    \"anthropic\": { \"name\": \"Anthropic\" }\n  }\n}\n",
            )],
        );
    }
}

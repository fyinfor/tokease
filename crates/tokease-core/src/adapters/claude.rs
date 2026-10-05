//! Claude Code adapter.
//!
//! File: `$CLAUDE_CONFIG_DIR/settings.json` (default `~/.claude/settings.json`;
//! the pre-1.0 `claude.json` is used when it is the only one present).
//!
//! Enabling clears the key fields of whatever provider was active — every
//! `env.ANTHROPIC_*` / `AWS_*` / `VERTEX_REGION_*` key, the protocol
//! selectors (`CLAUDE_CODE_USE_BEDROCK`, …), OAuth tokens, and top-level
//! `apiKeyHelper` / `model` / `apiKey` and friends (see
//! [`crate::floor`]) — and writes:
//!
//! ```json
//! "env": {
//!   "ANTHROPIC_BASE_URL": "https://www.tokease.cn",
//!   "ANTHROPIC_AUTH_TOKEN": "…",
//!   "ANTHROPIC_MODEL": "code-best",
//!   "ANTHROPIC_DEFAULT_OPUS_MODEL": "code-best",
//!   "ANTHROPIC_DEFAULT_SONNET_MODEL": "code-fast",
//!   "ANTHROPIC_DEFAULT_HAIKU_MODEL": "code-cheap"
//! }
//! ```
//!
//! Only `ANTHROPIC_AUTH_TOKEN` is written (never `ANTHROPIC_API_KEY` too:
//! Claude Code warns when both are set). Permissions, hooks, plugins, theme,
//! … are preserved byte-for-byte in order.

use std::path::PathBuf;

use serde_json::{json, Value};

use super::{
    detect_tool, home_dir, Adapter, ClientId, ConnectionSpec, CurrentConfig, Detection,
    PlannedFile, Validation,
};
use crate::error::Result;
use crate::floor;
use crate::fsutil;
use crate::patch::json::{self as jsonp, ClearScope, JsonPatch};
use crate::patch::{KeyPath, LivePatch};

pub struct ClaudeAdapter {
    config_dir: PathBuf,
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join(".claude"));
        Self::new(dir)
    }
}

impl ClaudeAdapter {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// `settings.json`, or the legacy `claude.json` when that is the only
    /// settings file present (CC Switch `get_claude_settings_path`).
    fn settings_path(&self) -> PathBuf {
        let modern = self.config_dir.join("settings.json");
        let legacy = self.config_dir.join("claude.json");
        if !modern.exists() && legacy.exists() {
            legacy
        } else {
            modern
        }
    }

    fn read_doc(&self) -> Result<(Option<Vec<u8>>, Value)> {
        let path = self.settings_path();
        let pre = fsutil::read_optional(&path)?;
        let (doc, _) = jsonp::parse(&path, pre.as_deref())?;
        Ok((pre, doc))
    }

    fn env_str(doc: &Value, key: &str) -> Option<String> {
        jsonp::value_at(doc, &KeyPath::new(&["env", key]))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    }

    fn patch(spec: &ConnectionSpec) -> JsonPatch {
        let env = |key: &str, value: &str| (KeyPath::new(&["env", key]), json!(value));
        JsonPatch {
            clear: vec![
                ClearScope {
                    parent: KeyPath::root(),
                    is_floor: floor::claude_floor_top,
                },
                ClearScope {
                    parent: KeyPath::new(&["env"]),
                    is_floor: floor::claude_floor_env,
                },
            ],
            set: vec![
                env("ANTHROPIC_BASE_URL", &spec.base_url),
                env("ANTHROPIC_AUTH_TOKEN", &spec.token),
                env("ANTHROPIC_MODEL", &spec.model),
                env("ANTHROPIC_DEFAULT_OPUS_MODEL", &spec.model),
                env("ANTHROPIC_DEFAULT_SONNET_MODEL", spec.fast()),
                env("ANTHROPIC_DEFAULT_HAIKU_MODEL", spec.cheap()),
            ],
            ..JsonPatch::default()
        }
    }
}

impl Adapter for ClaudeAdapter {
    fn id(&self) -> ClientId {
        ClientId::Claude
    }
    fn display_name(&self) -> &'static str {
        "Claude Code"
    }
    fn protocol(&self) -> &'static str {
        "anthropic"
    }

    fn detect(&self) -> Detection {
        detect_tool("claude", &[], &self.config_dir)
    }

    fn managed_paths(&self) -> Vec<PathBuf> {
        vec![self.settings_path()]
    }

    fn read_config(&self) -> CurrentConfig {
        let Ok((_, doc)) = self.read_doc() else {
            return CurrentConfig::default();
        };
        let has_credential = Self::env_str(&doc, "ANTHROPIC_AUTH_TOKEN").is_some()
            || Self::env_str(&doc, "ANTHROPIC_API_KEY").is_some()
            || Self::env_str(&doc, "CLAUDE_CODE_OAUTH_TOKEN").is_some()
            || doc.get("apiKeyHelper").is_some();
        CurrentConfig {
            base_url: Self::env_str(&doc, "ANTHROPIC_BASE_URL"),
            model: Self::env_str(&doc, "ANTHROPIC_MODEL"),
            has_credential,
        }
    }

    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
        let path = self.settings_path();
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
        let url = Self::env_str(&doc, "ANTHROPIC_BASE_URL");
        if url.as_deref() != Some(base_url) {
            problems.push(format!(
                "env.ANTHROPIC_BASE_URL is {url:?}, expected {base_url:?}"
            ));
        }
        if Self::env_str(&doc, "ANTHROPIC_AUTH_TOKEN").is_none() {
            problems.push("env.ANTHROPIC_AUTH_TOKEN is not set".into());
        }
        if Self::env_str(&doc, "ANTHROPIC_API_KEY").is_some() {
            problems.push(
                "env.ANTHROPIC_API_KEY is also set; Claude Code warns when both credentials exist"
                    .into(),
            );
        }
        for key in floor::CLAUDE_PROTOCOL_SELECTORS {
            if Self::env_str(&doc, key).is_some_and(|v| v != "0" && v != "false") {
                problems.push(format!(
                    "env.{key} is set and would bypass ANTHROPIC_BASE_URL"
                ));
            }
        }
        if doc.get("apiKeyHelper").is_some() {
            problems.push("apiKeyHelper is set and would override the token".into());
        }
        Validation::fail(problems)
    }

    fn env_conflict_prefixes(&self) -> &'static [&'static str] {
        &["ANTHROPIC_"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testutil;

    const SEED: &str = r#"{
  "apiKeyHelper": "~/bin/key.sh",
  "env": {
    "ANTHROPIC_BASE_URL": "https://old.example",
    "ANTHROPIC_API_KEY": "old",
    "CLAUDE_CODE_USE_BEDROCK": "1",
    "AWS_REGION": "us-east-1",
    "FOO": "bar",
    "CLAUDE_CODE_USE_POWERSHELL_TOOL": "1"
  },
  "permissions": { "allow": ["Bash(ls)"] },
  "model": "opus",
  "theme": "dark"
}
"#;

    #[test]
    fn closed_loop_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude");
        let a = ClaudeAdapter::new(&dir);
        testutil::closed_loop(&a, &[(&dir.join("settings.json"), SEED)]);
    }

    #[test]
    fn closed_loop_fresh() {
        let tmp = tempfile::tempdir().unwrap();
        let a = ClaudeAdapter::new(tmp.path().join(".claude"));
        testutil::closed_loop(&a, &[]);
    }

    #[test]
    fn plan_clears_floor_and_keeps_user_keys_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), SEED).unwrap();
        let files = ClaudeAdapter::new(&dir).plan(&testutil::spec()).unwrap();
        let text = String::from_utf8(files[0].content.clone()).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();

        assert!(v.get("apiKeyHelper").is_none());
        assert!(
            v.get("model").is_none(),
            "previous provider's /model choice cleared"
        );
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["permissions"]["allow"][0], "Bash(ls)");
        let env = v["env"].as_object().unwrap();
        assert_eq!(env["FOO"], "bar");
        assert_eq!(
            env["CLAUDE_CODE_USE_POWERSHELL_TOOL"], "1",
            "feature switch is not a key field"
        );
        assert!(env.get("CLAUDE_CODE_USE_BEDROCK").is_none());
        assert!(env.get("AWS_REGION").is_none());
        assert!(
            env.get("ANTHROPIC_API_KEY").is_none(),
            "only ANTHROPIC_AUTH_TOKEN is written"
        );
        assert_eq!(env["ANTHROPIC_BASE_URL"], "https://api.tokease.test/v1");
        assert_eq!(env["ANTHROPIC_AUTH_TOKEN"], "tk_test_1234567890abcdef");
        assert_eq!(env["ANTHROPIC_MODEL"], "code-best");
        assert_eq!(env["ANTHROPIC_DEFAULT_OPUS_MODEL"], "code-best");
        assert_eq!(env["ANTHROPIC_DEFAULT_SONNET_MODEL"], "code-fast");
        assert_eq!(env["ANTHROPIC_DEFAULT_HAIKU_MODEL"], "code-cheap");
        // Order: ANTHROPIC_BASE_URL rewritten in place stays first; untouched
        // keys keep their position; new keys are appended.
        let keys: Vec<&String> = env.keys().collect();
        assert_eq!(keys[0], "ANTHROPIC_BASE_URL");
        assert_eq!(keys[1], "FOO");
        assert!(text.ends_with("}\n"), "trailing newline preserved");
    }

    #[test]
    fn legacy_claude_json_is_used_when_it_is_the_only_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("claude.json"), "{}").unwrap();
        let a = ClaudeAdapter::new(&dir);
        assert_eq!(a.managed_paths(), vec![dir.join("claude.json")]);
        std::fs::write(dir.join("settings.json"), "{}").unwrap();
        assert_eq!(a.managed_paths(), vec![dir.join("settings.json")]);
    }
}

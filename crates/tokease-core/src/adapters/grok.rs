//! Grok Build adapter.
//!
//! File: `$GROK_HOME/config.toml` (default `~/.grok/config.toml`).
//!
//! Enabling points the default model at a BYOK table named `tokease` and
//! leaves every other model, MCP server, and UI setting in place. The
//! credential is the inline `api_key` (Grok also accepts `env_key`; that key
//! is removed on this table so a shell variable cannot override the token we
//! just wrote). `api_backend` is `responses` unless the platform sends
//! `chat_completions` or `messages`.

use std::path::PathBuf;

use toml_edit::{DocumentMut, Value};

use super::{
    detect_tool, home_dir, Adapter, ClientId, ConnectionSpec, CurrentConfig, Detection,
    PlannedFile, Validation,
};
use crate::error::Result;
use crate::fsutil;
use crate::patch::toml::{self as tomlp, put_value, str_at};
use crate::patch::PatchError;
use crate::PROVIDER_ID;

pub struct GrokAdapter {
    config_dir: PathBuf,
}

impl Default for GrokAdapter {
    fn default() -> Self {
        let dir = std::env::var_os("GROK_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| home_dir().join(".grok"));
        Self::new(dir)
    }
}

impl GrokAdapter {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    fn read_doc(&self) -> Result<(Option<Vec<u8>>, DocumentMut)> {
        let path = self.config_path();
        let pre = fsutil::read_optional(&path)?;
        let doc = tomlp::parse(&path, pre.as_deref())?;
        Ok((pre, doc))
    }

    fn backend(spec: &ConnectionSpec) -> &str {
        match spec.wire_api.as_deref() {
            Some(b @ ("responses" | "chat_completions" | "messages")) => b,
            _ => "responses",
        }
    }

    fn apply_patch(doc: &mut DocumentMut, spec: &ConnectionSpec) -> std::result::Result<(), PatchError> {
        let path = PathBuf::from("config.toml");
        {
            let root = doc.as_table_mut();
            let models = tomlp::ensure_table_mut(&path, root, &["models"])?;
            put_value(models, "default", Value::from(PROVIDER_ID));
        }
        {
            let root = doc.as_table_mut();
            let model = tomlp::ensure_table_mut(&path, root, &["model", PROVIDER_ID])?;
            put_value(model, "model", Value::from(spec.model.as_str()));
            put_value(model, "base_url", Value::from(spec.base_url.as_str()));
            put_value(model, "name", Value::from("Tokease"));
            put_value(model, "api_key", Value::from(spec.token.as_str()));
            put_value(model, "api_backend", Value::from(Self::backend(spec)));
            model.remove("env_key");
        }
        Ok(())
    }
}

impl Adapter for GrokAdapter {
    fn id(&self) -> ClientId {
        ClientId::Grok
    }
    fn display_name(&self) -> &'static str {
        "Grok"
    }
    fn protocol(&self) -> &'static str {
        "openai"
    }

    fn detect(&self) -> Detection {
        detect_tool("grok", &[], &self.config_dir)
    }

    fn managed_paths(&self) -> Vec<PathBuf> {
        vec![self.config_path()]
    }

    fn read_config(&self) -> CurrentConfig {
        let Ok((_, doc)) = self.read_doc() else {
            return CurrentConfig::default();
        };
        let root = doc.as_table();
        CurrentConfig {
            base_url: str_at(root, &["model", PROVIDER_ID, "base_url"]),
            model: str_at(root, &["model", PROVIDER_ID, "model"]),
            has_credential: str_at(root, &["model", PROVIDER_ID, "api_key"])
                .is_some_and(|s| !s.trim().is_empty()),
        }
    }

    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
        let path = self.config_path();
        let pre = fsutil::read_optional(&path)?;
        let mut doc = tomlp::parse(&path, pre.as_deref())?;
        Self::apply_patch(&mut doc, spec)?;
        let content = tomlp::serialize(&doc, pre.as_deref());
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
        let root = doc.as_table();
        let mut problems = Vec::new();
        let url = str_at(root, &["model", PROVIDER_ID, "base_url"]);
        if url.as_deref() != Some(base_url) {
            problems.push(format!(
                "model.{PROVIDER_ID}.base_url is {url:?}, expected {base_url:?}"
            ));
        }
        if str_at(root, &["model", PROVIDER_ID, "api_key"]).is_none() {
            problems.push(format!("model.{PROVIDER_ID}.api_key is not set"));
        }
        if str_at(root, &["models", "default"]).as_deref() != Some(PROVIDER_ID) {
            problems.push(format!("models.default is not {PROVIDER_ID}"));
        }
        Validation::fail(problems)
    }

    fn env_conflict_prefixes(&self) -> &'static [&'static str] {
        &[
            "XAI_API_KEY",
            "GROK_DEFAULT_MODEL",
            "GROK_XAI_API_BASE_URL",
            "GROK_MODELS_BASE_URL",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::testutil;

    #[test]
    fn closed_loop_keeps_other_models() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".grok");
        let path = dir.join("config.toml");
        testutil::closed_loop(
            &GrokAdapter::new(&dir),
            &[(
                path.as_path(),
                "[ui]\ntheme = \"auto\"\n\n[model.grok-4]\nmodel = \"grok-4.7\"\nbase_url = \"https://api.x.ai/v1\"\n",
            )],
        );
    }
}

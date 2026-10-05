//! Claude Desktop adapter.
//!
//! Separate from the Claude CLI. Desktop reads a third-party profile, not
//! `~/.claude/settings.json`. Enabling switches both config files to 3P mode
//! and writes the Tokease gateway into the profile CC Switch also uses as the
//! shape (see Anthropic's Claude Desktop 3P config):
//!
//! * `claude_desktop_config.json` in the normal and `Claude-3p` directories:
//!   `deploymentMode = "3p"` (every other key stays)
//! * `Claude-3p/configLibrary/<profile id>.json`: gateway URL, bearer token,
//!   and the three Desktop role ids (Opus / Sonnet / Haiku). The menu label
//!   is the Tokease logical model. Desktop rejects any model id outside those
//!   role families, so the id on the wire is the role id, not `code-best`.
//! * `Claude-3p/configLibrary/_meta.json`: this profile becomes `appliedId`.
//!   Other profile entries are kept.
//!
//! Paths follow the Desktop install: macOS `~/Library/Application Support`,
//! Windows `%LOCALAPPDATA%`, Linux `$XDG_CONFIG_HOME` or `~/.config`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{
    detect_tool, home_dir, Adapter, ClientId, ConnectionSpec, CurrentConfig, Detection,
    PlannedFile, Validation,
};
use crate::error::Result;
use crate::fsutil;
use crate::patch::json::{self as jsonp};

/// Stable profile id owned by Tokease. Distinct from CC Switch's id so the
/// two apps do not overwrite each other's profile file.
const PROFILE_ID: &str = "00000000-0000-4000-8000-00000000e45e";
const PROFILE_NAME: &str = "Tokease";
const CONFIG_FILE: &str = "claude_desktop_config.json";

/// Role ids Claude Desktop's model menu accepts. The label is the logical model.
const ROLE_OPUS: &str = "claude-opus-5";
const ROLE_SONNET: &str = "claude-sonnet-4-6";
const ROLE_HAIKU: &str = "claude-haiku-4-5";

pub struct ClaudeDesktopAdapter {
    normal_dir: PathBuf,
    threep_dir: PathBuf,
}

impl Default for ClaudeDesktopAdapter {
    fn default() -> Self {
        let (normal, threep) = default_dirs();
        Self::new(normal, threep)
    }
}

impl ClaudeDesktopAdapter {
    pub fn new(normal_dir: impl Into<PathBuf>, threep_dir: impl Into<PathBuf>) -> Self {
        Self {
            normal_dir: normal_dir.into(),
            threep_dir: threep_dir.into(),
        }
    }

    fn normal_config(&self) -> PathBuf {
        self.normal_dir.join(CONFIG_FILE)
    }
    fn threep_config(&self) -> PathBuf {
        self.threep_dir.join(CONFIG_FILE)
    }
    fn library_dir(&self) -> PathBuf {
        self.threep_dir.join("configLibrary")
    }
    fn profile_path(&self) -> PathBuf {
        self.library_dir().join(format!("{PROFILE_ID}.json"))
    }
    fn meta_path(&self) -> PathBuf {
        self.library_dir().join("_meta.json")
    }

    fn edit(path: &Path, secret: bool, edit: impl FnOnce(&mut Value)) -> Result<PlannedFile> {
        let pre = fsutil::read_optional(path)?;
        let (mut doc, style) = jsonp::parse(path, pre.as_deref())?;
        edit(&mut doc);
        let content = jsonp::serialize(path, &doc, &style)?;
        Ok(PlannedFile {
            path: path.to_path_buf(),
            pre,
            content,
            secret,
        })
    }

    fn read_value(path: &Path) -> Result<Value> {
        let pre = fsutil::read_optional(path)?;
        let (doc, _) = jsonp::parse(path, pre.as_deref())?;
        Ok(doc)
    }
}

fn default_dirs() -> (PathBuf, PathBuf) {
    if cfg!(target_os = "macos") {
        let app = home_dir().join("Library").join("Application Support");
        return (app.join("Claude"), app.join("Claude-3p"));
    }
    if cfg!(windows) {
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home_dir().join("AppData").join("Local"));
        return (local.join("Claude"), local.join("Claude-3p"));
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home_dir().join(".config"));
    (config.join("Claude"), config.join("Claude-3p"))
}

fn set_deployment_mode(doc: &mut Value) {
    if let Some(obj) = doc.as_object_mut() {
        obj.insert("deploymentMode".into(), json!("3p"));
    }
}

fn write_profile(doc: &mut Value, spec: &ConnectionSpec) {
    let Some(obj) = doc.as_object_mut() else {
        return;
    };
    obj.insert("coworkEgressAllowedHosts".into(), json!(["*"]));
    obj.insert("disableDeploymentModeChooser".into(), json!(true));
    obj.insert("inferenceProvider".into(), json!("gateway"));
    obj.insert(
        "inferenceGatewayBaseUrl".into(),
        json!(spec.base_url),
    );
    obj.insert("inferenceGatewayAuthScheme".into(), json!("bearer"));
    obj.insert("inferenceGatewayApiKey".into(), json!(spec.token));
    obj.insert(
        "inferenceModels".into(),
        json!([
            { "name": ROLE_OPUS, "labelOverride": spec.model },
            { "name": ROLE_SONNET, "labelOverride": spec.fast() },
            { "name": ROLE_HAIKU, "labelOverride": spec.cheap() },
        ]),
    );
}

fn write_meta(doc: &mut Value) {
    let Some(obj) = doc.as_object_mut() else {
        return;
    };
    let mut entries = obj
        .get("entries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    entries.retain(|entry| entry.get("id").and_then(Value::as_str) != Some(PROFILE_ID));
    entries.push(json!({ "id": PROFILE_ID, "name": PROFILE_NAME }));
    obj.insert("entries".into(), Value::Array(entries));
    obj.insert("appliedId".into(), json!(PROFILE_ID));
}

fn desktop_binaries() -> Vec<PathBuf> {
    let mut extra = crate::adapters::locate::candidates(
        "claude-desktop",
        &[
            "/Applications/Claude.app/Contents/MacOS/Claude",
            "/usr/bin/claude-desktop",
        ],
    );
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let local = PathBuf::from(local);
        extra.push(local.join("AnthropicClaude").join("claude.exe"));
        extra.push(local.join("Programs").join("Claude").join("Claude.exe"));
    }
    extra
}

impl Adapter for ClaudeDesktopAdapter {
    fn id(&self) -> ClientId {
        ClientId::ClaudeDesktop
    }
    fn display_name(&self) -> &'static str {
        "Claude Desktop"
    }
    fn protocol(&self) -> &'static str {
        "anthropic"
    }

    fn detect(&self) -> Detection {
        let mut det = detect_tool("claude-desktop", &[], &self.normal_dir);
        if det.binary_path.is_none() {
            det.binary_path = crate::adapters::locate::find_binary(&["Claude"], &desktop_binaries());
            det.version = det.binary_path.as_deref().and_then(crate::adapters::locate::version_of);
        }
        det.installed = det.binary_path.is_some()
            || self.normal_dir.is_dir()
            || self.threep_dir.is_dir();
        det
    }

    fn managed_paths(&self) -> Vec<PathBuf> {
        vec![
            self.normal_config(),
            self.threep_config(),
            self.profile_path(),
            self.meta_path(),
        ]
    }

    fn read_config(&self) -> CurrentConfig {
        let Ok(profile) = Self::read_value(&self.profile_path()) else {
            return CurrentConfig::default();
        };
        let base_url = profile
            .get("inferenceGatewayBaseUrl")
            .and_then(Value::as_str)
            .map(str::to_string);
        let has_credential = profile
            .get("inferenceGatewayApiKey")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty());
        let model = profile
            .get("inferenceModels")
            .and_then(Value::as_array)
            .and_then(|models| {
                models.iter().find_map(|m| {
                    m.get("labelOverride")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
            });
        CurrentConfig {
            base_url,
            model,
            has_credential,
        }
    }

    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
        Ok(vec![
            Self::edit(&self.normal_config(), false, set_deployment_mode)?,
            Self::edit(&self.threep_config(), false, set_deployment_mode)?,
            Self::edit(&self.profile_path(), true, |doc| write_profile(doc, spec))?,
            Self::edit(&self.meta_path(), false, write_meta)?,
        ])
    }

    fn validate_config(&self, base_url: &str) -> Validation {
        let mut problems = Vec::new();
        for path in [self.normal_config(), self.threep_config()] {
            match Self::read_value(&path) {
                Ok(doc) => {
                    if doc.get("deploymentMode").and_then(Value::as_str) != Some("3p") {
                        problems.push(format!(
                            "{} deploymentMode is not 3p",
                            path.display()
                        ));
                    }
                }
                Err(e) => problems.push(e.to_string()),
            }
        }
        match Self::read_value(&self.profile_path()) {
            Ok(profile) => {
                let url = profile
                    .get("inferenceGatewayBaseUrl")
                    .and_then(Value::as_str);
                if url != Some(base_url) {
                    problems.push(format!(
                        "inferenceGatewayBaseUrl is {url:?}, expected {base_url:?}"
                    ));
                }
                if profile.get("inferenceProvider").and_then(Value::as_str) != Some("gateway") {
                    problems.push("inferenceProvider is not gateway".into());
                }
                if profile
                    .get("inferenceGatewayApiKey")
                    .and_then(Value::as_str)
                    .is_none_or(|s| s.trim().is_empty())
                {
                    problems.push("inferenceGatewayApiKey is not set".into());
                }
            }
            Err(e) => problems.push(e.to_string()),
        }
        match Self::read_value(&self.meta_path()) {
            Ok(meta) => {
                if meta.get("appliedId").and_then(Value::as_str) != Some(PROFILE_ID) {
                    problems.push("Claude Desktop applied profile is not Tokease".into());
                }
            }
            Err(e) => problems.push(e.to_string()),
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
    use crate::backup::BackupStore;

    fn adapter(root: &Path) -> ClaudeDesktopAdapter {
        ClaudeDesktopAdapter::new(root.join("Claude"), root.join("Claude-3p"))
    }

    #[test]
    fn closed_loop_preserves_existing_bytes_after_restore() {
        let tmp = tempfile::tempdir().unwrap();
        let a = adapter(tmp.path());
        let meta = a.meta_path();
        testutil::closed_loop(
            &a,
            &[
                (
                    a.normal_config().as_path(),
                    "{\n  \"theme\": \"dark\"\n}\n",
                ),
                (
                    meta.as_path(),
                    "{\n  \"entries\": [{ \"id\": \"other\", \"name\": \"Other\" }],\n  \"appliedId\": \"other\"\n}\n",
                ),
            ],
        );
    }

    #[test]
    fn apply_keeps_user_keys_and_other_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        let backups = BackupStore::new(tmp.path().join("bak"));
        let a = adapter(tmp.path());
        std::fs::create_dir_all(a.normal_config().parent().unwrap()).unwrap();
        std::fs::write(a.normal_config(), "{\n  \"theme\": \"dark\"\n}\n").unwrap();
        std::fs::create_dir_all(a.meta_path().parent().unwrap()).unwrap();
        std::fs::write(
            a.meta_path(),
            r#"{"entries":[{"id":"other","name":"Other"}],"appliedId":"other"}"#,
        )
        .unwrap();

        a.apply_config(&backups, &testutil::spec()).unwrap();

        let normal: Value = serde_json::from_slice(&std::fs::read(a.normal_config()).unwrap()).unwrap();
        assert_eq!(normal["theme"], json!("dark"));
        assert_eq!(normal["deploymentMode"], json!("3p"));

        let profile: Value =
            serde_json::from_slice(&std::fs::read(a.profile_path()).unwrap()).unwrap();
        assert_eq!(profile["inferenceGatewayBaseUrl"], json!(testutil::spec().base_url));
        assert_eq!(profile["inferenceModels"][0]["name"], json!(ROLE_OPUS));
        assert_eq!(
            profile["inferenceModels"][0]["labelOverride"],
            json!("code-best")
        );

        let meta: Value = serde_json::from_slice(&std::fs::read(a.meta_path()).unwrap()).unwrap();
        assert_eq!(meta["appliedId"], json!(PROFILE_ID));
        let ids: Vec<&str> = meta["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|e| e["id"].as_str())
            .collect();
        assert!(ids.contains(&"other"));
        assert!(ids.contains(&PROFILE_ID));
    }
}

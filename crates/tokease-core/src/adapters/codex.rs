//! Codex CLI adapter.
//!
//! Only `config.toml` (under `$CODEX_HOME`, default `~/.codex`) is written;
//! `auth.json` — the ChatGPT login — is never touched. Codex ≥ 0.149 reads a
//! provider's credential from `[model_providers.<id>].experimental_bearer_token`,
//! so the Tokease token goes there and the user's ChatGPT login survives a
//! round trip untouched (this is CC Switch's current Codex write model).
//!
//! What we write (everything else in the file, comments included, stays):
//!
//! ```toml
//! model_provider = "tokease"
//! model = "code-best"
//! disable_response_storage = true
//! model_reasoning_effort = "high"      # seeded only when absent
//!
//! [model_providers.tokease]
//! name = "Tokease"
//! base_url = "https://www.tokease.cn/v1"
//! wire_api = "responses"
//! requires_openai_auth = false         # true while a ChatGPT login exists on disk
//! experimental_bearer_token = "…"
//! ```
//!
//! Key fields of a previous provider (`openai_base_url`, `review_model`,
//! top-level `experimental_bearer_token`/`base_url`/`wire_api`, nested
//! sub-agent/memory model names) are cleared so no request leaks past
//! Tokease. Reasoning-effort settings are user preferences and are kept.

use std::path::PathBuf;

use serde_json::Value as Json;
use toml_edit::{DocumentMut, Item, Table, Value};

use super::{detect_tool, home_dir, Adapter, ClientId, ConnectionSpec, CurrentConfig, Detection, PlannedFile, Validation};
use crate::error::Result;
use crate::floor;
use crate::fsutil;
use crate::patch::toml::{self as tomlp, put_table, put_value, remove_nested, str_at, table_at};
use crate::patch::PatchError;
use crate::PROVIDER_ID;

/// `experimental_bearer_token` is honoured from this Codex version on.
pub const MIN_VERSION: &str = "0.149.0";

/// Top-level keys cleared on enable: the routing/credential/model-name part
/// of [`floor::CODEX_FLOOR_TOP`]. Reasoning effort and the model catalog
/// pointer are user preferences Tokease does not own.
const CLEAR_TOP: &[&str] = &[
    "model_provider",
    "openai_base_url",
    "model",
    "review_model",
    "disable_response_storage",
    "experimental_bearer_token",
    "base_url",
    "wire_api",
];

/// Nested model names of a previous provider (subset of
/// [`floor::CODEX_FLOOR_NESTED`] that names models rather than efforts).
const CLEAR_NESTED: &[&[&str]] =
    &[&["agents", "default_subagent_model"], &["memories", "extract_model"], &["memories", "consolidation_model"]];

pub struct CodexAdapter {
    config_dir: PathBuf,
}

impl Default for CodexAdapter {
    fn default() -> Self {
        let dir = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home_dir().join(".codex"));
        Self::new(dir)
    }
}

impl CodexAdapter {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self { config_dir: config_dir.into() }
    }

    fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }
    fn auth_path(&self) -> PathBuf {
        self.config_dir.join("auth.json")
    }

    fn read_doc(&self) -> Result<(Option<Vec<u8>>, DocumentMut)> {
        let path = self.config_path();
        let pre = fsutil::read_optional(&path)?;
        let doc = tomlp::parse(&path, pre.as_deref())?;
        Ok((pre, doc))
    }

    /// Does `auth.json` hold something Codex can log in with? Unreadable or
    /// malformed files count as "no login".
    fn login_on_disk(&self) -> bool {
        fsutil::read_optional(&self.auth_path())
            .ok()
            .flatten()
            .and_then(|b| serde_json::from_slice::<Json>(&b).ok())
            .is_some_and(|v| auth_has_account_material(&v))
    }

    /// The provider table Tokease owns.
    fn provider_table(spec: &ConnectionSpec, login_on_disk: bool) -> Table {
        let mut t = Table::new();
        t.insert("name", Item::Value(Value::from("Tokease")));
        t.insert("base_url", Item::Value(Value::from(spec.base_url.as_str())));
        t.insert("wire_api", Item::Value(Value::from(spec.wire_api.as_deref().unwrap_or("responses"))));
        // Keeps the account UI alive only when there is a login to show;
        // the request itself is authenticated by the bearer token below.
        t.insert("requires_openai_auth", Item::Value(Value::from(login_on_disk)));
        t.insert("experimental_bearer_token", Item::Value(Value::from(spec.token.as_str())));
        t
    }

    fn apply_patch(&self, doc: &mut DocumentMut, spec: &ConnectionSpec) -> std::result::Result<(), PatchError> {
        let path = self.config_path();
        let root = doc.as_table_mut();

        // 1. Clear a previous provider's key fields (ours are rewritten in
        //    place below, so skip them to keep their position).
        let targets = ["model_provider", "model", "disable_response_storage"];
        let doomed: Vec<String> = root
            .iter()
            .map(|(k, _)| k.to_string())
            .filter(|k| CLEAR_TOP.contains(&k.as_str()) && !targets.contains(&k.as_str()))
            .collect();
        for key in doomed {
            root.remove(&key);
        }
        for segments in CLEAR_NESTED {
            remove_nested(root, segments);
        }

        // 2. Routing values, in place.
        put_value(root, "model_provider", Value::from(PROVIDER_ID));
        put_value(root, "model", Value::from(spec.model.as_str()));
        put_value(root, "disable_response_storage", Value::from(true));
        if !root.contains_key("model_reasoning_effort") {
            put_value(root, "model_reasoning_effort", Value::from("high"));
        }

        // 3. The provider table.
        let container_inline = matches!(root.get("model_providers"), Some(Item::Value(_)));
        if root.get("model_providers").is_some_and(|i| i.as_table_like().is_none()) {
            return Err(PatchError::shape(&path, &["model_providers".to_string()], "a table"));
        }
        let providers = tomlp::ensure_table_mut(&path, root, &["model_providers"])?;

        // Tables under reserved ids make Codex 0.148+ refuse the whole file;
        // rename rather than delete (we do not know which keys matter).
        for id in floor::CODEX_RESERVED_PROVIDER_IDS {
            if providers.get(id).is_some_and(|i| i.as_table_like().is_some()) {
                let item = providers.remove(id).expect("present");
                let mut renamed = format!("{id}-legacy");
                let mut n = 2;
                while providers.contains_key(&renamed) {
                    renamed = format!("{id}-legacy-{n}");
                    n += 1;
                }
                log::warn!("codex: renamed reserved [model_providers.{id}] to [model_providers.{renamed}]");
                providers.insert(&renamed, item);
            }
        }
        put_table(providers, PROVIDER_ID, Self::provider_table(spec, self.login_on_disk()), container_inline);

        // 4. The active profile must not reroute away from Tokease.
        check_effective_route(doc.as_table())
    }

    fn inspect(root: &Table) -> Inspection {
        let provider = str_at(root, &["model_provider"]);
        let table = provider.as_deref().and_then(|p| table_at(root, &["model_providers", p]));
        Inspection {
            base_url: table.and_then(|t| t.get("base_url")).and_then(Item::as_str).map(str::to_string),
            bearer: table
                .and_then(|t| t.get("experimental_bearer_token"))
                .and_then(Item::as_str)
                .is_some_and(|s| !s.trim().is_empty()),
            model: str_at(root, &["model"]),
            provider,
        }
    }
}

struct Inspection {
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    bearer: bool,
}

/// Something Codex would log in with: an API key, ChatGPT tokens, a PAT
/// or an agent identity (CC Switch `codex_auth_has_openai_account_material`,
/// simplified).
fn auth_has_account_material(auth: &Json) -> bool {
    let Some(obj) = auth.as_object() else { return false };
    let present = |v: &Json| match v {
        Json::Null => false,
        Json::String(s) => !s.trim().is_empty(),
        Json::Array(a) => !a.is_empty(),
        Json::Object(m) => !m.is_empty(),
        _ => true,
    };
    obj.get("OPENAI_API_KEY").is_some_and(present)
        || obj.get("personal_access_token").is_some_and(present)
        || obj.get("agent_identity").is_some_and(present)
        || obj
            .get("tokens")
            .and_then(Json::as_object)
            .is_some_and(|t| ["id_token", "access_token", "refresh_token"].iter().any(|k| t.get(*k).is_some_and(present)))
}

/// The top-level `profile` selects `[profiles.<name>]`, whose
/// `model_provider` / `openai_base_url` / `experimental_bearer_token`
/// override ours: refuse rather than write a config that silently routes
/// elsewhere.
fn check_effective_route(root: &Table) -> std::result::Result<(), PatchError> {
    let Some(name) = str_at(root, &["profile"]) else { return Ok(()) };
    let Some(profile) = table_at(root, &["profiles", &name]) else { return Ok(()) };
    let overridden = ["model_provider", "openai_base_url", "experimental_bearer_token"].into_iter().find(|key| {
        match profile.get(key).and_then(Item::as_str).map(str::trim) {
            None | Some("") => false,
            Some(id) if *key == "model_provider" => id != PROVIDER_ID,
            Some(_) => true,
        }
    });
    match overridden {
        Some(key) => Err(PatchError::Route { profile: name, key: key.to_string() }),
        None => Ok(()),
    }
}

impl Adapter for CodexAdapter {
    fn id(&self) -> ClientId {
        ClientId::Codex
    }
    fn display_name(&self) -> &'static str {
        "Codex CLI"
    }
    fn protocol(&self) -> &'static str {
        "openai"
    }

    fn detect(&self) -> Detection {
        // The ChatGPT desktop app bundles a codex binary that is not on PATH.
        detect_tool("codex", &["/usr/lib/chatgpt/resources/codex"], &self.config_dir)
    }

    fn managed_paths(&self) -> Vec<PathBuf> {
        vec![self.config_path()]
    }

    fn read_config(&self) -> CurrentConfig {
        let Ok((_, doc)) = self.read_doc() else { return CurrentConfig::default() };
        let i = Self::inspect(doc.as_table());
        let has_credential = i.bearer || (i.provider.as_deref().is_none_or(|p| p == "openai") && self.login_on_disk());
        CurrentConfig { base_url: i.base_url, model: i.model, has_credential }
    }

    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
        let (pre, mut doc) = self.read_doc()?;
        self.apply_patch(&mut doc, spec)?;
        let content = tomlp::serialize(&doc, pre.as_deref());
        // The file now carries the token: private permissions.
        Ok(vec![PlannedFile { path: self.config_path(), pre, content, secret: true }])
    }

    fn validate_config(&self, base_url: &str) -> Validation {
        let (_, doc) = match self.read_doc() {
            Ok(d) => d,
            Err(e) => return Validation::fail(vec![e.to_string()]),
        };
        let root = doc.as_table();
        let i = Self::inspect(root);
        let mut problems = Vec::new();
        if i.provider.as_deref() != Some(PROVIDER_ID) {
            problems.push(format!("model_provider is {:?}, expected \"{PROVIDER_ID}\"", i.provider));
        }
        if i.base_url.as_deref() != Some(base_url) {
            problems.push(format!("model_providers.{PROVIDER_ID}.base_url is {:?}, expected {base_url:?}", i.base_url));
        }
        if !i.bearer {
            problems.push(format!("model_providers.{PROVIDER_ID}.experimental_bearer_token is not set"));
        }
        if i.model.is_none() {
            problems.push("model is not set".into());
        }
        if str_at(root, &["openai_base_url"]).is_some() {
            problems.push("top-level openai_base_url is set and would reroute requests".into());
        }
        if let Err(e) = check_effective_route(root) {
            problems.push(e.to_string());
        }
        Validation::fail(problems)
    }

    fn env_conflict_prefixes(&self) -> &'static [&'static str] {
        &["OPENAI_API_KEY", "OPENAI_BASE_URL"]
    }

    fn min_version(&self) -> Option<&'static str> {
        Some(MIN_VERSION)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::adapters::testutil;

    const SEED_TOML: &str = r#"# my codex config
model = "gpt-5" # chosen in the picker
model_reasoning_effort = "medium"
openai_base_url = "https://relay.example/v1"
review_model = "gpt-5-mini"

[projects."/home/me/proj"]
trust_level = "trusted"

[agents]
default_subagent_model = "gpt-5-mini"
max_threads = 4

[mcp_servers.fs]
command = "mcp-fs"
"#;

    const CHATGPT_AUTH: &str = r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"tokens":{"id_token":"x","access_token":"y"},"last_refresh":"t"}"#;

    fn plan_with(dir: &Path, toml: Option<&str>, auth: Option<&str>) -> String {
        std::fs::create_dir_all(dir).unwrap();
        if let Some(t) = toml {
            std::fs::write(dir.join("config.toml"), t).unwrap();
        }
        if let Some(a) = auth {
            std::fs::write(dir.join("auth.json"), a).unwrap();
        }
        let files = CodexAdapter::new(dir).plan(&testutil::spec()).unwrap();
        assert_eq!(files.len(), 1, "only config.toml is written");
        String::from_utf8(files[0].content.clone()).unwrap()
    }

    #[test]
    fn closed_loop_with_existing_config() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".codex");
        let a = CodexAdapter::new(&dir);
        testutil::closed_loop(&a, &[(&dir.join("config.toml"), SEED_TOML), (&dir.join("auth.json"), CHATGPT_AUTH)]);
        // auth.json is not a managed file and must be exactly as seeded.
        assert_eq!(std::fs::read_to_string(dir.join("auth.json")).unwrap(), CHATGPT_AUTH);
    }

    #[test]
    fn closed_loop_without_any_config() {
        let tmp = tempfile::tempdir().unwrap();
        let a = CodexAdapter::new(tmp.path().join(".codex"));
        testutil::closed_loop(&a, &[]);
    }

    #[test]
    fn plan_clears_floor_keeps_user_keys_and_leaves_auth_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let toml = plan_with(&tmp.path().join(".codex"), Some(SEED_TOML), Some(CHATGPT_AUTH));

        assert!(toml.starts_with("# my codex config\n"));
        assert!(toml.contains("model = \"code-best\" # chosen in the picker"), "in place, comment kept:\n{toml}");
        assert!(toml.contains("model_reasoning_effort = \"medium\""), "user preference kept");
        assert!(!toml.contains("openai_base_url"), "legacy reroute cleared");
        assert!(!toml.contains("review_model"), "previous provider's model cleared");
        assert!(!toml.contains("default_subagent_model"));
        assert!(toml.contains("max_threads = 4"), "rest of [agents] kept");
        assert!(toml.contains("[projects.\"/home/me/proj\"]"));
        assert!(toml.contains("model_provider = \"tokease\""));
        assert!(toml.contains("disable_response_storage = true"));
        assert!(toml.contains("[model_providers.tokease]"));
        assert!(toml.contains("base_url = \"https://api.tokease.test/v1\""));
        assert!(toml.contains("wire_api = \"responses\""));
        assert!(toml.contains("experimental_bearer_token = \"tk_test_1234567890abcdef\""));
        assert!(toml.contains("requires_openai_auth = true"), "login exists on disk");
        assert!(!toml.contains("\n[model_providers]\n"), "no empty header:\n{toml}");
    }

    #[test]
    fn requires_openai_auth_follows_the_login_on_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let toml = plan_with(&tmp.path().join("a"), None, None);
        assert!(toml.contains("requires_openai_auth = false"));
        assert!(toml.contains("model_reasoning_effort = \"high\""), "seeded when absent");

        let toml = plan_with(&tmp.path().join("b"), None, Some(r#"{"auth_mode":"chatgpt","tokens":{}}"#));
        assert!(toml.contains("requires_openai_auth = false"), "empty tokens are not a login");

        let toml = plan_with(&tmp.path().join("c"), None, Some(r#"{"OPENAI_API_KEY":"sk-x"}"#));
        assert!(toml.contains("requires_openai_auth = true"));
    }

    #[test]
    fn reserved_provider_tables_are_renamed_not_deleted() {
        let tmp = tempfile::tempdir().unwrap();
        let toml = plan_with(
            &tmp.path().join(".codex"),
            Some("[model_providers.openai]\nbase_url = \"https://x\"\n[model_providers.ollama]\nname = \"o\"\n"),
            None,
        );
        assert!(toml.contains("[model_providers.openai-legacy]"));
        assert!(toml.contains("[model_providers.ollama-legacy]"));
        assert!(toml.contains("base_url = \"https://x\""));
        assert!(!toml.contains("[model_providers.openai]\n"));
    }

    #[test]
    fn inline_container_stays_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let toml = plan_with(&tmp.path().join(".codex"), Some("model_providers = { relay = { name = \"r\" } }\n"), None);
        assert!(toml.replace(" ,", ",").contains("model_providers = { relay = { name = \"r\" }, tokease = {"), "{toml}");
        assert!(!toml.contains("[model_providers.tokease]"));
    }

    #[test]
    fn profile_override_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "profile = \"work\"\n[profiles.work]\nmodel_provider = \"relay\"\n").unwrap();
        let err = CodexAdapter::new(&dir).plan(&testutil::spec()).unwrap_err();
        assert!(matches!(err, crate::Error::Patch(PatchError::Route { .. })), "{err}");

        // A profile that picks tokease itself is fine.
        std::fs::write(dir.join("config.toml"), "profile = \"work\"\n[profiles.work]\nmodel_provider = \"tokease\"\n").unwrap();
        CodexAdapter::new(&dir).plan(&testutil::spec()).unwrap();
    }

    #[test]
    fn broken_toml_is_refused_not_overwritten() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), "model = \n").unwrap();
        let err = CodexAdapter::new(&dir).plan(&testutil::spec()).unwrap_err();
        assert!(matches!(err, crate::Error::Patch(PatchError::Parse { line: 1, .. })), "{err}");
    }
}

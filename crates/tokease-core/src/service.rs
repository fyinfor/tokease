//! The one object the UI (Tauri commands) and the CLI talk to.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::adapters::{self, locate, Adapter, ClientId, ConnectionSpec, CurrentConfig};
use crate::api::{ApiClient, ClientConfig, DevicePoll, DeviceStart, DeviceStatus, UserInfo};
use crate::backup::BackupStore;
use crate::envcheck::{self, EnvConflict};
use crate::error::{Error, Result};
use crate::redact;
use crate::secrets::{SecretStore, StorageBackend};
use crate::state::{EnabledRecord, StateStore};
use crate::DEFAULT_SERVER_URL;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub logged_in: bool,
    pub user: Option<UserInfo>,
    pub server_url: String,
    pub storage_backend: StorageBackend,
    /// Masked, for the Advanced page only.
    pub token_preview: Option<String>,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientStatus {
    pub id: ClientId,
    pub name: String,
    pub installed: bool,
    pub binary_path: Option<PathBuf>,
    /// `--version` output, when the binary answered.
    pub version: Option<String>,
    /// Oldest CLI version whose config format we write.
    pub min_version: Option<String>,
    /// `version` parsed below `min_version`.
    pub outdated: bool,
    pub config_dir: PathBuf,
    pub managed_files: Vec<PathBuf>,
    /// Allowed by the server's `/client/config` (defaults to true when unknown).
    pub available: bool,
    /// Config on disk points at Tokease *and* we have a backup to go back to.
    pub enabled: bool,
    pub current: CurrentConfig,
    pub problems: Vec<String>,
    /// Shell/process environment variables that override the config file.
    pub env_conflicts: Vec<EnvConflict>,
    pub enabled_at: Option<String>,
    pub backup_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSummary {
    pub id: String,
    pub created_at: String,
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum LoginPoll {
    Pending,
    Authorized { session: SessionInfo },
    Expired,
    Denied,
}

pub struct Tokease {
    data_dir: PathBuf,
    state: StateStore,
    secrets: SecretStore,
    backups: BackupStore,
    adapters: Vec<Arc<dyn Adapter>>,
    /// One write lock per client: enable/restore never interleave.
    locks: BTreeMap<ClientId, Mutex<()>>,
}

impl Tokease {
    /// `data_dir` defaults to `~/.tokease` (override: `TOKEASE_DATA_DIR`).
    pub fn new() -> Result<Self> {
        let data_dir = std::env::var_os("TOKEASE_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| adapters::home_dir().join(".tokease"));
        Self::with_data_dir(data_dir, adapters::all())
    }

    pub fn with_data_dir(data_dir: PathBuf, adapters: Vec<Box<dyn Adapter>>) -> Result<Self> {
        std::fs::create_dir_all(&data_dir).map_err(|e| Error::io(&data_dir, e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700));
        }
        let adapters: Vec<Arc<dyn Adapter>> = adapters.into_iter().map(Arc::from).collect();
        Ok(Self {
            state: StateStore::new(&data_dir),
            secrets: SecretStore::new(&data_dir),
            backups: BackupStore::new(data_dir.join("backups")),
            locks: adapters.iter().map(|a| (a.id(), Mutex::new(()))).collect(),
            adapters,
            data_dir,
        })
    }

    fn lock(&self, id: ClientId) -> std::sync::MutexGuard<'_, ()> {
        self.locks.get(&id).expect("lock for every adapter").lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    // ----- server / session -------------------------------------------------

    pub fn server_url(&self) -> String {
        if let Ok(u) = std::env::var("TOKEASE_SERVER_URL") {
            if !u.trim().is_empty() {
                return u.trim().trim_end_matches('/').to_string();
            }
        }
        self.state
            .load()
            .ok()
            .and_then(|s| s.server_url)
            .unwrap_or_else(|| DEFAULT_SERVER_URL.to_string())
    }

    pub fn set_server_url(&self, url: &str) -> Result<SessionInfo> {
        let url = url.trim().trim_end_matches('/');
        url::Url::parse(url).map_err(|e| Error::Other(format!("invalid server URL: {e}")))?;
        self.state.update(|s| s.server_url = Some(url.to_string()))?;
        self.session()
    }

    fn api(&self) -> ApiClient {
        ApiClient::new(&self.server_url())
    }

    fn token(&self) -> Result<Option<(String, StorageBackend)>> {
        self.secrets.get_token()
    }

    /// Keychain access can block on D-Bus / OS prompts; keep it off the
    /// async executor threads.
    async fn token_async(&self) -> Result<Option<(String, StorageBackend)>> {
        let secrets = self.secrets.clone();
        tokio::task::spawn_blocking(move || secrets.get_token())
            .await
            .map_err(|e| Error::Other(e.to_string()))?
    }

    async fn set_token_async(&self, token: String) -> Result<StorageBackend> {
        let secrets = self.secrets.clone();
        tokio::task::spawn_blocking(move || secrets.set_token(&token))
            .await
            .map_err(|e| Error::Other(e.to_string()))?
    }

    async fn require_token(&self) -> Result<String> {
        self.token_async().await?.map(|(t, _)| t).ok_or(Error::NotLoggedIn)
    }

    pub fn session(&self) -> Result<SessionInfo> {
        let state = self.state.load()?;
        let tok = self.token()?;
        Ok(SessionInfo {
            logged_in: tok.is_some(),
            user: if tok.is_some() { state.user } else { None },
            server_url: self.server_url(),
            storage_backend: tok.as_ref().map(|(_, b)| *b).unwrap_or(StorageBackend::None),
            token_preview: tok.as_ref().map(|(t, _)| redact::token(t)),
            data_dir: self.data_dir.clone(),
        })
    }

    async fn complete_login(&self, token: String, user: UserInfo) -> Result<SessionInfo> {
        log::info!("logged in as {} (token {})", user.id, redact::token(&token));
        let backend = self.set_token_async(token).await?;
        log::info!("token stored in {backend:?}");
        self.state.update(|s| s.user = Some(user))?;
        // Warm the platform config cache; failure here is not fatal.
        if let Err(e) = self.refresh_platform_config().await {
            log::warn!("could not fetch /client/config right after login: {e}");
        }
        self.session()
    }

    pub async fn login_password(&self, email: &str, password: &str) -> Result<SessionInfo> {
        let r = self.api().login_password(email, password).await?;
        self.complete_login(r.access_token, r.user).await
    }

    pub async fn device_start(&self) -> Result<DeviceStart> {
        self.api().device_start().await
    }

    pub async fn device_poll(&self, device_code: &str) -> Result<LoginPoll> {
        let DevicePoll { status, access_token, user } = self.api().device_poll(device_code).await?;
        Ok(match status {
            DeviceStatus::Pending => LoginPoll::Pending,
            DeviceStatus::Expired => LoginPoll::Expired,
            DeviceStatus::Denied => LoginPoll::Denied,
            DeviceStatus::Authorized => {
                let token = access_token.ok_or_else(|| Error::Other("server said authorized but sent no token".into()))?;
                let user = user.unwrap_or(UserInfo { id: "me".into(), email: None, name: None });
                LoginPoll::Authorized { session: self.complete_login(token, user).await? }
            }
        })
    }

    /// Forgets the token. Tool configs are left untouched (use restore for that).
    pub fn logout(&self) -> Result<SessionInfo> {
        self.secrets.clear()?;
        self.state.update(|s| s.user = None)?;
        self.session()
    }

    // ----- platform config --------------------------------------------------

    pub async fn refresh_platform_config(&self) -> Result<ClientConfig> {
        let token = self.require_token().await?;
        let cfg = self.api().client_config(&token).await?;
        self.state.update(|s| s.platform_config = Some(cfg.clone()))?;
        Ok(cfg)
    }

    pub fn cached_platform_config(&self) -> Result<Option<ClientConfig>> {
        Ok(self.state.load()?.platform_config)
    }

    /// Live `/client/config`, else the cached copy (offline), else — when the
    /// server simply does not implement the endpoint yet — the built-in
    /// defaults derived from the server URL.
    async fn platform_config(&self) -> Result<ClientConfig> {
        match self.refresh_platform_config().await {
            Ok(c) => Ok(c),
            Err(e @ Error::NotLoggedIn) => Err(e),
            Err(e) => {
                if let Some(c) = self.cached_platform_config()? {
                    log::warn!("using cached /client/config ({e})");
                    return Ok(c);
                }
                match e {
                    Error::Api { status: 404 | 405 | 501, .. } => {
                        log::warn!("server has no /client/config ({e}); using built-in defaults");
                        Ok(ClientConfig::builtin(&self.server_url()))
                    }
                    other => Err(other),
                }
            }
        }
    }

    // ----- clients ----------------------------------------------------------

    fn adapter(&self, id: ClientId) -> &dyn Adapter {
        self.adapters
            .iter()
            .find(|a| a.id() == id)
            .map(|a| a.as_ref())
            .expect("adapter registered for every ClientId")
    }

    pub fn client_statuses(&self) -> Result<Vec<ClientStatus>> {
        let state = self.state.load()?;
        self.adapters.iter().map(|a| self.status_of(a.as_ref(), &state)).collect()
    }

    pub fn client_status(&self, id: ClientId) -> Result<ClientStatus> {
        let state = self.state.load()?;
        self.status_of(self.adapter(id), &state)
    }

    fn status_of(&self, a: &dyn Adapter, state: &crate::state::AppState) -> Result<ClientStatus> {
        let det = a.detect();
        let record = state.clients.get(a.id().as_str());
        let current = a.read_config();
        let available = state
            .platform_config
            .as_ref()
            .map(|c| c.client(a.id().as_str()).enabled)
            .unwrap_or(true);

        // "enabled" means: we applied it (record exists) and it still validates.
        let (enabled, problems) = match record {
            Some(r) => {
                let v = a.validate_config(&r.base_url);
                (v.ok, v.problems)
            }
            None => (false, vec![]),
        };

        let outdated = match (a.min_version(), det.version.as_deref()) {
            (Some(min), Some(have)) => match (locate::parse_version(min), locate::parse_version(have)) {
                (Some(min), Some(have)) => have < min,
                _ => false,
            },
            _ => false,
        };

        Ok(ClientStatus {
            id: a.id(),
            name: a.display_name().to_string(),
            installed: det.installed,
            binary_path: det.binary_path,
            version: det.version,
            min_version: a.min_version().map(str::to_string),
            outdated,
            config_dir: det.config_dir,
            managed_files: a.managed_paths(),
            available,
            enabled,
            current,
            problems,
            env_conflicts: envcheck::check(a.env_conflict_prefixes()),
            enabled_at: record.map(|r| r.enabled_at.clone()),
            backup_id: record.map(|r| r.backup_id.clone()),
        })
    }

    /// One-click enable: fetch platform config, build the spec, let the
    /// adapter do backup -> apply -> validate, then remember the backup id.
    pub async fn enable(&self, id: ClientId) -> Result<ClientStatus> {
        let a = self.adapter(id);
        if !a.detect().installed {
            return Err(Error::NotInstalled { client: a.display_name().into() });
        }
        let token = self.require_token().await?;
        let cfg = self.platform_config().await?;
        let flag = cfg.client(id.as_str());
        if !flag.enabled {
            return Err(Error::ClientDisabled { client: a.display_name().into() });
        }
        let spec = ConnectionSpec {
            base_url: cfg.endpoint(a.protocol()).to_string(),
            token,
            model: cfg.default_model.clone(),
            fast_model: cfg.tier("fast"),
            cheap_model: cfg.tier("cheap"),
            wire_api: flag.wire_api,
        };
        log::info!("enabling {} -> {} model={}", id, spec.base_url, spec.model);

        let _guard = self.lock(id);
        let manifest = a.apply_config(&self.backups, &spec)?;

        self.state.update(|s| {
            // Keep the *first* backup as the restore point if the user clicks
            // "enable" twice: that one holds their pre-Tokease config.
            let existing = s.clients.get(id.as_str()).cloned();
            let backup_id = match existing {
                Some(r) if self.backups.load(id, &r.backup_id).is_ok() => r.backup_id,
                _ => manifest.id.clone(),
            };
            s.clients.insert(
                id.as_str().into(),
                EnabledRecord {
                    enabled_at: chrono::Local::now().to_rfc3339(),
                    backup_id,
                    base_url: spec.base_url.clone(),
                    model: spec.model.clone(),
                },
            );
        })?;
        self.client_status(id)
    }

    /// Restore the pre-Tokease config (or a specific backup id).
    pub fn restore(&self, id: ClientId, backup_id: Option<&str>) -> Result<ClientStatus> {
        let a = self.adapter(id);
        let _guard = self.lock(id);
        let state = self.state.load()?;
        let chosen = match backup_id {
            Some(b) => b.to_string(),
            None => match state.clients.get(id.as_str()) {
                Some(r) => r.backup_id.clone(),
                None => self
                    .backups
                    .latest(id)?
                    .map(|m| m.id)
                    .ok_or_else(|| Error::NoBackup { client: a.display_name().into() })?,
            },
        };
        let manifest = self.backups.load(id, &chosen)?;
        // Safety net: snapshot the current state before overwriting it.
        a.backup_config(&self.backups)?;
        a.restore_config(&self.backups, &manifest)?;
        self.state.update(|s| {
            s.clients.remove(id.as_str());
        })?;
        self.client_status(id)
    }

    pub fn backups(&self, id: ClientId) -> Result<Vec<BackupSummary>> {
        Ok(self
            .backups
            .list(id)?
            .into_iter()
            .map(|m| BackupSummary {
                id: m.id,
                created_at: m.created_at,
                dir: m.dir,
                files: m.files.into_iter().filter(|f| f.existed).map(|f| f.original_path).collect(),
            })
            .collect())
    }
}

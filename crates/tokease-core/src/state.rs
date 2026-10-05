//! Non-secret persistent state: `<data_dir>/state.json`.
//! Never contains the access token.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::api::{ClientConfig, UserInfo};
use crate::error::{Error, Result};
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnabledRecord {
    pub enabled_at: String,
    /// Backup taken right before we applied our config. Used by "restore".
    pub backup_id: String,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default)]
    pub user: Option<UserInfo>,
    /// Last `/client/config` we fetched (so status works offline).
    #[serde(default)]
    pub platform_config: Option<ClientConfig>,
    #[serde(default)]
    pub clients: BTreeMap<String, EnabledRecord>,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    path: PathBuf,
}

impl StateStore {
    pub fn new(data_dir: &Path) -> Self {
        Self { path: data_dir.join("state.json") }
    }

    pub fn load(&self) -> Result<AppState> {
        match fsutil::read_optional(&self.path)? {
            None => Ok(AppState::default()),
            Some(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| Error::Json { path: self.path.clone(), message: e.to_string() }),
        }
    }

    pub fn save(&self, state: &AppState) -> Result<()> {
        let json = serde_json::to_vec_pretty(state).map_err(|e| Error::Other(e.to_string()))?;
        fsutil::atomic_write(&self.path, &json, Some(0o600))
    }

    pub fn update(&self, f: impl FnOnce(&mut AppState)) -> Result<AppState> {
        let mut s = self.load()?;
        f(&mut s);
        self.save(&s)?;
        Ok(s)
    }
}

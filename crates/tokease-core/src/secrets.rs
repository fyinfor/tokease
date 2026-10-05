//! Access-token storage.
//!
//! Primary: the OS credential store (macOS Keychain, Windows Credential
//! Manager, Secret Service on Linux) through the `keyring` crate.
//! Fallback: a 0600 file under the Tokease data dir, used only when the OS
//! store is unavailable (e.g. headless Linux without a Secret Service daemon).
//! Which backend is active is reported to the UI so it is never silent.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fsutil;

const SERVICE: &str = "com.tokease.desktop";
const ACCOUNT: &str = "access_token";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageBackend {
    Keychain,
    EncryptedFileFallback,
    None,
}

#[derive(Debug, Clone)]
pub struct SecretStore {
    fallback_file: PathBuf,
    /// Tests and headless CI can force the file backend.
    force_file: bool,
}

#[derive(Serialize, Deserialize)]
struct FileSecrets {
    access_token: String,
}

impl SecretStore {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            fallback_file: data_dir.join("credentials.json"),
            force_file: std::env::var("TOKEASE_SECRET_BACKEND").as_deref() == Ok("file"),
        }
    }

    fn entry() -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| Error::Secrets(e.to_string()))
    }

    /// The Linux Secret Service backend (zbus) drives its own executor and
    /// panics if invoked from a thread already running a tokio runtime. Run
    /// every keyring call on a fresh plain thread so callers may be sync or
    /// async alike.
    fn off_runtime<T: Send>(f: impl FnOnce() -> Result<T> + Send) -> Result<T> {
        std::thread::scope(|s| {
            s.spawn(f)
                .join()
                .unwrap_or_else(|_| Err(Error::Secrets("keyring call panicked".into())))
        })
    }

    fn keyring_set(token: &str) -> Result<()> {
        Self::off_runtime(|| {
            Self::entry()?
                .set_password(token)
                .map_err(|e| Error::Secrets(e.to_string()))
        })
    }

    fn keyring_get() -> Result<Option<String>> {
        Self::off_runtime(|| match Self::entry()?.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Secrets(e.to_string())),
        })
    }

    fn keyring_delete() -> Result<()> {
        Self::off_runtime(|| match Self::entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Secrets(e.to_string())),
        })
    }

    pub fn set_token(&self, token: &str) -> Result<StorageBackend> {
        if !self.force_file {
            match Self::keyring_set(token) {
                Ok(()) => {
                    // A stale fallback file must not shadow the keychain.
                    let _ = fsutil::remove_if_exists(&self.fallback_file);
                    return Ok(StorageBackend::Keychain);
                }
                Err(e) => log::warn!("keychain unavailable ({e}); using file fallback"),
            }
        }
        let json = serde_json::to_vec(&FileSecrets {
            access_token: token.to_string(),
        })
        .map_err(|e| Error::Secrets(e.to_string()))?;
        fsutil::atomic_write(&self.fallback_file, &json, Some(0o600))?;
        Ok(StorageBackend::EncryptedFileFallback)
    }

    /// Returns the token and where it came from. `None` when logged out.
    pub fn get_token(&self) -> Result<Option<(String, StorageBackend)>> {
        if !self.force_file {
            match Self::keyring_get() {
                Ok(Some(p)) => return Ok(Some((p, StorageBackend::Keychain))),
                Ok(None) => {}
                Err(e) => log::warn!("keychain read failed ({e}); checking file fallback"),
            }
        }
        match fsutil::read_optional(&self.fallback_file)? {
            Some(bytes) => {
                let f: FileSecrets = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::Secrets(format!("corrupt credentials file: {e}")))?;
                Ok(Some((
                    f.access_token,
                    StorageBackend::EncryptedFileFallback,
                )))
            }
            None => Ok(None),
        }
    }

    pub fn clear(&self) -> Result<()> {
        if !self.force_file {
            if let Err(e) = Self::keyring_delete() {
                log::warn!("keychain delete failed: {e}");
            }
        }
        fsutil::remove_if_exists(&self.fallback_file)
    }
}

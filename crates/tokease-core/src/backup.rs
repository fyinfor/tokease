//! Point-in-time backups of the files an adapter manages.
//!
//! Layout: `<root>/<client>/<backup_id>/manifest.json` + one stored copy per
//! file. Files that did not exist at backup time are recorded as such so that
//! `restore` can remove whatever we created.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::adapters::ClientId;
use crate::error::{Error, Result};
use crate::fsutil;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    /// Absolute original location.
    pub original_path: PathBuf,
    /// Whether the file existed when the backup was taken.
    pub existed: bool,
    /// File name inside the backup directory (when `existed`).
    pub stored_as: Option<String>,
    /// Unix permission bits (when known).
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupManifest {
    pub id: String,
    pub client: ClientId,
    pub created_at: String,
    pub files: Vec<BackupFile>,
    #[serde(skip)]
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BackupStore {
    root: PathBuf,
}

impl BackupStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn client_dir(&self, client: ClientId) -> PathBuf {
        self.root.join(client.as_str())
    }

    /// Snapshot `paths` into a new backup directory.
    pub fn create(&self, client: ClientId, paths: &[PathBuf]) -> Result<BackupManifest> {
        let now = chrono::Local::now();
        let base_id = now.format("%Y%m%d-%H%M%S").to_string();
        let client_dir = self.client_dir(client);
        fs::create_dir_all(&client_dir).map_err(|e| Error::io(&client_dir, e))?;

        // Avoid collisions if two backups happen within the same second.
        let mut id = base_id.clone();
        let mut n = 1;
        while client_dir.join(&id).exists() {
            id = format!("{base_id}-{n}");
            n += 1;
        }
        let dir = client_dir.join(&id);
        fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;

        let mut files = Vec::with_capacity(paths.len());
        for (i, p) in paths.iter().enumerate() {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "file".into());
            let stored = format!("{i}-{name}");
            let existed = p.is_file();
            if existed {
                fs::copy(p, dir.join(&stored)).map_err(|e| Error::io(p, e))?;
            }
            files.push(BackupFile {
                original_path: p.clone(),
                existed,
                stored_as: existed.then_some(stored),
                mode: fsutil::file_mode(p),
            });
        }

        let manifest = BackupManifest {
            id,
            client,
            created_at: now.to_rfc3339(),
            files,
            dir: dir.clone(),
        };
        let json = serde_json::to_vec_pretty(&manifest).map_err(|e| Error::Other(e.to_string()))?;
        fsutil::atomic_write(&dir.join("manifest.json"), &json, Some(0o600))?;
        log::info!(
            "backup {} created for {} ({} files)",
            manifest.id,
            client,
            manifest.files.len()
        );
        Ok(manifest)
    }

    /// Put every file back the way it was. Files that did not exist are removed.
    pub fn restore(&self, manifest: &BackupManifest) -> Result<()> {
        for f in &manifest.files {
            if f.existed {
                let stored = manifest
                    .dir
                    .join(f.stored_as.as_deref().unwrap_or_default());
                let bytes = fs::read(&stored).map_err(|e| Error::io(&stored, e))?;
                fsutil::atomic_write(&f.original_path, &bytes, f.mode)?;
            } else {
                fsutil::remove_if_exists(&f.original_path)?;
            }
        }
        log::info!("backup {} restored for {}", manifest.id, manifest.client);
        Ok(())
    }

    pub fn load(&self, client: ClientId, id: &str) -> Result<BackupManifest> {
        let dir = self.client_dir(client).join(id);
        let path = dir.join("manifest.json");
        let bytes = fs::read(&path).map_err(|e| Error::io(&path, e))?;
        let mut m: BackupManifest = serde_json::from_slice(&bytes).map_err(|e| Error::Json {
            path: path.clone(),
            message: e.to_string(),
        })?;
        m.dir = dir;
        Ok(m)
    }

    /// All backups for a client, newest first.
    pub fn list(&self, client: ClientId) -> Result<Vec<BackupManifest>> {
        let dir = self.client_dir(client);
        let mut out = Vec::new();
        let rd = match fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(Error::io(&dir, e)),
        };
        for entry in rd.flatten() {
            if entry.path().join("manifest.json").is_file() {
                if let Ok(m) = self.load(client, &entry.file_name().to_string_lossy()) {
                    out.push(m);
                }
            }
        }
        out.sort_by(|a, b| b.id.cmp(&a.id));
        Ok(out)
    }

    pub fn latest(&self, client: ClientId) -> Result<Option<BackupManifest>> {
        Ok(self.list(client)?.into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_including_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let store = BackupStore::new(tmp.path().join("backups"));
        let existing = tmp.path().join("cfg.toml");
        let missing = tmp.path().join("auth.json");
        fs::write(&existing, "original").unwrap();

        let m = store
            .create(ClientId::Codex, &[existing.clone(), missing.clone()])
            .unwrap();
        assert!(m.files[0].existed && !m.files[1].existed);

        fs::write(&existing, "changed").unwrap();
        fs::write(&missing, "created").unwrap();

        let loaded = store.load(ClientId::Codex, &m.id).unwrap();
        store.restore(&loaded).unwrap();
        assert_eq!(fs::read_to_string(&existing).unwrap(), "original");
        assert!(!missing.exists());
        assert_eq!(store.list(ClientId::Codex).unwrap().len(), 1);
    }
}

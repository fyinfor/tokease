//! One adapter per supported developer tool.
//!
//! An adapter knows *where* a tool keeps its config and *how* to express
//! "talk to Tokease" in that format, using the order-preserving patchers in
//! [`crate::patch`] and the key-field definitions in [`crate::floor`].
//! Everything else (backup, conflict detection, atomic writes, rollback,
//! bookkeeping) is shared and lives in this module, so adding a new tool
//! means implementing a handful of small methods, see [`Adapter`].

pub mod claude;
pub mod codex;
pub mod gemini;
pub(crate) mod locate;

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::backup::{BackupManifest, BackupStore};
use crate::error::{Error, Result};
use crate::fsutil;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientId {
    Codex,
    Claude,
    Gemini,
}

impl ClientId {
    pub const ALL: [ClientId; 3] = [ClientId::Codex, ClientId::Claude, ClientId::Gemini];

    pub fn as_str(&self) -> &'static str {
        match self {
            ClientId::Codex => "codex",
            ClientId::Claude => "claude",
            ClientId::Gemini => "gemini",
        }
    }

    pub fn parse(s: &str) -> Option<ClientId> {
        ClientId::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

impl fmt::Display for ClientId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Everything an adapter needs to point a tool at Tokease.
/// The token lives here only transiently; it is never persisted by us
/// except inside the tool's own config file.
#[derive(Debug, Clone)]
pub struct ConnectionSpec {
    pub base_url: String,
    pub token: String,
    /// Primary logical model (`code-best`).
    pub model: String,
    /// Mid tier (`code-fast`), when the platform offers one. Falls back to
    /// `model`.
    pub fast_model: Option<String>,
    /// Cheap tier (`code-cheap`), when the platform offers one. Falls back
    /// to `fast_model`, then `model`.
    pub cheap_model: Option<String>,
    /// Optional protocol hint from the server (Codex `wire_api`).
    pub wire_api: Option<String>,
}

impl ConnectionSpec {
    pub fn fast(&self) -> &str {
        self.fast_model.as_deref().unwrap_or(&self.model)
    }
    pub fn cheap(&self) -> &str {
        self.cheap_model.as_deref().unwrap_or_else(|| self.fast())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub installed: bool,
    pub binary_path: Option<PathBuf>,
    /// First line of `--version`, when the binary answered.
    pub version: Option<String>,
    pub config_dir: PathBuf,
}

/// What the tool is currently pointed at, as far as we can tell.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurrentConfig {
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub has_credential: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Validation {
    pub ok: bool,
    pub problems: Vec<String>,
}

impl Validation {
    pub fn ok() -> Self {
        Self { ok: true, problems: vec![] }
    }
    pub fn fail(problems: Vec<String>) -> Self {
        Self { ok: problems.is_empty(), problems }
    }
}

/// A file the adapter wants written.
#[derive(Debug, Clone)]
pub struct PlannedFile {
    pub path: PathBuf,
    /// Bytes observed while planning (`None`: file did not exist). Used to
    /// skip no-op writes and to detect concurrent edits before the rename.
    pub pre: Option<Vec<u8>>,
    pub content: Vec<u8>,
    /// Contains a credential: always written 0600. Otherwise the existing
    /// mode is preserved (0644 for new files).
    pub secret: bool,
}

impl PlannedFile {
    fn mode(&self) -> Option<u32> {
        if self.secret {
            Some(0o600)
        } else {
            fsutil::file_mode(&self.path).or(Some(0o644))
        }
    }

    fn is_noop(&self) -> bool {
        self.pre.as_deref() == Some(self.content.as_slice())
    }
}

pub trait Adapter: Send + Sync {
    fn id(&self) -> ClientId;
    fn display_name(&self) -> &'static str;
    /// Which protocol endpoint from `/client/config` this tool speaks.
    fn protocol(&self) -> &'static str;

    /// Is the CLI installed? Where is its config directory?
    fn detect(&self) -> Detection;

    /// Every file this adapter may modify. Backups snapshot exactly this set.
    fn managed_paths(&self) -> Vec<PathBuf>;

    /// Read current config (for status display). Must not fail on a missing
    /// or malformed file: return what can be determined.
    fn read_config(&self) -> CurrentConfig;

    /// Compute the new contents of the managed files, merging into whatever
    /// exists today. Pure with respect to the filesystem (read-only).
    fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>>;

    /// Check that the on-disk config points at `base_url` with a credential.
    fn validate_config(&self, base_url: &str) -> Validation;

    /// Environment variable names that, when set in the shell, override
    /// this tool's config file (used by the env-conflict checker). A
    /// trailing `_` means prefix match.
    fn env_conflict_prefixes(&self) -> &'static [&'static str];

    /// Oldest CLI version whose config format we write, if there is one.
    fn min_version(&self) -> Option<&'static str> {
        None
    }

    // ----- shared behaviour, rarely overridden ------------------------------

    fn backup_config(&self, backups: &BackupStore) -> Result<BackupManifest> {
        backups.create(self.id(), &self.managed_paths())
    }

    /// plan -> backup -> conflict check -> write (atomic) -> validate; roll
    /// back on any failure. Files whose content would not change are not
    /// touched at all.
    fn apply_config(&self, backups: &BackupStore, spec: &ConnectionSpec) -> Result<BackupManifest> {
        let planned = self.plan(spec)?;
        let changed: Vec<&PlannedFile> = planned.iter().filter(|f| !f.is_noop()).collect();
        for f in planned.iter().filter(|f| f.is_noop()) {
            log::info!("{}: {} already up to date", self.id(), f.path.display());
        }
        let manifest = self.backup_config(backups)?;

        let result = (|| -> Result<()> {
            // The file must still be what we planned against: another tool
            // (or the CLI itself) may have written it meanwhile.
            for f in &changed {
                if fsutil::read_optional(&f.path)? != f.pre {
                    return Err(Error::Conflict { path: f.path.clone() });
                }
            }
            for f in &changed {
                fsutil::atomic_write(&f.path, &f.content, f.mode())?;
                log::info!("{}: wrote {}", self.id(), f.path.display());
            }
            let v = self.validate_config(&spec.base_url);
            if v.ok {
                Ok(())
            } else {
                Err(Error::Validation(v.problems))
            }
        })();

        match result {
            Ok(()) => Ok(manifest),
            Err(source) => {
                log::error!("{}: apply failed ({source}); rolling back", self.id());
                match backups.restore(&manifest) {
                    Ok(()) => Err(Error::RolledBack { source: Box::new(source) }),
                    Err(rollback) => Err(Error::RollbackFailed {
                        source: Box::new(source),
                        rollback: Box::new(rollback),
                        backup_dir: manifest.dir.clone(),
                    }),
                }
            }
        }
    }

    fn restore_config(&self, backups: &BackupStore, manifest: &BackupManifest) -> Result<()> {
        backups.restore(manifest)
    }
}

pub(crate) fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

/// Shared detection: binary on PATH / known locations, or the config dir
/// exists (the tool was used before even if we cannot see its binary).
pub(crate) fn detect_tool(name: &str, absolute: &[&str], config_dir: &std::path::Path) -> Detection {
    let binary_path = locate::find_binary(&[name], &locate::candidates(name, absolute));
    let version = binary_path.as_deref().and_then(locate::version_of);
    Detection {
        installed: binary_path.is_some() || config_dir.is_dir(),
        binary_path,
        version,
        config_dir: config_dir.to_path_buf(),
    }
}

/// Instantiate all built-in adapters with default (env-aware) paths.
pub fn all() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(codex::CodexAdapter::default()),
        Box::new(claude::ClaudeAdapter::default()),
        Box::new(gemini::GeminiAdapter::default()),
    ]
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;

    pub fn spec() -> ConnectionSpec {
        ConnectionSpec {
            base_url: "https://api.tokease.test/v1".into(),
            token: "tk_test_1234567890abcdef".into(),
            model: "code-best".into(),
            fast_model: Some("code-fast".into()),
            cheap_model: Some("code-cheap".into()),
            wire_api: None,
        }
    }

    /// Generic closed-loop test: existing config -> apply -> validate ->
    /// restore -> byte-identical.
    pub fn closed_loop(adapter: &dyn Adapter, seed: &[(&std::path::Path, &str)]) {
        let tmp = tempfile::tempdir().unwrap();
        let backups = BackupStore::new(tmp.path());
        for (p, c) in seed {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, c).unwrap();
        }
        let before: Vec<Option<Vec<u8>>> =
            adapter.managed_paths().iter().map(|p| std::fs::read(p).ok()).collect();

        let s = spec();
        assert!(!adapter.validate_config(&s.base_url).ok, "must not validate before apply");
        let manifest = adapter.apply_config(&backups, &s).unwrap();
        let v = adapter.validate_config(&s.base_url);
        assert!(v.ok, "validate after apply: {:?}", v.problems);
        let cur = adapter.read_config();
        assert_eq!(cur.base_url.as_deref(), Some(s.base_url.as_str()));
        assert!(cur.has_credential);

        // Enabling twice is a no-op: the second plan changes nothing.
        let again = adapter.plan(&s).unwrap();
        assert!(again.iter().all(|f| f.is_noop()), "second apply must be a no-op");

        adapter.restore_config(&backups, &manifest).unwrap();
        let after: Vec<Option<Vec<u8>>> =
            adapter.managed_paths().iter().map(|p| std::fs::read(p).ok()).collect();
        assert_eq!(before, after, "restore must be byte-identical");
        assert!(!adapter.validate_config(&s.base_url).ok);
    }

    /// Writes a file but never validates: exercises the rollback path.
    struct BrokenAdapter(PathBuf);

    impl Adapter for BrokenAdapter {
        fn id(&self) -> ClientId {
            ClientId::Codex
        }
        fn display_name(&self) -> &'static str {
            "broken"
        }
        fn protocol(&self) -> &'static str {
            "openai"
        }
        fn detect(&self) -> Detection {
            Detection { installed: true, binary_path: None, version: None, config_dir: self.0.clone() }
        }
        fn managed_paths(&self) -> Vec<PathBuf> {
            vec![self.0.join("cfg"), self.0.join("new-file")]
        }
        fn read_config(&self) -> CurrentConfig {
            CurrentConfig::default()
        }
        fn plan(&self, _: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
            Ok(vec![
                PlannedFile {
                    path: self.0.join("cfg"),
                    pre: fsutil::read_optional(&self.0.join("cfg"))?,
                    content: b"broken".to_vec(),
                    secret: false,
                },
                PlannedFile { path: self.0.join("new-file"), pre: None, content: b"x".to_vec(), secret: true },
            ])
        }
        fn validate_config(&self, _: &str) -> Validation {
            Validation::fail(vec!["always invalid".into()])
        }
        fn env_conflict_prefixes(&self) -> &'static [&'static str] {
            &[]
        }
    }

    #[test]
    fn failed_validation_rolls_back() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("tool");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cfg"), "original").unwrap();
        let backups = BackupStore::new(tmp.path().join("backups"));
        let a = BrokenAdapter(dir.clone());

        let err = a.apply_config(&backups, &spec()).unwrap_err();
        assert!(matches!(err, Error::RolledBack { .. }), "{err}");
        assert_eq!(std::fs::read_to_string(dir.join("cfg")).unwrap(), "original");
        assert!(!dir.join("new-file").exists(), "file created during apply must be removed");
    }

    #[test]
    fn concurrent_edit_is_detected_before_writing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("tool");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cfg"), "original").unwrap();
        let backups = BackupStore::new(tmp.path().join("backups"));

        struct Racy(PathBuf);
        impl Adapter for Racy {
            fn id(&self) -> ClientId {
                ClientId::Codex
            }
            fn display_name(&self) -> &'static str {
                "racy"
            }
            fn protocol(&self) -> &'static str {
                "openai"
            }
            fn detect(&self) -> Detection {
                Detection { installed: true, binary_path: None, version: None, config_dir: self.0.clone() }
            }
            fn managed_paths(&self) -> Vec<PathBuf> {
                vec![self.0.join("cfg")]
            }
            fn read_config(&self) -> CurrentConfig {
                CurrentConfig::default()
            }
            fn plan(&self, _: &ConnectionSpec) -> Result<Vec<PlannedFile>> {
                // Someone else writes the file right after we planned.
                std::fs::write(self.0.join("cfg"), "someone else").unwrap();
                Ok(vec![PlannedFile {
                    path: self.0.join("cfg"),
                    pre: Some(b"original".to_vec()),
                    content: b"ours".to_vec(),
                    secret: false,
                }])
            }
            fn validate_config(&self, _: &str) -> Validation {
                Validation::ok()
            }
            fn env_conflict_prefixes(&self) -> &'static [&'static str] {
                &[]
            }
        }

        let err = Racy(dir.clone()).apply_config(&backups, &spec()).unwrap_err();
        assert!(matches!(err, Error::RolledBack { ref source } if matches!(**source, Error::Conflict { .. })), "{err}");
        assert_eq!(std::fs::read_to_string(dir.join("cfg")).unwrap(), "someone else");
    }
}

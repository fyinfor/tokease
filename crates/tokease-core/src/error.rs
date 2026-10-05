use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid TOML in {path}: {message}")]
    Toml { path: PathBuf, message: String },

    #[error("invalid JSON in {path}: {message}")]
    Json { path: PathBuf, message: String },

    /// The file could not be patched safely (unparsable, wrong shape, or a
    /// Codex profile overrides the route). Nothing was written.
    #[error(transparent)]
    Patch(#[from] crate::patch::PatchError),

    /// The file changed on disk between planning and writing (another tool
    /// or the CLI itself wrote it). Nothing was written; just retry.
    #[error(
        "{path} changed while Tokease was preparing the write; nothing was written, please retry"
    )]
    Conflict { path: PathBuf },

    #[error("{client} is not installed")]
    NotInstalled { client: String },

    #[error("{client} is not available on your Tokease plan")]
    ClientDisabled { client: String },

    #[error("not logged in")]
    NotLoggedIn,

    #[error("Tokease API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("secure storage error: {0}")]
    Secrets(String),

    #[error("no backup found for {client}")]
    NoBackup { client: String },

    #[error("validation failed after apply: {0:?}")]
    Validation(Vec<String>),

    #[error("apply failed ({source}); original config was restored")]
    RolledBack {
        #[source]
        source: Box<Error>,
    },

    #[error(
        "apply failed ({source}) AND rollback failed ({rollback}); backup kept at {backup_dir}"
    )]
    RollbackFailed {
        source: Box<Error>,
        rollback: Box<Error>,
        backup_dir: PathBuf,
    },

    #[error("{0}")]
    Other(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        // reqwest errors may embed the URL but never the body/token.
        Error::Network(e.without_url().to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

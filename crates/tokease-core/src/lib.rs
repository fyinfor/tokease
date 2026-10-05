//! Tokease core: everything that touches the local machine (CLI detection,
//! config files, backups, secure token storage) plus the thin Tokease API
//! client. The Tauri shell and the `tokease-cli` binary are both consumers of
//! [`service::Tokease`]; neither contains business logic of its own.
//!
//! Config writing follows CC Switch's model: provider-owned *key fields*
//! ([`floor`]) are cleared and rewritten through order-preserving patchers
//! ([`patch`]); everything else in the user's files is left alone; a file
//! that cannot be parsed is never overwritten; writes are atomic, backed up
//! and checked for concurrent edits ([`adapters::Adapter::apply_config`]).

pub mod adapters;
pub mod history;
pub mod skills;
pub mod api;
pub mod backup;
pub mod envcheck;
pub mod error;
pub mod floor;
pub mod fsutil;
pub mod patch;
pub mod redact;
pub mod secrets;
pub mod service;
pub mod state;

pub use adapters::{Adapter, ClientId, ConnectionSpec, Detection, Validation};
pub use api::{ClientConfig, ModelInfo, UserInfo};
pub use error::{Error, Result};
pub use history::{ChatMessage, ChatSession, ChatTranscript};
pub use skills::LocalSkill;
pub use service::{ClientStatus, SessionInfo, Tokease};

/// Default Tokease API server. Auth endpoints (`/auth/*`, `/client/config`)
/// hang off this URL and it doubles as the OpenAI-compatible base URL when
/// the server does not send its own.
pub const DEFAULT_SERVER_URL: &str = "https://www.tokease.cn/v1";

/// The only servers the desktop app will save. Developers can still point a
/// local process at the mock with `TOKEASE_SERVER_URL`.
pub const SERVER_URLS: [&str; 2] = ["https://www.tokease.cn/v1", "https://www.tokease.com/v1"];

/// Returns the canonical official URL, ignoring a trailing slash.
pub fn official_server(url: &str) -> Option<&'static str> {
    let url = url.trim().trim_end_matches('/');
    SERVER_URLS.into_iter().find(|candidate| *candidate == url)
}

/// Identifier written into every config we manage so that
/// `validate()` can recognise its own work.
pub const PROVIDER_ID: &str = "tokease";

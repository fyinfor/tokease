use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use tokease_core::api::{ClientConfig, DeviceStart};
use tokease_core::service::{BackupSummary, LoginPoll};
use tokease_core::{ClientId, ClientStatus, Error, SessionInfo, Tokease};

type Core<'a> = State<'a, Arc<Tokease>>;

/// Error shape sent to the frontend. `kind` lets the UI pick a message
/// (e.g. show "install Codex first" instead of a generic failure).
#[derive(Debug, Serialize)]
pub struct CmdError {
    pub kind: &'static str,
    pub message: String,
}

impl From<Error> for CmdError {
    fn from(e: Error) -> Self {
        let kind = match &e {
            Error::NotInstalled { .. } => "not_installed",
            Error::ClientDisabled { .. } => "client_disabled",
            Error::NotLoggedIn => "not_logged_in",
            Error::Api { status: 401, .. } => "unauthorized",
            Error::Api { .. } => "api",
            Error::Network(_) => "network",
            Error::Secrets(_) => "secrets",
            Error::NoBackup { .. } => "no_backup",
            Error::RolledBack { source } if matches!(**source, Error::Conflict { .. }) => {
                "conflict"
            }
            Error::RolledBack { .. } => "rolled_back",
            Error::RollbackFailed { .. } => "rollback_failed",
            Error::Patch(_) => "patch",
            Error::Conflict { .. } => "conflict",
            _ => "other",
        };
        CmdError {
            kind,
            message: e.to_string(),
        }
    }
}

type CmdResult<T> = Result<T, CmdError>;

fn parse_client(id: &str) -> CmdResult<ClientId> {
    ClientId::parse(id).ok_or_else(|| CmdError {
        kind: "other",
        message: format!("unknown client {id:?}"),
    })
}

/// Run a synchronous core call off the async executor (fs + keychain).
async fn blocking<T: Send + 'static>(
    core: &Arc<Tokease>,
    f: impl FnOnce(&Tokease) -> tokease_core::Result<T> + Send + 'static,
) -> CmdResult<T> {
    let core = core.clone();
    tauri::async_runtime::spawn_blocking(move || f(&core))
        .await
        .map_err(|e| CmdError {
            kind: "other",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn get_session(core: Core<'_>) -> CmdResult<SessionInfo> {
    blocking(&core, |c| c.session()).await
}

#[tauri::command]
pub async fn set_server_url(core: Core<'_>, url: String) -> CmdResult<SessionInfo> {
    blocking(&core, move |c| c.set_server_url(&url)).await
}

#[tauri::command]
pub async fn start_device_login(core: Core<'_>) -> CmdResult<DeviceStart> {
    Ok(core.device_start().await?)
}

#[tauri::command]
pub async fn poll_device_login(core: Core<'_>, device_code: String) -> CmdResult<LoginPoll> {
    Ok(core.device_poll(&device_code).await?)
}

#[tauri::command]
pub async fn login_with_password(
    core: Core<'_>,
    email: String,
    password: String,
) -> CmdResult<SessionInfo> {
    Ok(core.login_password(&email, &password).await?)
}

#[tauri::command]
pub async fn logout(core: Core<'_>) -> CmdResult<SessionInfo> {
    blocking(&core, |c| c.logout()).await
}

#[tauri::command]
pub async fn list_clients(core: Core<'_>) -> CmdResult<Vec<ClientStatus>> {
    blocking(&core, |c| c.client_statuses()).await
}

#[tauri::command]
pub async fn enable_client(core: Core<'_>, id: String) -> CmdResult<ClientStatus> {
    let id = parse_client(&id)?;
    Ok(core.enable(id).await?)
}

#[tauri::command]
pub async fn restore_client(
    core: Core<'_>,
    id: String,
    backup_id: Option<String>,
) -> CmdResult<ClientStatus> {
    let id = parse_client(&id)?;
    blocking(&core, move |c| c.restore(id, backup_id.as_deref())).await
}

#[tauri::command]
pub async fn list_backups(core: Core<'_>, id: String) -> CmdResult<Vec<BackupSummary>> {
    let id = parse_client(&id)?;
    blocking(&core, move |c| c.backups(id)).await
}

#[tauri::command]
pub async fn get_platform_config(core: Core<'_>) -> CmdResult<Option<ClientConfig>> {
    blocking(&core, |c| c.cached_platform_config()).await
}

#[tauri::command]
pub async fn refresh_platform_config(core: Core<'_>) -> CmdResult<ClientConfig> {
    Ok(core.refresh_platform_config().await?)
}

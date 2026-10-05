//! Thin client for the Tokease server.
//!
//! Endpoints (see README "服务端接口"):
//!   POST /auth/device            -> start device-code login
//!   GET  /auth/device/status     -> poll device-code login
//!   POST /auth/login             -> email + password login
//!   GET  /client/config          -> base URL, models, enabled clients

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::redact;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Per-client switch. The server may send a plain boolean or an object with
/// extra protocol hints (e.g. `{"enabled": true, "wire_api": "chat"}`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(from = "ClientFlagWire")]
pub struct ClientFlag {
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wire_api: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ClientFlagWire {
    Bool(bool),
    Obj {
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default)]
        wire_api: Option<String>,
    },
}

fn default_true() -> bool {
    true
}

impl From<ClientFlagWire> for ClientFlag {
    fn from(w: ClientFlagWire) -> Self {
        match w {
            ClientFlagWire::Bool(b) => ClientFlag { enabled: b, wire_api: None },
            ClientFlagWire::Obj { enabled, wire_api } => ClientFlag { enabled, wire_api },
        }
    }
}

/// `GET /client/config` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// OpenAI-compatible base URL (`.../v1`). Used when `endpoints` has no
    /// protocol-specific entry.
    pub base_url: String,
    /// Optional protocol-specific base URLs: `openai`, `anthropic`, `gemini`.
    #[serde(default)]
    pub endpoints: BTreeMap<String, String>,
    pub models: Vec<ModelInfo>,
    pub default_model: String,
    #[serde(default)]
    pub clients: BTreeMap<String, ClientFlag>,
    /// Optional explicit tier mapping (`"fast"` / `"cheap"` → model id).
    /// Without it the tiers are derived from the model ids (`code-fast`, …).
    #[serde(default)]
    pub tiers: BTreeMap<String, String>,
}

impl ClientConfig {
    pub fn endpoint(&self, protocol: &str) -> &str {
        self.endpoints.get(protocol).map(String::as_str).unwrap_or(&self.base_url)
    }

    pub fn client(&self, id: &str) -> ClientFlag {
        self.clients.get(id).cloned().unwrap_or(ClientFlag { enabled: true, wire_api: None })
    }

    /// Model id for a tier (`fast`, `cheap`), if the platform offers one.
    pub fn tier(&self, name: &str) -> Option<String> {
        if let Some(id) = self.tiers.get(name) {
            return Some(id.clone());
        }
        let suffix = format!("-{name}");
        self.models.iter().find(|m| m.id.ends_with(&suffix) || m.id == name).map(|m| m.id.clone())
    }

    /// What we assume when the server has no `/client/config` yet: the
    /// server URL is the OpenAI-compatible `/v1` base; Anthropic and Gemini
    /// SDKs add their own `/v1/...` / `/v1beta/...` so they get the root.
    pub fn builtin(server_url: &str) -> ClientConfig {
        let trimmed = server_url.trim_end_matches('/');
        let (root, v1) = match trimmed.strip_suffix("/v1") {
            Some(root) => (root.to_string(), trimmed.to_string()),
            None => (trimmed.to_string(), format!("{trimmed}/v1")),
        };
        let model = |id: &str, name: &str| ModelInfo { id: id.into(), name: name.into(), description: None };
        ClientConfig {
            base_url: v1.clone(),
            endpoints: BTreeMap::from([
                ("openai".to_string(), v1),
                ("anthropic".to_string(), root.clone()),
                ("gemini".to_string(), root),
            ]),
            models: vec![model("code-best", "最佳编程"), model("code-fast", "快速编程"), model("code-cheap", "经济编程")],
            default_model: "code-best".into(),
            clients: BTreeMap::new(),
            tiers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserInfo {
    pub id: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStart {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    Pending,
    Authorized,
    Expired,
    Denied,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DevicePoll {
    pub status: DeviceStatus,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub user: Option<UserInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub user: UserInfo,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    #[serde(alias = "error", alias = "message")]
    message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    base: String,
    http: reqwest::Client,
}

impl ApiClient {
    pub fn new(server_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("tokease-desktop/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client");
        Self { base: server_url.trim_end_matches('/').to_string(), http }
    }

    pub fn server_url(&self) -> &str {
        &self.base
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }

    async fn handle<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
        let status = resp.status();
        if status.is_success() {
            return Ok(resp.json::<T>().await?);
        }
        let message = resp
            .json::<ApiErrorBody>()
            .await
            .ok()
            .and_then(|b| b.message)
            .unwrap_or_else(|| status.canonical_reason().unwrap_or("request failed").to_string());
        Err(Error::Api { status: status.as_u16(), message })
    }

    pub async fn device_start(&self) -> Result<DeviceStart> {
        let resp = self
            .http
            .post(self.url("/auth/device"))
            .json(&serde_json::json!({ "client": "tokease-desktop" }))
            .send()
            .await?;
        Self::handle(resp).await
    }

    pub async fn device_poll(&self, device_code: &str) -> Result<DevicePoll> {
        let resp = self
            .http
            .get(self.url("/auth/device/status"))
            .query(&[("device_code", device_code)])
            .send()
            .await?;
        Self::handle(resp).await
    }

    pub async fn login_password(&self, email: &str, password: &str) -> Result<LoginResponse> {
        let resp = self
            .http
            .post(self.url("/auth/login"))
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send()
            .await?;
        Self::handle(resp).await
    }

    pub async fn client_config(&self, token: &str) -> Result<ClientConfig> {
        log::debug!("fetching /client/config with token {}", redact::token(token));
        let resp = self
            .http
            .get(self.url("/client/config"))
            .bearer_auth(token)
            .send()
            .await?;
        Self::handle(resp).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_example_payload() {
        let json = r#"{
          "base_url": "https://api.tokease.com/v1",
          "models": [{"id":"code-best","name":"最佳编程"},{"id":"code-fast","name":"快速编程"}],
          "default_model": "code-best",
          "clients": {"codex": true, "claude": {"enabled": false}, "gemini": {"wire_api": "x"}}
        }"#;
        let c: ClientConfig = serde_json::from_str(json).unwrap();
        assert_eq!(c.endpoint("anthropic"), "https://api.tokease.com/v1");
        assert!(c.client("codex").enabled);
        assert!(!c.client("claude").enabled);
        assert!(c.client("gemini").enabled);
        assert!(c.client("unknown").enabled);
        assert_eq!(c.tier("fast").as_deref(), Some("code-fast"));
        assert_eq!(c.tier("cheap"), None);
    }

    #[test]
    fn builtin_config_derives_endpoints_from_the_server_url() {
        let c = ClientConfig::builtin("https://www.tokease.cn/v1/");
        assert_eq!(c.endpoint("openai"), "https://www.tokease.cn/v1");
        assert_eq!(c.endpoint("anthropic"), "https://www.tokease.cn");
        assert_eq!(c.endpoint("gemini"), "https://www.tokease.cn");
        assert_eq!(c.default_model, "code-best");
        assert_eq!(c.tier("cheap").as_deref(), Some("code-cheap"));

        let c = ClientConfig::builtin("http://127.0.0.1:8787");
        assert_eq!(c.endpoint("openai"), "http://127.0.0.1:8787/v1");
        assert_eq!(c.endpoint("anthropic"), "http://127.0.0.1:8787");
    }
}

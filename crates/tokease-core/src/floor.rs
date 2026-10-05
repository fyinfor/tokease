//! Key fields ("the floor") — ported from CC Switch's `live/floor.rs`.
//!
//! Key fields answer four questions: where do requests go, what credential
//! is used, which model name, which protocol. They belong entirely to the
//! active provider: enabling Tokease clears them all and writes Tokease's
//! values; everything else in the file belongs to the user and the client
//! and is neither written nor removed.
//!
//! Prefix matching is used only where a whole prefix is about connection
//! and auth (`ANTHROPIC_*`, `AWS_*`, Gemini's `GOOGLE_*`). A key we fail to
//! list is kept as a user key, so mistakes fail in the safe direction.

/// Claude Code protocol selectors.
///
/// `CLAUDE_CODE_USE_` cannot be matched by prefix: the same prefix carries
/// `USE_POWERSHELL_TOOL`, `USE_NATIVE_FILE_SEARCH`, `USE_COWORK_PLUGINS`,
/// `USE_CCR_V2` — feature switches unrelated to the provider.
pub const CLAUDE_PROTOCOL_SELECTORS: &[&str] = &[
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
    "CLAUDE_CODE_USE_GATEWAY",
    "CLAUDE_CODE_USE_MANTLE",
    "CLAUDE_CODE_USE_ANTHROPIC_AWS",
    "CLAUDE_CODE_USE_ANTHROPIC_GOOGLE_CLOUD",
];

/// `env` prefixes that are entirely connection/auth: `ANTHROPIC_*` (URL,
/// credentials, per-tier model names, custom headers), `AWS_*` (Bedrock
/// region and credentials), `VERTEX_REGION_*` (Vertex per-model regions).
pub const CLAUDE_FLOOR_ENV_PREFIXES: &[&str] = &["ANTHROPIC_", "AWS_", "VERTEX_REGION_"];

/// `env` key fields listed by name (besides the protocol selectors).
pub const CLAUDE_FLOOR_ENV_KEYS: &[&str] = &[
    "CLAUDE_CODE_SUBAGENT_MODEL",
    "CLAUDE_CODE_SUBAGENT_MODEL_FORCE",
    "CLOUD_ML_REGION",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "CLAUDE_CODE_OAUTH_REFRESH_TOKEN",
    "CLAUDE_CODE_OAUTH_SCOPES",
    "CLAUDE_CODE_API_KEY_HELPER_TTL_MS",
];

/// Is this key in Claude Code's `settings.json` `env` a key field?
pub fn claude_floor_env(key: &str) -> bool {
    CLAUDE_FLOOR_ENV_PREFIXES.iter().any(|p| key.starts_with(p))
        || CLAUDE_PROTOCOL_SELECTORS.contains(&key)
        || CLAUDE_FLOOR_ENV_KEYS.contains(&key)
        || (key.starts_with("CLAUDE_CODE_SKIP_") && key.ends_with("_AUTH"))
}

/// Top-level key fields of Claude Code's `settings.json`.
pub const CLAUDE_FLOOR_TOP: &[&str] = &[
    "apiKeyHelper",
    "apiBaseUrl",
    "primaryModel",
    "smallFastModel",
    "apiKey",
    // `/model` saves its choice here; it belongs to the provider active then.
    "model",
    "fallbackModel",
    "modelOverrides",
    // advisor is only available on the Anthropic API.
    "advisorModel",
    "awsAuthRefresh",
    "awsCredentialExport",
    "gcpAuthRefresh",
];

pub fn claude_floor_top(key: &str) -> bool {
    CLAUDE_FLOOR_TOP.contains(&key)
}

/// Claude Code provider-exclusive `env` fields: upstream compatibility
/// switches and window values. Written on enable, removed on restore only
/// through the backup (we never guess the user's intent).
pub const CLAUDE_EXCLUSIVE_ENV: &[&str] = &[
    "CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS",
    "CLAUDE_CODE_DISABLE_ARTIFACT",
    "ENABLE_TOOL_SEARCH",
    "CLAUDE_CODE_DISABLE_THINKING",
    "DISABLE_INTERLEAVED_THINKING",
    "CLAUDE_CODE_ALWAYS_ENABLE_EFFORT",
    "CLAUDE_CODE_EXTRA_BODY",
    "CLAUDE_CODE_ENABLE_FINE_GRAINED_TOOL_STREAMING",
    "CLAUDE_CODE_MAX_CONTEXT_TOKENS",
    "CLAUDE_CODE_AUTO_COMPACT_WINDOW",
    "CLAUDE_CODE_MAX_OUTPUT_TOKENS",
    "CLAUDE_CODE_DISABLE_1M_CONTEXT",
    "CLAUDE_CODE_DISABLE_UNKNOWN_MODEL_WINDOW_ENFORCEMENT",
    "CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY",
];

/// Top-level key fields of Codex `config.toml`. The `[model_providers.*]`
/// table Tokease owns is handled separately.
pub const CODEX_FLOOR_TOP: &[&str] = &[
    // Routing: `openai_base_url` reroutes the built-in openai provider.
    "model_provider",
    "openai_base_url",
    "model",
    "review_model",
    "model_reasoning_effort",
    "plan_mode_reasoning_effort",
    "disable_response_storage",
    "model_catalog_json",
    // Fallback spelling puts the key at the top level.
    "experimental_bearer_token",
    // Old shape: without `model_provider` these sat at the top level.
    "base_url",
    "wire_api",
];

/// Model names nested in user-owned tables: only these keys are cleared,
/// the rest of each table is untouched.
pub const CODEX_FLOOR_NESTED: &[&[&str]] = &[
    &["agents", "default_subagent_model"],
    &["agents", "default_subagent_reasoning_effort"],
    &["memories", "extract_model"],
    &["memories", "consolidation_model"],
];

/// Codex provider-exclusive top-level fields (window values / switches that
/// depend on the upstream model).
pub const CODEX_EXCLUSIVE_TOP: &[&str] = &[
    "web_search",
    "model_context_window",
    "model_auto_compact_token_limit",
    "model_supports_reasoning_summaries",
    "model_verbosity",
];

/// `[model_providers.<id>]` ids Codex (0.148+) refuses to load a config for.
pub const CODEX_RESERVED_PROVIDER_IDS: &[&str] = &["openai", "ollama", "lmstudio"];

/// Is this `.env` key a Gemini CLI key field?
///
/// `GOOGLE_*` is entirely connection/auth. `GEMINI_*` cannot be matched by
/// prefix: `GEMINI_CLI_HOME`, `GEMINI_SANDBOX`, `GEMINI_SYSTEM_MD`, telemetry
/// switches and so on are user settings.
pub fn gemini_floor_env(key: &str) -> bool {
    key.starts_with("GOOGLE_")
        || matches!(
            key,
            "GEMINI_API_KEY"
                | "GEMINI_MODEL"
                | "GEMINI_API_KEY_AUTH_MECHANISM"
                | "GEMINI_CLI_CUSTOM_HEADERS"
                | "GEMINI_DEFAULT_AUTH_TYPE"
                | "GEMINI_CLI_USE_COMPUTE_ADC"
                | "CODE_ASSIST_ENDPOINT"
                | "CODE_ASSIST_API_VERSION"
        )
}

/// Key fields in Gemini CLI `settings.json`, by key path.
pub const GEMINI_FLOOR_SETTINGS: &[&[&str]] =
    &[&["security", "auth", "selectedType"], &["model", "name"]];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_floor_covers_connection_keys_and_leaves_switches() {
        for key in [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
            "AWS_REGION",
            "VERTEX_REGION_CLAUDE_4_5_SONNET",
            "CLOUD_ML_REGION",
            "CLAUDE_CODE_OAUTH_TOKEN",
        ] {
            assert!(claude_floor_env(key), "{key}");
        }
        for key in [
            "CLAUDE_CODE_USE_POWERSHELL_TOOL",
            "CLAUDE_CODE_SKIP_PROMPT_HISTORY",
            "API_TIMEOUT_MS",
            "DISABLE_TELEMETRY",
            "CLAUDE_CODE_DISABLE_ARTIFACT",
        ] {
            assert!(!claude_floor_env(key), "{key}");
        }
        for key in ["hooks", "permissions", "env", "statusLine"] {
            assert!(!claude_floor_top(key), "{key}");
        }
    }

    #[test]
    fn gemini_floor_leaves_user_settings() {
        for key in [
            "GOOGLE_GEMINI_BASE_URL",
            "GOOGLE_API_KEY",
            "GEMINI_API_KEY",
            "CODE_ASSIST_ENDPOINT",
        ] {
            assert!(gemini_floor_env(key), "{key}");
        }
        for key in [
            "GEMINI_CLI_HOME",
            "GEMINI_SANDBOX",
            "GEMINI_SYSTEM_MD",
            "DEBUG",
        ] {
            assert!(!gemini_floor_env(key), "{key}");
        }
    }

    #[test]
    fn exclusive_fields_never_overlap_key_fields() {
        for key in CLAUDE_EXCLUSIVE_ENV {
            assert!(!claude_floor_env(key), "{key}");
        }
        for key in CODEX_EXCLUSIVE_TOP {
            assert!(!CODEX_FLOOR_TOP.contains(key), "{key}");
        }
    }
}

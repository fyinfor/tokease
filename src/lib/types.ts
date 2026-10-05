// Mirrors the serde shapes in crates/tokease-core (service.rs / api.rs).

export type ClientId = "codex" | "claude" | "claude-desktop" | "gemini" | "grok" | "opencode";

export interface UserInfo {
  id: string;
  email?: string | null;
  name?: string | null;
}

export type StorageBackend = "keychain" | "encrypted_file_fallback" | "none";

export interface SessionInfo {
  logged_in: boolean;
  user: UserInfo | null;
  server_url: string;
  storage_backend: StorageBackend;
  token_preview: string | null;
  data_dir: string;
}

export interface CurrentConfig {
  base_url: string | null;
  model: string | null;
  has_credential: boolean;
}

/** A shell/process environment variable that overrides the config file. */
export interface EnvConflict {
  name: string;
  /** "process" or the shell file (e.g. "~/.zshrc"). */
  source: string;
  /** Masked value, never the full secret. */
  preview: string;
}

export interface ClientStatus {
  id: ClientId;
  name: string;
  installed: boolean;
  binary_path: string | null;
  version: string | null;
  min_version: string | null;
  outdated: boolean;
  config_dir: string;
  managed_files: string[];
  available: boolean;
  enabled: boolean;
  current: CurrentConfig;
  problems: string[];
  env_conflicts: EnvConflict[];
  enabled_at: string | null;
  backup_id: string | null;
}

export interface ModelInfo {
  id: string;
  name: string;
  description?: string;
}

export interface ClientConfig {
  base_url: string;
  endpoints: Record<string, string>;
  models: ModelInfo[];
  default_model: string;
  clients: Record<string, { enabled: boolean; wire_api?: string }>;
  tiers?: Record<string, string>;
}

export interface DeviceStart {
  device_code: string;
  user_code: string;
  verification_uri: string;
  verification_uri_complete?: string | null;
  expires_in: number;
  interval: number;
}

export type LoginPoll =
  | { status: "pending" }
  | { status: "authorized"; session: SessionInfo }
  | { status: "expired" }
  | { status: "denied" };

export interface LocalSkill {
  id: string;
  name: string;
  description: string;
  source: "agents" | "codex" | "claude" | "cursor" | string;
  kind: "user" | "system" | "plugin" | "catalog" | "mcp" | string;
  path: string;
}

export interface ChatSession {
  id: string;
  client: ClientId;
  title: string;
  model: string | null;
  cwd: string | null;
  updated_at: string | null;
  message_count: number | null;
}

export interface ChatMessage {
  role: string;
  text: string;
  at: string | null;
}

export interface ChatTranscript {
  session: ChatSession;
  messages: ChatMessage[];
  truncated: boolean;
  missing_file: boolean;
}

export interface BackupSummary {
  id: string;
  created_at: string;
  dir: string;
  files: string[];
}

export interface CmdError {
  kind:
    | "not_installed"
    | "client_disabled"
    | "not_logged_in"
    | "unauthorized"
    | "api"
    | "network"
    | "secrets"
    | "no_backup"
    | "rolled_back"
    | "rollback_failed"
    | "patch"
    | "conflict"
    | "other";
  message: string;
}

export function isCmdError(e: unknown): e is CmdError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

export function errorText(e: unknown): string {
  if (isCmdError(e)) {
    switch (e.kind) {
      case "not_installed":
        return "未检测到该工具，请先安装后再启用。";
      case "client_disabled":
        return "当前套餐不支持该客户端。";
      case "not_logged_in":
        return "请先登录 Tokease。";
      case "unauthorized":
        return "登录已失效，请重新登录。";
      case "network":
        return "网络连接失败，请检查网络或服务器地址。";
      case "rolled_back":
        return "写入配置后校验失败，已自动恢复原配置。";
      case "rollback_failed":
        return "写入失败且自动恢复失败，请到设置页面手动恢复备份。";
      case "no_backup":
        return "没有可恢复的备份。";
      case "patch":
        return `无法安全修改配置文件，未写入任何内容：${e.message}`;
      case "conflict":
        return "配置文件在写入前被其他程序修改了，本次未写入，请重试。";
      default:
        return e.message;
    }
  }
  return e instanceof Error ? e.message : String(e);
}

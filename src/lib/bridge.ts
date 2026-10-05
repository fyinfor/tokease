// Typed wrappers around Tauri IPC. When the page is opened in a plain browser
// (`pnpm dev` without Tauri) a small in-memory mock is used so the UI can be
// developed and screenshot-tested without the native shell.

import type {
  BackupSummary,
  ChatSession,
  LocalSkill,
  ChatTranscript,
  ClientConfig,
  ClientId,
  ClientStatus,
  DeviceStart,
  LoginPoll,
  SessionInfo,
} from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Api = {
  getSession(): Promise<SessionInfo>;
  setServerUrl(url: string): Promise<SessionInfo>;
  startDeviceLogin(): Promise<DeviceStart>;
  pollDeviceLogin(deviceCode: string): Promise<LoginPoll>;
  loginWithPassword(email: string, password: string): Promise<SessionInfo>;
  logout(): Promise<SessionInfo>;
  listClients(): Promise<ClientStatus[]>;
  enableClient(id: ClientId): Promise<ClientStatus>;
  restoreClient(id: ClientId, backupId?: string): Promise<ClientStatus>;
  listBackups(id: ClientId): Promise<BackupSummary[]>;
  getPlatformConfig(): Promise<ClientConfig | null>;
  refreshPlatformConfig(): Promise<ClientConfig>;
  listSkills(): Promise<LocalSkill[]>;
  listChatSessions(): Promise<ChatSession[]>;
  readChatSession(id: string): Promise<ChatTranscript>;
  openUrl(url: string): Promise<void>;
  revealPath(path: string): Promise<void>;
};

async function tauriApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const opener = await import("@tauri-apps/plugin-opener");
  return {
    getSession: () => invoke("get_session"),
    setServerUrl: (url) => invoke("set_server_url", { url }),
    startDeviceLogin: () => invoke("start_device_login"),
    pollDeviceLogin: (deviceCode) => invoke("poll_device_login", { deviceCode }),
    loginWithPassword: (email, password) => invoke("login_with_password", { email, password }),
    logout: () => invoke("logout"),
    listClients: () => invoke("list_clients"),
    enableClient: (id) => invoke("enable_client", { id }),
    restoreClient: (id, backupId) => invoke("restore_client", { id, backupId: backupId ?? null }),
    listBackups: (id) => invoke("list_backups", { id }),
    getPlatformConfig: () => invoke("get_platform_config"),
    refreshPlatformConfig: () => invoke("refresh_platform_config"),
    listSkills: () => invoke("list_skills"),
    listChatSessions: () => invoke("list_chat_sessions"),
    readChatSession: (id) => invoke("read_chat_session", { id }),
    openUrl: (url) => opener.openUrl(url),
    revealPath: (path) => opener.revealItemInDir(path),
  };
}

// ---------------------------------------------------------------------------
// Browser mock (dev only). Deliberately tiny; mirrors core semantics loosely.
// ---------------------------------------------------------------------------
function mockApi(): Api {
  const delay = (ms: number) => new Promise((r) => setTimeout(r, ms));
  let session: SessionInfo = {
    logged_in: false,
    user: null,
    server_url: "http://127.0.0.1:8787/v1",
    storage_backend: "none",
    token_preview: null,
    data_dir: "/home/you/.tokease",
  };
  const config: ClientConfig = {
    base_url: "http://127.0.0.1:8787/v1",
    endpoints: { openai: "http://127.0.0.1:8787/v1", anthropic: "http://127.0.0.1:8787", gemini: "http://127.0.0.1:8787" },
    models: [
      { id: "code-best", name: "最佳编程" },
      { id: "code-fast", name: "快速编程" },
      { id: "code-cheap", name: "经济编程" },
    ],
    default_model: "code-best",
    clients: { codex: { enabled: true }, claude: { enabled: true }, "claude-desktop": { enabled: true }, gemini: { enabled: true }, grok: { enabled: true }, opencode: { enabled: true } },
  };
  const base = { available: true, enabled: false, problems: [], env_conflicts: [], enabled_at: null, backup_id: null, min_version: null, outdated: false };
  const clients: ClientStatus[] = [
    { ...base, id: "codex", name: "Codex CLI", installed: true, binary_path: "/usr/local/bin/codex", version: "codex-cli 0.160.0", min_version: "0.149.0", config_dir: "~/.codex", managed_files: ["~/.codex/config.toml"], current: { base_url: null, model: "gpt-5", has_credential: true } },
    { ...base, id: "claude", name: "Claude CLI", installed: true, binary_path: "/usr/local/bin/claude", version: "2.1.29 (Claude Code)", config_dir: "~/.claude", managed_files: ["~/.claude/settings.json"], current: { base_url: null, model: null, has_credential: false }, env_conflicts: [{ name: "ANTHROPIC_BASE_URL", source: "~/.zshrc", preview: "http…mple (23 chars)" }] },
    { ...base, id: "claude-desktop", name: "Claude Desktop", installed: true, binary_path: null, version: null, config_dir: "~/.config/Claude", managed_files: ["~/.config/Claude-3p/configLibrary/_meta.json"], current: { base_url: null, model: null, has_credential: false } },
    { ...base, id: "gemini", name: "Gemini CLI", installed: false, binary_path: null, version: null, config_dir: "~/.gemini", managed_files: ["~/.gemini/.env", "~/.gemini/settings.json"], current: { base_url: null, model: null, has_credential: false } },
    { ...base, id: "grok", name: "Grok", installed: true, binary_path: "/usr/local/bin/grok", version: "0.1.0", config_dir: "~/.grok", managed_files: ["~/.grok/config.toml"], current: { base_url: null, model: null, has_credential: false } },
    { ...base, id: "opencode", name: "OpenCode", installed: false, binary_path: null, version: null, config_dir: "~/.config/opencode", managed_files: ["~/.config/opencode/opencode.json"], current: { base_url: null, model: null, has_credential: false } },
  ];
  const backups: Record<string, BackupSummary[]> = { codex: [], claude: [], "claude-desktop": [], gemini: [], grok: [], opencode: [] };
  const chats: ChatSession[] = [
    { id: "codex:1", client: "codex", title: "把窗口圆角在最大化后贴齐屏幕", model: "code-best", cwd: "/home/aipanda/product/tokease", updated_at: "2026-10-05T10:20:00Z", message_count: 4 },
    { id: "claude:1", client: "claude", title: "拆开 Claude CLI 和 Desktop", model: "code-fast", cwd: "/home/aipanda/product/tokease", updated_at: "2026-10-04T08:00:00Z", message_count: 2 },
    { id: "opencode:1", client: "opencode", title: "补上 Grok 和 OpenCode", model: "code-cheap", cwd: "/home/aipanda/product/tokease", updated_at: "2026-10-03T03:10:00Z", message_count: 2 },
  ];
  const transcripts: Record<string, ChatTranscript> = {
    "codex:1": { session: chats[0], truncated: false, missing_file: false, messages: [
      { role: "user", text: "最大化后窗口没有贴着屏幕。", at: "2026-10-05T10:18:00Z" },
      { role: "assistant", text: "最大化时去掉外边距和圆角，还原时再加回来。", at: "2026-10-05T10:19:00Z" },
    ] },
    "claude:1": { session: chats[1], truncated: false, missing_file: false, messages: [
      { role: "user", text: "Claude 分成 CLI 和 Desktop。", at: "2026-10-04T08:00:00Z" },
      { role: "assistant", text: "CLI 继续写 settings.json，Desktop 写 3P profile。", at: "2026-10-04T08:01:00Z" },
    ] },
    "opencode:1": { session: chats[2], truncated: false, missing_file: false, messages: [
      { role: "user", text: "再补 Grok 和 OpenCode。", at: "2026-10-03T03:10:00Z" },
      { role: "assistant", text: "Grok 写 config.toml，OpenCode 写 opencode.json。", at: "2026-10-03T03:11:00Z" },
    ] },
  };
  const err = (kind: string, message: string) => Promise.reject({ kind, message });
  const login = (): SessionInfo => (session = { ...session, logged_in: true, user: { id: "u_demo", email: "demo@tokease.com", name: "Demo" }, storage_backend: "keychain", token_preview: "tk_l…bb59 (44 chars)" });
  let pollCount = 0;
  return {
    getSession: async () => session,
    setServerUrl: async (url) => (session = { ...session, server_url: url }),
    startDeviceLogin: async () => {
      pollCount = 0;
      return { device_code: "dc_mock", user_code: "AB12-CD34", verification_uri: "http://127.0.0.1:8787/device", verification_uri_complete: "http://127.0.0.1:8787/device?user_code=AB12-CD34", expires_in: 600, interval: 1 };
    },
    pollDeviceLogin: async () => (++pollCount < 4 ? { status: "pending" } : { status: "authorized", session: login() }),
    loginWithPassword: async (_e, p) => (p === "demo" ? login() : err("unauthorized", "邮箱或密码错误")),
    logout: async () => (session = { ...session, logged_in: false, user: null, storage_backend: "none", token_preview: null }),
    listClients: async () => clients.map((c) => ({ ...c })),
    enableClient: async (id) => {
      await delay(600);
      const c = clients.find((x) => x.id === id)!;
      if (!session.logged_in) return err("not_logged_in", "not logged in");
      if (!c.installed) return err("not_installed", `${c.name} is not installed`);
      const bid = new Date().toISOString().replace(/[-:T]/g, "").slice(0, 15);
      backups[id].unshift({ id: bid, created_at: new Date().toISOString(), dir: `${session.data_dir}/backups/${id}/${bid}`, files: c.managed_files });
      const protocol = id === "gemini" ? "gemini" : id === "claude" || id === "claude-desktop" ? "anthropic" : "openai";
      Object.assign(c, { enabled: true, enabled_at: new Date().toISOString(), backup_id: c.backup_id ?? bid, current: { base_url: config.endpoints[protocol], model: config.default_model, has_credential: true } });
      return { ...c };
    },
    restoreClient: async (id) => {
      await delay(400);
      const c = clients.find((x) => x.id === id)!;
      if (!c.backup_id && !backups[id].length) return err("no_backup", "no backup");
      Object.assign(c, { enabled: false, enabled_at: null, backup_id: null, current: { base_url: null, model: id === "codex" ? "gpt-5" : null, has_credential: id === "codex" } });
      return { ...c };
    },
    listBackups: async (id) => backups[id],
    getPlatformConfig: async () => (session.logged_in ? config : null),
    refreshPlatformConfig: async () => (session.logged_in ? config : err("not_logged_in", "not logged in")),
    listSkills: async () => [
      { id: "agents:brandkit", name: "brandkit", description: "做品牌板、标志和视觉规范。", source: "agents", kind: "user", path: "/home/aipanda/.agents/skills/brandkit" },
      { id: "codex:review", name: "review-agent", description: "审阅代码改动。", source: "codex", kind: "system", path: "/home/aipanda/.codex/skills/.system/review-agent" },
      { id: "claude:frontend", name: "frontend-design", description: "按现有界面做前端。", source: "claude", kind: "plugin", path: "/home/aipanda/.claude/plugins/frontend-design" },
      { id: "claude:build-mcp", name: "build-mcp-server", description: "教你怎么写 MCP 服务。", source: "claude", kind: "catalog", path: "/home/aipanda/.claude/plugins/marketplaces/mcp-server-dev/skills/build-mcp-server" },
      { id: "claude:github", name: "github", description: "MCP 服务 · https://api.githubcopilot.com/mcp/", source: "claude", kind: "mcp", path: "/home/aipanda/.claude/plugins/marketplaces/github/.mcp.json" },
    ],
    listChatSessions: async () => chats.map((c) => ({ ...c })),
    readChatSession: async (id) => transcripts[id] ?? err("other", "找不到这场会话"),
    openUrl: async (url) => void window.open(url, "_blank"),
    revealPath: async (p) => alert(`(mock) reveal ${p}`),
  };
}

let apiPromise: Promise<Api> | null = null;
export function api(): Promise<Api> {
  if (!apiPromise) apiPromise = isTauri ? tauriApi() : Promise.resolve(mockApi());
  return apiPromise;
}

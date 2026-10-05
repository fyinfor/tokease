# Tokease Desktop

轻量级桌面 Connector：登录 Tokease，点一下「一键启用」，本地的 **Codex CLI / Claude Code / Gemini CLI** 就接到 Tokease 聚合 API 上。用户不需要知道 Base URL、API Key、Provider、模型 ID 或配置文件在哪。

```
安装 Tokease → 登录 → 点「启用 Codex」→ 完成
```

- 启用前自动备份原配置，随时「恢复原配置」
- 只改必要的键，用户其余配置原样保留（TOML 注释都保留）
- 原子写入 + 写后校验 + 失败自动回滚
- Token 存系统钥匙串（macOS Keychain / Windows 凭据管理器 / Linux Secret Service），不进前端、不进日志
- 客户端只用 Tokease 逻辑模型名（`code-best` / `code-fast` / `code-cheap`），真实上游由服务端路由

## 目录结构

```
tokease/
├─ src/                        React + TypeScript 前端（只做 UI 和状态展示）
│  ├─ pages/Home.tsx           首页：Logo、登录状态、三张客户端卡片
│  ├─ pages/Advanced.tsx       隐藏的高级页：服务器地址、会话、平台配置、备份
│  ├─ components/              Logo / ClientCard / LoginPanel
│  └─ lib/bridge.ts            Tauri IPC 封装；非 Tauri 环境自动切换到浏览器 mock
├─ src-tauri/                  Tauri 2 壳层：仅把 core 暴露为 IPC 命令
│  ├─ src/commands.rs
│  ├─ tauri.conf.json
│  └─ capabilities/default.json
├─ crates/tokease-core/        Rust 核心库（不依赖 Tauri，可单独测试）
│  ├─ src/adapters/            codex.rs / claude.rs / gemini.rs + 公共 trait
│  ├─ src/backup.rs            备份 / 恢复
│  ├─ src/fsutil.rs            原子写入
│  ├─ src/secrets.rs           keyring + 文件回退
│  ├─ src/api.rs               Tokease API 客户端
│  ├─ src/service.rs           Tokease 门面（UI 和 CLI 都调它）
│  └─ src/bin/tokease-cli.rs   无头 CLI，便于调试 / CI
├─ mock-server/server.mjs      零依赖 Mock Tokease API（Node）
└─ README.md
```

## 环境要求

- Node.js ≥ 20，pnpm（`corepack enable`）
- Rust stable（<https://rustup.rs>）
- Tauri 2 系统依赖：<https://tauri.app/start/prerequisites/>

### Windows

1. 安装 [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)（勾选「使用 C++ 的桌面开发」）
2. 安装 [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（Win 11 自带）
3. `winget install Rustlang.Rustup`，然后 `rustup default stable-msvc`

### macOS

```sh
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Linux（Debian / Ubuntu）

```sh
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

> Linux 上 Token 存入 Secret Service（GNOME Keyring / KWallet）。无 Secret Service 的无头环境会自动回退为 `~/.tokease/credentials.json`（0600），高级页会显示当前存储位置。

## 运行

```sh
pnpm install

# 终端 1：Mock 服务端（任意邮箱 + 密码 demo；设备码 8 秒后自动通过）
pnpm mock

# 终端 2：桌面应用（指向 Mock）
TOKEASE_SERVER_URL=http://127.0.0.1:8787 pnpm tauri:dev
#   Windows PowerShell:  $env:TOKEASE_SERVER_URL="http://127.0.0.1:8787"; pnpm tauri:dev
```

不设置 `TOKEASE_SERVER_URL` 时默认连 `https://api.tokease.com`，也可以在高级页里改。

打包：`pnpm tauri:build`（产物在 `src-tauri/target/release/bundle/`）。

### 仅看 UI（不编译 Rust）

```sh
pnpm dev   # http://localhost:1420 ，用内置浏览器 mock 跑整条交互
```

### 无头 CLI（调试 / CI）

```sh
pnpm core:test                                   # 核心库单测（含 3 个 Adapter 闭环 + 回滚）
pnpm cli -- status
pnpm cli -- login --email demo@tokease.com --password demo
pnpm cli -- enable codex
pnpm cli -- restore codex
pnpm cli -- backups codex
```

CLI 和桌面端共用 `~/.tokease`（`TOKEASE_DATA_DIR` 可覆盖）。本地测试不想碰真实配置时：

```sh
export HOME=/tmp/fakehome CODEX_HOME=/tmp/fakehome/.codex CLAUDE_CONFIG_DIR=/tmp/fakehome/.claude TOKEASE_DATA_DIR=/tmp/fakehome/.tokease
```

## 各客户端写了什么

| 客户端 | 文件 | 改动 |
|---|---|---|
| Codex CLI | `~/.codex/config.toml` | `model_provider = "tokease"`、`model = "<默认逻辑模型>"`、新增 `[model_providers.tokease]`（`base_url`、`wire_api`、`requires_openai_auth = true`） |
| | `~/.codex/auth.json` | `OPENAI_API_KEY = <token>`；若原来是 ChatGPT 登录，移除 `tokens`/`last_refresh` 并置 `auth_mode = "apikey"`（原文件完整在备份中） |
| Claude Code | `~/.claude/settings.json` | `env.ANTHROPIC_BASE_URL`、`env.ANTHROPIC_AUTH_TOKEN`、`env.ANTHROPIC_MODEL` |
| Gemini CLI | `~/.gemini/.env` | `GEMINI_API_KEY`、`GOOGLE_GEMINI_BASE_URL`、`GEMINI_MODEL` |
| | `~/.gemini/settings.json` | `security.auth.selectedType = "gemini-api-key"` |

路径遵循各工具自己的环境变量：`CODEX_HOME`、`CLAUDE_CONFIG_DIR`。

### 启用流程（每个 Adapter 一致）

```
detect() → plan()（只读，算出新内容） → backup_config() → 原子写入 → validate_config()
                                                   └── 任一步失败 → 用刚才的备份回滚
```

- 备份在 `~/.tokease/backups/<client>/<时间戳>/`，带 `manifest.json`；原本不存在的文件在恢复时会被删除
- 重复点「启用」不会覆盖最初的恢复点（始终指向用户接入 Tokease 之前的那份）
- 「恢复原配置」前会再做一次快照，所以恢复也可撤销（高级页 → 备份 → 恢复到此）
- 「已启用」= 本地有恢复点记录 **且** 当前文件仍指向 Tokease；用户手改过配置会显示「配置已被修改，可重新启用」

## 新增一个客户端

在 `crates/tokease-core/src/adapters/` 里加一个文件，实现 `Adapter` trait 的 6 个方法：

```rust
fn id(&self) -> ClientId;              fn detect(&self) -> Detection;
fn managed_paths(&self) -> Vec<PathBuf>; fn read_config(&self) -> CurrentConfig;
fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>>;
fn validate_config(&self, base_url: &str) -> Validation;
```

`backup_config / apply_config / restore_config` 由 trait 默认实现提供（备份、原子写、回滚）。然后在 `ClientId` 加一个变体、在 `adapters::all()` 注册即可；前端卡片自动出现。测试直接复用 `adapters::testutil::closed_loop`。

## 服务端接口

| 方法 | 路径 | 说明 |
|---|---|---|
| `POST` | `/auth/device` | 开始设备码登录 → `{device_code, user_code, verification_uri, verification_uri_complete?, expires_in, interval}` |
| `GET` | `/auth/device/status?device_code=` | `{status: "pending" \| "authorized" \| "expired" \| "denied", access_token?, user?}` |
| `POST` | `/auth/login` | `{email, password}` → `{access_token, user}` |
| `GET` | `/client/config` | `Authorization: Bearer <token>` |

`/client/config` 响应：

```json
{
  "base_url": "https://api.tokease.com/v1",
  "endpoints": {
    "openai": "https://api.tokease.com/v1",
    "anthropic": "https://api.tokease.com",
    "gemini": "https://api.tokease.com"
  },
  "models": [
    { "id": "code-best", "name": "最佳编程" },
    { "id": "code-fast", "name": "快速编程" },
    { "id": "code-cheap", "name": "经济编程" }
  ],
  "default_model": "code-best",
  "clients": { "codex": true, "claude": true, "gemini": { "enabled": true } }
}
```

- `endpoints` 可选：不同协议的根路径不同时用它（Codex 要 `/v1`，Claude / Gemini 通常是根路径）；缺省回落到 `base_url`
- `clients.<id>` 可以是布尔，也可以是对象 `{ "enabled": bool, "wire_api"?: "responses" | "chat" }`（`wire_api` 仅 Codex 使用，默认 `responses`）
- 客户端只把 `default_model` 写进工具配置，真实上游模型映射完全在服务端

## 安全

- 前端代码不含任何密钥；Token 只在 Rust 侧从钥匙串读出、写入工具配置文件
- 日志只打印脱敏 Token（`tk_l…bb59 (44 chars)`）
- `~/.tokease/state.json` 只有用户信息、服务器地址、平台配置缓存和恢复点 ID，不含 Token
- 含密钥的文件（`auth.json`、`.env`、Claude `settings.json`）强制 0600
- 所有写入走「同目录临时文件 → fsync → rename」

## 已知限制 / 后续

- 第一版只写默认模型；Claude Code 的 Opus/Sonnet/Haiku 分档、Codex `model_reasoning_effort` 等留给服务端在 `/client/config` 里扩展
- 备份不自动清理（都很小，但可在高级页手动查看）
- 本机 Linux 开发时若装不了 `libwebkit2gtk-4.1-dev`，可以先用 `pnpm dev` + `pnpm cli` 覆盖全部逻辑

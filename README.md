# Tokease Desktop

轻量级桌面 Connector：登录 Tokease，点一下「一键启用」，本地的 **Codex CLI / Claude CLI / Claude Desktop / Gemini CLI / Grok / OpenCode** 就接到 Tokease 聚合 API 上。用户不需要知道 Base URL、API Key、Provider、模型 ID 或配置文件在哪。

```
安装 Tokease → 登录 → 点「启用 Codex」→ 完成
```

- 启用前自动备份原配置，随时「恢复原配置」
- 配置写入沿用 [CC Switch](https://github.com/farion1231/cc-switch) 的核心模型：只清掉「关键字段」（地址 / 凭据 / 模型名 / 协议开关），其余键、顺序、注释、缩进原样保留
- 解析失败的文件绝不覆盖；写前检测并发修改；原子写入 + 写后校验 + 失败自动回滚
- Codex 不碰 `auth.json`：Token 写进 `[model_providers.tokease].experimental_bearer_token`，ChatGPT 登录保持原样
- Token 存系统钥匙串（macOS Keychain / Windows 凭据管理器 / Linux Secret Service），不进前端、不进日志
- 客户端只用 Tokease 逻辑模型名（`code-best` / `code-fast` / `code-cheap`），真实上游由服务端路由
- 检测 shell 环境变量冲突（`ANTHROPIC_*` / `OPENAI_API_KEY` / `GEMINI_API_KEY` …）并在卡片上提示

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
│  ├─ src/adapters/            codex / claude / claude_desktop / gemini / grok / opencode + 公共 trait
│  │  └─ locate.rs             找 CLI 可执行文件（含登录 shell 的 PATH）、读版本
│  ├─ src/floor.rs             关键字段定义（移植自 CC Switch live/floor.rs）
│  ├─ src/patch/               保序补丁器：json.rs / dotenv.rs / toml.rs
│  ├─ src/envcheck.rs          环境变量冲突检测
│  ├─ src/backup.rs            备份 / 恢复
│  ├─ src/fsutil.rs            原子写入
│  ├─ src/secrets.rs           keyring + 文件回退
│  ├─ src/api.rs               Tokease API 客户端（含服务端没有 /client/config 时的内置默认值）
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

# 终端 2：桌面应用（指向 Mock；Mock 同时在 /v1 前缀下提供所有接口，和线上布局一致）
TOKEASE_SERVER_URL=http://127.0.0.1:8787/v1 pnpm tauri:dev
#   Windows PowerShell:  $env:TOKEASE_SERVER_URL="http://127.0.0.1:8787/v1"; pnpm tauri:dev
```

不设置 `TOKEASE_SERVER_URL` 时默认连 **`https://www.tokease.cn/v1`**，也可以在高级页里改。认证接口（`/auth/*`、`/client/config`）挂在这个地址下；服务端还没实现 `/client/config`（404）时，客户端用内置默认值：OpenAI 兼容地址 = 服务器地址本身，Anthropic / Gemini = 去掉 `/v1` 的根地址，模型 `code-best / code-fast / code-cheap`。

打包：`pnpm tauri:build`（产物在 `src-tauri/target/release/bundle/`）。

> Linux 上如果 `pnpm tauri:dev` 报 `Too many open files`（inotify 实例被 IDE 等占满），可以不走 Tauri CLI 的文件监视：
> `pnpm build && pnpm exec vite preview --port 1420 --strictPort &` 然后 `cargo build -p tokease && ./target/debug/tokease`。

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
export HOME=/tmp/fakehome CODEX_HOME=/tmp/fakehome/.codex CLAUDE_CONFIG_DIR=/tmp/fakehome/.claude GROK_HOME=/tmp/fakehome/.grok XDG_CONFIG_HOME=/tmp/fakehome/.config TOKEASE_DATA_DIR=/tmp/fakehome/.tokease
```

## 各客户端写了什么

写法遵循 CC Switch 的「关键字段（floor）」模型：每个工具里**归供应商所有**的键（请求发到哪、凭什么鉴权、哪个模型名、哪种协议）在启用时先全部清掉再写 Tokease 的值；其余键归用户和工具，不写也不删。漏列的键会被当作用户键保留，失败方向是安全的。完整清单见 `crates/tokease-core/src/floor.rs`。

| 客户端 | 文件 | 清掉的关键字段 | 写入 |
|---|---|---|---|
| Codex CLI（≥ 0.149） | `~/.codex/config.toml` | 顶层 `model_provider`、`openai_base_url`、`model`、`review_model`、`disable_response_storage`、`experimental_bearer_token`、`base_url`、`wire_api`；嵌套的 `agents.default_subagent_model`、`memories.extract_model`、`memories.consolidation_model`；保留 id（`openai`/`ollama`/`lmstudio`）的 provider 表改名为 `<id>-legacy`（Codex 0.148+ 遇到会拒绝加载） | `model_provider = "tokease"`、`model`、`disable_response_storage = true`、`model_reasoning_effort = "high"`（仅缺失时补）、`[model_providers.tokease]`：`name`、`base_url`、`wire_api`、`experimental_bearer_token = <token>`、`requires_openai_auth = <auth.json 里是否有登录>` |
| | `~/.codex/auth.json` | **不读不写**（只用来判断是否存在 ChatGPT 登录） | — |
| Claude CLI | `~/.claude/settings.json`（仅有旧 `claude.json` 时用它） | 顶层 `apiKeyHelper`、`apiBaseUrl`、`apiKey`、`model`、`primaryModel`、`smallFastModel`、`fallbackModel`、`modelOverrides`、`advisorModel`、`awsAuthRefresh`、`awsCredentialExport`、`gcpAuthRefresh`；`env` 里的 `ANTHROPIC_*`、`AWS_*`、`VERTEX_REGION_*`、`CLAUDE_CODE_USE_BEDROCK/VERTEX/FOUNDRY/GATEWAY/MANTLE/…`、`CLAUDE_CODE_OAUTH_*`、`CLAUDE_CODE_SUBAGENT_MODEL*`、`CLAUDE_CODE_SKIP_*_AUTH`、`CLOUD_ML_REGION`、`GOOGLE_APPLICATION_CREDENTIALS` | `env.ANTHROPIC_BASE_URL`、`env.ANTHROPIC_AUTH_TOKEN`（只写这一个凭据键）、`env.ANTHROPIC_MODEL`、`env.ANTHROPIC_DEFAULT_OPUS_MODEL = code-best`、`…_SONNET_MODEL = code-fast`、`…_HAIKU_MODEL = code-cheap` |
| Claude Desktop | `~/Library/Application Support/Claude/claude_desktop_config.json`（Linux 为 `~/.config/Claude/`，Windows 为 `%LOCALAPPDATA%\Claude\`），以及旁边的 `Claude-3p/` | —（只改下面列出的键） | 两个 `claude_desktop_config.json` 的 `deploymentMode = "3p"`；`Claude-3p/configLibrary/00000000-0000-4000-8000-00000000e45e.json` 的网关地址、Bearer Token、三个角色模型（菜单名是逻辑模型）；`_meta.json` 的 `appliedId` 指向这份 profile，其它 profile 条目保留 |
| Gemini CLI | `~/.gemini/.env` | `GOOGLE_*`、`GEMINI_API_KEY`、`GEMINI_MODEL`、`GEMINI_API_KEY_AUTH_MECHANISM`、`GEMINI_CLI_CUSTOM_HEADERS`、`GEMINI_DEFAULT_AUTH_TYPE`、`GEMINI_CLI_USE_COMPUTE_ADC`、`CODE_ASSIST_ENDPOINT`、`CODE_ASSIST_API_VERSION`（`GEMINI_SANDBOX` 等不是） | `GEMINI_API_KEY`、`GOOGLE_GEMINI_BASE_URL`、`GEMINI_MODEL` |
| | `~/.gemini/settings.json` | — | `security.auth.selectedType = "gemini-api-key"`、`model.name` |
| Grok | `~/.grok/config.toml`（`GROK_HOME` 可覆盖） | 只动 Tokease 自己的 `[model.tokease]`：清掉其中的 `env_key` | `[models].default = "tokease"`、`[model.tokease]` 的 `model`、`base_url`、`name`、`api_key`、`api_backend`（默认 `responses`）。其它模型表和 UI 设置保留 |
| OpenCode | `~/.config/opencode/opencode.json`（仅有 `opencode.jsonc` 时用它；`$XDG_CONFIG_HOME` 可覆盖） | — | `provider.tokease`（`@ai-sdk/openai-compatible`、`options.baseURL`、`options.apiKey`、三个逻辑模型）、`model`、`small_model`。其它 provider 保留 |

- Codex 若顶层 `profile` 指向的 `[profiles.<name>]` 设置了 `model_provider`（非 tokease）/ `openai_base_url` / `experimental_bearer_token`，会拒绝写入并指出是哪个 profile（否则请求不会到 Tokease）
- Codex 的 `model_reasoning_effort` / `plan_mode_reasoning_effort` / `model_catalog_json` 视为用户偏好，保留不动
- 路径遵循各工具自己的环境变量：`CODEX_HOME`、`CLAUDE_CONFIG_DIR`、`GROK_HOME`、`XDG_CONFIG_HOME`（OpenCode 与 Linux 上的 Claude Desktop）
- Claude Desktop 的模型菜单只接受 Opus / Sonnet / Haiku 角色 id。启用后菜单显示名是 `code-best` / `code-fast` / `code-cheap`，发给网关的模型 id 仍是 `claude-opus-5`、`claude-sonnet-4-6`、`claude-haiku-4-5`。改完需要完全退出再打开 Claude Desktop
- 文件格式细节保留：JSON 的缩进（空格/Tab）、CRLF、BOM、末尾换行；`.env` 的注释、顺序、`export ` 前缀；TOML 的注释、行尾注释、表的位置、内联表形态

### 启用流程（每个 Adapter 一致）

```
detect() → plan()（只读：解析 → 补丁 → 新内容） → backup_config() → 并发修改检查 → 原子写入 → validate_config()
                 └── 解析失败 / 形状不对 / profile 改道 → 直接报错，不写      └── 任一步失败 → 用刚才的备份回滚
```

- 解析不了的文件（坏 JSON / 坏 TOML / 根不是对象）**绝不**退化成空文档重写，错误里带行列号
- `plan()` 记下读到的原始字节；写入前再读一次，若文件已被别的程序改过则报 `conflict` 并回滚，提示重试；内容没变化的文件不写
- 每个客户端一把写锁，启用 / 恢复不会交错
- 备份在 `~/.tokease/backups/<client>/<时间戳>/`，带 `manifest.json`；原本不存在的文件在恢复时会被删除
- 重复点「启用」不会覆盖最初的恢复点（始终指向用户接入 Tokease 之前的那份）
- 「恢复原配置」前会再做一次快照，所以恢复也可撤销（高级页 → 备份 → 恢复到此）
- 「已启用」= 本地有恢复点记录 **且** 当前文件仍指向 Tokease；用户手改过配置会显示「配置已被修改，可重新启用」

### 检测与提示

- 可执行文件查找：当前 PATH → 登录 shell 的 PATH（`$SHELL -lc /usr/bin/env`，桌面启动的 GUI 拿不到 nvm / volta / Homebrew 的 PATH）→ 常见安装目录（`~/.local/bin`、`~/.npm-global/bin`、`~/.volta/bin`、`%APPDATA%\npm` …）→ ChatGPT 桌面版自带的 `/usr/lib/chatgpt/resources/codex`
- 读取 `--version`（带超时、按路径缓存）；Codex 低于 0.149 会在卡片上提示升级（`experimental_bearer_token` 从该版本起生效）
- 环境变量冲突：扫描进程环境和 `~/.bashrc`、`~/.zshrc`、`~/.profile`、`~/.config/fish/config.fish` 等，发现 `ANTHROPIC_*`（Claude CLI）、`OPENAI_API_KEY` / `OPENAI_BASE_URL`（Codex）、`GEMINI_API_KEY` / `GOOGLE_GEMINI_BASE_URL` / `GOOGLE_API_KEY`（Gemini）、`XAI_API_KEY` / `GROK_DEFAULT_MODEL` / `GROK_XAI_API_BASE_URL` / `GROK_MODELS_BASE_URL`（Grok）时提示用户自行清理（Tokease 不改 shell 文件，值只显示脱敏预览）

## 新增一个客户端

在 `crates/tokease-core/src/adapters/` 里加一个文件，实现 `Adapter` trait：

```rust
fn id(&self) -> ClientId;              fn detect(&self) -> Detection;
fn managed_paths(&self) -> Vec<PathBuf>; fn read_config(&self) -> CurrentConfig;
fn plan(&self, spec: &ConnectionSpec) -> Result<Vec<PlannedFile>>;
fn validate_config(&self, base_url: &str) -> Validation;
fn env_conflict_prefixes(&self) -> &'static [&'static str];
```

`plan()` 用 `patch::json::JsonPatch` / `patch::dotenv::DotenvPatch` / `patch::toml` 算新内容，把读到的原始字节放进 `PlannedFile.pre`。`backup_config / apply_config / restore_config` 由 trait 默认实现提供（备份、冲突检查、原子写、回滚）。然后在 `ClientId` 加一个变体、在 `adapters::all()` 注册即可；前端卡片自动出现。测试直接复用 `adapters::testutil::closed_loop`（它同时断言第二次启用是 no-op、恢复后字节一致）。

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
  "base_url": "https://www.tokease.cn/v1",
  "endpoints": {
    "openai": "https://www.tokease.cn/v1",
    "anthropic": "https://www.tokease.cn",
    "gemini": "https://www.tokease.cn"
  },
  "models": [
    { "id": "code-best", "name": "最佳编程" },
    { "id": "code-fast", "name": "快速编程" },
    { "id": "code-cheap", "name": "经济编程" }
  ],
  "default_model": "code-best",
  "tiers": { "fast": "code-fast", "cheap": "code-cheap" },
  "clients": { "codex": true, "claude": true, "gemini": { "enabled": true } }
}
```

- `endpoints` 可选：不同协议的根路径不同时用它（Codex 要 `/v1`，Claude / Gemini 是根路径，SDK 自己拼 `/v1/messages`、`/v1beta/...`）；缺省回落到 `base_url`
- `tiers` 可选：Claude Code 的 Sonnet / Haiku 分档写哪个逻辑模型；缺省按模型 id 后缀（`-fast` / `-cheap`）推断，推断不到就都用 `default_model`
- `clients.<id>` 可以是布尔，也可以是对象 `{ "enabled": bool, "wire_api"?: "responses" | "chat" }`（`wire_api` 仅 Codex 使用，默认 `responses`）
- 返回 404 / 501 时客户端使用内置默认值（见「运行」一节），不影响启用
- 客户端只把逻辑模型名写进工具配置，真实上游模型映射完全在服务端

## 安全

- 前端代码不含任何密钥；Token 只在 Rust 侧从钥匙串读出、写入工具配置文件
- 日志只打印脱敏 Token（`tk_l…bb59 (44 chars)`）；环境变量冲突提示里的值同样脱敏
- `~/.tokease/state.json` 只有用户信息、服务器地址、平台配置缓存和恢复点 ID，不含 Token
- 含密钥的文件（Codex `config.toml`、Gemini `.env`、Claude `settings.json`）强制 0600
- 所有写入走「同目录临时文件 → fsync → rename」，写前比对原始字节防并发覆盖
- 解析失败的配置文件绝不被重写

## 与 CC Switch 的关系

移植了它的核心写入模型（`live/floor.rs` 的关键字段定义、`live/patch` 的保序 JSON / dotenv 补丁器、Codex 投影里的 TOML 原位改值 / 表形态保持 / 保留 id 改名 / profile 改道校验、写引擎的「解析失败即停止、写前哈希比对、no-op 检测、每应用一把锁」、`env_checker` 的环境变量冲突扫描），按 Tokease「单供应商、一键启用 / 恢复」的场景简化：没有多供应商切换、独有字段残留清理、代理、MCP / Skills / 用量同步等模块。UI 完全独立，不依赖 CC Switch 代码。模型切换、本地路由和手机端任务控制不在这次移植里，见下方路线图。

## 路线图

现在启用时把服务端下发的逻辑模型（`code-best` / `code-fast` / `code-cheap`）写进 CLI 配置，卡片只展示当前模型，不能改。真实上游由服务端路由。下面三步按依赖往前排。

- [ ] **模型切换**：重构模型 UI，做成和 [CC Switch](https://github.com/farion1231/cc-switch) 一样的切换。选一个模型，配置立刻换成它，地址和凭据不动。供应商仍是 Tokease。
  - [ ] 每个客户端卡片上直接选模型，不必进高级页，也不必先恢复再重新启用
  - [ ] 可选列表来自 `/client/config` 的 `models`。Claude Code 的 Opus / Sonnet / Haiku 分档各自选；Codex、Gemini 选当前主模型
  - [ ] 切换只改模型相关的关键字段（见「各客户端写了什么」），其余键、注释、格式保持原样；同样经过备份、并发检查、原子写入和失败回滚
  - [ ] 多供应商档案、代理、MCP、用量同步留到后面
- [ ] **本地智能路由**：手动切换之后，由本机决定这次任务用哪个模型。
  - [ ] 策略在本机：按任务类型、上下文长度、延迟和费用，在已配置的模型之间选择
  - [ ] 可以随时关掉，退回「固定模型」
  - [ ] 选中的模型仍通过现有 Adapter 写进各 CLI 配置。路由决策和密钥留在这台电脑上
- [ ] **手机控制与任务管理**：用微信、飞书等应用连上这台电脑上正在运行的 Tokease，在手机上派任务、看进度。真正调用 Codex、Claude Code 等仍由桌面端在本机执行。
  - [ ] 桌面端是执行端：收到任务后启动对应 CLI，跑完把状态和结果送回手机
  - [ ] 手机上看得到排队、进行中、完成、失败，可以继续或取消
  - [ ] 通道只传任务和结果。Token 与 CLI 配置留在本机钥匙串和配置文件里，不发到微信或飞书
  - [ ] 先做微信和飞书，其它应用按同一套任务协议接

## 已知限制

- Codex < 0.149 不支持 `experimental_bearer_token`，卡片会提示升级；不再回退到改写 `auth.json`
- 备份不自动清理（都很小，但可在高级页手动查看）
- 本机 Linux 开发时若装不了 `libwebkit2gtk-4.1-dev`，可以先用 `pnpm dev` + `pnpm cli` 覆盖全部逻辑

// Mock Tokease API. Zero dependencies: `node mock-server/server.mjs`.
//
//   POST /auth/device            start device-code login
//   GET  /auth/device/status     poll (device_code=...)
//   GET  /device?user_code=...   browser page the user "approves" on
//   POST /auth/login             { email, password }  (any password = "demo")
//   GET  /client/config          Bearer token required
//
// Every route is also served under the `/v1` prefix so the mock can stand in
// for the real server URL layout (`https://www.tokease.cn/v1`):
// point the app at `TOKEASE_SERVER_URL=http://127.0.0.1:8787/v1`.
//
// Env: PORT (default 8787), AUTO_APPROVE_SECONDS (default 8; 0 = manual only),
//      NO_CLIENT_CONFIG=1 (answer 404 on /client/config to exercise the
//      app's built-in defaults)

import http from "node:http";
import { randomBytes } from "node:crypto";

const PORT = Number(process.env.PORT || 8787);
const AUTO_APPROVE_SECONDS = Number(process.env.AUTO_APPROVE_SECONDS ?? 8);
const NO_CLIENT_CONFIG = process.env.NO_CLIENT_CONFIG === "1";
const ORIGIN = `http://127.0.0.1:${PORT}`;

const tokens = new Map(); // token -> user
const devices = new Map(); // device_code -> { user_code, status, created, token }

const DEMO_USER = { id: "u_demo", email: "demo@tokease.com", name: "Demo User" };

function issueToken(user) {
  const t = "tk_live_" + randomBytes(18).toString("hex");
  tokens.set(t, user);
  return t;
}

function clientConfig() {
  return {
    base_url: `${ORIGIN}/v1`,
    endpoints: {
      openai: `${ORIGIN}/v1`,
      anthropic: ORIGIN,
      gemini: ORIGIN,
    },
    models: [
      { id: "code-best", name: "最佳编程", description: "质量优先" },
      { id: "code-fast", name: "快速编程", description: "速度优先" },
      { id: "code-cheap", name: "经济编程", description: "成本优先" },
    ],
    default_model: "code-best",
    clients: { codex: true, claude: true, gemini: true },
  };
}

function json(res, status, body) {
  const data = JSON.stringify(body);
  res.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "authorization, content-type",
    "access-control-allow-methods": "GET, POST, OPTIONS",
  });
  res.end(data);
}

function html(res, status, body) {
  res.writeHead(status, { "content-type": "text/html; charset=utf-8" });
  res.end(body);
}

async function readJson(req) {
  let raw = "";
  for await (const chunk of req) raw += chunk;
  try {
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function bearer(req) {
  const h = req.headers.authorization || "";
  return h.startsWith("Bearer ") ? h.slice(7) : null;
}

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, ORIGIN);
  const path = url.pathname.replace(/^\/v1(?=\/|$)/, "") || "/";
  console.log(new Date().toISOString(), req.method, url.pathname);

  if (req.method === "OPTIONS") return json(res, 204, {});

  // ---- device code flow ---------------------------------------------------
  if (req.method === "POST" && path === "/auth/device") {
    const device_code = "dc_" + randomBytes(16).toString("hex");
    const user_code = randomBytes(2).toString("hex").toUpperCase() + "-" + randomBytes(2).toString("hex").toUpperCase();
    devices.set(device_code, { user_code, status: "pending", created: Date.now(), token: null });
    if (AUTO_APPROVE_SECONDS > 0) {
      setTimeout(() => {
        const d = devices.get(device_code);
        if (d && d.status === "pending") {
          d.status = "authorized";
          d.token = issueToken(DEMO_USER);
          console.log(`  auto-approved ${user_code}`);
        }
      }, AUTO_APPROVE_SECONDS * 1000);
    }
    return json(res, 200, {
      device_code,
      user_code,
      verification_uri: `${ORIGIN}/device`,
      verification_uri_complete: `${ORIGIN}/device?user_code=${user_code}`,
      expires_in: 600,
      interval: 2,
    });
  }

  if (req.method === "GET" && path === "/auth/device/status") {
    const d = devices.get(url.searchParams.get("device_code"));
    if (!d) return json(res, 404, { error: "unknown device_code" });
    if (Date.now() - d.created > 600_000) d.status = "expired";
    if (d.status === "authorized") {
      return json(res, 200, { status: "authorized", access_token: d.token, user: DEMO_USER });
    }
    return json(res, 200, { status: d.status });
  }

  if (path === "/device") {
    const code = url.searchParams.get("user_code") || "";
    if (req.method === "POST") {
      const body = await readJson(req);
      const entry = [...devices.values()].find((d) => d.user_code === (body.user_code || code));
      if (!entry) return json(res, 404, { error: "unknown code" });
      entry.status = body.approve === false ? "denied" : "authorized";
      if (entry.status === "authorized") entry.token = issueToken(DEMO_USER);
      return json(res, 200, { ok: true, status: entry.status });
    }
    return html(
      res,
      200,
      `<!doctype html><meta charset="utf-8"><title>Tokease – 授权设备</title>
<style>body{font-family:system-ui;max-width:420px;margin:12vh auto;padding:0 20px;color:#111}
code{font-size:28px;letter-spacing:.2em;display:block;margin:20px 0;padding:12px;background:#f3f3f3;border-radius:8px;text-align:center}
button{font-size:16px;padding:10px 22px;border-radius:8px;border:0;background:#111;color:#fff;cursor:pointer}
#ok{display:none;color:#0a7d3a}</style>
<h2>Tokease Mock – 授权桌面客户端</h2>
<p>确认下方代码与 Tokease 客户端中显示的一致，然后点击「允许」。</p>
<code id="c">${code || "——"}</code>
<button onclick="approve(true)">允许</button> <button onclick="approve(false)" style="background:#777">拒绝</button>
<p id="ok">已完成，可以关闭此页面回到 Tokease。</p>
<script>
async function approve(a){const r=await fetch('/device',{method:'POST',headers:{'content-type':'application/json'},body:JSON.stringify({user_code:${JSON.stringify(code)},approve:a})});
if(r.ok){document.getElementById('ok').style.display='block'}else{alert('未知代码')}}
</script>`
    );
  }

  // ---- password login -----------------------------------------------------
  if (req.method === "POST" && path === "/auth/login") {
    const { email, password } = await readJson(req);
    if (!email || password !== "demo") return json(res, 401, { error: "邮箱或密码错误（mock 密码为 demo）" });
    const user = { id: "u_" + email.split("@")[0], email, name: email.split("@")[0] };
    return json(res, 200, { access_token: issueToken(user), user });
  }

  // ---- platform config ----------------------------------------------------
  if (req.method === "GET" && path === "/client/config") {
    const t = bearer(req);
    if (!t || !tokens.has(t)) return json(res, 401, { error: "invalid or missing token" });
    if (NO_CLIENT_CONFIG) return json(res, 404, { error: "not implemented" });
    return json(res, 200, clientConfig());
  }

  // ---- fake upstream so a configured CLI gets *something* -----------------
  if (path.startsWith("/v1/") || path.startsWith("/v1beta/")) {
    const t = bearer(req) || req.headers["x-api-key"] || req.headers["x-goog-api-key"];
    if (!t || !tokens.has(t)) return json(res, 401, { error: { message: "invalid token" } });
    return json(res, 200, {
      id: "mock",
      object: "mock",
      note: "This is the Tokease mock server. Model routing happens on the real server.",
    });
  }

  json(res, 404, { error: "not found" });
});

server.listen(PORT, "127.0.0.1", () => {
  console.log(`Tokease mock API listening on ${ORIGIN}`);
  console.log(`  password login: any email, password "demo"`);
  console.log(`  device login auto-approves after ${AUTO_APPROVE_SECONDS}s (or visit /device?user_code=...)`);
});

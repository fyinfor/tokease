import { useEffect, useRef, useState } from "react";
import { api } from "../lib/bridge";
import type { DeviceStart, SessionInfo } from "../lib/types";
import { errorText, isCmdError } from "../lib/types";

// On the login screen a 401 means "wrong credentials", so show the server's own message.
const loginError = (e: unknown) => (isCmdError(e) && e.kind === "unauthorized" ? e.message : errorText(e));

interface Props {
  onLoggedIn: (s: SessionInfo) => void;
}

export function LoginPanel({ onLoggedIn }: Props) {
  const [device, setDevice] = useState<DeviceStart | null>(null);
  const [showPassword, setShowPassword] = useState(false);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const cancelled = useRef(false);

  useEffect(() => {
    cancelled.current = false;
    return () => {
      cancelled.current = true;
    };
  }, []);

  const startDevice = async () => {
    setError(null);
    setBusy(true);
    try {
      const a = await api();
      const d = await a.startDeviceLogin();
      setDevice(d);
      await a.openUrl(d.verification_uri_complete || d.verification_uri);
      const deadline = Date.now() + d.expires_in * 1000;
      // Poll until the server says yes/no. `cancelled` stops the loop when the
      // user clicks cancel or the panel unmounts.
      while (!cancelled.current && Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, Math.max(1, d.interval) * 1000));
        if (cancelled.current) break;
        const p = await a.pollDeviceLogin(d.device_code);
        if (p.status === "pending") continue;
        if (p.status === "authorized") {
          onLoggedIn(p.session);
          return;
        }
        setError(p.status === "expired" ? "登录码已过期，请重试。" : "授权被拒绝。");
        break;
      }
      if (!cancelled.current && Date.now() >= deadline) setError("登录码已过期，请重试。");
    } catch (e) {
      setError(loginError(e));
    } finally {
      setBusy(false);
      setDevice(null);
    }
  };

  const cancelDevice = () => {
    cancelled.current = true;
    setDevice(null);
    setBusy(false);
    // re-arm for a later attempt
    setTimeout(() => (cancelled.current = false), 0);
  };

  const submitPassword = async (ev: React.FormEvent) => {
    ev.preventDefault();
    setError(null);
    setBusy(true);
    try {
      const s = await (await api()).loginWithPassword(email.trim(), password);
      onLoggedIn(s);
    } catch (e) {
      setError(loginError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="login">
      <h2>登录 Tokease</h2>
      <p className="muted">登录后即可一键把本地开发工具接入 Tokease，无需手动填写任何地址或密钥。</p>

      {device ? (
        <div className="device">
          <p className="muted">浏览器中确认以下代码后，这里会自动完成登录：</p>
          <code className="device__code">{device.user_code}</code>
          <div className="row">
            <button className="btn" onClick={() => api().then((a) => a.openUrl(device.verification_uri_complete || device.verification_uri))}>
              重新打开浏览器
            </button>
            <button className="btn btn--ghost" onClick={cancelDevice}>
              取消
            </button>
          </div>
        </div>
      ) : (
        <button className="btn btn--primary btn--block" disabled={busy} onClick={startDevice}>
          {busy ? <span className="spinner" /> : "通过浏览器登录"}
        </button>
      )}

      {!device && (
        <button className="link" onClick={() => setShowPassword((v) => !v)}>
          {showPassword ? "收起" : "使用账号密码登录"}
        </button>
      )}

      {showPassword && !device && (
        <form className="form" onSubmit={submitPassword}>
          <input type="email" placeholder="邮箱" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="username" />
          <input type="password" placeholder="密码" value={password} onChange={(e) => setPassword(e.target.value)} required autoComplete="current-password" />
          <button className="btn btn--block" type="submit" disabled={busy}>
            {busy ? <span className="spinner spinner--dark" /> : "登录"}
          </button>
        </form>
      )}

      {error && (
        <p className="card__error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

import { useState } from "react";
import type { ClientStatus } from "../lib/types";
import { errorText } from "../lib/types";

interface Props {
  client: ClientStatus;
  loggedIn: boolean;
  onEnable: () => Promise<void>;
  onRestore: () => Promise<void>;
}

const ICONS: Record<string, string> = { codex: "◆", claude: "✱", gemini: "✦" };

export function ClientCard({ client, loggedIn, onEnable, onRestore }: Props) {
  const [busy, setBusy] = useState<"enable" | "restore" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [flash, setFlash] = useState<string | null>(null);

  const run = async (kind: "enable" | "restore", fn: () => Promise<void>, done: string) => {
    setBusy(kind);
    setError(null);
    setFlash(null);
    try {
      await fn();
      setFlash(done);
      setTimeout(() => setFlash(null), 2500);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const canEnable = loggedIn && client.installed && client.available && !busy;
  const canRestore = (client.enabled || !!client.backup_id) && !busy;
  const hint = !client.installed
    ? "未检测到安装"
    : !client.available
      ? "当前套餐不可用"
      : !loggedIn
        ? "登录后即可一键启用"
        : client.enabled
          ? `已接入 · 模型 ${client.current.model ?? "-"}`
          : client.problems.length
            ? "配置已被修改，可重新启用"
            : "尚未接入 Tokease";

  return (
    <section className={`card ${client.enabled ? "card--on" : ""}`}>
      <header className="card__head">
        <span className="card__icon" aria-hidden>
          {ICONS[client.id]}
        </span>
        <div className="card__title">
          <h2>{client.name}</h2>
          <p className="card__hint">{hint}</p>
        </div>
        <div className="pills">
          <span className={`pill ${client.installed ? "pill--ok" : "pill--muted"}`}>{client.installed ? "已安装" : "未安装"}</span>
          <span className={`pill ${client.enabled ? "pill--on" : "pill--muted"}`}>{client.enabled ? "已启用" : "未启用"}</span>
        </div>
      </header>

      <div className="card__actions">
        <button className="btn btn--primary" disabled={!canEnable} onClick={() => run("enable", onEnable, "已启用")}>
          {busy === "enable" ? <span className="spinner" /> : client.enabled ? "重新启用" : "一键启用"}
        </button>
        <button className="btn" disabled={!canRestore} onClick={() => run("restore", onRestore, "已恢复原配置")}>
          {busy === "restore" ? <span className="spinner spinner--dark" /> : "恢复原配置"}
        </button>
        {flash && <span className="flash">{flash}</span>}
      </div>

      {error && (
        <p className="card__error" role="alert">
          {error}
        </p>
      )}

      {client.outdated && (
        <p className="card__warn">
          {client.name} 版本 {client.version} 过旧，需 ≥ {client.min_version} 才支持 Tokease 的接入方式，请先升级。
        </p>
      )}
      {client.env_conflicts.length > 0 && (
        <div className="card__warn">
          <p>以下环境变量会覆盖配置文件，Tokease 不会修改它们，请自行清理：</p>
          <ul>
            {client.env_conflicts.map((c) => (
              <li key={`${c.source}:${c.name}`}>
                <code>{c.name}</code> 来自 {c.source === "process" ? "当前进程环境" : c.source}（{c.preview}）
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

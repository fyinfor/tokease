import { useState } from "react";
import type { ClientId, ClientStatus } from "../lib/types";
import { errorText } from "../lib/types";
import { MarkClaude, MarkCodex, MarkGemini, MarkGrok, MarkOpenCode } from "./icons";

interface Props {
  client: ClientStatus;
  loggedIn: boolean;
  onEnable: () => Promise<void>;
  onRestore: () => Promise<void>;
  onNeedLogin: () => void;
  onSessions: () => void;
}

const META: Record<ClientId, { vendor: string; kind: string; mark: typeof MarkCodex }> = {
  codex: { vendor: "OpenAI", kind: "CLI", mark: MarkCodex },
  claude: { vendor: "Anthropic", kind: "CLI", mark: MarkClaude },
  "claude-desktop": { vendor: "Anthropic", kind: "Desktop", mark: MarkClaude },
  gemini: { vendor: "Google", kind: "CLI", mark: MarkGemini },
  grok: { vendor: "xAI", kind: "CLI", mark: MarkGrok },
  opencode: { vendor: "OpenCode", kind: "CLI", mark: MarkOpenCode },
};

export function ClientCard({ client, loggedIn, onEnable, onRestore, onNeedLogin, onSessions }: Props) {
  const [busy, setBusy] = useState<"enable" | "restore" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [flash, setFlash] = useState<string | null>(null);
  const meta = META[client.id];
  const Mark = meta.mark;

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

  const needsLogin = !loggedIn && client.installed && client.available && !busy;
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
    <article className={`tool tool--${client.id} ${client.installed ? "" : "tool--off"}`}>
      <div className="tool__row">
        <span className={`mark mark--${client.id}`} aria-hidden>
          <Mark />
        </span>
        <div className="tool__copy">
          <div className="tool__title">
            <h2>{client.name}</h2>
            <span className="tag">{meta.kind}</span>
            <span className="tag tag--vendor">{meta.vendor}</span>
          </div>
          <p>{hint}</p>
        </div>
        <div className="tool__side">
          <span className={client.installed ? "install install--on" : "install"}>
            <i />
            {client.installed ? "已安装" : "未安装"}
          </span>
          <div className="tool__actions">
            <button
              type="button"
              className="btn btn--primary"
              disabled={!canEnable && !needsLogin}
              onClick={() => (needsLogin ? onNeedLogin() : run("enable", onEnable, client.enabled ? "已重新启用" : "已启用"))}
            >
              {busy === "enable" ? <span className="spinner" /> : client.enabled ? "重新启用" : "一键启用"}
            </button>
            <button type="button" className="btn btn--quiet" disabled={!canRestore} onClick={() => run("restore", onRestore, "已恢复原配置")}>
              {busy === "restore" ? <span className="spinner" /> : "恢复原配置"}
            </button>
            <button type="button" className="btn btn--quiet" onClick={onSessions}>
              会话记录
            </button>
          </div>
          {flash && <span className="flash">{flash}</span>}
        </div>
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
    </article>
  );
}

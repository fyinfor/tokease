import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/bridge";
import type { ChatSession, ChatTranscript, ClientId, ClientStatus } from "../lib/types";
import { errorText } from "../lib/types";

interface Props {
  clientId: ClientId;
  clients: ClientStatus[];
  onBack: () => void;
}

function when(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

export function Sessions({ clientId, clients, onBack }: Props) {
  const [rows, setRows] = useState<ChatSession[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<ChatTranscript | null>(null);
  const [opening, setOpening] = useState(false);

  useEffect(() => {
    let alive = true;
    api()
      .then((a) => a.listChatSessions())
      .then((list) => {
        if (alive) setRows(list);
      })
      .catch((e) => {
        if (alive) setError(errorText(e));
      });
    return () => {
      alive = false;
    };
  }, []);

  const names = useMemo(() => {
    const map = new Map<ClientId, string>();
    for (const c of clients) map.set(c.id, c.name);
    return map;
  }, [clients]);

  const name = names.get(clientId) ?? clientId;
  const visible = (rows ?? []).filter((row) => row.client === clientId);

  const openRow = async (id: string) => {
    setOpening(true);
    setError(null);
    try {
      setOpen(await (await api()).readChatSession(id));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setOpening(false);
    }
  };

  return (
    <div className="subpage subpage--session">
      <header className="pagehead pagehead--split">
        <div>
          <h1>{open ? "会话内容" : `${name} 的会话记录`}</h1>
          <p>
            {open
              ? `${name} · ${open.session.model ?? "未记录模型"}`
              : `只显示 ${name} 保存在本机的历史对话。`}
          </p>
        </div>
        <button type="button" className="btn btn--quiet" onClick={() => (open ? setOpen(null) : onBack())}>
          {open ? "返回列表" : "返回首页"}
        </button>
      </header>

      {error && (
        <p className="card__error" role="alert">
          {error}
        </p>
      )}

      {open ? (
        <section className="thread" aria-label="会话内容">
          <div className="thread__meta">
            <strong>{open.session.title}</strong>
            <span>{when(open.session.updated_at)}</span>
            {open.session.cwd && <span className="mono">{open.session.cwd}</span>}
          </div>
          {open.missing_file && <p className="card__warn">会话文件已经不在磁盘上，只能看到索引里的摘要。</p>}
          {open.messages.length === 0 && !open.missing_file && <p className="muted">这场会话里没有可显示的消息。</p>}
          <ol className="thread__list">
            {open.messages.map((m, i) => (
              <li key={`${m.at ?? "t"}-${i}`} className={m.role === "user" ? "turn turn--user" : "turn"}>
                <span className="turn__who">{m.role === "user" ? "你" : "助手"}</span>
                <p>{m.text}</p>
              </li>
            ))}
          </ol>
          {open.truncated && <p className="muted">这场会话较长，这里只显示前面一部分。</p>}
        </section>
      ) : rows === null && !error ? (
        <div className="stage__wait">
          <span className="spinner" />
        </div>
      ) : visible.length === 0 ? (
        <section className="panel panel--empty">
          <p>还没有读到 {name} 的历史会话。</p>
        </section>
      ) : (
        <ul className="sessions">
            {visible.map((row) => (
              <li key={row.id}>
                <button type="button" className="session" onClick={() => void openRow(row.id)} disabled={opening}>
                  <span className="session__top">
                    <strong>{row.title}</strong>
                    <span>{when(row.updated_at)}</span>
                  </span>
                  <span className="session__meta">
                    <span className="tag">{names.get(row.client) ?? row.client}</span>
                    {row.model && <span className="tag tag--vendor">{row.model}</span>}
                    {row.cwd && <span className="mono session__cwd">{row.cwd}</span>}
                  </span>
                </button>
              </li>
            ))}
        </ul>
      )}
    </div>
  );
}

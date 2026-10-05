import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/bridge";
import type { LocalSkill } from "../lib/types";
import { errorText } from "../lib/types";

const SOURCES = [
  { id: "all", label: "全部" },
  { id: "agents", label: "共享" },
  { id: "codex", label: "Codex" },
  { id: "claude", label: "Claude" },
  { id: "cursor", label: "Cursor" },
] as const;

const KIND: Record<string, string> = {
  user: "已安装",
  system: "系统",
  plugin: "插件",
};

export function Skills() {
  const [rows, setRows] = useState<LocalSkill[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [source, setSource] = useState<(typeof SOURCES)[number]["id"]>("all");
  const [query, setQuery] = useState("");

  useEffect(() => {
    let alive = true;
    api()
      .then((a) => a.listSkills())
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

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (rows ?? []).filter((row) => {
      if (source !== "all" && row.source !== source) return false;
      if (!q) return true;
      return `${row.name} ${row.description} ${row.path}`.toLowerCase().includes(q);
    });
  }, [query, rows, source]);

  return (
    <div className="subpage subpage--session">
      <header className="pagehead">
        <h1>Skill管理</h1>
        <p>读取本机已安装的 Skill，包括共享目录、Codex、Claude 和 Cursor。</p>
      </header>

      <div className="filters">
        {SOURCES.map((item) => (
          <button key={item.id} type="button" className={source === item.id ? "chip chip--on" : "chip"} onClick={() => setSource(item.id)}>
            {item.label}
          </button>
        ))}
      </div>
      <input aria-label="筛选 Skill" placeholder="按名称、说明或路径筛选" value={query} onChange={(e) => setQuery(e.target.value)} />

      {error && (
        <p className="card__error" role="alert">
          {error}
        </p>
      )}

      {rows === null && !error ? (
        <div className="stage__wait">
          <span className="spinner" />
        </div>
      ) : visible.length === 0 ? (
        <section className="panel panel--empty">
          <p>{rows && rows.length > 0 ? "没有符合筛选的 Skill。" : "还没有读到本机的 Skill。"}</p>
        </section>
      ) : (
        <ul className="sessions">
          {visible.map((row) => (
            <li key={row.id} className="session skill">
              <span className="session__top">
                <strong>{row.name}</strong>
                <span>{SOURCES.find((s) => s.id === row.source)?.label ?? row.source}</span>
              </span>
              {row.description && <p className="skill__desc">{row.description}</p>}
              <span className="session__meta">
                <span className="tag">{KIND[row.kind] ?? row.kind}</span>
                <span className="mono session__cwd">{row.path}</span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

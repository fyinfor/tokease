import { Fragment, useEffect, useRef, useState } from "react";
import { api } from "../lib/bridge";
import type { BackupSummary, ClientConfig, ClientId, ClientStatus, SessionInfo } from "../lib/types";
import { errorText } from "../lib/types";

interface Props {
  session: SessionInfo;
  clients: ClientStatus[];
  onSession: (s: SessionInfo) => void;
  onClient: (c: ClientStatus) => void;
}

const BACKEND_LABEL: Record<SessionInfo["storage_backend"], string> = {
  keychain: "系统钥匙串 / 凭据管理器",
  encrypted_file_fallback: "本地文件（0600，系统钥匙串不可用时的回退）",
  none: "—",
};

const SERVERS = ["https://www.tokease.cn/v1", "https://www.tokease.com/v1"] as const;

function canonical(url: string) {
  return url.trim().replace(/\/+$/, "");
}

export function Advanced({ session, clients, onSession, onClient }: Props) {
  const [config, setConfig] = useState<ClientConfig | null>(null);
  const [backups, setBackups] = useState<Record<string, BackupSummary[]>>({});
  const [tab, setTab] = useState("server");
  const [serverOpen, setServerOpen] = useState(false);
  const serverBox = useRef<HTMLDivElement>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const tabs = [
    { id: "server", label: "服务器" },
    { id: "session", label: "会话" },
    { id: "platform", label: "平台配置" },
    ...clients.map((c) => ({ id: `client:${c.id}`, label: c.name })),
  ];
  const active = tabs.some((item) => item.id === tab) ? tab : "server";
  const activeClient = clients.find((c) => `client:${c.id}` === active);

  const selectTab = (id: string) => {
    setTab(id);
    setMsg(null);
    setErr(null);
  };

  const reloadBackups = async () => {
    const a = await api();
    const entries = await Promise.all(clients.map(async (c) => [c.id, await a.listBackups(c.id)] as const));
    setBackups(Object.fromEntries(entries));
  };

  useEffect(() => {
    if (!serverOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (!serverBox.current?.contains(e.target as Node)) setServerOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setServerOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [serverOpen]);

  useEffect(() => {
    api().then((a) => a.getPlatformConfig().then(setConfig).catch(() => setConfig(null)));
    reloadBackups().catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clients.map((c) => c.backup_id).join(",")]);

  const wrap = async (fn: () => Promise<string | void>) => {
    setErr(null);
    setMsg(null);
    try {
      const m = await fn();
      if (m) setMsg(m);
    } catch (e) {
      setErr(errorText(e));
    }
  };

  const chooseServer = (url: (typeof SERVERS)[number]) => {
    setServerOpen(false);
    if (canonical(session.server_url) === url) return;
    return wrap(async () => {
      onSession(await (await api()).setServerUrl(url));
      return "已切换服务器";
    });
  };

  const refreshConfig = () =>
    wrap(async () => {
      setConfig(await (await api()).refreshPlatformConfig());
      return "平台配置已刷新";
    });

  const logout = () =>
    wrap(async () => {
      onSession(await (await api()).logout());
      return "已退出登录（本地工具配置未改动，可在首页恢复）";
    });

  const currentServer = SERVERS.find((url) => url === canonical(session.server_url)) ?? SERVERS[0];

  const restoreTo = (id: ClientId, backupId: string) =>
    wrap(async () => {
      onClient(await (await api()).restoreClient(id, backupId));
      await reloadBackups();
      return `已恢复到备份 ${backupId}`;
    });

  return (
    <main className="advanced">
      <h2>设置</h2>

      <div className="tabs" role="tablist" aria-label="设置">
        {tabs.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            id={`adv-tab-${item.id}`}
            aria-selected={active === item.id}
            aria-controls={`adv-panel-${item.id}`}
            className={active === item.id ? "tab tab--on" : "tab"}
            onClick={() => selectTab(item.id)}
          >
            {item.label}
          </button>
        ))}
      </div>

      {active === "server" && (
      <section className="block" role="tabpanel" id="adv-panel-server" aria-labelledby="adv-tab-server">
        <h3>服务器</h3>
        <div className="select" ref={serverBox}>
          <button
            type="button"
            className="select__btn"
            aria-label="服务器"
            aria-haspopup="listbox"
            aria-expanded={serverOpen}
            onClick={() => setServerOpen((v) => !v)}
          >
            <span>{currentServer}</span>
            <i className="select__chev" />
          </button>
          {serverOpen && (
            <ul className="select__menu" role="listbox" aria-label="服务器">
              {SERVERS.map((url) => (
                <li key={url}>
                  <button type="button" role="option" aria-selected={url === currentServer} className={url === currentServer ? "select__opt select__opt--on" : "select__opt"} onClick={() => chooseServer(url)}>
                    {url}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>
      )}

      {active === "session" && (
      <section className="block" role="tabpanel" id="adv-panel-session" aria-labelledby="adv-tab-session">
        <h3>会话</h3>
        <dl className="kv">
          <dt>状态</dt>
          <dd>{session.logged_in ? `已登录 ${session.user?.email ?? session.user?.id ?? ""}` : "未登录"}</dd>
          <dt>Token</dt>
          <dd>{session.token_preview ?? "—"}</dd>
          <dt>存储位置</dt>
          <dd>{BACKEND_LABEL[session.storage_backend]}</dd>
          <dt>数据目录</dt>
          <dd className="mono">{session.data_dir}</dd>
        </dl>
        {session.logged_in && (
          <button className="btn btn--ghost" onClick={logout}>
            退出登录
          </button>
        )}
      </section>
      )}

      {active === "platform" && (
      <section className="block" role="tabpanel" id="adv-panel-platform" aria-labelledby="adv-tab-platform">
        <h3>
          平台配置{" "}
          {session.logged_in && (
            <button className="link" onClick={refreshConfig}>
              刷新
            </button>
          )}
        </h3>
        {config ? (
          <dl className="kv">
            <dt>Base URL</dt>
            <dd className="mono">{config.base_url}</dd>
            {Object.entries(config.endpoints ?? {}).map(([k, v]) => (
              <Fragment key={k}>
                <dt>{k}</dt>
                <dd className="mono">{v}</dd>
              </Fragment>
            ))}
            <dt>默认模型</dt>
            <dd>{config.default_model}</dd>
            <dt>模型</dt>
            <dd>
              {config.models.map((m) => (
                <span key={m.id} className="pill pill--muted" title={m.description}>
                  {m.id} · {m.name}
                </span>
              ))}
            </dd>
          </dl>
        ) : (
          <p className="muted">登录后自动获取。</p>
        )}
      </section>
      )}

      {activeClient && (
        <section className="block" role="tabpanel" id={`adv-panel-client:${activeClient.id}`} aria-labelledby={`adv-tab-client:${activeClient.id}`}>
          <h3>{activeClient.name}</h3>
          <dl className="kv">
            <dt>可执行文件</dt>
            <dd className="mono">{activeClient.binary_path ?? "未找到"}</dd>
            <dt>版本</dt>
            <dd className="mono">
              {activeClient.version ?? "—"}
              {activeClient.min_version ? `（需 ≥ ${activeClient.min_version}）` : ""}
            </dd>
            <dt>配置目录</dt>
            <dd className="mono">{activeClient.config_dir}</dd>
            <dt>管理的文件</dt>
            <dd className="mono">
              {activeClient.managed_files.map((f) => (
                <div key={f}>{f}</div>
              ))}
            </dd>
            <dt>当前指向</dt>
            <dd className="mono">{activeClient.current.base_url ?? "—"}</dd>
            {activeClient.problems.length > 0 && (
              <>
                <dt>问题</dt>
                <dd>
                  {activeClient.problems.map((p) => (
                    <div key={p} className="small">
                      {p}
                    </div>
                  ))}
                </dd>
              </>
            )}
          </dl>
          <h4>备份</h4>
          {(backups[activeClient.id] ?? []).length === 0 ? (
            <p className="muted small">暂无备份。</p>
          ) : (
            <ul className="backups">
              {(backups[activeClient.id] ?? []).map((b) => (
                <li key={b.id}>
                  <span className="mono">{b.id}</span>
                  {b.id === activeClient.backup_id && <span className="pill pill--on">恢复点</span>}
                  <span className="muted small">{b.files.length} 个文件</span>
                  <button className="link" onClick={() => api().then((a) => a.revealPath(b.dir))}>
                    打开
                  </button>
                  <button className="link" onClick={() => restoreTo(activeClient.id, b.id)}>
                    恢复到此
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      {msg && <p className="flash">{msg}</p>}
      {err && (
        <p className="card__error" role="alert">
          {err}
        </p>
      )}
    </main>
  );
}

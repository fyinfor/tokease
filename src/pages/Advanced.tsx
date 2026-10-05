import { Fragment, useEffect, useState } from "react";
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

export function Advanced({ session, clients, onSession, onClient }: Props) {
  const [serverUrl, setServerUrl] = useState(session.server_url);
  const [config, setConfig] = useState<ClientConfig | null>(null);
  const [backups, setBackups] = useState<Record<string, BackupSummary[]>>({});
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);

  const reloadBackups = async () => {
    const a = await api();
    const entries = await Promise.all(clients.map(async (c) => [c.id, await a.listBackups(c.id)] as const));
    setBackups(Object.fromEntries(entries));
  };

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

  const saveServer = () =>
    wrap(async () => {
      onSession(await (await api()).setServerUrl(serverUrl));
      return "服务器地址已保存";
    });

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

  const restoreTo = (id: ClientId, backupId: string) =>
    wrap(async () => {
      onClient(await (await api()).restoreClient(id, backupId));
      await reloadBackups();
      return `已恢复到备份 ${backupId}`;
    });

  return (
    <main className="advanced">
      <h2>高级</h2>
      <p className="muted">普通使用无需改动这里。</p>

      <section className="block">
        <h3>服务器</h3>
        <div className="row">
          <input value={serverUrl} onChange={(e) => setServerUrl(e.target.value)} spellCheck={false} />
          <button className="btn" onClick={saveServer}>
            保存
          </button>
        </div>
        <p className="muted small">也可用环境变量 TOKEASE_SERVER_URL 覆盖（例如本地 Mock：http://127.0.0.1:8787）。</p>
      </section>

      <section className="block">
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

      <section className="block">
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

      {clients.map((c) => (
        <section className="block" key={c.id}>
          <h3>{c.name}</h3>
          <dl className="kv">
            <dt>可执行文件</dt>
            <dd className="mono">{c.binary_path ?? "未找到"}</dd>
            <dt>版本</dt>
            <dd className="mono">
              {c.version ?? "—"}
              {c.min_version ? `（需 ≥ ${c.min_version}）` : ""}
            </dd>
            <dt>配置目录</dt>
            <dd className="mono">{c.config_dir}</dd>
            <dt>管理的文件</dt>
            <dd className="mono">
              {c.managed_files.map((f) => (
                <div key={f}>{f}</div>
              ))}
            </dd>
            <dt>当前指向</dt>
            <dd className="mono">{c.current.base_url ?? "—"}</dd>
            {c.problems.length > 0 && (
              <>
                <dt>问题</dt>
                <dd>
                  {c.problems.map((p) => (
                    <div key={p} className="small">
                      {p}
                    </div>
                  ))}
                </dd>
              </>
            )}
          </dl>
          <h4>备份</h4>
          {(backups[c.id] ?? []).length === 0 ? (
            <p className="muted small">暂无备份。</p>
          ) : (
            <ul className="backups">
              {(backups[c.id] ?? []).map((b) => (
                <li key={b.id}>
                  <span className="mono">{b.id}</span>
                  {b.id === c.backup_id && <span className="pill pill--on">恢复点</span>}
                  <span className="muted small">{b.files.length} 个文件</span>
                  <button className="link" onClick={() => api().then((a) => a.revealPath(b.dir))}>
                    打开
                  </button>
                  <button className="link" onClick={() => restoreTo(c.id, b.id)}>
                    恢复到此
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      ))}

      {msg && <p className="flash">{msg}</p>}
      {err && (
        <p className="card__error" role="alert">
          {err}
        </p>
      )}
    </main>
  );
}

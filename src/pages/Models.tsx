import { useEffect, useState } from "react";
import { api } from "../lib/bridge";
import type { ClientConfig, ClientStatus, SessionInfo } from "../lib/types";

interface Props {
  session: SessionInfo;
  clients: ClientStatus[];
  onNeedLogin: () => void;
}

export function Models({ session, clients, onNeedLogin }: Props) {
  const [config, setConfig] = useState<ClientConfig | null>(null);

  useEffect(() => {
    if (!session.logged_in) {
      setConfig(null);
      return;
    }
    api().then((a) => a.getPlatformConfig().then(setConfig).catch(() => setConfig(null)));
  }, [session.logged_in]);

  return (
    <div className="subpage">
      <header className="pagehead">
        <h1>模型切换</h1>
        <p>每个客户端单独选择模型。列表来自平台配置，切换只改模型字段。</p>
      </header>

      {!session.logged_in ? (
        <section className="panel panel--empty">
          <p>登录后可以查看并选择平台上的模型。</p>
          <button type="button" className="btn btn--primary" onClick={onNeedLogin}>
            登录
          </button>
        </section>
      ) : (
        <>
          <div className="catalog">
            {(config?.models ?? []).map((m) => (
              <article key={m.id} className={`catalog__item ${m.id === config?.default_model ? "catalog__item--on" : ""}`}>
                <div>
                  <h2>{m.name}</h2>
                  <p className="mono">{m.id}</p>
                </div>
                {m.id === config?.default_model && <span className="pill pill--live">默认</span>}
              </article>
            ))}
            {!config && <p className="muted">正在读取平台模型…</p>}
          </div>
          <div className="tools">
            {clients.map((c) => (
              <article key={c.id} className="tool">
                <div className="tool__row">
                  <div className="tool__copy">
                    <div className="tool__title">
                      <h2>{c.name}</h2>
                    </div>
                    <p>{c.enabled ? `当前模型 ${c.current.model ?? "—"}` : "尚未接入，启用后写入默认模型"}</p>
                  </div>
                  <span className={c.enabled ? "install install--on" : "install"}>
                    <i />
                    {c.enabled ? "已接入" : "未接入"}
                  </span>
                </div>
              </article>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

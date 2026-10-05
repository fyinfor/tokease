import { ClientCard } from "../components/ClientCard";
import { LoginPanel } from "../components/LoginPanel";
import { api } from "../lib/bridge";
import type { ClientId, ClientStatus, SessionInfo } from "../lib/types";

interface Props {
  session: SessionInfo;
  clients: ClientStatus[];
  onSession: (s: SessionInfo) => void;
  onClient: (c: ClientStatus) => void;
}

export function Home({ session, clients, onSession, onClient }: Props) {
  const act = (id: ClientId, kind: "enable" | "restore") => async () => {
    const a = await api();
    const c = kind === "enable" ? await a.enableClient(id) : await a.restoreClient(id);
    onClient(c);
  };

  return (
    <main className="home">
      <div className={`session ${session.logged_in ? "session--on" : ""}`}>
        <span className="dot" />
        {session.logged_in ? (
          <span>
            已登录 · <strong>{session.user?.name || session.user?.email || session.user?.id}</strong>
          </span>
        ) : (
          <span>未登录</span>
        )}
      </div>

      {!session.logged_in && <LoginPanel onLoggedIn={onSession} />}

      <div className="cards">
        {clients.map((c) => (
          <ClientCard key={c.id} client={c} loggedIn={session.logged_in} onEnable={act(c.id, "enable")} onRestore={act(c.id, "restore")} />
        ))}
      </div>

      <p className="foot muted">启用前会自动备份原配置，随时可以一键恢复。</p>
    </main>
  );
}

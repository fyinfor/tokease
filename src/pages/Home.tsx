import { ClientCard } from "../components/ClientCard";
import { PhonePanel } from "../components/PhonePanel";
import { api } from "../lib/bridge";
import type { ClientId, ClientStatus, SessionInfo } from "../lib/types";

interface Props {
  session: SessionInfo;
  clients: ClientStatus[];
  onClient: (c: ClientStatus) => void;
  onNeedLogin: () => void;
  onSessions: (id: ClientId) => void;
}

export function Home({ session, clients, onClient, onNeedLogin, onSessions }: Props) {
  const act = (id: ClientId, kind: "enable" | "restore") => async () => {
    const a = await api();
    const c = kind === "enable" ? await a.enableClient(id) : await a.restoreClient(id);
    onClient(c);
  };

  return (
    <div className="home">
      <section className="home__main">
        <header className="pagehead">
          <h1>AI 模型与工具</h1>
          <p>一键启用，快速切换，释放更强生产力。</p>
        </header>
        <div className="tools">
          {clients.map((c) => (
            <ClientCard
              key={c.id}
              client={c}
              loggedIn={session.logged_in}
              onEnable={act(c.id, "enable")}
              onRestore={act(c.id, "restore")}
              onNeedLogin={onNeedLogin}
              onSessions={() => onSessions(c.id)}
            />
          ))}
        </div>
      </section>
      <aside className="home__side">
        <PhonePanel />
      </aside>
    </div>
  );
}

import { ClientCard } from "../components/ClientCard";
import { PhonePanel } from "../components/PhonePanel";
import { RoutePanel, type RouteMode } from "../components/RoutePanel";
import { api } from "../lib/bridge";
import type { ClientId, ClientStatus, SessionInfo } from "../lib/types";

interface Props {
  session: SessionInfo;
  clients: ClientStatus[];
  routeMode: RouteMode;
  onRouteMode: (mode: RouteMode) => void;
  onClient: (c: ClientStatus) => void;
  onNeedLogin: () => void;
}

export function Home({ session, clients, routeMode, onRouteMode, onClient, onNeedLogin }: Props) {
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
            />
          ))}
        </div>
      </section>
      <aside className="home__side">
        <RoutePanel mode={routeMode} onChange={onRouteMode} />
        <PhonePanel />
      </aside>
    </div>
  );
}

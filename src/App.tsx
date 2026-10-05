import { useCallback, useEffect, useState } from "react";
import { Logo } from "./components/Logo";
import { api } from "./lib/bridge";
import type { ClientStatus, SessionInfo } from "./lib/types";
import { errorText } from "./lib/types";
import { Advanced } from "./pages/Advanced";
import { Home } from "./pages/Home";

export default function App() {
  const [page, setPage] = useState<"home" | "advanced">("home");
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [clients, setClients] = useState<ClientStatus[]>([]);
  const [fatal, setFatal] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const a = await api();
      const [s, c] = await Promise.all([a.getSession(), a.listClients()]);
      setSession(s);
      setClients(c);
    } catch (e) {
      setFatal(errorText(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    // Re-check installs / external edits whenever the window regains focus.
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  const onSession = (s: SessionInfo) => {
    setSession(s);
    // Login/logout changes availability flags; reload cards.
    api().then((a) => a.listClients()).then(setClients).catch(() => {});
  };
  const onClient = (c: ClientStatus) => setClients((prev) => prev.map((x) => (x.id === c.id ? c : x)));

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <Logo />
          <span>Tokease</span>
        </div>
        <button
          className="icon-btn"
          title={page === "home" ? "高级" : "返回"}
          aria-label={page === "home" ? "高级" : "返回"}
          onClick={() => setPage(page === "home" ? "advanced" : "home")}
        >
          {page === "home" ? "⚙" : "←"}
        </button>
      </header>

      {fatal ? (
        <main className="home">
          <p className="card__error">{fatal}</p>
        </main>
      ) : !session ? (
        <main className="home center">
          <span className="spinner spinner--dark" />
        </main>
      ) : page === "home" ? (
        <Home session={session} clients={clients} onSession={onSession} onClient={onClient} />
      ) : (
        <Advanced session={session} clients={clients} onSession={onSession} onClient={onClient} />
      )}
    </div>
  );
}

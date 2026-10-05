import { useCallback, useEffect, useMemo, useState } from "react";
import { CommandPalette, type PaletteItem } from "./components/CommandPalette";
import { IconClose, IconCube, IconHome, IconMcp, IconMinus, IconSearch, IconSkill, IconSquare, IconTasks, IconUser } from "./components/icons";
import { LoginPanel } from "./components/LoginPanel";
import { Logo } from "./components/Logo";
import { TaskDock } from "./components/TaskDock";
import { isTauri, api } from "./lib/bridge";
import type { ClientStatus, SessionInfo } from "./lib/types";
import { errorText } from "./lib/types";
import { Advanced } from "./pages/Advanced";
import { Home } from "./pages/Home";
import { Models } from "./pages/Models";
import { Sessions } from "./pages/Sessions";
import { Mcps } from "./pages/Mcps";
import { Skills } from "./pages/Skills";
import { Tasks } from "./pages/Tasks";

type Page = "home" | "models" | "sessions" | "skills" | "mcp" | "tasks" | "advanced";

const NAV: { id: Page; label: string; icon: typeof IconHome }[] = [
  { id: "home", label: "首页", icon: IconHome },
  { id: "models", label: "模型切换", icon: IconCube },
  { id: "skills", label: "Skill管理", icon: IconSkill },
  { id: "mcp", label: "MCP管理", icon: IconMcp },
  { id: "tasks", label: "任务管理", icon: IconTasks },
];

async function withWindow(
  fn: (win: {
    minimize: () => Promise<void>;
    toggleMaximize: () => Promise<void>;
    close: () => Promise<void>;
    isMaximized: () => Promise<boolean>;
  }) => Promise<void>,
) {
  if (!isTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await fn(getCurrentWindow());
}

export default function App() {
  const [page, setPage] = useState<Page>("home");
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [clients, setClients] = useState<ClientStatus[]>([]);
  const [fatal, setFatal] = useState<string | null>(null);
  const [loginOpen, setLoginOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [maximized, setMaximized] = useState(false);
  const [sessionClient, setSessionClient] = useState<ClientStatus["id"] | null>(null);

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
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  useEffect(() => {
    if (!isTauri) return;
    let stop = () => {};
    let alive = true;
    (async () => {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const win = getCurrentWindow();
      const sync = async () => {
        try {
          const on = await win.isMaximized();
          if (alive) setMaximized(on);
        } catch {
          /* 非桌面环境或权限未就绪时保持窗口态 */
        }
      };
      await sync();
      const unlist = await win.onResized(() => {
        void sync();
      });
      if (!alive) unlist();
      else stop = unlist;
    })();
    return () => {
      alive = false;
      stop();
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPaletteOpen(true);
        setQuery("");
      } else if (e.key === "Escape") {
        setPaletteOpen(false);
        setMenuOpen(false);
        setLoginOpen(false);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onSession = (s: SessionInfo) => {
    setSession(s);
    api().then((a) => a.listClients()).then(setClients).catch(() => {});
  };
  const onClient = (c: ClientStatus) => setClients((prev) => prev.map((x) => (x.id === c.id ? c : x)));
  const openLogin = () => {
    setMenuOpen(false);
    setLoginOpen(true);
  };

  const commands = useMemo<PaletteItem[]>(() => {
    const items: PaletteItem[] = [
      ...NAV.map((n) => ({ id: `page:${n.id}`, label: n.label, hint: "页面" })),
      { id: "page:advanced", label: "高级设置", hint: "页面" },
    ];
    if (session?.logged_in) items.push({ id: "logout", label: "退出登录", hint: session.user?.email ?? undefined });
    else items.push({ id: "login", label: "登录 Tokease", hint: "账号" });
    for (const c of clients) {
      if (c.installed) items.push({ id: `enable:${c.id}`, label: `启用 ${c.name}`, hint: c.enabled ? "已接入" : "客户端" });
    }
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter((item) => `${item.label} ${item.hint ?? ""}`.toLowerCase().includes(q));
  }, [clients, query, session]);

  const pick = async (id: string) => {
    setPaletteOpen(false);
    setQuery("");
    if (id.startsWith("page:")) {
      setPage(id.slice(5) as Page);
      return;
    }
    if (id === "login") {
      openLogin();
      return;
    }
    if (id === "logout") {
      onSession(await (await api()).logout());
      return;
    }
    if (id.startsWith("enable:")) {
      if (!session?.logged_in) {
        openLogin();
        return;
      }
      const cid = id.slice(7) as ClientStatus["id"];
      onClient(await (await api()).enableClient(cid));
      setPage("home");
    }
  };

  const userLabel = session?.logged_in ? session.user?.name || session.user?.email || session.user?.id || "已登录" : "未登录";

  return (
    <div className={maximized ? "shell shell--max" : "shell"}>
      <a className="skip" href="#content">
        跳到内容
      </a>
      <header className="top">
        <div className="brand" data-tauri-drag-region>
          <Logo />
          <span>Tokease</span>
        </div>
        <button type="button" className="search" onClick={() => { setPaletteOpen(true); setQuery(""); }}>
          <IconSearch size={15} />
          <span>搜索模型、任务或输入命令...</span>
          <kbd>Ctrl K</kbd>
        </button>
        <div className="top__drag" data-tauri-drag-region />
        <div className="top__right">
          <button type="button" className={`who ${session?.logged_in ? "who--on" : ""}`} onClick={() => (session?.logged_in ? setMenuOpen((v) => !v) : openLogin())}>
            <i />
            <span>
              {session?.logged_in ? "已登录" : "未登录"}
              {session?.logged_in ? ` · ${userLabel}` : ""}
            </span>
          </button>
          <button type="button" className="avatar" aria-label="账户" aria-expanded={menuOpen} onClick={() => setMenuOpen((v) => !v)}>
            <IconUser size={16} />
          </button>
          {menuOpen && (
            <div className="menu" role="menu">
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); setPage("advanced"); }}>
                高级设置
              </button>
              {session?.logged_in ? (
                <button
                  type="button"
                  role="menuitem"
                  onClick={() => {
                    setMenuOpen(false);
                    api().then((a) => a.logout()).then(onSession).catch(() => {});
                  }}
                >
                  退出登录
                </button>
              ) : (
                <button type="button" role="menuitem" onClick={openLogin}>
                  登录
                </button>
              )}
            </div>
          )}
          <div className="wins">
            <button type="button" aria-label="最小化" onClick={() => withWindow((w) => w.minimize())}>
              <IconMinus size={14} />
            </button>
            <button
              type="button"
              aria-label="最大化"
              onClick={() =>
                withWindow(async (w) => {
                  await w.toggleMaximize();
                  setMaximized(await w.isMaximized());
                })
              }
            >
              <IconSquare size={14} />
            </button>
            <button type="button" className="wins__close" aria-label="关闭" onClick={() => withWindow((w) => w.close())}>
              <IconClose size={14} />
            </button>
          </div>
        </div>
      </header>

      <div className="workspace">
        <nav className="side" aria-label="主导航">
          {NAV.map((item) => {
            const Icon = item.icon;
            const on = item.id === "home" ? page === "home" || page === "sessions" : page === item.id;
            return (
              <button key={item.id} type="button" className={on ? "navbtn navbtn--on" : "navbtn"} aria-current={on ? "page" : undefined} onClick={() => setPage(item.id)}>
                <Icon size={18} />
                {item.label}
              </button>
            );
          })}
        </nav>

        <div className="stage" id="content">
          {fatal ? (
            <p className="card__error" role="alert">
              {fatal}
            </p>
          ) : !session ? (
            <div className="stage__wait">
              <span className="spinner" />
            </div>
          ) : page === "home" ? (
            <Home
              session={session}
              clients={clients}
              onClient={onClient}
              onNeedLogin={openLogin}
              onSessions={(id) => {
                setSessionClient(id);
                setPage("sessions");
              }}
            />
          ) : page === "models" ? (
            <Models session={session} clients={clients} onNeedLogin={openLogin} />
          ) : page === "sessions" && sessionClient ? (
            <Sessions key={sessionClient} clientId={sessionClient} clients={clients} onBack={() => setPage("home")} />
          ) : page === "skills" ? (
            <Skills />
          ) : page === "mcp" ? (
            <Mcps />
          ) : page === "tasks" ? (
            <Tasks />
          ) : (
            <Advanced session={session} clients={clients} onSession={onSession} onClient={onClient} />
          )}
        </div>
      </div>

      <TaskDock onTasks={() => setPage("tasks")} />

      {loginOpen && (
        <div className="modal" role="presentation" onMouseDown={() => setLoginOpen(false)}>
          <div className="modal__panel" role="dialog" aria-label="登录" onMouseDown={(e) => e.stopPropagation()}>
            <LoginPanel
              onLoggedIn={(s) => {
                onSession(s);
                setLoginOpen(false);
              }}
            />
          </div>
        </div>
      )}

      <CommandPalette open={paletteOpen} query={query} items={commands} onQuery={setQuery} onClose={() => setPaletteOpen(false)} onPick={pick} />
    </div>
  );
}

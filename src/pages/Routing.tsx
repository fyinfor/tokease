import { RoutePanel, type RouteMode } from "../components/RoutePanel";

const COPY: Record<RouteMode, string> = {
  direct: "请求直接发往当前选中的模型，路径最短。",
  local: "由这台电脑按任务在已配置的模型之间选择，密钥留在本机。",
  hub: "交给 Tokease 聚合平台，按逻辑模型名做服务端路由。",
};

export function Routing({ mode, onChange }: { mode: RouteMode; onChange: (mode: RouteMode) => void }) {
  return (
    <div className="subpage">
      <header className="pagehead">
        <h1>智能路由</h1>
        <p>在本机决定这次任务用哪个模型。可以随时退回固定模型。</p>
      </header>
      <div className="subpage__split">
        <RoutePanel mode={mode} onChange={onChange} />
        <section className="panel">
          <h2 className="panel__solo">当前策略</h2>
          <p className="panel__desc">{COPY[mode]}</p>
          <p className="panel__desc">选中的模型仍通过现有适配器写进 Codex、Claude Code 和 Gemini CLI 的配置。</p>
        </section>
      </div>
    </div>
  );
}

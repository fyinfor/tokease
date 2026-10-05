import { IconBolt, IconGlobe, IconNodes, IconStack } from "./icons";

export type RouteMode = "direct" | "local" | "hub";

const MODES: { id: RouteMode; title: string; hint: string; icon: typeof IconBolt }[] = [
  { id: "direct", title: "直连", hint: "速度快", icon: IconBolt },
  { id: "local", title: "本地路由", hint: "更稳定", icon: IconGlobe },
  { id: "hub", title: "聚合平台", hint: "多模型", icon: IconStack },
];

interface Props {
  mode: RouteMode;
  onChange: (mode: RouteMode) => void;
}

export function RoutePanel({ mode, onChange }: Props) {
  return (
    <section className="panel">
      <header className="panel__head">
        <h2>
          <IconNodes size={16} />
          本地智能路由
        </h2>
        <span className="pill pill--live">
          <i />
          运行中
        </span>
      </header>
      <p className="panel__desc">自由选择模型访问方式，获得更稳定的体验。</p>
      <div className="modes" role="radiogroup" aria-label="路由方式">
        {MODES.map((m) => {
          const Icon = m.icon;
          const on = mode === m.id;
          return (
            <button key={m.id} type="button" role="radio" aria-checked={on} className={`mode ${on ? "mode--on" : ""}`} onClick={() => onChange(m.id)}>
              <Icon />
              <strong>{m.title}</strong>
              <span>{m.hint}</span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

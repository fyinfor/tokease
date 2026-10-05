import { IconPhone, MarkFeishu, MarkWechat, QrMark } from "./icons";

const CHANNELS = [
  { id: "wechat", name: "微信", connected: true, icon: <MarkWechat /> },
  { id: "feishu", name: "飞书", connected: false, icon: <MarkFeishu /> },
  { id: "app", name: "手机 App", connected: false, icon: <IconPhone size={16} /> },
];

export function PhonePanel() {
  return (
    <section className="panel">
      <header className="panel__head">
        <h2>
          <IconPhone size={16} />
          手机控制
        </h2>
        <span className="pill pill--live">
          <i />
          支持远程任务
        </span>
      </header>
      <p className="panel__desc">通过手机发送任务到 Tokease，随时随地开始工作。</p>
      <div className="phone">
        <QrMark />
        <ul className="channels">
          {CHANNELS.map((c) => (
            <li key={c.id} className={c.connected ? "channel channel--on" : "channel"}>
              <span className="channel__icon">{c.icon}</span>
              <span>{c.name}</span>
              <span className="channel__state">
                <i />
                {c.connected ? "已连接" : "未连接"}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

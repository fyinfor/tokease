import { IconPhone, IconSend, MarkClaude, MarkCodex, MarkGemini, MarkGrok, MarkOpenCode } from "../components/icons";

export function Tasks() {
  return (
    <div className="subpage">
      <header className="pagehead">
        <h1>任务管理</h1>
        <p>手机发来的任务会在这台电脑上执行，并调用已启用的 Codex、Claude、Gemini、Grok 或 OpenCode。</p>
      </header>
      <section className="panel task-empty">
        <span className="dock__plane" aria-hidden>
          <IconSend />
        </span>
        <h2>还没有任务</h2>
        <p>在微信、飞书或手机 App 里发送后，进度会显示为排队、进行中、完成或失败。</p>
        <div className="dock__flow">
          <span className="flow flow--static">
            <IconPhone size={14} />
            手机发送任务
          </span>
          <span className="dock__dash" aria-hidden />
          <span className="flow flow--static">
            <MarkCodex size={12} />
            Codex
          </span>
          <span className="flow flow--static">
            <MarkClaude size={12} />
            Claude
          </span>
          <span className="flow flow--static">
            <MarkGemini size={12} />
            Gemini
          </span>
          <span className="flow flow--static">
            <MarkGrok size={12} />
            Grok
          </span>
          <span className="flow flow--static">
            <MarkOpenCode size={12} />
            OpenCode
          </span>
        </div>
      </section>
    </div>
  );
}

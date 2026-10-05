import { IconPhone, IconSend, MarkClaude, MarkCodex, MarkGemini } from "./icons";

export function TaskDock({ onTasks }: { onTasks: () => void }) {
  return (
    <footer className="dock">
      <span className="dock__plane" aria-hidden>
        <IconSend />
      </span>
      <div className="dock__copy">
        <strong>从手机发送任务</strong>
        <p>在微信 / 飞书 / 手机 App 中发送任务，自动选择合适的模型处理。</p>
      </div>
      <div className="dock__flow">
        <button type="button" className="flow" onClick={onTasks}>
          <IconPhone size={14} />
          手机发送任务
        </button>
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
        <span className="flow flow--more" aria-hidden>
          ···
        </span>
      </div>
    </footer>
  );
}

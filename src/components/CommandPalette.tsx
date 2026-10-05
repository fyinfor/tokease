import { useEffect, useRef } from "react";
import { IconSearch } from "./icons";

export interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
}

interface Props {
  open: boolean;
  query: string;
  items: PaletteItem[];
  onQuery: (value: string) => void;
  onClose: () => void;
  onPick: (id: string) => void;
}

export function CommandPalette({ open, query, items, onQuery, onClose, onPick }: Props) {
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) input.current?.focus();
  }, [open]);

  if (!open) return null;

  return (
    <div className="modal" role="presentation" onMouseDown={onClose}>
      <div className="palette" role="dialog" aria-label="命令" onMouseDown={(e) => e.stopPropagation()}>
        <div className="palette__search">
          <IconSearch size={16} />
          <input ref={input} aria-label="搜索" value={query} placeholder="搜索模型、任务或输入命令..." onChange={(e) => onQuery(e.target.value)} />
          <kbd>Esc</kbd>
        </div>
        <ul className="palette__list">
          {items.length === 0 && <li className="palette__empty">没有匹配的命令</li>}
          {items.map((item) => (
            <li key={item.id}>
              <button type="button" onClick={() => onPick(item.id)}>
                <span>{item.label}</span>
                {item.hint && <em>{item.hint}</em>}
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

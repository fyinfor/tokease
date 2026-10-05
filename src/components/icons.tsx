import type { ReactNode } from "react";

interface IconProps {
  size?: number;
  className?: string;
}

function Svg({ size = 18, className, children }: IconProps & { children: ReactNode }) {
  return (
    <svg className={className} width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden>
      {children}
    </svg>
  );
}

export function IconSearch(p: IconProps) {
  return (
    <Svg {...p}>
      <circle cx="11" cy="11" r="6.25" stroke="currentColor" strokeWidth="1.7" />
      <path d="M16 16.5 20 20.5" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </Svg>
  );
}

export function IconHome(p: IconProps) {
  return (
    <Svg {...p}>
      <path d="M4.5 10.5 12 4.5l7.5 6V19a1.5 1.5 0 0 1-1.5 1.5h-4.2v-5.2H10.2V20.5H6A1.5 1.5 0 0 1 4.5 19v-8.5Z" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
    </Svg>
  );
}

export function IconCube(p: IconProps) {
  return (
    <Svg {...p}>
      <path d="M12 3.5 20 8v8l-8 4.5L4 16V8l8-4.5Z" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
      <path d="M12 12.2 20 8M12 12.2 4 8M12 12.2V20.5" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
    </Svg>
  );
}

export function IconSkill(p: IconProps) {
  return (
    <Svg {...p}>
      <path d="M8 4.5h8.2A1.8 1.8 0 0 1 18 6.3V20H8.2A2.2 2.2 0 0 1 6 17.8V6.7A2.2 2.2 0 0 1 8 4.5Z" stroke="currentColor" strokeWidth="1.7" strokeLinejoin="round" />
      <path d="M8 4.5v13.2A2.2 2.2 0 0 1 6 20" stroke="currentColor" strokeWidth="1.7" />
      <path d="M10.2 9h5M10.2 12.5h3.6" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </Svg>
  );
}

export function IconMcp(p: IconProps) {
  return (
    <Svg {...p}>
      <path d="M8 8.5V5.2A1.2 1.2 0 0 1 9.2 4h1.6A1.2 1.2 0 0 1 12 5.2V8" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
      <path d="M12 8v2.2" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
      <rect x="6.5" y="10.2" width="11" height="6.2" rx="1.6" stroke="currentColor" strokeWidth="1.7" />
      <path d="M9.2 16.4v2.2M14.8 16.4v2.2" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </Svg>
  );
}

export function IconTasks(p: IconProps) {
  return (
    <Svg {...p}>
      <rect x="6" y="3.5" width="12" height="17" rx="2" stroke="currentColor" strokeWidth="1.7" />
      <path d="M9 3.8h6v2.2H9z" stroke="currentColor" strokeWidth="1.7" />
      <path d="M9 11h6M9 14.5h4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </Svg>
  );
}

export function IconUser(p: IconProps) {
  return (
    <Svg {...p}>
      <circle cx="12" cy="9" r="3.1" stroke="currentColor" strokeWidth="1.7" />
      <path d="M6.2 18.6c1.2-2.6 3.2-3.8 5.8-3.8s4.6 1.2 5.8 3.8" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </Svg>
  );
}

export function IconMinus(p: IconProps) {
  return (
    <Svg {...p}>
      <path d="M6 12h12" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </Svg>
  );
}

export function IconSquare(p: IconProps) {
  return (
    <Svg {...p}>
      <rect x="6.5" y="6.5" width="11" height="11" rx="1.4" stroke="currentColor" strokeWidth="1.6" />
    </Svg>
  );
}

export function IconClose(p: IconProps) {
  return (
    <Svg {...p}>
      <path d="M7 7l10 10M17 7 7 17" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </Svg>
  );
}

export function IconPhone(p: IconProps) {
  return (
    <Svg {...p}>
      <rect x="7" y="3.2" width="10" height="17.6" rx="2" stroke="currentColor" strokeWidth="1.7" />
      <path d="M11 17.8h2" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
    </Svg>
  );
}

export function IconSend(p: IconProps) {
  return (
    <Svg {...p} size={p.size ?? 22}>
      <path d="M4 12.2 20 4.8l-6.2 14.4-2.2-5.6L4 12.2Z" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
      <path d="M11.6 13.6 20 4.8" stroke="currentColor" strokeWidth="1.6" />
    </Svg>
  );
}

export function MarkCodex({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path d="M12 2.8 20.2 12 12 21.2 3.8 12 12 2.8Z" fill="#f4f7ff" />
    </svg>
  );
}

export function MarkClaude({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path
        d="M12 2.2 13.5 8.2 18.8 4.8 15.6 10 21.8 12 15.6 14 18.8 19.2 13.5 15.8 12 21.8 10.5 15.8 5.2 19.2 8.4 14 2.2 12 8.4 10 5.2 4.8 10.5 8.2 12 2.2Z"
        fill="#ff7949"
      />
    </svg>
  );
}

export function MarkGrok({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path d="M4 6.2h10.2L8.4 18H4.2L4 6.2Z" fill="#f5f7ff" />
      <path d="M13.2 6.2H20L14.2 18h-4.2l3.2-11.8Z" fill="#27d8ff" />
    </svg>
  );
}

export function MarkOpenCode({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path d="M8.2 7.2 3.6 12l4.6 4.8-1.5 1.5L1 12l5.7-6.3 1.5 1.5Z" fill="#28e3a2" />
      <path d="M15.8 7.2 20.4 12l-4.6 4.8 1.5 1.5L23 12l-5.7-6.3-1.5 1.5Z" fill="#28e3a2" />
      <path d="m13.6 4.4-3.2 15.2h-1.7L11.9 4.4h1.7Z" fill="#f5f7ff" />
    </svg>
  );
}

export function MarkGemini({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path d="M12 1.6 14.2 9.8 22.4 12 14.2 14.2 12 22.4 9.8 14.2 1.6 12 9.8 9.8 12 1.6Z" fill="#8069ff" />
    </svg>
  );
}

export function MarkWechat({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path
        d="M9.2 4.2c-3.8 0-6.7 2.6-6.7 5.8 0 1.9 1.1 3.5 2.8 4.6l-.7 2.2 2.5-1.3c.6.2 1.3.3 2.1.3.3 0 .6 0 .9-.1-.2-.5-.3-1.1-.3-1.6 0-3.1 2.9-5.6 6.4-5.6.3 0 .6 0 .9.1C16.2 6.1 13 4.2 9.2 4.2Zm-2.2 4.6a.9.9 0 1 1 0-1.8.9.9 0 0 1 0 1.8Zm4.4 0a.9.9 0 1 1 0-1.8.9.9 0 0 1 0 1.8Z"
        fill="#3ddc84"
      />
      <path
        d="M17.2 9.4c-3 0-5.4 2-5.4 4.6s2.4 4.6 5.4 4.6c.6 0 1.1-.1 1.6-.2l2 1-.6-1.8c1.3-.9 2.2-2.1 2.2-3.6 0-2.6-2.4-4.6-5.2-4.6Zm-1.8 3.6a.75.75 0 1 1 0-1.5.75.75 0 0 1 0 1.5Zm3.6 0a.75.75 0 1 1 0-1.5.75.75 0 0 1 0 1.5Z"
        fill="#3ddc84"
      />
    </svg>
  );
}

export function MarkFeishu({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
      <path d="M4 15.5c3.2-1 5.2-3.6 6.2-6.2.4-1.1 1.6-1.2 2.2-.2 1.2 2 3.4 3.4 6.6 4.2-2.6 1.6-5.2 2.2-7.6 2.2-2.6 0-5.2-.8-7.4-2Z" fill="#4c8dff" />
      <path d="M13.2 4.2c.8 2.4.2 4.2-1.2 5.6 1.6.2 3 .1 4.4-.6-1-2.2-1.6-3.8-3.2-5Z" fill="#7eb0ff" />
    </svg>
  );
}

export function QrMark() {
  const n = 25;
  const finder = (ox: number, oy: number, x: number, y: number) => {
    if (x < ox || y < oy || x >= ox + 7 || y >= oy + 7) return false;
    const lx = x - ox;
    const ly = y - oy;
    const edge = lx === 0 || ly === 0 || lx === 6 || ly === 6;
    const core = lx >= 2 && lx <= 4 && ly >= 2 && ly <= 4;
    return edge || core;
  };
  const cells: { x: number; y: number }[] = [];
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      if (x > 8 && x < 16 && y > 8 && y < 16) continue;
      const hit = finder(0, 0, x, y) || finder(18, 0, x, y) || finder(0, 18, x, y);
      const noise = (x * 13 + y * 7 + (x * y) % 5) % 4 !== 0;
      if (hit || noise) cells.push({ x, y });
    }
  }
  const s = 4;
  return (
    <svg className="qr" viewBox={`0 0 ${n * s} ${n * s}`} role="img" aria-label="连接二维码">
      <rect width={n * s} height={n * s} rx="10" fill="#fff" />
      {cells.map((c) => (
        <rect key={`${c.x}-${c.y}`} x={c.x * s + 0.4} y={c.y * s + 0.4} width={s - 0.8} height={s - 0.8} fill="#141820" />
      ))}
      <rect x={9 * s} y={9 * s} width={7 * s} height={7 * s} rx="6" fill="#fff" />
      <rect x={10.15 * s} y={10.15 * s} width={4.7 * s} height={4.7 * s} rx="4" fill="#111" />
      <path d="M47.5 45.2h9.2v2H53v6.6h-2.4v-6.6h-3.1v-2Z" fill="#fff" />
    </svg>
  );
}

export function Logo({ size = 30 }: { size?: number }) {
  return (
    <svg className="logo" width={size} height={size} viewBox="0 0 32 32" aria-label="Tokease" role="img">
      <rect width="32" height="32" rx="8" fill="url(#tokease-logo)" />
      <rect x="0.6" y="0.6" width="30.8" height="30.8" rx="7.5" fill="none" stroke="rgba(82,155,255,0.45)" />
      <path d="M8.2 9.1h15.6v3.15h-6.15V23h-3.3V12.25H8.2V9.1Z" fill="url(#tokease-t)" />
      <defs>
        <linearGradient id="tokease-logo" x1="4" y1="2" x2="28" y2="30">
          <stop stopColor="#04070d" />
          <stop offset="1" stopColor="#0f1828" />
        </linearGradient>
        <linearGradient id="tokease-t" x1="8" y1="9" x2="24" y2="23">
          <stop stopColor="#ffffff" />
          <stop offset="1" stopColor="#79e7ff" />
        </linearGradient>
      </defs>
    </svg>
  );
}

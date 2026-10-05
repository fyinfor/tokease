export function Logo({ size = 28 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-label="Tokease" role="img">
      <rect x="2" y="2" width="60" height="60" rx="16" fill="#111" />
      {/* T: the crossbar is the "bridge" between tools and the platform */}
      <rect x="14" y="17" width="36" height="8" rx="4" fill="#fff" />
      <rect x="28" y="23" width="8" height="24" rx="4" fill="#fff" />
      <circle cx="46" cy="44" r="5" fill="#2bd576" />
    </svg>
  );
}

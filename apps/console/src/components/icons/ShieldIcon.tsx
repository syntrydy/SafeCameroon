interface ShieldIconProps {
  className?: string;
}

export function ShieldIcon({ className }: ShieldIconProps) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" className={className}>
      <path
        d="M12 2.5l7.5 3v5.2c0 4.86-3.2 9.24-7.5 10.8-4.3-1.56-7.5-5.94-7.5-10.8V5.5l7.5-3z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinejoin="round"
        fill="currentColor"
        fillOpacity="0.1"
      />
      <path
        d="M8.5 12.2l2.4 2.4 4.6-5.1"
        stroke="currentColor"
        strokeWidth="1.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

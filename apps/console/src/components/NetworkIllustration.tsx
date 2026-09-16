// Abstract "protection network" motif -- reports flowing in from a
// community, met by a verified, central response. Deliberately not
// photographic: this platform handles reports about at-risk people, so no
// stock imagery of children or victims belongs on the sign-in screen.
export function NetworkIllustration() {
  return (
    <div className="relative w-full max-w-md" aria-hidden="true">
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="h-72 w-72 rounded-full border border-emerald-500/10 animate-spin-very-slow" />
      </div>
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="h-56 w-56 rounded-full border border-blue-500/10 animate-spin-reverse" />
      </div>
      <div className="absolute inset-0 flex items-center justify-center">
        <div className="h-40 w-40 rounded-full border border-emerald-400/10 animate-spin-very-slow" />
      </div>

      <svg viewBox="0 0 400 300" fill="none" className="relative w-full h-auto">
        <line x1="200" y1="150" x2="80" y2="60" stroke="url(#lineGrad1)" strokeWidth="1" strokeOpacity="0.4" strokeDasharray="4 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-16" dur="3s" repeatCount="indefinite" />
        </line>
        <line x1="200" y1="150" x2="320" y2="60" stroke="url(#lineGrad1)" strokeWidth="1" strokeOpacity="0.4" strokeDasharray="4 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-16" dur="3.5s" repeatCount="indefinite" />
        </line>
        <line x1="200" y1="150" x2="50" y2="220" stroke="url(#lineGrad2)" strokeWidth="1" strokeOpacity="0.4" strokeDasharray="4 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-16" dur="4s" repeatCount="indefinite" />
        </line>
        <line x1="200" y1="150" x2="350" y2="220" stroke="url(#lineGrad2)" strokeWidth="1" strokeOpacity="0.4" strokeDasharray="4 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-16" dur="3.2s" repeatCount="indefinite" />
        </line>
        <line x1="200" y1="150" x2="200" y2="270" stroke="url(#lineGrad1)" strokeWidth="1" strokeOpacity="0.4" strokeDasharray="4 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-16" dur="2.8s" repeatCount="indefinite" />
        </line>
        <line x1="80" y1="60" x2="50" y2="220" stroke="url(#lineGrad2)" strokeWidth="0.5" strokeOpacity="0.2" strokeDasharray="2 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-12" dur="5s" repeatCount="indefinite" />
        </line>
        <line x1="320" y1="60" x2="350" y2="220" stroke="url(#lineGrad2)" strokeWidth="0.5" strokeOpacity="0.2" strokeDasharray="2 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-12" dur="4.5s" repeatCount="indefinite" />
        </line>
        <line x1="80" y1="60" x2="320" y2="60" stroke="url(#lineGrad1)" strokeWidth="0.5" strokeOpacity="0.15" strokeDasharray="2 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-12" dur="6s" repeatCount="indefinite" />
        </line>
        <line x1="50" y1="220" x2="200" y2="270" stroke="url(#lineGrad1)" strokeWidth="0.5" strokeOpacity="0.15" strokeDasharray="2 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-12" dur="5.5s" repeatCount="indefinite" />
        </line>
        <line x1="350" y1="220" x2="200" y2="270" stroke="url(#lineGrad2)" strokeWidth="0.5" strokeOpacity="0.15" strokeDasharray="2 4">
          <animate attributeName="stroke-dashoffset" from="0" to="-12" dur="4.8s" repeatCount="indefinite" />
        </line>

        <circle cx="200" cy="150" r="28" fill="url(#centerGlow)" opacity="0.6">
          <animate attributeName="r" values="28;32;28" dur="4s" repeatCount="indefinite" />
        </circle>
        <circle cx="80" cy="60" r="16" fill="url(#nodeGlow)" opacity="0.4">
          <animate attributeName="r" values="16;18;16" dur="3s" repeatCount="indefinite" />
        </circle>
        <circle cx="320" cy="60" r="16" fill="url(#nodeGlow)" opacity="0.4">
          <animate attributeName="r" values="16;18;16" dur="3.5s" repeatCount="indefinite" />
        </circle>
        <circle cx="50" cy="220" r="14" fill="url(#nodeGlow2)" opacity="0.4">
          <animate attributeName="r" values="14;16;14" dur="4s" repeatCount="indefinite" />
        </circle>
        <circle cx="350" cy="220" r="14" fill="url(#nodeGlow2)" opacity="0.4">
          <animate attributeName="r" values="14;16;14" dur="3.2s" repeatCount="indefinite" />
        </circle>
        <circle cx="200" cy="270" r="14" fill="url(#nodeGlow)" opacity="0.4">
          <animate attributeName="r" values="14;16;14" dur="3.8s" repeatCount="indefinite" />
        </circle>

        <circle cx="80" cy="60" r="6" fill="#34d399" opacity="0.8">
          <animate attributeName="opacity" values="0.8;0.5;0.8" dur="3s" repeatCount="indefinite" />
        </circle>
        <circle cx="320" cy="60" r="6" fill="#60a5fa" opacity="0.8">
          <animate attributeName="opacity" values="0.8;0.5;0.8" dur="3.5s" repeatCount="indefinite" />
        </circle>
        <circle cx="50" cy="220" r="5" fill="#34d399" opacity="0.7">
          <animate attributeName="opacity" values="0.7;0.4;0.7" dur="4s" repeatCount="indefinite" />
        </circle>
        <circle cx="350" cy="220" r="5" fill="#60a5fa" opacity="0.7">
          <animate attributeName="opacity" values="0.7;0.4;0.7" dur="3.2s" repeatCount="indefinite" />
        </circle>
        <circle cx="200" cy="270" r="5" fill="#34d399" opacity="0.7">
          <animate attributeName="opacity" values="0.7;0.4;0.7" dur="3.8s" repeatCount="indefinite" />
        </circle>

        <circle cx="200" cy="150" r="20" fill="rgba(16,185,129,0.1)" stroke="rgba(16,185,129,0.3)" strokeWidth="1.5" />
        <g transform="translate(188, 138) scale(1)">
          <path
            d="M12 2.5l7.5 3v5.2c0 4.86-3.2 9.24-7.5 10.8-4.3-1.56-7.5-5.94-7.5-10.8V5.5l7.5-3z"
            stroke="#34d399"
            strokeWidth="1.4"
            strokeLinejoin="round"
            fill="#34d399"
            fillOpacity="0.15"
          />
          <path
            d="M8.5 12.2l2.4 2.4 4.6-5.1"
            stroke="#34d399"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>

        <circle cx="140" cy="100" r="1.5" fill="#34d399" opacity="0.6">
          <animate attributeName="cy" values="100;95;100" dur="4s" repeatCount="indefinite" />
          <animate attributeName="opacity" values="0.6;0.2;0.6" dur="4s" repeatCount="indefinite" />
        </circle>
        <circle cx="260" cy="110" r="1" fill="#60a5fa" opacity="0.5">
          <animate attributeName="cy" values="110;105;110" dur="3.5s" repeatCount="indefinite" />
          <animate attributeName="opacity" values="0.5;0.2;0.5" dur="3.5s" repeatCount="indefinite" />
        </circle>
        <circle cx="160" cy="200" r="1" fill="#34d399" opacity="0.4">
          <animate attributeName="cy" values="200;195;200" dur="5s" repeatCount="indefinite" />
          <animate attributeName="opacity" values="0.4;0.1;0.4" dur="5s" repeatCount="indefinite" />
        </circle>
        <circle cx="250" cy="210" r="1.5" fill="#60a5fa" opacity="0.5">
          <animate attributeName="cy" values="210;205;210" dur="4.2s" repeatCount="indefinite" />
          <animate attributeName="opacity" values="0.5;0.2;0.5" dur="4.2s" repeatCount="indefinite" />
        </circle>

        <defs>
          <linearGradient id="lineGrad1" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#34d399" />
            <stop offset="100%" stopColor="#60a5fa" />
          </linearGradient>
          <linearGradient id="lineGrad2" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#60a5fa" />
            <stop offset="100%" stopColor="#34d399" />
          </linearGradient>
          <radialGradient id="centerGlow">
            <stop offset="0%" stopColor="#34d399" stopOpacity="0.3" />
            <stop offset="100%" stopColor="#34d399" stopOpacity="0" />
          </radialGradient>
          <radialGradient id="nodeGlow">
            <stop offset="0%" stopColor="#60a5fa" stopOpacity="0.3" />
            <stop offset="100%" stopColor="#60a5fa" stopOpacity="0" />
          </radialGradient>
          <radialGradient id="nodeGlow2">
            <stop offset="0%" stopColor="#34d399" stopOpacity="0.3" />
            <stop offset="100%" stopColor="#34d399" stopOpacity="0" />
          </radialGradient>
        </defs>
      </svg>
    </div>
  );
}

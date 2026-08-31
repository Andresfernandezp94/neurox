// AppLogo — logo SVG de la app (hexágono con nodo central y 3 puntos
// periféricos). Usado en AppHeader y LoginScreen para que la identidad
// visual sea consistente.
//
// El fill usa un gradiente lineal metálico que se anima de izquierda
// a derecha para dar el efecto de "brillo metalizado" (shimmer).
// `currentColor` sigue dictando la base del color via el accent.

export interface AppLogoProps {
  className?: string;
}

export function AppLogo({ className = "app-logo" }: AppLogoProps) {
  return (
    <svg
      className={className}
      viewBox="0 0 512 512"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <defs>
        <linearGradient
          id="neurox-shine"
          gradientUnits="userSpaceOnUse"
          x1="56"
          y1="48"
          x2="456"
          y2="464"
        >
          <stop offset="0%" stopColor="currentColor" />
          <stop offset="45%" stopColor="currentColor" />
          <stop offset="50%" stopColor="#ffffff" />
          <stop offset="55%" stopColor="currentColor" />
          <stop offset="100%" stopColor="currentColor" />
          <animate
            attributeName="x1"
            values="-400;400"
            dur="4s"
            repeatCount="indefinite"
          />
          <animate
            attributeName="x2"
            values="0;800"
            dur="4s"
            repeatCount="indefinite"
          />
        </linearGradient>
      </defs>
      <path
        d="M256 48L440 154v204L256 464 72 358V154L256 48z"
        stroke="url(#neurox-shine)"
        strokeWidth="28"
        strokeLinejoin="round"
      />
      <circle cx="256" cy="256" r="40" fill="url(#neurox-shine)" />
      <line x1="256" y1="216" x2="256" y2="128" stroke="url(#neurox-shine)" strokeWidth="18" strokeLinecap="round" />
      <line x1="290" y1="276" x2="360" y2="320" stroke="url(#neurox-shine)" strokeWidth="18" strokeLinecap="round" />
      <line x1="222" y1="276" x2="152" y2="320" stroke="url(#neurox-shine)" strokeWidth="18" strokeLinecap="round" />
      <circle cx="256" cy="116" r="16" fill="url(#neurox-shine)" />
      <circle cx="370" cy="328" r="16" fill="url(#neurox-shine)" />
      <circle cx="142" cy="328" r="16" fill="url(#neurox-shine)" />
    </svg>
  );
}

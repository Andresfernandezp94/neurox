// LoginScreen — gate para acceder al admin cuando el daemon requiere auth.
// EP-0007: user/password form (reemplaza el antiguo token-input form).
// EP-0026-UX: rediseño premium con split layout (hero + form), glassmorphism,
// inputs con iconos, gradient animado de fondo, footer informativo.
//
// Estructura (BEM):
//   .login-screen
//   ├── .login-screen__bg          — capa de gradientes animados
//   ├── .login-screen__theme-toggle
//   └── .login-screen__container
//       ├── .login-screen__hero    — branding + features (60% desktop)
//       └── .login-screen__form    — form card (40% desktop)

import { useCallback, useEffect, useRef, useState } from "react";
import { useAuth } from "../hooks/useAuth";
import { login } from "../api/auth";
import { ApiError } from "../api/client";
import { ThemeToggle } from "../shared/components/ThemeToggle";
import { AppLogo } from "../shared/components/AppLogo";
import { ErrorBanner } from "../shared/components/molecules/ErrorBanner";
import { Spinner } from "../shared/components/atoms/Spinner";

export interface LoginScreenProps {
  /** Vuelve a la landing pública. Escape para cuando el usuario no
   *  tiene credenciales a mano: el login es la puerta del panel, no un
   *  muro que tapa el producto. */
  onBack?: () => void;
  /** Login exitoso. App.tsx lo usa para navegar a /app con replace,
   *  para que el botón atrás no devuelva al formulario ya autenticado. */
  onSuccess?: () => void;
}

export function LoginScreen({ onBack, onSuccess }: LoginScreenProps): React.JSX.Element {
  const { setSession } = useAuth();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);
  const passwordRef = useRef<HTMLInputElement | null>(null);
  const usernameRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    usernameRef.current?.focus();
  }, []);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      const u = username.trim();
      const p = password.trim(); // trim() también en password
      if (!u || !p) {
        setError("Usuario y contraseña son requeridos");
        return;
      }
      setLoading(true);
      setError(null);
      try {
        const res = await login(u, p);
        setSuccess(true);
        // Pequeño delay para que la success animation alcance a verse
        // antes de que el parent (AuthProvider) desmonte LoginScreen.
        setTimeout(() => {
          setSession(res.token, res.user);
          // Navegación post-login la maneja App.tsx; se invoca después
          // de setSession para que el token ya esté guardado cuando
          // /app monte y pida datos.
          onSuccess?.();
        }, 360);
      } catch (e) {
        if (e instanceof ApiError && e.status === 401) {
          setError("Credenciales inválidas");
        } else if (e instanceof Error) {
          setError(`Error: ${e.message}`);
        } else {
          setError("Error desconocido");
        }
        setLoading(false);
      }
    },
    [username, password, setSession, onSuccess],
  );

  const handleUsernameKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      passwordRef.current?.focus();
    }
  };

  // El toggle de tema vive en <ThemeToggle>; acá ya no hace falta.

  return (
    <div className="login-screen">
      {/* El fondo (grilla + cristal) es global: ver <GridBackdrop fixed />
          en App.tsx. El login sólo pone su layout encima. */}

      {/* El toggle se extrajo a <ThemeToggle> para no duplicar la lógica de
       * resolución del modo efectivo. La clase del login se conserva como
       * hook de positioning. */}
      <ThemeToggle className="login-screen__theme-toggle" />

      <div className="login-screen__container">
        {/* HERO — features. Se oculta en mobile. */}
        <aside className="login-screen__hero" aria-hidden="true">
          <ul className="login-screen__hero-features">
            <li className="login-screen__hero-feature">
              <span className="login-screen__hero-feature-icon login-screen__hero-feature-icon--accent">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <circle cx="12" cy="12" r="3" />
                  <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
                </svg>
              </span>
              <div>
                <strong>Agentes persistentes y efímeros</strong>
                <span>In-process, MCP externos y supervisados.</span>
              </div>
            </li>
            <li className="login-screen__hero-feature">
              <span className="login-screen__hero-feature-icon login-screen__hero-feature-icon--tools">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                </svg>
              </span>
              <div>
                <strong>Sandbox con permisos</strong>
                <span>Aislamiento por paths y herramientas.</span>
              </div>
            </li>
            <li className="login-screen__hero-feature">
              <span className="login-screen__hero-feature-icon login-screen__hero-feature-icon--danger">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
                </svg>
              </span>
              <div>
                <strong>Eventos en tiempo real</strong>
                <span>WebSocket /v1/events, baja latencia.</span>
              </div>
            </li>
          </ul>
        </aside>

        {/* Volver a la landing: va FUERA de la card, arriba a la izquierda,
         * junto al theme toggle. Adentro quedaba sobre el glassmorphism
         * y se perdía. `login-screen__topbar` lo posiciona. */}
        {onBack && (
          <button
            type="button"
            className="login-screen__back"
            onClick={onBack}
            data-testid="login-back"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <line x1="19" y1="12" x2="5" y2="12" />
              <polyline points="12 19 5 12 12 5" />
            </svg>
            Volver al inicio
          </button>
        )}

        {/* FORM — card con glassmorphism y los inputs */}
        <section
          className={`login-screen__form${success ? " login-screen__form--success" : ""}`}
        >
          <div className="login-screen__hero-brand">
            <div className="login-screen__hero-brand-row">
              <div className="login-screen__hero-logo">
                <AppLogo className="login-screen__hero-logo-svg" />
              </div>
              <h1 className="login-screen__hero-title">
                <span className="login-screen__hero-title-main">neurox</span>
              </h1>
            </div>
          </div>

          <form onSubmit={handleSubmit} className="login-screen__form-body" noValidate>
            <div className="login-screen__field">
              <label htmlFor="login-username" className="login-screen__label">
                Usuario
              </label>
              <div className="login-screen__input-wrap">
                <span className="login-screen__input-icon" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2" />
                    <circle cx="12" cy="7" r="4" />
                  </svg>
                </span>
                <input
                  ref={usernameRef}
                  id="login-username"
                  type="text"
                  autoComplete="username"
                  placeholder="user"
                  value={username}
                  onChange={(e) => {
                    setUsername(e.target.value);
                    if (error) setError(null);
                  }}
                  onKeyDown={handleUsernameKeyDown}
                  className="login-screen__input"
                  disabled={loading || success}
                  data-testid="login-username"
                />
              </div>
            </div>

            <div className="login-screen__field">
              <label htmlFor="login-password" className="login-screen__label">
                Contraseña
              </label>
              <div className="login-screen__input-wrap">
                <span className="login-screen__input-icon" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                    <path d="M7 11V7a5 5 0 0 1 10 0v4" />
                  </svg>
                </span>
                <input
                  ref={passwordRef}
                  id="login-password"
                  type={showPassword ? "text" : "password"}
                  autoComplete="current-password"
                  placeholder="password"
                  value={password}
                  onChange={(e) => {
                    setPassword(e.target.value);
                    if (error) setError(null);
                  }}
                  className="login-screen__input"
                  disabled={loading || success}
                  data-testid="login-password"
                />
                <button
                  type="button"
                  className="login-screen__toggle-password"
                  onClick={() => setShowPassword((s) => !s)}
                  title={showPassword ? "Ocultar contraseña" : "Mostrar contraseña"}
                  aria-label={showPassword ? "Ocultar contraseña" : "Mostrar contraseña"}
                  tabIndex={-1}
                >
                  {showPassword ? (
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
                      <line x1="1" y1="1" x2="23" y2="23" />
                    </svg>
                  ) : (
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
                      <circle cx="12" cy="12" r="3" />
                    </svg>
                  )}
                </button>
              </div>
            </div>

            {error && (
              <div className="login-screen__error">
                <ErrorBanner variant="error">{error}</ErrorBanner>
              </div>
            )}

            <button
              type="submit"
              className="login-screen__submit"
              disabled={loading || success || !username.trim() || !password}
              data-testid="login-submit"
            >
              <span className="login-screen__submit-shine" aria-hidden="true" />
              <span className="login-screen__submit-content">
                {loading ? (
                  <>
                    <Spinner size="sm" label="Ingresando" />
                    <span>Ingresando...</span>
                  </>
                ) : success ? (
                  <>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                    <span>Bienvenido</span>
                  </>
                ) : (
                  <>
                    <span>Iniciar sesión</span>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                      <line x1="5" y1="12" x2="19" y2="12" />
                      <polyline points="12 5 19 12 12 19" />
                    </svg>
                  </>
                )}
              </span>
            </button>
          </form>
        </section>
      </div>
    </div>
  );
}

// ThemeToggle — botón que alterna entre tema claro y oscuro.
//
// Se extrajo del `LoginScreen` (que tenía el toggle inline) para que la
// landing pueda usar el mismo control sin duplicar la lógica de
// resolución del modo efectivo. El comportamiento es idéntico al que ya
// tenía LoginScreen: calcula el tema efectivo resolviendo "system"
// contra la preferencia del SO, y togglea a su opuesto.

import { useTheme, resolveTheme } from "../hooks/useTheme";

export interface ThemeToggleProps {
  className?: string;
}

export function ThemeToggle({ className = "" }: ThemeToggleProps) {
  const { mode, setMode } = useTheme();

  const toggleTheme = () => {
    const effective =
      mode === "system"
        ? window.matchMedia("(prefers-color-scheme: light)").matches
          ? "light"
          : "dark"
        : mode;
    setMode(effective === "dark" ? "light" : "dark");
  };

  const isDark = resolveTheme(mode) === "dark";

  return (
    <button
      type="button"
      className={`theme-toggle ${className}`.trim()}
      onClick={toggleTheme}
      title="Cambiar tema"
      aria-label="Cambiar tema"
      data-testid="theme-toggle"
    >
      {isDark ? (
        /* sol: el tema actual es oscuro, se ofrece pasar a claro */
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <circle cx="12" cy="12" r="5" />
          <line x1="12" y1="1" x2="12" y2="3" />
          <line x1="12" y1="21" x2="12" y2="23" />
          <line x1="4.2" y1="4.2" x2="5.6" y2="5.6" />
          <line x1="18.4" y1="18.4" x2="19.8" y2="19.8" />
          <line x1="1" y1="12" x2="3" y2="12" />
          <line x1="21" y1="12" x2="23" y2="12" />
          <line x1="4.2" y1="19.8" x2="5.6" y2="18.4" />
          <line x1="18.4" y1="5.6" x2="19.8" y2="4.2" />
        </svg>
      ) : (
        /* luna: el tema actual es claro, se ofrece pasar a oscuro */
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      )}
    </button>
  );
}
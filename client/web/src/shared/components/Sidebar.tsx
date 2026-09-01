// ============================================================
// Sidebar.tsx — port one-way desde agent-studio (EP-0002-01)
// EP-0024: NavId reducido a 6 items. Logo + título + theme-switcher
// se movieron al AppHeader (header persistente). El pin sigue en el
// sidebar para pinear/expander.
// EP-0029: NavId extendido con "workspace" (vista reservada, contenido
// pendiente). Vivía en el grupo "Intelligence" después de providers.
// EP-0030: agents/providers movidos a tabs dentro de Config. Sidebar
// quedaba con 5 items. Workspace vivía en el grupo Intelligence
// (segundo grupo, arriba de Conversation).
// EP-hide-header: workspace se reubicó como 4º elemento del sidebar
// (entre Conversation y System).
// 2026-08-15: workspace y sessions se movieron a Config como tabs
// internas (NeuralNetwork y SessionsPanel). El sidebar queda con 3
// items (status, chat, config). El sidebar es ahora un menú de
// nivel superior — los detalles viven en Config.
// ============================================================

import { type ReactNode, useEffect, useRef, useState } from "react";
import {
  IconStatus,
  IconConfig,
  IconChat,
  IconPin,
  IconPinFilled,
  IconIntegrations,
  IconClipboard,
  IconPower,
} from "./Icons";
import { AppLogo } from "./AppLogo";
import { StatusBar } from "./StatusBar";
import { UserPlaceholder } from "../../components/UserPlaceholder";
import { useI18n } from "../hooks/useI18n";
import { useConnectionState } from "../../store/StoreContext";
import { useAuth } from "../../hooks/useAuth";
import { logout } from "../../api/auth";

export type NavId =
  | "status"
  | "chat"
  | "config"
  | "sessions"
  | "workspace";

export interface SidebarProps {
  view: NavId;
  onTabChange: (id: NavId) => void;
  /** EP-0026-UX: ocultar el sidebar completamente (gana sobre `pinned`). */
  hidden?: boolean;
  /** EP-2026-08-15: forzar collapsed (rail de iconos, sin expandir) sin
   * importar `pinned` ni hover. Usado en vistas densas como Settings/Config
   * para liberar ancho horizontal sin perder acceso a la navegación. */
  forceCollapsed?: boolean;
}

interface NavItem {
  id: NavId;
  label: string;
  icon: () => ReactNode;
}

interface NavGroup {
  label: string;
  items: NavItem[];
}

export function Sidebar({ view, onTabChange, hidden = false, forceCollapsed = false }: SidebarProps) {
  const { t } = useI18n();
  const { clear } = useAuth();
  const connAvatarClass = useConnAvatarClass();
  // EP-0024: en mobile (max-width: 1024px) la sidebar es bottom-bar, no aplica
  // el concepto de "pinned" (siempre se muestra). Forzamos pinned=false en mobile.
  // Declarada como `function` (no `const arrow`) para que se hoisted y
  // sea seguro usarla desde el initializer del `useState` de abajo.
  function isMobile(): boolean {
    return (
      typeof window !== "undefined" &&
      window.matchMedia("(max-width: 1024px)").matches
    );
  }

  // EP-0024: persistencia de `pinned` en localStorage (no del hidden —
  // ese es efímero y se pasa por prop).
  const [pinned, setPinned] = useState<boolean>(
    () => !isMobile() && localStorage.getItem("sidebar-pinned") === "true",
  );

  // EP-0026-UX: sincroniza el pinned con `data-sidebar-pinned` en <body>
  // para que `tokens.css` ajuste el `margin-left` / `max-width` de
  // `.app-main` cuando el usuario pinea. Single source of truth.
  useEffect(() => {
    if (typeof document === "undefined") return;
    document.body.dataset.sidebarPinned = pinned ? "true" : "false";
  }, [pinned]);

  // Collapsible state: pinned (persistent) overrides todo lo demás.
  // Si pinned=true, la sidebar siempre está expandida — incluso en
  // vistas con forceCollapsed (config) o en mobile.
  const [_hovered, setHovered] = useState<boolean>(false);
  const collapsed = !pinned && (forceCollapsed ? true : isMobile() ? false : !_hovered);
  const asideRef = useRef<HTMLElement>(null);

  // EP-0026-UX: si entramos a mobile, forzamos pinned=false (la sidebar
  // se muestra como bottom-bar siempre, sin importar pin).
  useEffect(() => {
    if (isMobile() && pinned) setPinned(false);
  }, [pinned, setPinned]);

  // Listener de resize: si pasamos a mobile, forzamos pinned=false.
  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(max-width: 1024px)");
    const onChange = () => {
      if (mq.matches && pinned) setPinned(false);
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [pinned, setPinned]);

  useEffect(() => {
    if (pinned) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (
        asideRef.current &&
        !asideRef.current.contains(e.target as Node)
      ) {
        setHovered(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [pinned]);

  const navGroups: NavGroup[] = [
    {
      label: "Overview",
      items: [
        { id: "status", label: t("sidebar.status"), icon: IconStatus },
      ],
    },
    {
      label: "Intelligence",
      items: [
        { id: "workspace", label: t("sidebar.workspace"), icon: IconIntegrations },
      ],
    },
    {
      label: "Conversation",
      items: [
        { id: "chat", label: t("sidebar.chat"), icon: IconChat },
        { id: "sessions", label: t("panels.sessions.title"), icon: IconClipboard },
      ],
    },
    {
      label: "System",
      items: [
        { id: "config", label: t("sidebar.config"), icon: IconConfig },
      ],
    },
  ];

  return (
    <div className="sidebar-wrapper" hidden={hidden} style={hidden ? { display: "none" } : undefined}>
      <aside
        ref={asideRef}
        className={`sidebar-panel ${collapsed ? "collapsed" : ""}`}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
        onClick={() => setHovered(true)}
      >
        {/* EP-0024: topbar con el botón pin. Logo + título vienen del AppHeader. */}
        <div className="sidebar-panel__topbar">
          <div className="sidebar-panel__brand">
            <AppLogo />
            <h1 className="sidebar-panel__title">neurox</h1>
          </div>
          <button
            type="button"
            className={`sidebar-pin ${pinned ? "active" : ""}`}
            onClick={() => setPinned((p) => !p)}
            title={pinned ? "Unpin sidebar" : "Pin sidebar open"}
            aria-label={pinned ? "Unpin sidebar" : "Pin sidebar open"}
            data-testid="sidebar-pin"
          >
            {pinned ? <IconPinFilled /> : <IconPin />}
          </button>
        </div>

        <nav className="sidebar-panel__nav">
          {navGroups.map((group) => (
            <div key={group.label} className="nav-group">
              <span className="nav-group__label">{group.label}</span>
              {group.items.map(({ id, label, icon: Icon }) => (
                <button
                  key={id}
                  className={`nav-btn ${view === id ? "active" : ""}`}
                  onClick={() => onTabChange(id)}
                  data-testid={`sidebar-nav-${id}`}
                >
                  <Icon />
                  <span>{label}</span>
                </button>
              ))}
            </div>
          ))}
        </nav>

        <div className="sidebar-panel__spacer" />

        <div className="sidebar-panel__footer">
          {/* DOT-fix: status bar con dot pulsante para que el usuario
              vea de un vistazo si el daemon responde. La barra va
              ARRIBA del avatar para que sea lo primero que mire al
              scrollear al final del sidebar. */}
          <SidebarStatusBar />
          <UserPlaceholder
            avatarClassName={connAvatarClass}
            footer={
              <button
                type="button"
                className="sidebar-logout"
                title="Logout"
                aria-label="Logout"
                data-testid="sidebar-logout"
                onClick={async () => {
                  try {
                    await logout();
                  } catch {
                    // ignore
                  }
                  clear();
                }}
              >
                <IconPower />
              </button>
            }
          />
        </div>
      </aside>
    </div>
  );
}

/** EP-0024: clase CSS del estado combinado de conexión.
 *  Se aplica directo al .user-avatar (no a un wrapper) para que el
 *  halo pulsante quede solo al rededor del círculo del avatar. */
function useConnAvatarClass(): string {
  const conn = useConnectionState();
  const combined = computeCombinedForIndicator(conn);
  return `conn-avatar conn-avatar--${combined === 'offline' ? 'unreachable' : combined}`;
}

function computeCombinedForIndicator(conn: ReturnType<typeof useConnectionState>): Combined {
  if (conn.ws === 'closed') return 'offline';
  if (conn.health === null && conn.ws === 'open') return 'degraded';
  if (conn.isZombie) return 'degraded';
  if (conn.latencyMs !== null && conn.latencyMs > 500) return 'degraded';
  if (conn.lastRetryAt !== null && Date.now() - conn.lastRetryAt < 30_000) {
    return 'degraded';
  }
  return 'ok';
}

type Combined = 'offline' | 'degraded' | 'ok';

/** DOT-fix: status bar con dot pulsante renderizada en el footer
 *  del sidebar (siempre visible). El color del dot sigue el
 *  estado de la conexión WS del daemon:
 *    ok         → verde pulsante
 *    degraded   → amarillo pulsante
 *    error      → rojo fijo
 *    connecting → azul pulsante
 *  La versión y uptime se leen del último /health que el store
 *  cacheó — están en `conn.health`. */
function SidebarStatusBar() {
  const conn = useConnectionState();
  const combined = computeCombinedForIndicator(conn);
  const version = conn.health?.version;
  const uptime = conn.health?.uptime_seconds;
  return (
    <StatusBar
      version={version ?? "—"}
      status={combined === "offline" ? "error" : combined}
      uptimeSeconds={uptime}
      showDot
    />
  );
}
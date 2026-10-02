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
// 2026-09-17: el pin (IconPin) se reemplazó por un toggle de sidebar
// (IconSidebarOpen/IconSidebarClose). El expand/colapso es solo por
// click — se eliminó el hover-to-expand.
// ============================================================

import { type ReactNode, useEffect, useState } from "react";
import {
  IconConfig,
  IconChat,
  IconSidebar,
  IconBell,
  IconPower,
  IconGrid,
  IconWorkspace,
  IconRobot,
} from "./Icons";
import { UserPlaceholder } from "../../components/UserPlaceholder";
import { ThemeToggle } from "./ThemeToggle";
import { useI18n } from "../hooks/useI18n";
import { useConnectionState, useStore } from "../../store/StoreContext";
import { useAuth } from "../../hooks/useAuth";
import { logout } from "../../api/auth";

export type NavId = "status" | "chat" | "intelligence" | "config" | "workspace";

export interface SidebarProps {
  view: NavId;
  onTabChange: (id: NavId) => void;
  /** EP-0026-UX: ocultar el sidebar completamente. */
  hidden?: boolean;
  /** Tras cerrar sesión. App.tsx navega a /login con replace. */
  onLogout?: () => void;
}

interface NavItem {
  id: NavId;
  label: string;
  icon: () => ReactNode;
}

export function Sidebar({ view, onTabChange, hidden = false, onLogout }: SidebarProps) {
  const { t } = useI18n();
  const { clear } = useAuth();
  // Aprobaciones pendientes para el badge de la campana. Viene del store
  // (mantenido por el WS), así que el contador se actualiza solo.
  const { state } = useStore();
  const pendingApprovals = state.approvals.size;
  const connAvatarClass = useConnAvatarClass();
  // EP-0024: en mobile (max-width: 1024px) la sidebar es bottom-bar, no aplica
  // el concepto de expand/collapse (siempre se muestra como barra horizontal).
  // Declarada como `function` (no `const arrow`) para que se hoisted y
  // sea seguro usarla desde el initializer del `useState` de abajo.
  function isMobile(): boolean {
    return (
      typeof window !== "undefined" &&
      window.matchMedia("(max-width: 1024px)").matches
    );
  }

  // 2026-09-17: estado simple de expand/colapse, persistido. Por defecto
  // expandida para que la navegación sea visible sin depender del hover.
  const [expanded, setExpanded] = useState<boolean>(
    () => !isMobile() && localStorage.getItem("sidebar-expanded") !== "false",
  );

  // Mantenemos el atributo con el nombre histórico (`sidebar-pinned`)
  // porque `tokens.css` ajusta el `margin-left` de `.app-main` con él.
  useEffect(() => {
    if (typeof document === "undefined") return;
    document.body.dataset.sidebarPinned = expanded ? "true" : "false";
  }, [expanded]);

  // En mobile la sidebar es bottom-bar: sin expand/collapse.
  const collapsed = isMobile() ? false : !expanded;

  // EP-0026-UX: si entramos a mobile, forzamos expanded=false.
  useEffect(() => {
    if (isMobile() && expanded) setExpanded(false);
  }, [expanded, setExpanded]);

  // Listener de resize: si pasamos a mobile, forzamos expanded=false.
  useEffect(() => {
    if (typeof window === "undefined") return;
    const mq = window.matchMedia("(max-width: 1024px)");
    const onChange = () => {
      if (mq.matches && expanded) setExpanded(false);
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [expanded, setExpanded]);

  const navItems: NavItem[] = [
    // Orden deliberado: Overview primero porque es la pantalla default
    // tras el login y es lo accionable. Workspace antes de Config porque
    // Config agrupa todo lo de configuracion (providers, keys, usuarios)
    // y es un destino de tarea puntual, mientras que Workspace es trabajo
    // en curso; el orden pone primero lo que se consulta a diario.
    { id: "status", label: t("sidebar.overview"), icon: IconGrid },
    { id: "chat", label: t("sidebar.chat"), icon: IconChat },
    // Intelligence va pegado a Chat y antes de Workspace: es donde se
    // habla con los agentes, asi que es lo que se consulta mientras se
    // trabaja, igual que Chat. El icono es un robot, que es la lectura
    // literal del nombre; el set no tiene cerebro ni foco.
    { id: "intelligence", label: t("sidebar.intelligence"), icon: IconRobot },
    { id: "workspace", label: t("sidebar.workspace"), icon: IconWorkspace },
    { id: "config", label: t("sidebar.config"), icon: IconConfig },
  ];

  return (
    <div className="sidebar-wrapper" hidden={hidden} style={hidden ? { display: "none" } : undefined}>
      <aside className={`sidebar-panel ${collapsed ? "collapsed" : ""}`}>
        {/* topbar con el toggle de sidebar: expandir/contraer por click. */}
        <div className="sidebar-panel__topbar">
          <button
            type="button"
            className="sidebar-toggle"
            onClick={() => setExpanded((e) => !e)}
            title={expanded ? "Collapse sidebar" : "Expand sidebar"}
            aria-label={expanded ? "Collapse sidebar" : "Expand sidebar"}
            aria-expanded={expanded}
            data-testid="sidebar-toggle"
          >
            <IconSidebar />
          </button>
        </div>

        <nav className="sidebar-panel__nav">
          {navItems.map(({ id, label, icon: Icon }) => (
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
        </nav>

        <div className="sidebar-panel__spacer" />

        <div className="sidebar-panel__footer">
          {/* EP-2026-09-02: solo la tarjeta del usuario. El indicador de
              conexión (ok/degraded/unreachable) vive en el halo del
              avatar vía .conn-avatar. El badge con versión + estado +
              uptime se quitó: duplicaba lo que muestra StatusPanel y
              rompía la jerarquía visual del footer. */}
          <UserPlaceholder
            avatarClassName={connAvatarClass}
            footer={
              /* Acciones de sesión al pie: campana · tema · logout. La
               * campana muestra las aprobaciones pendientes, que es data
               * real del store y lo único accionable hoy. */
              <div className="sidebar-footer-actions">
                <button
                  type="button"
                  className="sidebar-footer-action sidebar-bell"
                  title={
                    pendingApprovals > 0
                      ? `${pendingApprovals} aprobación${pendingApprovals === 1 ? "" : "es"} pendiente${pendingApprovals === 1 ? "" : "s"}`
                      : "Notificaciones"
                  }
                  aria-label={
                    pendingApprovals > 0
                      ? `Notificaciones: ${pendingApprovals} aprobación${pendingApprovals === 1 ? "" : "es"} pendiente${pendingApprovals === 1 ? "" : "s"}`
                      : "Notificaciones"
                  }
                  data-testid="sidebar-bell"
                >
                  <IconBell />
                  {pendingApprovals > 0 && (
                    <span
                      className="sidebar-bell__badge"
                      aria-hidden="true"
                      data-testid="sidebar-bell-badge"
                    >
                      {pendingApprovals}
                    </span>
                  )}
                </button>
                <ThemeToggle className="sidebar-footer-action" />
                <button
                  type="button"
                  className="sidebar-logout sidebar-footer-action"
                  title="Cerrar sesión"
                  aria-label="Cerrar sesión"
                  data-testid="sidebar-logout"
                  onClick={async () => {
                    try {
                      await logout();
                    } catch {
                      // ignore
                    }
                    clear();
                    // La navegación a /login la hace App.tsx: el sidebar no
                    // conoce el router.
                    onLogout?.();
                  }}
                >
                  <IconPower />
                </button>
              </div>
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
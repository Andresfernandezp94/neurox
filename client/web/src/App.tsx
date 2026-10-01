// App.tsx — rutas separadas por URL:
//
//   /       landing pública (presentación del producto, sin login)
//   /login  pantalla de login
//   /app    panel de administración (sidebar + Status/Chat/Config/
//           Workspace), detrás del login
//
// El router es mínimo (shared/hooks/useRoute.ts sobre History API).
// Antes esto era un `useState` booleano: todo vivía en la misma URL, así
// que el login no era enlazable ni recargable por separado.

import { useEffect, useState } from "react";
import { Sidebar, type NavId } from "./shared/components/Sidebar";
import { useTheme } from "./shared/hooks/useTheme";
import { useAuth, AuthProvider } from "./hooks/useAuth";
import { useVisualViewportHeight } from "./hooks/useVisualViewportHeight";
import {
  StoreProvider,
  useStore,
  useStoreDispatch,
} from "./store/StoreContext";
import { useBootGate } from "./hooks/useBootGate";
import { useRoute } from "./shared/hooks/useRoute";
import { ROUTES } from "./shared/routes";
import { isMockMode, mockSnapshots } from "./shared/mock/mockData";
import { OverviewPage } from "./components/OverviewPage";
import { ChatPanel } from "./components/ChatPanel";
import { ConfigViewer } from "./components/ConfigViewer";
import { LoginScreen } from "./components/LoginScreen";
import { PanelFrame } from "./shared/components/PanelFrame";
import { WorkspaceViewer } from "./components/WorkspaceViewer";
import { BootLoader } from "./shared/components/BootLoader";
import { HomePage } from "./components/HomePage";
import { GridBackdrop } from "./shared/components/GridBackdrop";

const VALID_NAV_IDS: NavId[] = ["status", "chat", "config", "workspace"];

function isValidNavId(s: string): boolean {
  return (VALID_NAV_IDS as string[]).includes(s);
}

// ────────────────────────────────────────────────────────────────────
// Router — decide qué pantalla mostrar según la URL.
// ────────────────────────────────────────────────────────────────────

function Router() {
  const { route, go } = useRoute();
  const { state } = useStore();
  const { token } = useAuth();

  // Guarda contra deep-link a /app sin sesión: redirige a /login en vez
  // de dejar el panel a medias pidiendo datos que van a fallar con 401.
  const authRequired = state.connection.health?.auth_required === true;

  if (route === ROUTES.app) {
    if (authRequired && !token) {
      return <LoginScreen onBack={() => go(ROUTES.landing)} onSuccess={undefined} />;
    }
    return (
      <Admin
        onExitLanding={() => go(ROUTES.landing)}
        onLogout={() => go(ROUTES.login, { replace: true })}
      />
    );
  }

  if (route === ROUTES.login) {
    return (
      <LoginScreen
        onBack={() => go(ROUTES.landing)}
        // replace: el botón atrás no debe devolver al login ya autenticado.
        onSuccess={() => go(ROUTES.app, { replace: true })}
      />
    );
  }

  // `version` es lo único público que necesita la landing. Viene de
  // /health, que el daemon expone sin token.
  return (
    <HomePage
      version={state.connection.health?.version}
      onEnter={() => go(ROUTES.login)}
    />
  );
}

// ────────────────────────────────────────────────────────────────────
// Admin — la herramienta, detrás del login.
// ────────────────────────────────────────────────────────────────────

interface AdminProps {
  /** Vuelve a la landing pública. */
  onExitLanding?: () => void;
  /** Tras cerrar sesión: navegar a /login. */
  onLogout?: () => void;
}

function Admin({ onExitLanding, onLogout }: AdminProps) {
  const [activeTab, setActiveTab] = useState<NavId>(() => {
    if (typeof window === "undefined") return "status";
    const persisted = window.localStorage.getItem("active-tab");
    if (persisted && isValidNavId(persisted)) return persisted as NavId;
    return "status";
  });

  useEffect(() => {
    try {
      window.localStorage.setItem("active-tab", activeTab);
    } catch {
      // ignore
    }
  }, [activeTab]);

  // EP-0007: re-render on auth-expired event (daemon returned 401).
  const [, setAuthTick] = useState(0);
  useEffect(() => {
    const onExpired = () => setAuthTick((n) => n + 1);
    window.addEventListener("neurox:auth-expired", onExpired);
    return () => window.removeEventListener("neurox:auth-expired", onExpired);
  }, []);

  const { state } = useStore();
  const dispatch = useStoreDispatch();
  const health = state.connection.health;
  const healthLoaded = state.loaded.health;
  const { token } = useAuth();

  // Modo mock: hidrata el store con datos de `shared/mock/mockData.ts` en
  // lugar de esperar al daemon. Se despachan las MISMAS acciones que
  // dispara la respuesta real, así los reducers y las vistas ejercitan
  // el mismo camino de código. Para volver al modo real: apagá
  // `VITE_NEUROX_MOCK` en client/web/.env.
  const mock = isMockMode();
  useEffect(() => {
    if (!mock) return;
    const { health, agents, sessions, approvals, latencyMs } = mockSnapshots();
    dispatch({ type: "SNAPSHOT_HEALTH", health });
    dispatch({ type: "SNAPSHOT_AGENTS", agents });
    dispatch({ type: "SNAPSHOT_SESSIONS", sessions });
    dispatch({ type: "SNAPSHOT_APPROVALS", approvals });
    dispatch({ type: "WS_STATUS_CHANGED", status: "open" });
    // La latencia la setea el heartbeat del WS en producción; en mock se
    // inyecta para que las stats no salgan en "—".
    dispatch({
      type: "HEARTBEAT_TICK",
      latencyMs,
      lastPongAt: Date.now(),
      isZombie: false,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mock, dispatch]);

  useTheme();

  // EP-2026-08-15: sincroniza `--app-h` con `window.visualViewport.height`
  useVisualViewportHeight();

  const [sidebarHidden] = useState(false);

  // Gate de arranque: el loader se retira solo. `useBootGate` impone un
  // mínimo (para que no parpadee) y un máximo (para que un daemon caído
  // no deje la app inaccesible). Antes esto era `if (!healthLoaded)`, que
  // con el daemon caído quedaba pegado para siempre porque ese flag solo
  // pasa a `true` si el GET /health resuelve.
  const { booting } = useBootGate({ ready: healthLoaded });

  if (booting) {
    return <BootLoader />;
  }

  const authRequired = health?.auth_required === true;
  // Guarda por si el token expira mientras se está en /app: el store
  // dispara el evento `neurox:auth-expired` y acá se re-renderiza.
  if (authRequired && !token) {
    return <LoginScreen onBack={onExitLanding} />;
  }

  return (
    <div className="app">
      <Sidebar
        view={activeTab}
        onTabChange={setActiveTab}
        hidden={sidebarHidden}
        onLogout={onLogout}
      />
      <main className="app-main">
        {activeTab === "status" && (
          <PanelFrame testId="status-panel">
            <div className="page-pad">
              <OverviewPage />
            </div>
          </PanelFrame>
        )}
        {activeTab === "chat" && (
          <PanelFrame testId="chat-panel">
            <ChatPanel />
          </PanelFrame>
        )}
        {activeTab === "config" && (
          <PanelFrame testId="config-panel">
            <ConfigViewer />
          </PanelFrame>
        )}
        {activeTab === "workspace" && (
          <PanelFrame testId="workspace-panel">
            <WorkspaceViewer />
          </PanelFrame>
        )}
      </main>
    </div>
  );
}

export default function App() {
  return (
    <StoreProvider>
      <AuthProvider>
        {/* Fondo general de la app: UNA sola grilla, fija al viewport,
            compartida por landing, login y panel.
            SIN blur: difuminar un patrón de líneas lo destruye — blur(r)
            deja la línea con w/2r de su intensidad, así que con 9px sobre
            líneas de 2px quedaba al 11% y no se veía en ninguna vista.
            El cristal (transparencia + blur) va en las CARDS, no acá: así
            la grilla queda nítida y cada card difumina sólo lo que tiene
            detrás. Por eso las superficies (.home, .login-screen, .app)
            son transparentes. El alpha sube a 50% porque es la única capa
            que aporta textura; con 35% quedaba muy tenue. */}
        <GridBackdrop
          fixed
          lineColor="color-mix(in oklab, var(--accent) 50%, transparent)"
          fadeEnd={80}
        />
        <div className="glass-wash" aria-hidden="true" />
        <Router />
      </AuthProvider>
    </StoreProvider>
  );
}
// App.tsx — shell de la app. Compone:
//   1. Sidebar — navegación, brand arriba, user abajo
//   2. Panel activo según activeTab (Status, Chat, Config, etc.)
//
// Providers (Store, Auth) wrappean todo el árbol.
// LoginScreen se renderiza cuando el daemon requiere auth y no hay token.

import { useEffect, useState } from "react";
import { Sidebar, type NavId } from "./shared/components/Sidebar";
import { useTheme } from "./shared/hooks/useTheme";
import { useAuth, AuthProvider } from "./hooks/useAuth";
import { useVisualViewportHeight } from "./hooks/useVisualViewportHeight";
import { StoreProvider, useStore } from "./store/StoreContext";
import { StatusPanel } from "./components/StatusPanel";
import { ChatPanel } from "./components/ChatPanel";
import { ConfigViewer } from "./components/ConfigViewer";
import { LoginScreen } from "./components/LoginScreen";
import { PanelFrame } from "./shared/components/PanelFrame";
import { SessionsPanel } from "./components/SessionsPanel";
import { WorkspaceViewer } from "./components/WorkspaceViewer";

const VALID_NAV_IDS: NavId[] = [
  "status",
  "chat",
  "config",
  "sessions",
  "workspace",
];

function isValidNavId(s: string): boolean {
  return (VALID_NAV_IDS as string[]).includes(s);
}

function AppInner() {
  const [activeTab, setActiveTab] = useState<NavId>(() => {
    if (typeof window === "undefined") return "chat";
    const persisted = window.localStorage.getItem("active-tab");
    if (persisted && isValidNavId(persisted)) return persisted as NavId;
    return "chat";
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
  const health = state.connection.health;
  const healthLoaded = state.loaded.health;
  const { token } = useAuth();

  useTheme();

  // EP-2026-08-15: sincroniza `--app-h` con `window.visualViewport.height`
  useVisualViewportHeight();

  const [sidebarHidden, setSidebarHidden] = useState(false);

  // Esperar a que /health termine antes de decidir. Sin esto, en el
  // estado inicial `health === null` → `authRequired` es false → se
  // renderiza la app completa durante un frame, luego salta al login
  // cuando llega la respuesta (FOUC de UI autenticada).
  if (!healthLoaded) {
    return null;
  }

  const authRequired = health?.auth_required === true;
  if (authRequired && !token) {
    return <LoginScreen />;
  }

  return (
    <div className="app">
      <Sidebar
        view={activeTab}
        onTabChange={setActiveTab}
        hidden={sidebarHidden}
        forceCollapsed={activeTab === "config"}
      />
      <main className="app-main">
        {activeTab === "status" && (
          <PanelFrame testId="status-panel">
            <StatusPanel />
          </PanelFrame>
        )}
        {activeTab === "chat" && (
          <PanelFrame testId="chat-panel">
            <ChatPanel
              onToggleSidebar={() => setSidebarHidden((h) => !h)}
              isSidebarHidden={sidebarHidden}
            />
          </PanelFrame>
        )}
        {activeTab === "config" && (
          <PanelFrame testId="config-panel">
            <ConfigViewer />
          </PanelFrame>
        )}
        {activeTab === "sessions" && (
          <PanelFrame testId="sessions-panel">
            <SessionsPanel />
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
        <AppInner />
      </AuthProvider>
    </StoreProvider>
  );
}

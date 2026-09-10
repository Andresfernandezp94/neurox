// ConfigViewer — shell de tabs internas del panel Configuración.
//
// EP-0026-UX: tabs restantes — Providers, Local, Environments, Users.
// Sandbox se movió a Workspace (top-level con tabs internas).
// Logs/Capabilities/Tools/Services se movieron a Status / AgentsPanel.
// MCP/Agents/Sessions/Workspace se promovieron al sidebar.
// cleanup-2026-08: el tab "Storage" (StorageTab) se eliminó junto con
// sus endpoints /v1/db/* y /v1/memory/*.
// Local = ModelsTab: modelos GGUF locales servidos por Ollama/llama server
// + búsqueda y descarga desde Hugging Face (EP-0020-02 / EP-0025).

import { useState } from "react";
import { Stack } from "../shared/components/molecules/Stack";
import { EnvTab } from "./EnvTab";
import { UsersPanel } from "./UsersPanel";
import { ProvidersPanel } from "./ProvidersPanel";
import { ModelsTab } from "./ModelsTab";
import { useAuth } from "../hooks/useAuth";
import { useI18n } from "../shared/hooks/useI18n";

type ConfigTab =
  | "providers"
  | "local"
  | "env"
  | "users";

export function ConfigViewer() {
  // EP-0026-UX: default tab ahora es "providers" (el primero de la
  // lista; "sandbox" se fue a Workspace, "logs" / "capabilities" /
  // "tools" / "services" viven en Status / AgentsPanel).
  const [tab, setTab] = useState<ConfigTab>("providers");
  const { user } = useAuth();
  const isAdmin = user?.role === "Admin";
  const { t } = useI18n();

  return (
    <>
      <div className="config-viewer__tabs" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={tab === "providers"}
          data-testid="config-tab-providers"
          className={`config-viewer__tab ${tab === "providers" ? "active" : ""}`}
          onClick={() => setTab("providers")}
        >
          Providers
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "local"}
          data-testid="config-tab-local"
          className={`config-viewer__tab ${tab === "local" ? "active" : ""}`}
          onClick={() => setTab("local")}
        >
          {t("config.tab.local")}
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === "env"}
          data-testid="config-tab-env"
          className={`config-viewer__tab ${tab === "env" ? "active" : ""}`}
          onClick={() => setTab("env")}
        >
          {t("config.tab.environments")}
        </button>
        {isAdmin && (
          <button
            type="button"
            role="tab"
            aria-selected={tab === "users"}
            data-testid="config-tab-users"
            className={`config-viewer__tab ${tab === "users" ? "active" : ""}`}
            onClick={() => setTab("users")}
          >
            {t("config.tab.users")}
          </button>
        )}
      </div>

      <div className="page-pad">
        <Stack gap="md">
          {tab === "env" && <EnvTab />}
          {tab === "users" && isAdmin && <UsersPanel />}
          {tab === "providers" && <ProvidersPanel />}
          {tab === "local" && <ModelsTab />}
        </Stack>
      </div>
    </>
  );
}

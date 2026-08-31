// ProviderSelector — EP-0009-06.
// Dropdown that lists LLM providers with active/configured badges.
// Migrated to atomic design in EP-0016 (F4.9).
// EP-0018-04: surfaces a minimal local-service badge next to the active
// provider when it has a `service_state`.

import { useCallback, useEffect, useState } from "react";
import {
  getProviders,
  setActiveProvider,
  type LlmProviderStatus,
  type ServiceState,
} from "../api/llm";
import { Row } from "../shared/components/molecules/Row";
import { Badge } from "../shared/components/atoms/Badge";

export function ProviderSelector() {
  const [providers, setProviders] = useState<LlmProviderStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await getProviders();
      setProviders(data.providers);
      setError(null);
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activeProvider = providers.find((p) => p.active);

  const handleChange = useCallback(
    async (e: React.ChangeEvent<HTMLSelectElement>) => {
      const id = e.target.value;
      if (!id || id === activeProvider?.id) return;
      setLoading(true);
      setError(null);
      try {
        await setActiveProvider(id);
        await refresh();
      } catch (err) {
        setError((err as Error).message);
      } finally {
        setLoading(false);
      }
    },
    [activeProvider, refresh],
  );

  if (providers.length === 0 && !error) return null;

  return (
    <Row gap="sm" className="provider-selector">
      <select
        value={activeProvider?.id ?? ""}
        onChange={(e) => void handleChange(e)}
        disabled={loading}
        aria-label="LLM Provider"
        className="provider-selector__select"
      >
        {providers.map((p) => (
          <option
            key={p.id}
            value={p.id}
            disabled={!p.configured}
          >
            {p.model} ({p.id}){p.active ? " ●" : ""}{!p.configured ? " — sin key" : ""}
          </option>
        ))}
      </select>
      {/* EP-0018-04: minimal local-service badge for the active provider.
          Native <option> elements can't render React children, so the
          badge lives outside the dropdown and reflects the active
          provider's runtime state. Does not block selection. */}
      {activeProvider?.service_state && (
        <SelectorServiceBadge state={activeProvider.service_state} />
      )}
      {loading && <span className="muted text-xs">switching…</span>}
      {error && <span className="provider-selector__error">{error}</span>}
    </Row>
  );
}

function SelectorServiceBadge({ state }: { state: ServiceState }) {
  const { label, variant } = SELECTOR_VARIANT[state];
  return (
    <Badge variant={variant} data-testid={`selector-svc-state-${state}`}>
      {label}
    </Badge>
  );
}

const SELECTOR_VARIANT: Record<
  ServiceState,
  { label: string; variant: "success" | "warn" | "danger" | "neutral" }
> = {
  ready: { label: "Ready", variant: "success" },
  starting: { label: "Starting…", variant: "warn" },
  running: { label: "Running", variant: "success" },
  stopped: { label: "Stopped", variant: "neutral" },
  failed: { label: "Failed", variant: "danger" },
};
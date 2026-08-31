// Port one-way desde agent-studio/src-ui/modules/shared/hooks/useI18n.ts
// Cambios: import relativo (en vez de @/i18n/es.json de agent-studio).

import es from "../i18n/es.json";

const dict = es as Record<string, string>;

export function useI18n() {
  const t = (key: string) => dict[key] || key;
  return { t };
}

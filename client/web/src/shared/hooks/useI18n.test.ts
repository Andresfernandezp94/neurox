// Tests para useI18n hook.
// Port one-way desde agent-studio/src-ui/modules/shared/hooks/useI18n.ts

import { describe, it, expect } from "vitest";
import { renderHook } from "@testing-library/react";
import { useI18n } from "./useI18n";

describe("useI18n", () => {
  it("returns translation for existing key", () => {
    const { result } = renderHook(() => useI18n());
    expect(result.current.t("app.title")).toBe("andres.fernandez");
    expect(result.current.t("sidebar.status")).toBe("Status");
  });

  it("returns the key itself for non-existing key (fallback)", () => {
    const { result } = renderHook(() => useI18n());
    expect(result.current.t("nonexistent.key")).toBe("nonexistent.key");
  });

  it("returns all required keys from the admin's es.json", () => {
    const { result } = renderHook(() => useI18n());
    const requiredKeys = [
      "app.title",
      "sidebar.status",
      "sidebar.agents",
      "sidebar.sessions",
      "sidebar.approvals",
      "sidebar.events",
      "sidebar.tools",
      "sidebar.config",
      "actions.approve",
      "actions.reject",
      "actions.start",
      "actions.stop",
      "actions.cancel",
      "error.boundary.title",
    ];

    for (const key of requiredKeys) {
      expect(result.current.t(key)).not.toBe(key);
    }
  });
});

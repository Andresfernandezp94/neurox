// Tests para Sidebar.
// Port one-way desde agent-studio. Adaptado a los items reales del admin.
// EP-0003-03: envuelve en StoreProvider porque el Sidebar ahora incluye
// ConnectionIndicator (que usa useStore).
// EP-0026-UX: el sidebar tiene 7 nav items: status, chat, config, mcp,
// agents, sessions, workspace. MCP/Agents/Sessions/Workspace se
// promovieron desde Config al top-level nav.
// Theme-switcher removido (vive en AppHeader).

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Sidebar, type NavId } from "./Sidebar";
import { StoreProvider } from "../../store/StoreContext";
import { AuthProvider } from "../../hooks/useAuth";

function renderWithProviders(ui: React.ReactElement) {
  return render(
    <AuthProvider>
      <StoreProvider eventsPath="/__test_no_ws__{Math.random()}">{ui}</StoreProvider>
    </AuthProvider>
  );
}

describe("Sidebar", () => {
  const onTabChange = vi.fn();

  beforeEach(() => {
    onTabChange.mockClear();
  });

  it("renders all 5 navigation items", () => {
    renderWithProviders(<Sidebar view="status" onTabChange={onTabChange} />);

    const ids: NavId[] = [
      "status",
      "chat",
      "sessions",
      "workspace",
      "config",
    ];
    for (const id of ids) {
      expect(screen.getByTestId(`sidebar-nav-${id}`)).toBeDefined();
    }
  });

  it("renders nav buttons in the expected top-to-bottom order", () => {
    const { container } = renderWithProviders(
      <Sidebar view="status" onTabChange={onTabChange} />,
    );

    const order = Array.from(
      container.querySelectorAll<HTMLElement>('[data-testid^="sidebar-nav-"]'),
    ).map((el) => el.dataset.testid);

    expect(order).toEqual([
      "sidebar-nav-status",
      "sidebar-nav-workspace",
      "sidebar-nav-chat",
      "sidebar-nav-sessions",
      "sidebar-nav-config",
    ]);
  });

  it("calls onTabChange with the correct id when an item is clicked", () => {
    renderWithProviders(<Sidebar view="status" onTabChange={onTabChange} />);

    fireEvent.click(screen.getByTestId("sidebar-nav-chat"));
    expect(onTabChange).toHaveBeenCalledWith("chat");

    fireEvent.click(screen.getByTestId("sidebar-nav-workspace"));
    expect(onTabChange).toHaveBeenCalledWith("workspace");
  });

  it("marks the active item with the 'active' class", () => {
    renderWithProviders(<Sidebar view="config" onTabChange={onTabChange} />);

    const configBtn = screen.getByTestId("sidebar-nav-config");
    const chatBtn = screen.getByTestId("sidebar-nav-chat");

    expect(configBtn.className).toContain("active");
    expect(chatBtn.className).not.toContain("active");
  });

  it("accepts all valid NavId values", () => {
    const validIds: NavId[] = [
      "status",
      "chat",
      "config",
      "sessions",
      "workspace",
    ];

    for (const id of validIds) {
      const { unmount } = renderWithProviders(
        <Sidebar view={id} onTabChange={onTabChange} />,
      );
      const btn = screen.getByTestId(`sidebar-nav-${id}`);
      expect(btn.className).toContain("active");
      unmount();
    }
  });

});
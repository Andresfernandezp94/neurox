// Tests para ThinkingNode y ToolNode (EP-0026 + EP-2026-08-15).
//
// ThinkingNode:
// - Thinking arranca colapsado durante streaming
// - Thinking cambia label a "Thought" cuando el stream termina
// - Thinking se colapsa automáticamente al terminar (transición)
// - Thinking permite expandir/colapsar tras terminar y persiste
//
// ToolNode (EP-2026-08-15 minimal):
// - Solo renderiza el header con el nombre de la tool y un spinner
//   mientras está pending.
// - Los args y el result siguen llegando al componente vía `activity`
//   pero NO se renderizan (se conservan en el state pero no aparecen
//   en el DOM). Esto es intencional — ver `ToolNode.tsx` para el
//   render viejo comentado como referencia.

import { describe, expect, it, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mockeamos el worker de typewriter y los subcomponentes que no nos importan.
vi.mock("./typewriter.worker.ts", () => ({
  default: class MockWorker {
    onmessage: ((e: MessageEvent) => void) | null = null;
    postMessage() { /* no-op */ }
    terminate() { /* no-op */ }
  },
}));

vi.mock("../shared/components/Markdown", () => ({
  Markdown: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

import { ThinkingNode } from "./chat/ThinkingNode";
import { ToolNode } from "./chat/ToolNode";
import type { ToolActivity } from "../types";

describe("ThinkingNode (EP-0026)", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  // EP-0026-rev-fix: starts collapsed (open defaults to false).
  // Body is hidden by default during stream AND after. User clicks
  // to expand.
  it("starts collapsed during streaming and shows 'Thinking…' label", () => {
    render(<ThinkingNode content="reasoning text" isStreaming={true} />);
    expect(screen.queryByTestId("thinking-content")).toBeNull();
    expect(screen.getByText(/Thinking…/)).toBeTruthy();
    const toggle = screen.getByTestId("thinking-toggle");
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
  });

  it("keeps the toggle button enabled during streaming", () => {
    render(<ThinkingNode content="abc" isStreaming={true} />);
    const toggle = screen.getByTestId("thinking-toggle");
    expect((toggle as HTMLButtonElement).disabled).toBe(false);
  });

  it("user can expand the body during streaming by clicking the toggle", () => {
    render(<ThinkingNode content="abc" isStreaming={true} />);
    // Body hidden by default (open=false).
    expect(screen.queryByTestId("thinking-content")).toBeNull();
    fireEvent.click(screen.getByTestId("thinking-toggle"));
    // Click toggles open from false to true → body expands.
    const body = screen.getByTestId("thinking-content");
    expect(body).toBeTruthy();
    expect(body.className).toContain("chat__thinking-body");
    expect(body.textContent).toContain("abc");
  });

  it("changes label to 'Thought · preview' when stream ends and stays collapsed", () => {
    const { rerender } = render(<ThinkingNode content="The user is asking about how tides work." isStreaming={true} />);
    rerender(<ThinkingNode content="The user is asking about how tides work." isStreaming={false} />);
    // Label flipped to Thought + preview.
    expect(screen.getByText(/Thought/)).toBeTruthy();
    expect(screen.getByTestId("thinking-preview").textContent).toContain("tides work");
    // Body stays collapsed (no auto-collapse, default open=false).
    expect(screen.queryByTestId("thinking-content")).toBeNull();
    expect(screen.getByTestId("thinking-toggle").getAttribute("aria-expanded")).toBe("false");
  });

  it("user can expand after stream ends (body shows, glyph flips)", () => {
    render(<ThinkingNode content="abc" isStreaming={false} />);
    expect(screen.queryByTestId("thinking-content")).toBeNull();
    fireEvent.click(screen.getByTestId("thinking-toggle"));
    expect(screen.getByTestId("thinking-content")).toBeTruthy();
    expect(screen.getByTestId("thinking-toggle").getAttribute("aria-expanded")).toBe("true");
    // Re-click collapses.
    fireEvent.click(screen.getByTestId("thinking-toggle"));
    expect(screen.queryByTestId("thinking-content")).toBeNull();
    expect(screen.getByTestId("thinking-toggle").getAttribute("aria-expanded")).toBe("false");
  });

  it("user's expanded state persists across re-renders", () => {
    const { rerender } = render(<ThinkingNode content="abc" isStreaming={false} />);
    fireEvent.click(screen.getByTestId("thinking-toggle")); // expand
    expect(screen.getByTestId("thinking-content")).toBeTruthy();
    rerender(<ThinkingNode content="abc more text" isStreaming={false} />);
    expect(screen.getByTestId("thinking-content")).toBeTruthy();
    expect(screen.getByTestId("thinking-content").textContent).toContain("more text");
  });

  it("shows a duration label once content is present", () => {
    const { rerender } = render(<ThinkingNode content="abc" isStreaming={true} />);
    rerender(<ThinkingNode content="abc" isStreaming={false} />);
    const duration = screen.getByTestId("thinking-duration");
    expect(duration.textContent).toMatch(/ms|s$/);
  });

  // EP-0026-rev-fix: after stream ends (or on reload from history with
  // `m.thinking` persisted by the daemon) the toggle groups a preview
  // of the first line of the reasoning in the label, same pattern as
  // args toggle (`→ {argsPreview}`) and result toggle
  // (`resultado…  {preview}`). Lets the user decide whether to expand
  // based on what the thinking is about.
  it("groups a preview of the first line of the thinking in the label when not streaming", () => {
    render(<ThinkingNode content="The user is asking about combustion engines. Let me think…" isStreaming={false} />);
    const preview = screen.getByTestId("thinking-preview");
    expect(preview.textContent).toContain("The user is asking about combustion engines.");
  });

  it("truncates the preview at 80 chars with an ellipsis when the first line is long", () => {
    const longLine = "x".repeat(200);
    render(<ThinkingNode content={longLine} isStreaming={false} />);
    const preview = screen.getByTestId("thinking-preview");
    expect(preview.textContent).toMatch(/^ · x{80}…$/);
  });

  it("does NOT show the preview while streaming (label is just 'Thinking…')", () => {
    render(<ThinkingNode content="Some reasoning here" isStreaming={true} />);
    expect(screen.queryByTestId("thinking-preview")).toBeNull();
    expect(screen.getByText(/^Thinking…$/)).toBeTruthy();
  });
});

describe("ToolNode — full render (args/result toggles, EP-2026-08-19)", () => {
  function makeActivity(overrides: Partial<ToolActivity> = {}): ToolActivity {
    return {
      tool: "shell",
      args: { cmd: "ls -la" },
      iteration: 0,
      ...overrides,
    };
  }

  // ─── Header (tool name + spinner) ───────────────────────────────

  it("renders the tool name in the wrapper toggle", () => {
    const { container } = render(<ToolNode activity={makeActivity({ tool: "shell" })} />);
    expect(container.querySelector(".chat__timeline-label")?.textContent).toBe("shell");
  });

  it("shows the 3-dots loader while streaming and no result yet", () => {
    const { container } = render(<ToolNode activity={makeActivity()} isStreaming />);
    expect(screen.getByTestId("tool-loading-dots")).toBeTruthy();
    expect(
      container.querySelector(".chat__timeline-tool-loading"),
    ).toBeTruthy();
  });

  it("does not show the 3-dots loader once the stream ends without result", () => {
    const { container } = render(<ToolNode activity={makeActivity()} />);
    expect(screen.queryByTestId("tool-loading-dots")).toBeNull();
    expect(container.querySelector(".chat__timeline-tool-loading")).toBeNull();
  });

  it("does not show the 3-dots loader when the tool has a result", () => {
    const { container } = render(
      <ToolNode activity={makeActivity({ result: "ok" })} isStreaming />,
    );
    expect(container.querySelector(".chat__timeline-tool-loading")).toBeNull();
  });

  it("starts collapsed — args/result toggles hidden until wrapper expanded", () => {
    render(<ToolNode activity={makeActivity({ result: "ok" })} />);
    expect(screen.queryByTestId("tool-args-toggle")).toBeNull();
    expect(screen.queryByTestId("tool-result-toggle")).toBeNull();
  });

  it("expands args + result toggles on wrapper click", () => {
    render(<ToolNode activity={makeActivity({ result: "ok" })} />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    expect(screen.getByTestId("tool-args-toggle")).toBeTruthy();
    expect(screen.getByTestId("tool-result-toggle")).toBeTruthy();
  });

  // ─── Args / result toggles (visible only after wrapper expanded) ─

  it("does not render the args toggle until wrapper is expanded", () => {
    render(<ToolNode activity={makeActivity()} />);
    expect(screen.queryByTestId("tool-args-toggle")).toBeNull();
  });

  it("does not render the args toggle when args is missing/empty", () => {
    render(<ToolNode activity={makeActivity({ args: {} })} />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    expect(screen.queryByTestId("tool-args-toggle")).toBeNull();
  });

  it("renders the args toggle button after wrapper expanded", () => {
    render(<ToolNode activity={makeActivity()} />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    expect(screen.getByTestId("tool-args-toggle")).toBeTruthy();
  });

  it("expands the args body on click", () => {
    render(<ToolNode activity={makeActivity()} />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    fireEvent.click(screen.getByTestId("tool-args-toggle"));
    expect(screen.getByTestId("tool-args-body")).toBeTruthy();
  });

  it("renders the result toggle button after wrapper expanded", () => {
    render(<ToolNode activity={makeActivity({ result: "ok" })} />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    expect(screen.getByTestId("tool-result-toggle")).toBeTruthy();
  });

  it("shows the 'running…' placeholder while pending (inside wrapper)", () => {
    const { container } = render(<ToolNode activity={makeActivity()} isStreaming />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    expect(container.textContent).toContain("running");
  });

  it("expands the SmartResult body on click", () => {
    render(<ToolNode activity={makeActivity({ result: "ok" })} />);
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    fireEvent.click(screen.getByTestId("tool-result-toggle"));
    // SmartResult falls back to the "rich" variant for plain text.
    expect(screen.getByTestId("smart-result-rich")).toBeTruthy();
  });
});

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

  // EP-2026-08-19: media (image/audio/video) rendering.
  //
  // Media used to render as <img src="/v1/files/..."> directly, which
  // (a) broke auth (browser can't send Bearer on <img> requests) and
  // (b) broke in dev when VITE_API_BASE was empty (double-slash path).
  // The renderer now goes through useMediaBlob: a fetch() with the
  // Bearer header, then a blob: object URL assigned to the <img>/<audio>
  // /<video> src. The tests verify two things:
  //   1. fetch is called with a URL that targets the daemon's file route
  //      (proves the path encoding is correct).
  //   2. Once the blob resolves, the media element appears with a blob:
  //      URL (proves the auth-aware flow works end-to-end).
  //
  // jsdom 25 doesn't implement URL.createObjectURL / revokeObjectURL,
  // so we shim them here. Each call returns a fresh opaque URL so the
  // <img>/<audio>/<video> elements pick up something src-ish.
  let blobSeq = 0;
  beforeEach(() => {
    blobSeq = 0;
    if (!globalThis.URL.createObjectURL) {
      // @ts-expect-error -- jsdom shim, only used in tests.
      globalThis.URL.createObjectURL = () => {
        blobSeq += 1;
        return `blob:test/${blobSeq}`;
      };
    }
    if (!globalThis.URL.revokeObjectURL) {
      // @ts-expect-error -- jsdom shim, only used in tests.
      globalThis.URL.revokeObjectURL = () => {};
    }
  });

  function mockMediaFetch() {
    const fakeBlob = new Blob([new Uint8Array(8)], { type: "image/png" });
    return vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(
        new Response(fakeBlob, {
          status: 200,
          headers: {
            "Content-Type": "image/png",
            "Content-Disposition": 'inline; filename="out.bin"',
          },
        }),
      );
  }

  it("fetches the daemon file URL and renders an <img> when the result is an image path", async () => {
    const fetchSpy = mockMediaFetch();
    render(
      <ToolNode
        activity={makeActivity({
          tool: "generate_image",
          result:
            "✓ Image generated and saved to /tmp/out.jpg (1234 bytes)\nPrompt: 'a cat'",
        })}
      />,
    );
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    fireEvent.click(screen.getByTestId("tool-result-toggle"));
    const img = (await screen.findByTestId(
      "smart-result-image",
    )) as HTMLImageElement;
    expect(img).toBeTruthy();
    expect(img.tagName).toBe("IMG");
    expect(img.getAttribute("src")).toMatch(/^blob:/);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    const [calledUrl] = fetchSpy.mock.calls[0] as [string];
    expect(calledUrl).toContain("/v1/files/");
    expect(calledUrl).toContain("out.jpg");
  });

  it("fetches the daemon file URL and renders an <audio> when the result is an audio path", async () => {
    const fetchSpy = mockMediaFetch();
    render(
      <ToolNode
        activity={makeActivity({
          tool: "generate_music",
          result:
            "✓ Music generated and saved to /tmp/out.mp3 (5678 bytes)\nPrompt: 'jazz'",
        })}
      />,
    );
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    fireEvent.click(screen.getByTestId("tool-result-toggle"));
    const audio = (await screen.findByTestId(
      "smart-result-audio",
    )) as HTMLAudioElement;
    expect(audio).toBeTruthy();
    expect(audio.querySelector("source")?.getAttribute("src")).toMatch(
      /^blob:/,
    );
    const [calledUrl] = fetchSpy.mock.calls[0] as [string];
    expect(calledUrl).toContain("/v1/files/");
    expect(calledUrl).toContain("out.mp3");
  });

  it("fetches the daemon file URL and renders a <video> when the result is a video path", async () => {
    const fetchSpy = mockMediaFetch();
    render(
      <ToolNode
        activity={makeActivity({
          tool: "generate_video",
          result:
            "✓ Video generated and saved to /tmp/out.mp4 (91011 bytes)\nPrompt: 'a wave'",
        })}
      />,
    );
    fireEvent.click(screen.getByTestId("tool-wrapper-toggle"));
    fireEvent.click(screen.getByTestId("tool-result-toggle"));
    const video = (await screen.findByTestId(
      "smart-result-video",
    )) as HTMLVideoElement;
    expect(video).toBeTruthy();
    expect(video.querySelector("source")?.getAttribute("src")).toMatch(
      /^blob:/,
    );
    const [calledUrl] = fetchSpy.mock.calls[0] as [string];
    expect(calledUrl).toContain("/v1/files/");
    expect(calledUrl).toContain("out.mp4");
  });
});

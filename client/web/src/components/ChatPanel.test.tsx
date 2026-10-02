// Tests de la sesion perezosa del ChatPanel.
//
// El ChatPanel NO crea la sesion al montar ni al abrir la pestana: la crea
// cuando el usuario manda el primer mensaje, con el agent_id resuelto desde
// `/v1/agents`, y manda ese primer mensaje a la sesion recien creada.
//
// Reemplazan a los de EP-0026-01 R3.2, que fijaban el contrato contrario
// (auto-create al activar la pestana, textarea deshabilitado con
// placeholder "Connecting…" hasta que la sesion existiera). Ese contrato es
// lo que producia las ventanas de chat vacias acumuladas.

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

// Mock the API before importing the component.
vi.mock("../api/sessions", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api/sessions")>();
  return {
    ...actual,
    createSession: vi.fn(),
    listSessions: vi.fn().mockResolvedValue({ sessions: [] }),
    getSessionMessages: vi.fn().mockResolvedValue({ messages: [] }),
    streamMessage: vi.fn().mockResolvedValue(undefined),
    cancelSession: vi.fn(),
    renameSession: vi.fn(),
  };
});

// Mock the model/agent/tab subcomponents — we don't care about
// their internals in this test, only that ChatPanel triggers
// createSession and the <textarea> reacts to the response.
vi.mock("./ModelSelector", () => ({
  ModelSelector: () => null,
}));
vi.mock("./AgentSelector", () => ({
  AgentSelector: () => null,
}));
vi.mock("./ChatTabs", () => ({
  ChatTabs: () => null,
}));
vi.mock("./ChatHistory", () => ({
  ChatHistory: () => null,
}));

// Mock the default status fetcher.
vi.mock("../api/default", () => ({
  getDefaultAgent: vi.fn().mockResolvedValue({}),
}));

import { ChatPanel } from "./ChatPanel";
import { createSession, streamMessage } from "../api/sessions";
import { StoreProvider } from "../store/StoreContext";

const mockCreateSession = createSession as ReturnType<typeof vi.fn>;
const mockStreamMessage = streamMessage as ReturnType<typeof vi.fn>;
const TEST_AGENT_ID = "default";

/**
 * Wraps with StoreProvider that has the test agent in its
 * in_process list (mirrors what `/v1/agents` returns from the
 * running daemon). Patches fetch so StoreProvider doesn't hit
 * the real daemon.
 */
function withStore({ children }: { children: ReactNode }) {
  const originalFetch = global.fetch;
  global.fetch = vi.fn((url: string | URL | Request, init?: RequestInit) => {
    const u = typeof url === "string" ? url : url instanceof URL ? url.toString() : url.url;
    const method = (init?.method ?? "GET").toUpperCase();
    if (u.includes("/v1/agents") && method === "GET") {
      return Promise.resolve(
        new Response(
          JSON.stringify({
            in_process: [{ id: TEST_AGENT_ID, kind: "in_process", status: "ready" }],
            persistent: [],
            ephemeral_templates: [],
            running: [],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } }),
      );
    }
    // GET /v1/sessions — return the empty list shape. Don't intercept
    // POST /v1/sessions — that's the createSession call from ChatPanel,
    // which is mocked via `vi.mock("../api/sessions", ...)` above.
    if (u.includes("/v1/sessions") && method === "GET") {
      return Promise.resolve(
        new Response(
          JSON.stringify({ sessions: [], specs: [], details: [], running: [] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }
    if (u.includes("/v1/approvals")) {
      // Daemon returns `{pending: Approval[]}` (NOT `approvals`).
      // StoreContext.tsx:303 reads `.pending` — match the real shape.
      return Promise.resolve(
        new Response(
          JSON.stringify({ pending: [] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }
    if (u.includes("/health")) {
      return Promise.resolve(
        new Response(
          JSON.stringify({
            status: "ok",
            service: "neurox",
            uptime_seconds: 0,
            version: "0.0.0-test",
            auth_required: false,
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }
    return originalFetch(url as Request, init);
  }) as typeof fetch;

  return (
    <StoreProvider eventsPath="/__test_no_ws__{Math.random()}">{children}</StoreProvider>
  );
}

describe("ChatPanel — sesion perezosa (se crea al primer mensaje)", () => {
  beforeEach(() => {
    window.localStorage.clear();
    vi.clearAllMocks();
  });

  afterEach(() => {
    window.localStorage.clear();
    vi.restoreAllMocks();
  });

  // La sesion se crea cuando el usuario manda el primer mensaje, no al
  // montar el panel ni al abrir la pestana. Antes se creaba en los dos
  // momentos, y como una sesion sin mensajes queda `ended_at = NULL`
  // para siempre, cada ventana abierta sin escribir dejaba una sesion
  // activa permanente (se acumularon 9, 8 de ellas vacias).

  it("does NOT create a session on mount", async () => {
    mockCreateSession.mockResolvedValue({
      session_id: "test-123",
      agent_id: TEST_AGENT_ID,
    });

    render(<ChatPanel />, { wrapper: withStore });

    // Dejamos correr los efectos iniciales.
    await screen.findByTestId("chat-input");
    await new Promise((r) => setTimeout(r, 50));
    expect(mockCreateSession).not.toHaveBeenCalled();
  });

  it("creates the session on the first message, with the resolved agent_id", async () => {
    mockCreateSession.mockResolvedValue({
      session_id: "test-123",
      agent_id: TEST_AGENT_ID,
    });

    render(<ChatPanel />, { wrapper: withStore });

    const textarea = await screen.findByTestId("chat-input");
    fireEvent.change(textarea, { target: { value: "hola" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    await waitFor(() => {
      expect(mockCreateSession).toHaveBeenCalledWith(TEST_AGENT_ID);
    });
  });

  it("sends the first message to the session it just created", async () => {
    mockCreateSession.mockResolvedValue({
      session_id: "test-123",
      agent_id: TEST_AGENT_ID,
    });

    render(<ChatPanel />, { wrapper: withStore });

    const textarea = await screen.findByTestId("chat-input");
    fireEvent.change(textarea, { target: { value: "hola" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    // El id de sesion viene del POST, no del closure del render (que aun
    // era ""). Si el override no llegara, el POST del mensaje iria a
    // `/v1/sessions//messages`.
    await waitFor(() => {
      expect(mockStreamMessage).toHaveBeenCalledTimes(1);
    });
    const call = mockStreamMessage.mock.calls[0];
    expect(call?.[0]).toBe("test-123");
    expect(call?.[1]).toBe(TEST_AGENT_ID);
    expect(call?.[2]).toBe("hola");
  });

  it("the <textarea> is usable before any session exists", async () => {
    mockCreateSession.mockResolvedValue({
      session_id: "test-123",
      agent_id: TEST_AGENT_ID,
    });

    render(<ChatPanel />, { wrapper: withStore });

    // Gatear el textarea por `sessionId` era un deadlock: sin sesion no se
    // puede escribir, y sin escribir no hay sesion que crear.
    const textarea = await screen.findByTestId("chat-input");
    expect(textarea).not.toBeDisabled();
    expect(textarea.getAttribute("placeholder")).toMatch(/Type a message/);
  });

  it("surfaces the error and keeps the message unsent when the session cannot be created", async () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    mockCreateSession.mockRejectedValue(new Error("backend boom"));

    render(<ChatPanel />, { wrapper: withStore });

    const textarea = await screen.findByTestId("chat-input");
    fireEvent.change(textarea, { target: { value: "hola" } });
    fireEvent.keyDown(textarea, { key: "Enter" });

    // El mensaje NO se manda: sin sesion no hay a donde mandarlo.
    await waitFor(() => {
      expect(screen.getByText(/Could not start the session/i)).toBeTruthy();
    });
    expect(mockStreamMessage).not.toHaveBeenCalled();

    consoleError.mockRestore();
  });

  it("does NOT wipe what the user typed while the first tab was being set up", async () => {
    // Regresion: el efecto de "reset input on tab switch" corria tambien
    // cuando `activeId` pasaba de null al id de la primera pestana, que es
    // la inicializacion y no un cambio de pestana. Con el textarea
    // deshabilitado durante esa ventana era invisible; con la sesion
    // perezosa el usuario puede escribir y perdia el texto.
    mockCreateSession.mockResolvedValue({
      session_id: "test-123",
      agent_id: TEST_AGENT_ID,
    });

    render(<ChatPanel />, { wrapper: withStore });

    const textarea = await screen.findByTestId("chat-input");
    // Escribimos de inmediato, antes de que la hidratacion asiente el
    // `activeId` de la pestana draft.
    fireEvent.change(textarea, { target: { value: "hola" } });
    await new Promise((r) => setTimeout(r, 100));
    expect((textarea as HTMLTextAreaElement).value).toBe("hola");
  });

  // EP-hide-header-followup (2026-08-15) fijo la convencion contraria
  // (Enter = salto de linea, Shift+Enter = enviar, al estilo de Slack y
  // Discord). Invertida a pedido del usuario: Enter envia y Shift+Enter
  // mete un salto. Los tests vuelven a fijar la convencion vigente, que es
  // lo que evita que vuelva a flippingar sola.
  describe("keyboard shortcuts", () => {
    function setupReady() {
      mockCreateSession.mockResolvedValue({
        session_id: "test-123",
        agent_id: TEST_AGENT_ID,
      });
      return render(<ChatPanel />, { wrapper: withStore });
    }

    // El textarea ya no espera a que exista la sesion: se habilita al
    // montar. Asi que no hay nada que esperar.
    async function readyTextarea() {
      return screen.findByTestId("chat-input");
    }

    it("Enter sends", async () => {
      setupReady();
      const textarea = await readyTextarea();
      fireEvent.change(textarea, { target: { value: "hola" } });
      fireEvent.keyDown(textarea, { key: "Enter" });
      // handleSend limpia el input (setInput("")).
      await waitFor(() => {
        expect(textarea).toHaveValue("");
      });
    });

    it("Shift+Enter does NOT send: it inserts a newline", async () => {
      setupReady();
      const textarea = await readyTextarea();
      fireEvent.change(textarea, { target: { value: "hola" } });
      fireEvent.keyDown(textarea, { key: "Enter", shiftKey: true });
      // No hay envio: el texto sigue ahi y el input no se limpio.
      await new Promise((r) => setTimeout(r, 50));
      expect((textarea as HTMLTextAreaElement).value).toBe("hola");
      expect(mockCreateSession).not.toHaveBeenCalled();
    });

    it("Ctrl+Enter sends", async () => {
      setupReady();
      const textarea = await readyTextarea();
      fireEvent.change(textarea, { target: { value: "hola" } });
      fireEvent.keyDown(textarea, { key: "Enter", ctrlKey: true });
      await waitFor(() => {
        expect(textarea).toHaveValue("");
      });
    });

    it("Enter does not send while an IME is composing", async () => {
      // Con pinyin/kana, Enter confirma el candidato: es el caracter que
      // se esta escribiendo, no el envio. Sin esta excepcion el mensaje
      // se mandaba a medio componer.
      setupReady();
      const textarea = await readyTextarea();
      fireEvent.change(textarea, { target: { value: "ni" } });
      fireEvent.keyDown(textarea, { key: "Enter", isComposing: true });
      await new Promise((r) => setTimeout(r, 50));
      expect((textarea as HTMLTextAreaElement).value).toBe("ni");
      expect(mockCreateSession).not.toHaveBeenCalled();
    });
  });
});

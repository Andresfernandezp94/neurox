// Tests del reducer para los wire-format quirks.
//
// Background: el backend a veces mete el output de una tool en
// chunks `content` (en lugar de chunks `tool_result` separados).
// Eso provoca que el output aparezca pegado a la respuesta del
// LLM en el chat. El fix defensivo está en el reducer: cuando
// llega un chunk `content` inmediatamente después de un tool_call
// o tool_result del mismo tool, se redirige al `tool_result` del
// tool pendiente en lugar de concatenarse al `state.content`.

import { describe, expect, it } from "vitest";
import { streamReducer, initStreamState } from "./reducer";

describe("streamReducer — wire-format quirks (tool output embedded in content)", () => {
  it("redirects only the FIRST content chunk after a tool_call (the main wire-format quirk)", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: { cmd: "rg 'fn chat'" }, iteration: 0 });

    // Backend buggy: el output de la tool llega como `content` chunks.
    // Solo el primero va al tool_result.
    s = streamReducer(s, { type: "content", text: 'no symbols matching "fn chat" found\n' });

    // Después del primer chunk redirigido, el modo "tool" se desactiva.
    expect(s.content).toBe(""); // el primer chunk se fue al tool_result
    expect(s.toolLog[0]!.result).toBe('no symbols matching "fn chat" found\n');

    // El segundo chunk va al state.content (no se redirige más).
    s = streamReducer(s, { type: "content", text: 'Some file:foo.py:5:...\n' });
    expect(s.content).toBe('Some file:foo.py:5:...\n');
    // El tool_result NO acumula el segundo chunk (single-shot redirect).
    expect(s.toolLog[0]!.result).toBe('no symbols matching "fn chat" found\n');
  });

  it("preserves the LLM's final response after a tool_call", () => {
    // Caso del usuario: la respuesta final del agente después del
    // tool NO debe ser redirigida al tool_result.
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });

    // Backend envía el output como un content chunk (primer chunk → redirigido).
    s = streamReducer(s, { type: "content", text: "shell output\n" });
    expect(s.toolLog[0]!.result).toBe("shell output\n");

    // La respuesta final del agente llega como un content chunk.
    // El modo "tool" ya está desactivado, así que va al state.content.
    s = streamReducer(s, { type: "content", text: "I executed that and got nothing." });
    expect(s.content).toBe("I executed that and got nothing.");
    // El tool_result NO se modifica.
    expect(s.toolLog[0]!.result).toBe("shell output\n");
  });

  it("switches back to normal `content` once a thinking chunk arrives", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    // Buggy output redirigido.
    s = streamReducer(s, { type: "content", text: "stdout\n" });
    expect(s.content).toBe("");

    // Ahora el LLM piensa, después habla. El content del LLM debe ir al state.content.
    s = streamReducer(s, { type: "thinking", text: "I should respond now.\n" });
    s = streamReducer(s, { type: "content", text: "I searched and found nothing matching that pattern." });

    expect(s.content).toBe("I searched and found nothing matching that pattern.");
    expect(s.toolLog[0]!.result).toBe("stdout\n"); // no se sobreescribe
    expect(s.thinking).toContain("respond now");
  });

  it("a legitimate `tool_result` still fills the tool entry (without redirecting further content)", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "fetch", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "tool_result", tool: "fetch", result: "ok", iteration: 0 });
    expect(s.toolLog[0]!.result).toBe("ok");

    // LLM comment tras el tool_result (wire format correcto).
    s = streamReducer(s, { type: "content", text: "Here is the summary." });
    expect(s.content).toBe("Here is the summary.");
    expect(s.toolLog[0]!.result).toBe("ok"); // intacto
  });

  it("ignores `tool_result` chunks after the matching tool_call has been closed by redirected content", () => {
    // Caso edge: backend manda tool_call, después varios content
    // (que redirigimos al tool_result), después un tool_result
    // legítimo. El tool_result SOBREESCRIBE el result (no acumula).
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "content", text: "first chunk\n" });
    // Ahora llega un tool_result formal — el reducer lo matchea con el
    // pending tool y le setea result, reemplazando lo redirigido.
    s = streamReducer(s, { type: "tool_result", tool: "shell", result: "official result", iteration: 0 });
    expect(s.toolLog[0]!.result).toBe("official result");
  });

  it("normal `content` chunks from the start go straight to state.content", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "content", text: "Hello" });
    s = streamReducer(s, { type: "content", text: " world" });
    // word-boundary fix ya agregado: "Hello" + " " + "world"
    expect(s.content).toBe("Hello world");
  });

  it("approval_request does NOT trigger the tool-output redirect", () => {
    // Si viene un approval y DESPUÉS un content, ese content debe
    // ir al state.content (es comentario del LLM, no output).
    let s = initStreamState();
    s = streamReducer(s, {
      type: "approval_request",
      id: "a1",
      tool: "dangerous_tool",
      reason: "needs review",
    });
    s = streamReducer(s, { type: "content", text: "Please review." });
    expect(s.content).toBe("Please review.");
  });

  it("multiple tool_calls in a row: each content chunk after a new tool_call redirect to the LATEST pending tool", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "content", text: "shell output\n" });
    expect(s.toolLog[0]!.result).toBe("shell output\n");

    s = streamReducer(s, { type: "tool_call", tool: "fetch", args: {}, iteration: 1 });
    // Ahora un content chunk debe ir a `fetch`, no a `shell`.
    s = streamReducer(s, { type: "content", text: "fetch output\n" });
    expect(s.toolLog[0]!.result).toBe("shell output\n"); // intacto
    expect(s.toolLog[1]!.result).toBe("fetch output\n");
  });
});

// ─── EP-2026-08-19: regression suite for the "agent hangs on shell/write" bug ─

describe("streamReducer — tool_result matches by iteration, not just name", () => {
  it("two shell calls with different iterations: each tool_result lands on its own entry", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: { cmd: "ls" }, iteration: 0 });
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: { cmd: "cat foo" }, iteration: 1 });

    // El result del primer shell no debe pisar al segundo (que aún no tiene).
    s = streamReducer(s, { type: "tool_result", tool: "shell", result: "ls output", iteration: 0 });
    expect(s.toolLog[0]!.result).toBe("ls output");
    expect(s.toolLog[1]!.result).toBeUndefined();

    // El result del segundo shell tampoco debe pisar al primero.
    s = streamReducer(s, { type: "tool_result", tool: "shell", result: "cat output", iteration: 1 });
    expect(s.toolLog[0]!.result).toBe("ls output");
    expect(s.toolLog[1]!.result).toBe("cat output");

    // Lo mismo en el timeline.
    const shellEntries = s.timeline.filter((e) => e.type === "tool" && e.tool === "shell");
    expect(shellEntries).toHaveLength(2);
    const [t0, t1] = shellEntries;
    expect(t0 && t0.type === "tool" && t0.result).toBe("ls output");
    expect(t1 && t1.type === "tool" && t1.result).toBe("cat output");
  });

  it("redirected content + tool_result for the same iteration replaces (not appends)", () => {
    // Caso que ya cubría el test anterior pero para el redirect path:
    // tool_call shell (iter 0) → content redirigido al result → tool_result formal.
    // El tool_result debe REEMPLAZAR lo redirigido (no acumular).
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "content", text: "redirected output\n" });
    expect(s.toolLog[0]!.result).toBe("redirected output\n");

    s = streamReducer(s, { type: "tool_result", tool: "shell", result: "authoritative", iteration: 0 });
    expect(s.toolLog[0]!.result).toBe("authoritative");
  });

  it("redirected content after the second shell goes to iteration 1, not 0", () => {
    // Variante del redirect multi-tool: el contenido redirigido
    // después del segundo tool_call debe apuntar al entry con
    // iteration=1 (no al iteration=0).
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 1 });

    s = streamReducer(s, { type: "content", text: "second shell stdout\n" });
    expect(s.toolLog[0]!.result).toBeUndefined();
    expect(s.toolLog[1]!.result).toBe("second shell stdout\n");
    expect(s.content).toBe(""); // no se filtró al reply del LLM
  });

  it("legacy tool_result without iteration falls back to name-only matching against the most recent entry", () => {
    // Backends viejos mandan tool_result sin iteration. La lógica
    // primaria (por iteration) falla, pero el fallback por nombre
    // sigue encontrando la entrada pendiente más reciente.
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 1 });

    s = streamReducer(s, { type: "tool_result", tool: "shell", result: "fallback match" });
    expect(s.toolLog[1]!.result).toBe("fallback match");
    expect(s.toolLog[0]!.result).toBeUndefined();
  });
});

describe("streamReducer — error chunks surface to StreamState.error", () => {
  it("an error chunk sets state.error without clearing partial content", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "content", text: "partial assistant text " });
    expect(s.content).toBe("partial assistant text ");
    expect(s.error).toBeNull();

    s = streamReducer(s, { type: "error", message: "shell failed: permission denied" });
    expect(s.error).toBe("shell failed: permission denied");
    // El contenido parcial se preserva — el caller decide si mostrarlo.
    expect(s.content).toBe("partial assistant text ");
  });

  it("the latest error wins (overwrites previous)", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "error", message: "first" });
    s = streamReducer(s, { type: "error", message: "second" });
    expect(s.error).toBe("second");
  });

  it("error after a tool_call does not pollute the tool entry", () => {
    let s = initStreamState();
    s = streamReducer(s, { type: "tool_call", tool: "shell", args: {}, iteration: 0 });
    s = streamReducer(s, { type: "error", message: "kaboom" });
    expect(s.error).toBe("kaboom");
    expect(s.toolLog[0]!.result).toBeUndefined();
    // La entrada queda "pending" — ToolNode la mostrará como
    // "interrupted (no result received)" cuando el stream termine.
  });
});

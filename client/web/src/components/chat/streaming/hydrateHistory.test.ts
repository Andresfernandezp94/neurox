import { describe, it, expect } from "vitest";
import { hydrateMessages } from "./hydrateHistory";
import type { TimelineEntry } from "../../../types";

/**
 * EP-2026-10-03: el daemon persiste cada evento del stream como una fila
 * (thinking / tool_call / tool) y cierra con el assistant. Estos tests
 * fijan el agrupado, que es lo que hace que un F5 a mitad de stream
 * muestre el progreso en vez de una sesión vacía.
 */

// El repo compila con `noUncheckedIndexedAccess`, así que los tests
// acceden al timeline por estos helpers: si la entrada no existe, el
// test falla con "undefined is not an object" en vez de pasar en falso.
type Msg = { timeline?: TimelineEntry[]; role: string; content: string; thinking?: string };
/** `msg(out, n)` sin optional-chaining: si el mensaje no existe, revienta. */
const msg = (out: Msg[], n: number): Msg => out[n] as Msg;
const tl = (m: Msg): TimelineEntry[] => m.timeline ?? [];
const at = (m: Msg, i: number): TimelineEntry => tl(m)[i] as TimelineEntry;

const row = (
  id: number,
  role: string,
  content: string,
  extra: Partial<{ tool_name: string | null; thinking: string | null }> = {},
) => ({
  id,
  role,
  content,
  ts: `2026-10-03T00:00:0${id}Z`,
  tool_name: extra.tool_name ?? null,
  thinking: extra.thinking ?? null,
});

describe("hydrateMessages", () => {
  it("pasa user/assistant tal cual, sin inventar timeline", () => {
    const out = hydrateMessages(
      [row(1, "user", "hola"), row(2, "assistant", "buenas")],
      "s1",
    );
    expect(out).toHaveLength(2);
    expect(msg(out, 0).role).toBe("user");
    expect(msg(out, 1).role).toBe("assistant");
    expect(msg(out, 1).timeline).toBeUndefined();
  });

  it("funde tool_call + tool en una sola entrada con args y result", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "listame /tmp"),
        row(2, "tool_call", '{"path":"/tmp"}', { tool_name: "list_dir" }),
        row(3, "tool", "a.txt\nb.txt", { tool_name: "list_dir" }),
        row(4, "assistant", "hay 2 archivos"),
      ],
      "s1",
    );
    expect(out).toHaveLength(2);
    const tl = msg(out, 1).timeline ?? [];
    expect(tl).toHaveLength(1);
    expect(tl[0]).toMatchObject({
      type: "tool",
      tool: "list_dir",
      args: { path: "/tmp" },
      result: "a.txt\nb.txt",
    });
  });

  it("un tool_call sin result se emite igual (tool en vuelo)", () => {
    // Es el caso del F5 durante un shell de 30s: el call se persistió
    // pero el result todavía no.
    const out = hydrateMessages(
      [
        row(1, "user", "corré un comando"),
        row(2, "tool_call", '{"command":"ls"}', { tool_name: "shell" }),
        row(3, "assistant", "listo"),
      ],
      "s1",
    );
    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(1);
    expect(at(msg(out, 1), 0)).toMatchObject({ type: "tool", tool: "shell" });
    expect((at(msg(out, 1), 0) as { result?: string }).result).toBeUndefined();
  });

  it("concatena fragmentos contiguos de thinking", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "analiza"),
        row(2, "thinking", "primera parte"),
        row(3, "thinking", "segunda parte"),
        row(4, "assistant", "listo"),
      ],
      "s1",
    );
    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(1);
    expect(at(msg(out, 1), 0)).toEqual({
      type: "thinking",
      text: "primera parte\n\nsegunda parte",
    });
  });

  it("respeta el orden real: think, tool, think, tool", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "hacé dos cosas"),
        row(2, "thinking", "primero leo"),
        row(3, "tool_call", '{"path":"a"}', { tool_name: "read_file" }),
        row(4, "tool", "contenido", { tool_name: "read_file" }),
        row(5, "thinking", "despues escribo"),
        row(6, "tool_call", '{"path":"b"}', { tool_name: "write_file" }),
        row(7, "tool", "ok", { tool_name: "write_file" }),
        row(8, "assistant", "hecho"),
      ],
      "s1",
    );
    expect(tl(msg(out, 1)).map((e) => e.type)).toEqual([
      "thinking",
      "tool",
      "thinking",
      "tool",
    ]);
    expect(at(msg(out, 1), 1)).toMatchObject({ tool: "read_file" });
    expect(at(msg(out, 1), 3)).toMatchObject({ tool: "write_file" });
  });

  it("el turno siguiente no hereda el timeline del anterior", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "primero"),
        row(2, "tool_call", "{}", { tool_name: "shell" }),
        row(3, "tool", "ok", { tool_name: "shell" }),
        row(4, "assistant", "uno"),
        row(5, "user", "segundo"),
        row(6, "assistant", "dos"),
      ],
      "s1",
    );
    expect(out).toHaveLength(4);
    expect(msg(out, 1).timeline).toHaveLength(1);
    expect(msg(out, 3).timeline).toBeUndefined();
  });

  it("no pierde progreso si el stream se cortó antes del assistant", () => {
    // El daemon ya persistió thinking+tool_call pero el turno murió antes
    // del texto final. Sin assistant que los cierre, las filas igual se
    // entregan: es exactamente el caso que reportaste.
    const out = hydrateMessages(
      [
        row(1, "user", "probá las tools"),
        row(2, "thinking", "voy a probar"),
        row(3, "tool_call", '{"path":"/tmp/x"}', { tool_name: "read_file" }),
      ],
      "s1",
    );
    expect(out).toHaveLength(2);
    expect(msg(out, 0).role).toBe("user");
    expect((msg(out, 1).timeline ?? [])).toHaveLength(2);
  });

  it("args que no parsean quedan como texto, no rompen", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", "no soy json", { tool_name: "shell" }),
        row(3, "assistant", "y"),
      ],
      "s1",
    );
    expect((at(msg(out, 1), 0) as { args?: unknown }).args).toBe("no soy json");
  });

  it("un tool_result sin call previo se emite suelto", () => {
    // El call se perdió pero el result quedó: mejor mostrarlo que
    // esconderlo.
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool", "resultado", { tool_name: "shell" }),
        row(3, "assistant", "y"),
      ],
      "s1",
    );
    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(1);
    expect(at(msg(out, 1), 0)).toMatchObject({ type: "tool", tool: "shell" });
  });

  it("conserva el thinking de columna del assistant", () => {
    const out = hydrateMessages(
      [row(1, "user", "x"), { ...row(2, "assistant", "y"), thinking: "razon" }],
      "s1",
    );
    expect(msg(out, 1).thinking).toBe("razon");
  });

  it("numeros los iterations de tool en orden de aparicion", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", "{}", { tool_name: "a" }),
        row(3, "tool", "1", { tool_name: "a" }),
        row(4, "tool_call", "{}", { tool_name: "b" }),
        row(5, "tool", "2", { tool_name: "b" }),
        row(6, "assistant", "y"),
      ],
      "s1",
    );
    expect((at(msg(out, 1), 0) as { iteration: number }).iteration).toBe(0);
    expect((at(msg(out, 1), 1) as { iteration: number }).iteration).toBe(1);
  });
});
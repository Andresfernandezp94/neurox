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
  extra: Partial<{
    tool_name: string | null;
    thinking: string | null;
    tool_call_id: string | null;
  }> = {},
) => ({
  id,
  role,
  content,
  ts: `2026-10-03T00:00:0${id}Z`,
  tool_name: extra.tool_name ?? null,
  thinking: extra.thinking ?? null,
  tool_call_id: extra.tool_call_id ?? null,
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
  // ── EP-2026-10-03: despacho en paralelo ────────────────────────────
  //
  // Con el lote ejecutado en paralelo el daemon emite TODOS los
  // `tool_call` seguidos y despues TODOS los `tool_result`, en el orden
  // del modelo. Estos tests fijan ese emparejado: antes el agrupador
  // llevaba un unico "abierto", de modo que el primer resultado se
  // pegaba a la ultima llamada y los demas salian sueltos sin `args`.

  it("empareja N tool_call seguidos con sus N resultados, en orden", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file" }),
        row(3, "tool_call", '{"path":"b"}', { tool_name: "read_file" }),
        row(4, "tool_call", '{"path":"c"}', { tool_name: "read_file" }),
        row(5, "tool", "contenido a", { tool_name: "read_file" }),
        row(6, "tool", "contenido b", { tool_name: "read_file" }),
        row(7, "tool", "contenido c", { tool_name: "read_file" }),
        row(8, "assistant", "listo"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    // Tres entradas, no tres sueltas mas tres con args huerfanos.
    expect(timeline).toHaveLength(3);
    expect(timeline.map((e) => (e as { args: unknown }).args)).toEqual([
      { path: "a" },
      { path: "b" },
      { path: "c" },
    ]);
    // Cada resultado cae en su tool, no todas en la ultima.
    expect(timeline.map((e) => (e as { result?: string }).result)).toEqual([
      "contenido a",
      "contenido b",
      "contenido c",
    ]);
  });

  it("cada tool_call conserva su args aunque se ejecute en paralelo", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file" }),
        row(3, "tool_call", '{"cmd":"ls"}', { tool_name: "shell" }),
        row(4, "tool", "contenido a", { tool_name: "read_file" }),
        row(5, "tool", "listing", { tool_name: "shell" }),
        row(6, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    // Herramientas distintas: el emparejado usa el nombre cuando puede, y
    // no depende de que el lote sea homogeneo.
    expect(at(msg(out, 1), 0)).toMatchObject({
      tool: "read_file",
      args: { path: "a" },
      result: "contenido a",
    });
    expect(at(msg(out, 1), 1)).toMatchObject({
      tool: "shell",
      args: { cmd: "ls" },
      result: "listing",
    });
  });

  it("deja en running las llamadas cuyo resultado aun no llego", () => {
    // A mitad de un lote en vuelo: hay mas tool_call que tool.
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file" }),
        row(3, "tool_call", '{"path":"b"}', { tool_name: "read_file" }),
        row(4, "tool_call", '{"path":"c"}', { tool_name: "read_file" }),
        row(5, "tool", "contenido a", { tool_name: "read_file" }),
        row(6, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(3);
    expect((at(msg(out, 1), 0) as { result?: string }).result).toBe(
      "contenido a",
    );
    // Las dos siguientes siguen sin resultado -> el renderer las muestra
    // girando. Y no se han comido el resultado de la primera.
    expect(
      (at(msg(out, 1), 1) as { result?: string }).result,
    ).toBeUndefined();
    expect(
      (at(msg(out, 1), 2) as { result?: string }).result,
    ).toBeUndefined();
    expect((at(msg(out, 1), 1) as { args: unknown }).args).toEqual({
      path: "b",
    });
  });

  it("un tool sin call se emite suelto sin romper el resto", () => {
    // Tool desconocida: el daemon persiste el resultado pero nunca el
    // call (prepare_tool_call sale antes de emitirlo).
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool", "[error] unknown tool: inventada", {
          tool_name: "inventada",
        }),
        row(3, "tool_call", '{"path":"a"}', { tool_name: "read_file" }),
        row(4, "tool", "contenido a", { tool_name: "read_file" }),
        row(5, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    expect(at(msg(out, 1), 0)).toMatchObject({
      tool: "inventada",
      result: "[error] unknown tool: inventada",
    });
    // El resultado siguiente NO se engancha a la tool suelta anterior.
    expect(at(msg(out, 1), 1)).toMatchObject({
      tool: "read_file",
      result: "contenido a",
    });
  });

  it("el emparejado no cruza un limite de turno", () => {
    // Turno 1 deja una tool en vuelo; el turno 2 trae sus propias calls.
    // Sin resetear la cola, el resultado del turno 2 cerraria la tool del
    // turno 1.
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file" }),
        row(3, "assistant", "sin terminar"),
        row(4, "user", "otra cosa"),
        row(5, "tool_call", '{"path":"b"}', { tool_name: "read_file" }),
        row(6, "tool", "contenido b", { tool_name: "read_file" }),
        row(7, "assistant", "y"),
      ],
      "s1",
    );

    expect(
      (at(msg(out, 1), 0) as { result?: string }).result,
    ).toBeUndefined();
    expect(at(msg(out, 3), 0)).toMatchObject({
      args: { path: "b" },
      result: "contenido b",
    });
  });
  // ── EP-2026-10-03: emparejado por tool_call_id ────────────────────────
  //
  // El daemon escribe el mismo `tool_call_id` en la fila de la llamada y
  // en la de su resultado. Es el emparejado exacto, y el que de verdad
  // usa la app: antes el `tool_call` se guardaba con role="tool" y no
  // existia ninguna fila `tool_call`, asi que este camino no se ejecucionaba
  // nunca contra datos reales.

  it("empareja por tool_call_id aunque las tools sean iguales", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file", tool_call_id: "c1" }),
        row(3, "tool_call", '{"path":"b"}', { tool_name: "read_file", tool_call_id: "c2" }),
        row(4, "tool", "contenido a", { tool_name: "read_file", tool_call_id: "c1" }),
        row(5, "tool", "contenido b", { tool_name: "read_file", tool_call_id: "c2" }),
        row(6, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    expect(timeline[0]).toMatchObject({
      args: { path: "a" },
      result: "contenido a",
      call_id: "c1",
    });
    expect(timeline[1]).toMatchObject({
      args: { path: "b" },
      result: "contenido b",
      call_id: "c2",
    });
  });

  it("el id manda sobre el orden: un resultado puede llegar antes que otro", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file", tool_call_id: "c1" }),
        row(3, "tool_call", '{"path":"b"}', { tool_name: "read_file", tool_call_id: "c2" }),
        // El resultado de c2 se persiste antes que el de c1.
        row(4, "tool", "contenido b", { tool_name: "read_file", tool_call_id: "c2" }),
        row(5, "tool", "contenido a", { tool_name: "read_file", tool_call_id: "c1" }),
        row(6, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    expect(timeline[0]).toMatchObject({ args: { path: "a" }, result: "contenido a" });
    expect(timeline[1]).toMatchObject({ args: { path: "b" }, result: "contenido b" });
  });

  it("mezcla de tools distintas con id: cada resultado va a la suya", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file", tool_call_id: "c1" }),
        row(3, "tool_call", '{"cmd":"ls"}', { tool_name: "shell", tool_call_id: "c2" }),
        row(4, "tool", "listing", { tool_name: "shell", tool_call_id: "c2" }),
        row(5, "tool", "contenido a", { tool_name: "read_file", tool_call_id: "c1" }),
        row(6, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    expect(timeline[0]).toMatchObject({ tool: "read_file", result: "contenido a" });
    expect(timeline[1]).toMatchObject({ tool: "shell", result: "listing" });
  });

  it("una tool en vuelo con id sigue mostrando su args sin resultado", () => {
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file", tool_call_id: "c1" }),
        row(3, "tool_call", '{"path":"b"}', { tool_name: "read_file", tool_call_id: "c2" }),
        row(4, "tool", "contenido a", { tool_name: "read_file", tool_call_id: "c1" }),
        row(5, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    expect(timeline[0]).toMatchObject({ args: { path: "a" }, result: "contenido a" });
    expect((timeline[1] as { result?: string }).result).toBeUndefined();
    expect((timeline[1] as { args: unknown }).args).toEqual({ path: "b" });
  });
  it("un tool_call_id sin fila tool_call se emite suelto sin robar otra", () => {
    // El daemon persiste el resultado de una tool cuya peticion no llego a
    // guardarse. Con id, se sabe que no pertenece a ninguna de las abiertas,
    // asi que va suelto en vez de meterse en la primera.
    const out = hydrateMessages(
      [
        row(1, "user", "x"),
        row(2, "tool_call", '{"path":"a"}', { tool_name: "read_file", tool_call_id: "c1" }),
        row(3, "tool", "huerfano", { tool_name: "read_file", tool_call_id: "perdida" }),
        row(4, "tool", "contenido a", { tool_name: "read_file", tool_call_id: "c1" }),
        row(5, "assistant", "y"),
      ],
      "s1",
    );

    const timeline = tl(msg(out, 1));
    expect(timeline).toHaveLength(2);
    // La peticion c1 acaba con SU resultado, no con el huerfano.
    expect(timeline[0]).toMatchObject({ args: { path: "a" }, result: "contenido a" });
    expect(timeline[1]).toMatchObject({ result: "huerfano" });
    expect((timeline[1] as { args?: unknown }).args).toBeUndefined();
  });
});

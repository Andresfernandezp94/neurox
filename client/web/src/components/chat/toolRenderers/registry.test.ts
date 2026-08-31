// Tests del registry + status parser de tool renderers.
//
// Cubren el contrato que usan ArgsBlock / ResultBlock para decidir
// cómo mostrar args y result de cada tool. La integración con el DOM
// se cubre en ToolNode.test.tsx (ver archivo hermano).

import { describe, expect, it } from "vitest";
import { TOOL_REGISTRY, getToolConfig } from "./registry";

describe("registry — shell", () => {
  const shell = getToolConfig("shell");

  it("declares labels legibles para los args comunes", () => {
    expect(shell.argLabels?.cmd?.label).toBe("Command");
    expect(shell.argLabels?.cmd?.presentation).toBe("code");
    expect(shell.argLabels?.cwd?.label).toBe("CWD");
    expect(shell.argLabels?.cwd?.presentation).toBe("path");
    expect(shell.argLabels?.timeout?.label).toBe("Timeout");
    expect(shell.argLabels?.timeout?.presentation).toBe("duration");
  });

  it("parseStatus detecta exit_code 0 como ok", () => {
    const parsed = shell.parseStatus?.(
      JSON.stringify({ exit_code: 0, stdout: "hello", stderr: "" }),
    );
    expect(parsed?.status).toBe("ok");
    expect(parsed?.code).toBe(0);
  });

  it("parseStatus detecta exit_code != 0 como fail y expone el primer line de stderr", () => {
    const parsed = shell.parseStatus?.(
      JSON.stringify({
        exit_code: 2,
        stdout: "",
        stderr: "permission denied\nstack trace here",
      }),
    );
    expect(parsed?.status).toBe("fail");
    expect(parsed?.code).toBe(2);
    expect(parsed?.message).toBe("permission denied");
  });

  it("parseStatus cae al genérico si no hay exit_code", () => {
    const parsed = shell.parseStatus?.(JSON.stringify({ ok: true }));
    expect(parsed?.status).toBe("ok");
  });

  it("caption devuelve 'exit N' cuando el output trae exit_code", () => {
    expect(shell.caption?.(JSON.stringify({ exit_code: 7 }), null)).toBe(
      "exit 7",
    );
    expect(shell.caption?.("plain text", null)).toBeNull();
  });
});

describe("registry — write_file / edit_file", () => {
  it("write_file muestra bytes_written como caption", () => {
    const cfg = getToolConfig("write_file");
    expect(
      cfg.caption?.(JSON.stringify({ ok: true, bytes_written: 142 }), null),
    ).toBe("142 bytes");
  });

  it("edit_file muestra replacements como caption (singular/plural)", () => {
    const cfg = getToolConfig("edit_file");
    expect(
      cfg.caption?.(JSON.stringify({ ok: true, replacements: 1 }), null),
    ).toBe("1 replacement");
    expect(
      cfg.caption?.(JSON.stringify({ ok: true, replacements: 3 }), null),
    ).toBe("3 replacements");
  });
});

describe("registry — grep / glob / search_memory", () => {
  it("grep marca output vacío como empty (no fail)", () => {
    const cfg = getToolConfig("grep");
    expect(cfg.parseStatus?.("")?.status).toBe("empty");
    expect(cfg.parseStatus?.("   \n  ")).toEqual(
      expect.objectContaining({ status: "empty" }),
    );
  });

  it("glob cuenta líneas del output como caption", () => {
    const cfg = getToolConfig("glob");
    expect(cfg.caption?.("a.txt\nb.txt\nc.txt", null)).toBe("3 files");
    expect(cfg.caption?.("single.txt", null)).toBe("1 file");
  });

  it("web_fetch reporta HTTP status como caption", () => {
    const cfg = getToolConfig("web_fetch");
    expect(
      cfg.caption?.(JSON.stringify({ status: 404 }), null),
    ).toBe("HTTP 404");
  });
});

describe("registry — genericParseStatus", () => {
  const shell = getToolConfig("shell");

  it("objeto JSON sin ok/error devuelve null (no signal claro)", () => {
    expect(shell.parseStatus?.(JSON.stringify({ foo: 1 }))).toBeNull();
  });

  it("objeto JSON con `ok: true` se trata como ok", () => {
    expect(
      shell.parseStatus?.(JSON.stringify({ ok: true, foo: 1 }))?.status,
    ).toBe("ok");
  });

  it("objeto JSON con `ok: false` y `error: string` se trata como fail", () => {
    const parsed = shell.parseStatus?.(
      JSON.stringify({ ok: false, error: "boom" }),
    );
    expect(parsed?.status).toBe("fail");
    expect(parsed?.message).toBe("boom");
  });

  it("objeto JSON con `status: 'success'` se trata como ok", () => {
    expect(
      shell.parseStatus?.(JSON.stringify({ status: "success" }))?.status,
    ).toBe("ok");
  });

  it("objeto JSON con `error: string` solo (sin ok) se trata como fail", () => {
    const parsed = shell.parseStatus?.(JSON.stringify({ error: "nope" }));
    expect(parsed?.status).toBe("fail");
    expect(parsed?.message).toBe("nope");
  });

  it("string vacío → empty", () => {
    expect(shell.parseStatus?.("")?.status).toBe("empty");
    expect(shell.parseStatus?.("\n\n")?.status).toBe("empty");
  });
});

describe("registry — fallbacks", () => {
  it("getToolConfig devuelve config vacío para tools desconocidas", () => {
    const cfg = getToolConfig("some_random_plugin_tool");
    expect(cfg).toEqual({});
    expect(cfg.argLabels).toBeUndefined();
    expect(cfg.parseStatus).toBeUndefined();
    expect(cfg.renderBody).toBeUndefined();
  });

  it("aliases comunes también resuelven", () => {
    expect(getToolConfig("shell_exec").argLabels?.cmd?.label).toBe("Command");
    expect(getToolConfig("edit").argLabels?.path?.label).toBe("Path");
  });
});

describe("TOOL_REGISTRY — invariantes", () => {
  it("cada tool con argLabels usa labels no vacíos", () => {
    for (const [tool, cfg] of Object.entries(TOOL_REGISTRY)) {
      for (const [key, spec] of Object.entries(cfg.argLabels ?? {})) {
        expect(spec.label.length, `${tool}.${key} should have a label`).toBeGreaterThan(0);
      }
    }
  });

  it("cada parser devuelve ParsedStatus|null (nunca tira)", () => {
    for (const [, cfg] of Object.entries(TOOL_REGISTRY)) {
      const parser = cfg.parseStatus;
      if (!parser) continue;
      expect(() => parser("")).not.toThrow();
      expect(() => parser("not json")).not.toThrow();
      expect(() => parser("{ malformed json")).not.toThrow();
      expect(() => parser('{"ok": true}')).not.toThrow();
    }
  });
});

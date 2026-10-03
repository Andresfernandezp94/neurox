// toolRenderers/registry.ts — configuración hardcoded por tool.
//
// Para los ~13 tools principales del agente (shell, read/write_file,
// edit_file, grep, glob, list_dir, web_*,
// generate_*, symbols) definimos:
//
//   - labels legibles para los parámetros más comunes
//   - un parser de status (ok/fail/empty) que inspecciona el output
//   - un renderer custom del body cuando aporta valor sobre el JSON
//     plano (ej. shell separa stdout/stderr, write_file dice "ok · N
//     bytes")
//
// Tools desconocidas caen al
// `SmartResult` existente — no rompemos nada.
//
// EP-2026-08-19.

import { ToolCodeBlock } from "../../../shared/components/molecules/ToolCodeBlock";
import type {
  ArgLabels,
  ParsedStatus,
  ResultStatus,
  ToolRegistry,
} from "./types";

// ─── helpers ─────────────────────────────────────────────────────────

/** ¿El output parsea como JSON object? */
function tryParseJsonObject(raw: string): Record<string, unknown> | null {
  const trimmed = raw.trim();
  if (!(trimmed.startsWith("{") && trimmed.endsWith("}"))) return null;
  try {
    const v = JSON.parse(trimmed);
    return v && typeof v === "object" && !Array.isArray(v)
      ? (v as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

/** Heurística genérica de status: busca campos ok/success/error/failed. */
function genericParseStatus(raw: string): ParsedStatus | null {
  const obj = tryParseJsonObject(raw);
  if (obj) {
    const status =
      typeof obj.status === "string" ? (obj.status as string).toLowerCase() : null;
    if (status === "ok" || status === "success" || status === "succeeded") {
      return { status: "ok" as ResultStatus };
    }
    if (status === "error" || status === "failed" || status === "failure") {
      return {
        status: "fail" as ResultStatus,
        message:
          (typeof obj.error === "string" && obj.error) ||
          (typeof obj.message === "string" && obj.message) ||
          undefined,
        code: typeof obj.code === "number" ? (obj.code as number) : undefined,
      };
    }
    if ("ok" in obj && typeof obj.ok === "boolean") {
      const ok = obj.ok as boolean;
      return ok
        ? { status: "ok" as ResultStatus }
        : {
            status: "fail" as ResultStatus,
            message:
              (typeof obj.error === "string" && obj.error) ||
              (typeof obj.message === "string" && obj.message) ||
              undefined,
            code:
              typeof obj.code === "number"
                ? (obj.code as number)
                : undefined,
          };
    }
    if (typeof obj.error === "string" && obj.error.length > 0) {
      return {
        status: "fail" as ResultStatus,
        message: obj.error as string,
      };
    }
    return null; // JSON object pero sin signal claro → renderer decide
  }
  // No es JSON. ¿Está vacío / whitespace?
  if (raw.trim().length === 0) {
    return { status: "empty" as ResultStatus };
  }
  return null;
}

// ─── per-tool configs ──────────────────────────────────────────────

const shellArgs: ArgLabels = {
  cmd: { label: "Command", presentation: "code" },
  command: { label: "Command", presentation: "code" },
  cwd: { label: "CWD", presentation: "path" },
  timeout: { label: "Timeout", presentation: "duration" },
  env: { label: "Env" },
};

const readFileArgs: ArgLabels = {
  path: { label: "Path", presentation: "path" },
  start_line: { label: "Start line", presentation: "number" },
  end_line: { label: "End line", presentation: "number" },
  max_bytes: { label: "Max bytes", presentation: "number" },
};

const writeFileArgs: ArgLabels = {
  path: { label: "Path", presentation: "path" },
  content: { label: "Content", presentation: "multiline-code" },
  append: { label: "Append", presentation: "boolean" },
};

const editFileArgs: ArgLabels = {
  path: { label: "Path", presentation: "path" },
  old_string: { label: "Old", presentation: "multiline-code" },
  new_string: { label: "New", presentation: "multiline-code" },
  replace_all: { label: "Replace all", presentation: "boolean" },
};

const grepArgs: ArgLabels = {
  pattern: { label: "Pattern", presentation: "code" },
  path: { label: "Path", presentation: "path" },
  glob_filter: { label: "Glob filter", presentation: "code" },
  context_lines: { label: "Context", presentation: "number" },
  case_insensitive: { label: "Case insensitive", presentation: "boolean" },
};

const globArgs: ArgLabels = {
  pattern: { label: "Pattern", presentation: "code" },
  path: { label: "Path", presentation: "path" },
};

const listDirArgs: ArgLabels = {
  path: { label: "Path", presentation: "path" },
  depth: { label: "Depth", presentation: "number" },
  show_hidden: { label: "Show hidden", presentation: "boolean" },
};

const webSearchArgs: ArgLabels = {
  query: { label: "Query" },
  max_results: { label: "Max results", presentation: "number" },
};

const webFetchArgs: ArgLabels = {
  url: { label: "URL" },
  max_bytes: { label: "Max bytes", presentation: "number" },
};

const symbolsArgs: ArgLabels = {
  query: { label: "Query" },
  path: { label: "Path", presentation: "path" },
};

// ─── registry ──────────────────────────────────────────────────────

export const TOOL_REGISTRY: ToolRegistry = {
  // ─── shell ────────────────────────────────────────────────────────
  shell: {
    argLabels: shellArgs,
    parseStatus: (raw) => {
      // shell suele devolver JSON {exit_code, stdout, stderr}.
      const obj = tryParseJsonObject(raw);
      if (obj && typeof obj.exit_code === "number") {
        const exit = obj.exit_code as number;
        return {
          status: exit === 0 ? "ok" : "fail",
          code: exit,
          message:
            typeof obj.stderr === "string" && obj.stderr.length > 0
              ? (obj.stderr as string).split("\n")[0]?.slice(0, 200)
              : undefined,
        };
      }
      return genericParseStatus(raw);
    },
    renderBody: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (!obj) return null; // cae al SmartResult
      const stdout = typeof obj.stdout === "string" ? (obj.stdout as string) : "";
      const stderr = typeof obj.stderr === "string" ? (obj.stderr as string) : "";
      const trimmed = typeof obj.truncated === "boolean" ? (obj.truncated as boolean) : false;
      if (!stdout && !stderr) return null;
      return (
        <div className="tool-renderer__shell">
          {stdout && (
            <ToolCodeBlock
              code={stdout}
              language="text"
              className="tool-renderer__shell-stdout"
              testId="tool-shell-stdout"
            />
          )}
          {stderr && (
            <ToolCodeBlock
              code={stderr}
              language="text"
              className="tool-renderer__shell-stderr"
              testId="tool-shell-stderr"
            />
          )}
          {trimmed && (
            <div className="tool-renderer__shell-meta" data-testid="tool-shell-truncated">
              ⚠ output truncated
            </div>
          )}
        </div>
      );
    },
    caption: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && typeof obj.exit_code === "number") {
        return `exit ${obj.exit_code as number}`;
      }
      return null;
    },
  },

  // ─── read_file ───────────────────────────────────────────────────
  read_file: {
    argLabels: readFileArgs,
    parseStatus: (raw) => genericParseStatus(raw),
    renderBody: (raw) => {
      const obj = tryParseJsonObject(raw);
      // Formato común: {"content": "...", "lines": N, "truncated": bool}
      if (obj && typeof obj.content === "string") {
        const content = obj.content as string;
        return (
          <ToolCodeBlock
            code={content}
            language="text"
            className="tool-renderer__file-content"
            testId="tool-readfile-content"
          />
        );
      }
      // Fallback: el output ya es el contenido crudo.
      if (raw.trim().length === 0) return null;
      return (
        <ToolCodeBlock
          code={raw}
          language="text"
          className="tool-renderer__file-content"
          testId="tool-readfile-content"
        />
      );
    },
  },

  // ─── write_file ──────────────────────────────────────────────────
  write_file: {
    argLabels: writeFileArgs,
    parseStatus: (raw) => genericParseStatus(raw),
    caption: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && typeof obj.bytes_written === "number") {
        return `${obj.bytes_written as number} bytes`;
      }
      return null;
    },
  },

  // ─── edit_file ───────────────────────────────────────────────────
  edit_file: {
    argLabels: editFileArgs,
    parseStatus: (raw) => genericParseStatus(raw),
    caption: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && typeof obj.replacements === "number") {
        const n = obj.replacements as number;
        return `${n} replacement${n === 1 ? "" : "s"}`;
      }
      return null;
    },
  },

  // ─── grep ────────────────────────────────────────────────────────
  grep: {
    argLabels: grepArgs,
    parseStatus: (raw) => {
      // grep suele devolver "filename:line:match" lines, o un JSON
      // con {matches: [...]}. Si está vacío, status "empty" (sin error).
      if (raw.trim().length === 0) return { status: "empty" };
      return genericParseStatus(raw);
    },
    renderBody: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && Array.isArray(obj.matches)) {
        return (
          <ul className="tool-renderer__matches">
            {(obj.matches as unknown[]).slice(0, 100).map((m, i) => {
              if (!m || typeof m !== "object") return null;
              const entry = m as Record<string, unknown>;
              const file = entry.file ?? entry.path;
              const line = entry.line;
              const text = entry.text ?? entry.match ?? entry.content;
              return (
                <li key={i} className="tool-renderer__match">
                  <span className="tool-renderer__match-loc">
                    {String(file ?? "?")}
                    {typeof line === "number" ? `:${line}` : ""}
                  </span>
                  <span className="tool-renderer__match-text">
                    {String(text ?? "")}
                  </span>
                </li>
              );
            })}
          </ul>
        );
      }
      return null; // cae a SmartResult para el formato crudo
    },
  },

  // ─── glob ────────────────────────────────────────────────────────
  glob: {
    argLabels: globArgs,
    parseStatus: (raw) =>
      raw.trim().length === 0 ? { status: "empty" } : genericParseStatus(raw),
    renderBody: (raw) => {
      const lines = raw
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean);
      if (lines.length < 2) return null; // muy corto → no vale la pena
      return (
        <ul className="tool-renderer__paths">
          {lines.slice(0, 200).map((p, i) => (
            <li key={i} className="tool-renderer__path">
              {p}
            </li>
          ))}
          {lines.length > 200 && (
            <li className="tool-renderer__path-more">
              … +{lines.length - 200} more
            </li>
          )}
        </ul>
      );
    },
    caption: (raw) => {
      const lines = raw
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean);
      return `${lines.length} file${lines.length === 1 ? "" : "s"}`;
    },
  },

  // ─── list_dir ────────────────────────────────────────────────────
  list_dir: {
    argLabels: listDirArgs,
    parseStatus: (raw) =>
      raw.trim().length === 0 ? { status: "empty" } : genericParseStatus(raw),
  },

  // ─── web_search ──────────────────────────────────────────────────
  web_search: {
    argLabels: webSearchArgs,
    parseStatus: (raw) => genericParseStatus(raw),
    renderBody: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && Array.isArray(obj.results)) {
        const items = (obj.results as unknown[]).slice(0, 20);
        return (
          <ul className="tool-renderer__search-results">
            {items.map((it, i) => {
              if (!it || typeof it !== "object") return null;
              const e = it as Record<string, unknown>;
              return (
                <li key={i} className="tool-renderer__search-result">
                  {typeof e.title === "string" && (
                    <div className="tool-renderer__search-title">
                      {String(e.title)}
                    </div>
                  )}
                  {typeof e.url === "string" && (
                    <a
                      className="tool-renderer__search-url"
                      href={String(e.url)}
                      target="_blank"
                      rel="noreferrer"
                    >
                      {String(e.url)}
                    </a>
                  )}
                  {typeof e.snippet === "string" && (
                    <p className="tool-renderer__search-snippet">
                      {String(e.snippet)}
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
        );
      }
      return null;
    },
  },

  // ─── web_fetch ───────────────────────────────────────────────────
  web_fetch: {
    argLabels: webFetchArgs,
    parseStatus: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && typeof obj.status === "number") {
        const code = obj.status as number;
        return {
          status: code >= 200 && code < 300 ? "ok" : "fail",
          code,
        };
      }
      return genericParseStatus(raw);
    },
    caption: (raw) => {
      const obj = tryParseJsonObject(raw);
      if (obj && typeof obj.status === "number") {
        return `HTTP ${obj.status as number}`;
      }
      return null;
    },
  },

  // ─── symbols ─────────────────────────────────────────────────────
  symbols: {
    argLabels: symbolsArgs,
    parseStatus: (raw) =>
      raw.trim().length === 0 ? { status: "empty" } : genericParseStatus(raw),
  },

  // ─── aliases comunes (algunos backends los nombran distinto) ─
  shell_exec: { argLabels: shellArgs },
  shell_run: { argLabels: shellArgs },
  write: { argLabels: writeFileArgs },
  edit: { argLabels: editFileArgs },
  search: { argLabels: grepArgs },
};

// Lookup defensivo: si el tool no está en el registry, devolvemos un
// config vacío (cáe todo al SmartResult / JSON de args).
export function getToolConfig(tool: string) {
  return TOOL_REGISTRY[tool] ?? {};
}

// toolRenderers/types.ts — contratos del módulo de renderers de tools.
//
// El chat usa tres tipos de bloques: args (input), result (output) y
// status (ok/fail/empty). Cada tool del agente tiene su propio
// `ToolRenderConfig` que describe:
//
//   - `argLabels`   — cómo mostrar cada key del args (label + tipo de
//                     presentación: path, code, number, etc.). Keys
//                     desconocidas caen a un bloque JSON.
//   - `parseStatus` — dado el output crudo, devuelve ok | fail | empty
//                     (o null si no se puede determinar).
//   - `renderBody`  — render custom del body cuando el registry lo
//                     conoce (ej. shell separa stdout/stderr). Si no,
//                     el dispatcher cae al `SmartResult` actual.
//
// EP-2026-08-19: este módulo reemplaza la antigua presentación de args
// como JSON crudo y de result como JSON/diff/text sin status.

import type { ReactNode } from "react";

export type ResultStatus = "ok" | "fail" | "empty" | "interrupted";

export type ArgPresentation =
  | "path"
  | "code"
  | "multiline-code"
  | "number"
  | "duration"
  | "boolean"
  | "text";

export interface ArgLabelSpec {
  /** Texto a mostrar (ej. "Path", "Pattern", "Límite"). */
  label: string;
  /** Cómo formatear el valor. Default: "text". */
  presentation?: ArgPresentation;
  /** Si es true, el campo se oculta cuando el valor es vacío/null/undefined. */
  hideIfEmpty?: boolean;
}

export type ArgLabels = Record<string, ArgLabelSpec>;

export interface ParsedStatus {
  status: ResultStatus;
  /** Mensaje legible cuando hay error (ej. "permission denied", "404 not found"). */
  message?: string;
  /** Código numérico cuando existe (ej. exit code, HTTP status). */
  code?: number;
}

export interface ToolRenderConfig {
  /** Args label map. Keys ausentes caen a JSON genérico. */
  argLabels?: ArgLabels;
  /** Detecta el status del output. Default: intenta parsear JSON y
   *  busca ok/error/success, sino devuelve null (renderer cae al
   *  SmartResult existente). */
  parseStatus?: (output: string) => ParsedStatus | null;
  /** Renderer custom del body del result (después del status badge).
   *  Si devuelve null, el dispatcher cae a SmartResult. */
  renderBody?: (output: string, parsed: ParsedStatus | null) => ReactNode;
  /** Pequeño caption que aparece en el wrapper del tool, al lado del
   *  nombre (ej. "shell · exit 0", "write_file · 142 bytes"). */
  caption?: (output: string, parsed: ParsedStatus | null) => string | null;
}

export interface ToolRegistry {
  [toolName: string]: ToolRenderConfig;
}

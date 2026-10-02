// Tipos compartidos del cliente admin. Reflejan los shapes de la API del
// daemon neurox (EP-0001). Mantener sincronizados con la API real.

export interface Health {
  service: string;
  status: string;
  version: string;
  /** EP-0003-04 RNF-01: opcional en backends viejos. */
  started_at?: string;
  /** EP-0003-04 RNF-01: opcional en backends viejos. */
  uptime_seconds?: number;
  /** EP-0023-02 R10: true if the daemon requires a Bearer token.
   * Optional for backward compat with older daemon versions that
   * don't return this field. Default to false when undefined. */
  auth_required?: boolean;
}

export interface Agent {
  id: string;
  kind?: 'persistent' | 'ephemeral' | 'in_process';
  type?: string;
  status?: string;
  command?: string;
  protocol?: string;
  transport?: string;
  restart_policy?: string;
  requires_approval?: string[];
  approval_timeout_secs?: number;
  args?: string[];
  depends_on?: string[];
  env?: Record<string, string>;
}

export interface AgentsResponse {
  persistent: Agent[];
  ephemeral_templates: Agent[];
  running: Agent[];
  in_process: Agent[];
}

export interface SessionSummary {
  session_id: string;
  agent_id: string;
  /** EP-0017-02 R5: persisted session-level provider (null → daemon default). */
  provider_id?: string | null;
  /** EP-0017-02 R5: persisted session-level model (null → daemon default). */
  model?: string | null;
  started_at: string;
  ended_at?: string | null;
  summary?: string | null;
  /** Per-session client_id (web / sidebar-*). Set by the daemon. */
  client_id?: string | null;
}

export interface SessionsResponse {
  sessions: SessionSummary[];
}

export interface MessageMetrics {
  durationMs: number;
  tokens: number;
  tokensPerSec: number;
}

export interface Message {
  id: number;
  session_id: string;
  role: 'user' | 'assistant' | 'system' | 'tool';
  content: string;
  /** Assistant-only: streaming thinking block (the `<think>…</think>` portion). */
  thinking?: string;
  /** Assistant-only: tool calls/results accumulated during the response. */
  toolLog?: ToolActivity[];
  /** Assistant-only: pending/resolved approval requests. */
  approvals?: ApprovalActivity[];
  /**
   * EP-0024-UX: chronological event log for the assistant message.
   * When present, the renderer prefers this over the split
   * thinking/toolLog/approvals/content fields so the conversation
   * reads top-to-bottom in the exact order the agent emitted each
   * event (tool → result → think → reply → tool → result → …)
   * instead of grouping all tool activity above all text.
   */
  timeline?: TimelineEntry[];
  ts: string;
  metrics?: MessageMetrics;
}

/**
 * One row in an assistant message's timeline. Multiple chunks of
 * the same kind collapse into the previous entry of that kind
 * (so a long stream of 'content' chunks is a single growing entry,
 * not dozens of small ones).
 */
export type TimelineEntry =
  | { type: 'thinking'; text: string }
  | { type: 'tool'; tool: string; args?: unknown; result?: string; iteration: number }
  | {
      type: 'approval';
      id: string;
      tool: string;
      args?: unknown;
      reason?: string;
      decision?: 'approve' | 'deny';
    }
  | { type: 'content'; text: string };

export interface ToolActivity {
  tool: string;
  args?: unknown;
  result?: string;
  iteration: number;
}

export interface ApprovalActivity {
  id: string;
  tool: string;
  args?: unknown;
  reason?: string;
  decision?: "approve" | "deny";
}

export interface MessagesResponse {
  messages: Message[];
}

export interface Approval {
  id: string;
  tool: string;
  args?: unknown;
  requested_at?: string;
}

export interface ApprovalsResponse {
  pending: Approval[];
}

export interface ToolSpec {
  name: string;
  description: string;
  parameters: Record<string, unknown>;
  requires_approval: boolean;
}

export interface ToolsResponse {
  tools: ToolSpec[];
}

// Eventos del WebSocket /v1/events. El shape exacto puede variar; este
// es un union flexible que el componente LiveEventsPanel renderiza.
export type DaemonEvent = {
  type: string;
  [k: string]: unknown;
};

// ─── EP-0020-02: Admin UI complete ──────────────────────────────────

/** `GET /v1/llm/models/local` — GGUF files in MODELS_DIR. */
export interface LocalModel {
  filename: string;
  path: string;
  size_bytes: number;
  /**
   * Categoría del modelo: el subdirectorio bajo `NEUROX_MODELS_DIR` más
   * cercano al archivo (`chat`, `embedding`, …). `unclassified` cuando el
   * `.gguf` está en la raíz, sin carpeta que lo organice.
   *
   * La carpeta ES la categoría: es la organización que el operador ya tiene
   * en disco, y es más fiable que inferir del nombre del archivo (`bge-m3`
   * no dice "embedding": hay que saberlo).
   */
  category: string;
}

export interface LocalModelsResponse {
  /**
   * Directorio resuelto, o `null` si no existe. Distinguir esto de "vacío"
   * importa: si el directorio no existe, el consejo correcto es crearlo o
   * corregir `NEUROX_MODELS_DIR`, no descargar un modelo de 4GB.
   */
  dir: string | null;
  env_var: string;
  models: LocalModel[];
}

/** `PUT /v1/llm/models/local/{filename}/config` body. */
export interface ModelConfig {
  temperature?: number;
  top_p?: number;
  top_k?: number;
  max_tokens?: number;
  tokens_per_second?: number;
  stop_sequences?: string[];
  /**
   * El daemon lo llama `system_prompt`, no `system`. Con `system` el PUT
   * devolvía 200 y guardaba solo el resto: el system prompt se perdía en
   * silencio, sin error visible.
   */
  system_prompt?: string;
}

/** `GET /v1/llm/models/hf?search=...` — Hugging Face search results. */
export interface HfModel {
  id: string;
  display_name: string;
  author: string;
  downloads: number;
  gated: boolean;
  has_gguf: boolean;
  last_modified: string | null;
}

export interface HfSearchResponse {
  query: string;
  models: HfModel[];
}

/** `PUT /v1/sandbox` body and response. */
export interface SandboxConfig {
  enabled: boolean;
  /** Paths where write tools are allowed (read tools also work here). */
  writable_paths: string[];
  /** Paths where read tools are allowed but writes are rejected. */
  readable_paths: string[];
  /** Max recursion depth for glob/grep. */
  max_recursion_depth: number;
}

/**
 * `GET /v1/env` — catálogo de env vars del daemon, con el estado de cada
 * una. Los valores NUNCA se devuelven.
 *
 * `vars` es el catálogo: las variables que neurox reconoce, estén o no
 * escritas. Eso es lo que permite agregar una desde la UI — antes el
 * endpoint solo listaba lo que ya estaba en el archivo, así que no había
 * nada que agregar. `unknown_vars` son las que alguien puso a mano, fuera
 * del catálogo, y se editan igual.
 */
export interface EnvVar {
  key: string;
  /** ¿Está presente en el archivo o en el proceso? */
  set: boolean;
  /** Solo lectura: son de infraestructura o keys de providers. */
  readOnly: boolean;
  /** Guarda un secreto → input tipo password, y jamás se devuelve. */
  sensitive: boolean;
  /** Id de categoría. Ver `EnvResponse.categories`. */
  category: string;
  /** Nombre legible de la categoría, para el header del grupo. */
  categoryLabel: string;
  description: string;
  /** Default con el que arranca el daemon si no se setea. */
  defaultValue: string | null;
  /** ¿Está en el archivo? Distingue "set" de "hereda del host". */
  inFile: boolean;
  /** ¿Está en el entorno del proceso? */
  inEnv: boolean;
}

export interface EnvCategory {
  id: string;
  label: string;
  description: string;
}

export interface EnvResponse {
  path: string;
  vars: EnvVar[];
  unknownVars: EnvVar[];
  categories: EnvCategory[];
}

/** `GET /v1/services` — external services the daemon is monitoring. */
export interface ServiceStatus {
  id: string;
  name: string;
  kind: string;
  status: string;
  endpoint: string;
  latency_ms?: number;
  description?: string;
  version?: string;
  metadata?: Record<string, string>;
}

export interface ServicesResponse {
  services: ServiceStatus[];
  server_time: string;
}

/** `GET /v1/default/status` — in-process default status. */
export interface DefaultAgentContext {
  has_summary: boolean;
  messages: number;
  needs_compaction: boolean;
  tokens_estimated: number;
}

/** Sandbox scope as exposed by `/v1/default/status` (read-only snapshot). */
export interface DefaultAgentSandbox {
  enabled: boolean;
  writable_paths: string[];
  readable_paths: string[];
  max_recursion_depth: number;
}

export interface DefaultAgentResponse {
  context: DefaultAgentContext;
  facts: number;
  model: string;
  provider: string;
  skills: number;
  /** EP-0024: workspace path the agent is operating on. */
  cwd?: string;
  /** EP-0024: current git branch (null if not a git repo / detached HEAD). */
  git_branch?: string | null;
  /** EP-0024: sandbox scope currently applied to the agent. */
  sandbox?: DefaultAgentSandbox;
}


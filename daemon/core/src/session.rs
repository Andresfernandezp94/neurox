use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

pub struct SessionStore {
    pool: sqlx::SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub agent_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub summary: Option<String>,
    /// Provider elegido para esta sesión (None = default del daemon).
    pub provider_id: Option<String>,
    /// Modelo elegido para esta sesión (None = default del daemon).
    pub model: Option<String>,
    /// Cumulative tokens consumed by the agent for this session
    /// (populated from the agent's `tokens_out` field on each
    /// response). `None` if no dispatch has completed yet.
    pub tokens_used: Option<u64>,
    /// EP-0016: UI mode per session — `"plan"` | `"build"`. Affects
    /// system prompt suffix and tool availability. `None` = use daemon default.
    pub ui_mode: Option<String>,
    /// EP-0016: tool selection per session — `"functions"` | `"search"` | `"none"`.
    /// `None` = use daemon default.
    pub tool_mode: Option<String>,
    /// EP-0016: sampling temperature per session (0.0–2.0). `None` = daemon default.
    pub temperature: Option<f64>,
    /// Stable client id that opened the session (e.g. "web",
    /// "sidebar-<instance>"). Used for telemetry / WebSocket routing
    /// hints. **Not** used for session list partitioning anymore —
    /// sessions are shared across devices via `user_id`.
    pub client_id: Option<String>,
    /// User id (UUID) that owns the session. Sessions are shared
    /// across every device logged in as this user, so tabs sync
    /// realtime via the `/v1/events` WebSocket (which is filtered
    /// by user_id). `None` only for legacy rows persisted before the
    /// per-user migration — those won't appear in `list_sessions`
    /// until someone re-opens them (they're still reachable by id).
    pub user_id: Option<String>,
}

type SessionRow = (
    String,           // 0: session_id
    String,           // 1: agent_id
    String,           // 2: started_at
    Option<String>,   // 3: ended_at
    Option<String>,   // 4: summary
    Option<String>,   // 5: provider_id
    Option<String>,   // 6: model
    Option<u64>,      // 7: tokens_used
    Option<String>,   // 8: ui_mode
    Option<String>,   // 9: tool_mode
    Option<f64>,      // 10: temperature
    Option<String>,   // 11: client_id
    Option<String>,   // 12: user_id
);
// (id, session_id, role, content, thinking, ts, tool_name, tool_call_id)
//
// `tool_name` / `tool_call_id` son NULL para user y assistant: solo los
// mensajes con role="tool" los traen, y son lo que permite reconstruir el
// timeline (tool_call + su tool_result) al recargar la pagina.
type MessageRow = (
    i64,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    Option<String>,
);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    /// EP-0026-rev-fix: assistant-only thinking block (the `<think>…</think>`
    /// portion). `None` for user messages or for old rows persisted before
    /// the column was added (idempotent ALTER applied on next daemon start).
    pub thinking: Option<String>,
    pub ts: String,
    /// Nombre del tool, solo para `role == "tool"`. `None` en user/assistant.
    pub tool_name: Option<String>,
    /// Id del `tool_call` al que responde este resultado, si viene del
    /// dispatcher. Permite reconstruir el par call→result al rehidratar.
    pub tool_call_id: Option<String>,
}

/// Roles que emite el forwarder del stream, ademas de `user` y
/// `assistant`.
///
/// EP-2026-10-03: antes solo se persistian los dosUltimos, asi que
/// recargar la pagina durante un stream dejaba la sesion sin el
/// progreso del agente. Cada evento del bus se guarda como su propia
/// fila y el frontend los agrupa por `assistant` al rehidratar.
pub const ROLE_THINKING: &str = "thinking";
pub const ROLE_TOOL_CALL: &str = "tool_call";
pub const ROLE_TOOL: &str = "tool";

impl SessionStore {
    pub async fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = sqlx::SqlitePool::connect(&url).await?;

        // EP-0012 P-001: WAL mode for concurrent readers + writers.
        // PRAGMA WAL is idempotent — no-op if already set.
        sqlx::query("PRAGMA journal_mode=WAL")
            .execute(&pool)
            .await
            .ok();
        sqlx::query("PRAGMA synchronous=NORMAL")
            .execute(&pool)
            .await
            .ok();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                summary TEXT
             )",
        )
        .execute(&pool)
        .await?;

        // Migración idempotente EP-0017: columnas provider_id/model para
        // selección de modelo por sesión. NULL = usar default del daemon.
        Self::ensure_column(&pool, "sessions", "provider_id").await?;
        Self::ensure_column(&pool, "sessions", "model").await?;
        // EP-0027: tokens_used — cumulative tokens consumed by the
        // agent for this session. NULL = no dispatch yet.
        Self::ensure_column(&pool, "sessions", "tokens_used").await?;
        // EP-0016: per-session UI mode (plan vs build). Affects system
        // prompt suffix and tool availability. NULL = use daemon default.
        Self::ensure_column(&pool, "sessions", "ui_mode").await?;
        // EP-0016: per-session tool selection (functions | search | none).
        // NULL = use daemon default (typically "search" or "functions").
        Self::ensure_column(&pool, "sessions", "tool_mode").await?;
        // EP-0016: per-session sampling temperature. NULL = use daemon default.
        Self::ensure_column(&pool, "sessions", "temperature").await?;
        // Per-session client_id (web/sidebar/etc.) for telemetry / WS
        // routing hints. No longer used for list partitioning.
        Self::ensure_column(&pool, "sessions", "client_id").await?;
        // Per-user owner (UUID). Sessions are shared across every
        // device logged in as the same user; the `/v1/events` WS
        // filters by user_id so each device only sees its own.
        // NULL = legacy row persisted before the migration.
        Self::ensure_column(&pool, "sessions", "user_id").await?;
        // Index on user_id for `list_sessions_by_user` (the hot path
        // for tab syncing across devices). Without this, every tab
        // refresh does a full table scan.
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS sessions_user_id_idx \
             ON sessions(user_id)",
        )
        .execute(&pool)
        .await
        .ok();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                ts TEXT NOT NULL
             )",
        )
        .execute(&pool)
        .await?;

        // EP-0012 P-002: index on messages(session_id, id) for get_messages()
        // hot path. Without this, queries do full table scans proportional
        // to history size.
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS messages_session_id_idx \
             ON messages(session_id, id ASC)",
        )
        .execute(&pool)
        .await
        .ok();

        // EP-0026-rev-fix: thinking — assistant-only reasoning block
        // accumulated during streaming. NULL = no thinking o user msg.
        // MUST run after the messages CREATE TABLE so the table exists
        // for `pragma_table_info` to query its column list.
        Self::ensure_column(&pool, "messages", "thinking").await?;
        // EP-2026-10-03: persistir el progreso del agente. Sin esto un
        // F5 durante un stream dejaba la sesion con solo user+assistant:
        // los tool_call / tool_result vivian en el bus de eventos y en el
        // SSE, pero nunca llegaban a SQLite.
        Self::ensure_column(&pool, "messages", "tool_name").await?;
        Self::ensure_column(&pool, "messages", "tool_call_id").await?;

        Ok(Self {
            pool,
        })
    }

    /// Idempotent column add for SQLite (no `ADD COLUMN IF NOT EXISTS` before
    /// SQLite 3.35; we check `PRAGMA table_info` instead).
    /// The ALTER statement uses a static literal per known (table, column)
    /// pair (sqlx 0.9 requires `SqlSafeStr` — no dynamic SQL strings).
    /// Both the table name and the column name must match a known migration.
    async fn ensure_column(
        pool: &sqlx::SqlitePool,
        table: &str,
        column: &str,
    ) -> anyhow::Result<()> {
        let cols: Vec<String> = match table {
            "sessions" => {
                sqlx::query_scalar("SELECT name FROM pragma_table_info('sessions')")
                    .fetch_all(pool)
                    .await?
            }
            "messages" => {
                sqlx::query_scalar("SELECT name FROM pragma_table_info('messages')")
                    .fetch_all(pool)
                    .await?
            }
            other => anyhow::bail!("unexpected table in migration: {other}"),
        };
        if !cols.iter().any(|c| c == column) {
            let sql = match (table, column) {
                ("sessions", "provider_id") => {
                    "ALTER TABLE sessions ADD COLUMN provider_id TEXT"
                }
                ("sessions", "model") => "ALTER TABLE sessions ADD COLUMN model TEXT",
                ("sessions", "tokens_used") => {
                    "ALTER TABLE sessions ADD COLUMN tokens_used INTEGER"
                }
                // EP-0016: UI mode per session.
                ("sessions", "ui_mode") => {
                    "ALTER TABLE sessions ADD COLUMN ui_mode TEXT"
                }
                // EP-0016: tool selection per session.
                ("sessions", "tool_mode") => {
                    "ALTER TABLE sessions ADD COLUMN tool_mode TEXT"
                }
                // EP-0016: temperature per session.
                ("sessions", "temperature") => {
                    "ALTER TABLE sessions ADD COLUMN temperature REAL"
                }
                // Per-session client_id (web/sidebar/etc.).
                ("sessions", "client_id") => {
                    "ALTER TABLE sessions ADD COLUMN client_id TEXT"
                }
                // Per-user owner of the session (UUID string). Sessions
                // are shared across every device logged in as this user.
                ("sessions", "user_id") => {
                    "ALTER TABLE sessions ADD COLUMN user_id TEXT"
                }
                // EP-0026-rev-fix: thinking persisted per assistant message.
                ("messages", "thinking") => {
                    "ALTER TABLE messages ADD COLUMN thinking TEXT"
                }
                // EP-2026-10-03: progreso del agente persistido.
                ("messages", "tool_name") => {
                    "ALTER TABLE messages ADD COLUMN tool_name TEXT"
                }
                ("messages", "tool_call_id") => {
                    "ALTER TABLE messages ADD COLUMN tool_call_id TEXT"
                }
                _ => anyhow::bail!("unexpected migration: {table}.{column}"),
            };
            sqlx::query(sql).execute(pool).await?;
        }
        Ok(())
    }

    pub async fn start_session(&self, session_id: Uuid, agent_id: &str) -> anyhow::Result<()> {
                let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO sessions (session_id, agent_id, started_at) VALUES (?, ?, ?)",
        )
        .bind(session_id.to_string())
        .bind(agent_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Start a session owned by a specific user. Use this from
    /// handlers that have an authenticated `UserContext` (the
    /// user_id is what gates the per-user WS feed and the
    /// list_sessions filter).
    pub async fn start_session_for_user(
        &self,
        session_id: Uuid,
        agent_id: &str,
        user_id: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO sessions \
             (session_id, agent_id, started_at, user_id) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(session_id.to_string())
        .bind(agent_id)
        .bind(now)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Idempotent: store the client_id that opened this session
    /// (e.g. "web" or "sidebar-<instance>"). Used to partition session
    /// lists so each client only sees its own. `None` for legacy
    /// callers — those sessions show up under client_id == NULL and
    /// are visible to every client unless they specify a `?client_id`
    /// query param.
    pub async fn set_client_id(
        &self,
        session_id: Uuid,
        client_id: Option<&str>,
    ) -> anyhow::Result<()> {
        let result = sqlx::query("UPDATE sessions SET client_id = ? WHERE session_id = ?")
            .bind(client_id)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            anyhow::bail!("session not found: {session_id}");
        }
        Ok(())
    }

    pub async fn get_client_id(&self, session_id: Uuid) -> anyhow::Result<Option<String>> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT client_id FROM sessions WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.flatten())
    }

    pub async fn end_session(&self, session_id: Uuid, summary: Option<&str>) -> anyhow::Result<()> {
                let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE sessions SET ended_at = ?, summary = ? WHERE session_id = ?")
            .bind(now)
            .bind(summary)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Permanently delete a session and all its messages. Unlike
    /// `end_session` (which only marks `ended_at`), this removes the
    /// rows entirely so the session disappears from `list_sessions`.
    pub async fn delete_session_row(&self, session_id: Uuid) -> anyhow::Result<()> {
        let sid = session_id.to_string();
        sqlx::query("DELETE FROM messages WHERE session_id = ?")
            .bind(&sid)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM sessions WHERE session_id = ?")
            .bind(&sid)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_summary(&self, session_id: Uuid, summary: &str) -> anyhow::Result<()> {
                sqlx::query("UPDATE sessions SET summary = ? WHERE session_id = ? AND summary IS NULL")
            .bind(summary)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn log_message(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        thinking: Option<&str>,
    ) -> anyhow::Result<i64> {
        self.log_message_full(session_id, role, content, thinking, None, None)
            .await
    }

    /// Persiste un bloque de thinking (assistant) en su propia fila.
    ///
    /// Se guarda aparte del texto porque el SSE los manda en eventos
    /// distintos y el timeline los muestra en bloques distintos.
    pub async fn log_thinking(
        &self,
        session_id: Uuid,
        thinking: &str,
    ) -> anyhow::Result<i64> {
        self.log_message_full(session_id, "thinking", thinking, None, None, None)
            .await
    }

    /// Persiste la PETICION de una tool (`role = "tool_call"`).
    ///
    /// EP-2026-10-03. Antes el forwarder guardaba la peticion con
    /// `log_tool_message`, que escribe `role = "tool"`: en la base de datos
    /// no habia ni una sola fila `tool_call`, asi que el agrupador del
    /// cliente (`hydrateHistory`) nunca encontraba el par del que deducir
    /// los args, y tras recargar la pagina cada tool aparecia duplicada —
    /// una fila con el JSON de los args presentada como resultado, y otra
    /// con el resultado real.
    ///
    /// `tool_call_id` se guarda en las DOS filas (peticion y resultado) para
    /// que el emparejado sea por id y no por posicion.
    pub async fn log_tool_call_message(
        &self,
        session_id: Uuid,
        tool: &str,
        args_json: &str,
        tool_call_id: Option<&str>,
    ) -> anyhow::Result<i64> {
        self.log_message_full(
            session_id,
            ROLE_TOOL_CALL,
            args_json,
            None,
            Some(tool),
            tool_call_id,
        )
        .await
    }

    /// Igual que `log_message` pero con los campos de tool.
    ///
    /// EP-2026-10-03: `tool_name` / `tool_call_id` solo se llenan para
    /// `role == "tool"`. Se mantiene `log_message` como atajo porque los
    /// call-sites de user/assistant no tienen nada que pasar.
    pub async fn log_tool_message(
        &self,
        session_id: Uuid,
        tool: &str,
        content: &str,
        tool_call_id: Option<&str>,
    ) -> anyhow::Result<i64> {
        self.log_message_full(
            session_id,
            "tool",
            content,
            None,
            Some(tool),
            tool_call_id,
        )
        .await
    }

    async fn log_message_full(
        &self,
        session_id: Uuid,
        role: &str,
        content: &str,
        thinking: Option<&str>,
        tool_name: Option<&str>,
        tool_call_id: Option<&str>,
    ) -> anyhow::Result<i64> {
                let now = chrono::Utc::now().to_rfc3339();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO messages \
               (session_id, role, content, thinking, ts, tool_name, tool_call_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(session_id.to_string())
        .bind(role)
        .bind(content)
        .bind(thinking)
        .bind(now)
        .bind(tool_name)
        .bind(tool_call_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn list_sessions(&self, limit: u32) -> anyhow::Result<Vec<SessionRecord>> {
                let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT session_id, agent_id, started_at, ended_at, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id
             FROM sessions ORDER BY started_at DESC LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(sid, agent, started, ended, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id)| {
                    SessionRecord {
                        session_id: sid,
                        agent_id: agent,
                        started_at: started,
                        ended_at: ended,
                        summary,
                        provider_id,
                        model,
                        tokens_used,
                        ui_mode,
                        tool_mode,
                        temperature,
                        client_id,
                        user_id,
                    }
                },
            )
            .collect())
    }

    /// List sessions filtered by `client_id`. Sessions with
    /// `client_id == NULL` are treated as legacy and are NOT
    /// returned here — they're only visible via `list_sessions()`.
    /// **Deprecated for the main UI path** (sessions are now
    /// partitioned by user, not client). Kept for the diagnostics
    /// `GET /v1/services` view + back-compat with older clients.
    pub async fn list_sessions_by_client(
        &self,
        client_id: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<SessionRecord>> {
        let rows: Vec<SessionRow> = sqlx::query_as(
            "SELECT session_id, agent_id, started_at, ended_at, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id
             FROM sessions WHERE client_id = ? ORDER BY started_at DESC LIMIT ?",
        )
        .bind(client_id)
        .bind(i64::from(limit))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(sid, agent, started, ended, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id)| {
                    SessionRecord {
                        session_id: sid,
                        agent_id: agent,
                        started_at: started,
                        ended_at: ended,
                        summary,
                        provider_id,
                        model,
                        tokens_used,
                        ui_mode,
                        tool_mode,
                        temperature,
                        client_id,
                        user_id,
                    }
                },
            )
            .collect())
    }

    /// List sessions owned by `user_id` (the primary UI path).
    /// Sessions with `user_id IS NULL` are legacy rows from before
    /// the per-user migration — they're not returned here (no owner
    /// = no visibility); the user can still resume them by id via
    /// `reactivate_session` after they re-authenticate.
    ///
    /// `include_inactive = false` (default) hides closed sessions —
    /// those should show in a separate "history" view, not as tabs.
    pub async fn list_sessions_by_user(
        &self,
        user_id: &str,
        include_inactive: bool,
        limit: u32,
    ) -> anyhow::Result<Vec<SessionRecord>> {
        let sql = if include_inactive {
            "SELECT session_id, agent_id, started_at, ended_at, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id
             FROM sessions WHERE user_id = ? ORDER BY started_at DESC LIMIT ?"
        } else {
            // Pestanas activas: `ended_at IS NULL` Y con al menos un mensaje.
            //
            // El filtro de mensajes es lo que implementa "una sesion solo
            // persiste si se manda un mensaje". `POST /v1/sessions` crea el
            // registro al abrir la pestana, y si el usuario nunca escribe
            // queda con `ended_at = NULL` para siempre: la hidratacion del
            // front lo mapeaba a una ventana de chat y se acumulaban
            // ventanas vacias (9 sesiones, 8 sin un solo mensaje).
            //
            // Se filtra en la consulta y no en el front para que "activa"
            // tenga un solo significado en todo el sistema: el daemon, la
            // hidratacion y el WS consultan la misma definicion.
            "SELECT session_id, agent_id, started_at, ended_at, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id
             FROM sessions
             WHERE user_id = ? AND ended_at IS NULL
               AND EXISTS (SELECT 1 FROM messages WHERE messages.session_id = sessions.session_id)
             ORDER BY started_at DESC LIMIT ?"
        };
        let rows: Vec<SessionRow> = sqlx::query_as(sql)
            .bind(user_id)
            .bind(i64::from(limit))
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(
                |(sid, agent, started, ended, summary, provider_id, model, tokens_used, ui_mode, tool_mode, temperature, client_id, user_id)| {
                    SessionRecord {
                        session_id: sid,
                        agent_id: agent,
                        started_at: started,
                        ended_at: ended,
                        summary,
                        provider_id,
                        model,
                        tokens_used,
                        ui_mode,
                        tool_mode,
                        temperature,
                        client_id,
                        user_id,
                    }
                },
            )
            .collect())
    }

    /// Look up the user_id that owns a session (for WS event
    /// filtering). Returns `None` if the session doesn't exist or
    /// has no owner (legacy rows).
    pub async fn get_session_user(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Option<String>> {
        let row: Option<Option<String>> = sqlx::query_scalar(
            "SELECT user_id FROM sessions WHERE session_id = ?",
        )
        .bind(session_id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.flatten())
    }

    /// Reactivate (reopen) a previously closed session for the given
    /// user. Sets `ended_at` back to `NULL` so it shows in the
    /// active tabs again. Returns `true` if the session was
    /// reactivated, `false` if it doesn't exist or isn't owned by
    /// `user_id` (no error — caller decides what to do with the
    /// 404 case at the HTTP layer).
    pub async fn reactivate_session(
        &self,
        session_id: Uuid,
        user_id: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET ended_at = NULL \
             WHERE session_id = ? AND user_id = ?",
        )
        .bind(session_id.to_string())
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Set the LLM provider+model for a session (per-session selection, EP-0017).
    pub async fn set_model(
        &self,
        session_id: Uuid,
        provider_id: &str,
        model: &str,
    ) -> anyhow::Result<()> {
                let result =
            sqlx::query("UPDATE sessions SET provider_id = ?, model = ? WHERE session_id = ?")
                .bind(provider_id)
                .bind(model)
                .bind(session_id.to_string())
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            anyhow::bail!("session not found: {session_id}");
        }
        Ok(())
    }

    /// Get the LLM provider+model for a session. `Ok(None)` means the session
    /// has no explicit selection (use the daemon default).
    pub async fn get_model(&self, session_id: Uuid) -> anyhow::Result<Option<(String, String)>> {
                let row: Option<(Option<String>, Option<String>)> =
            sqlx::query_as("SELECT provider_id, model FROM sessions WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        match row {
            Some((Some(provider_id), Some(model))) => Ok(Some((provider_id, model))),
            _ => Ok(None),
        }
    }

    /// Cumulative tokens used by the agent for this session. `None`
    /// if no dispatch has completed yet.
    pub async fn get_tokens_used(&self, session_id: Uuid) -> anyhow::Result<Option<u64>> {
                let row: Option<Option<i64>> =
            sqlx::query_scalar("SELECT tokens_used FROM sessions WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.flatten().and_then(|v| u64::try_from(v).ok()))
    }

    /// Update the cumulative tokens used by the agent for this session.
    /// Used by the dispatcher after each agent response to surface
    /// real LLM usage in `/health`.
    pub async fn set_tokens_used(
        &self,
        session_id: Uuid,
        tokens_used: u64,
    ) -> anyhow::Result<()> {
                sqlx::query("UPDATE sessions SET tokens_used = ? WHERE session_id = ?")
            .bind(i64::try_from(tokens_used).unwrap_or(i64::MAX))
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ─── EP-0016: per-session UI config (mode / tool-mode / temperature) ──

    /// Set the UI mode for a session. `"plan"` | `"build"`. Affects system
    /// prompt suffix and tool availability. `None` clears (use daemon default).
    pub async fn set_ui_mode(
        &self,
        session_id: Uuid,
        ui_mode: Option<&str>,
    ) -> anyhow::Result<()> {
        if let Some(m) = ui_mode {
            if m != "plan" && m != "build" {
                anyhow::bail!("ui_mode must be 'plan' or 'build', got '{m}'");
            }
        }
        let result = sqlx::query("UPDATE sessions SET ui_mode = ? WHERE session_id = ?")
            .bind(ui_mode)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            anyhow::bail!("session not found: {session_id}");
        }
        Ok(())
    }

    /// Get the UI mode for a session. `Ok(None)` means use daemon default.
    pub async fn get_ui_mode(&self, session_id: Uuid) -> anyhow::Result<Option<String>> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT ui_mode FROM sessions WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.flatten())
    }

    /// Set the tool selection mode for a session. `"functions"` | `"search"` | `"none"`.
    /// `None` clears (use daemon default).
    pub async fn set_tool_mode(
        &self,
        session_id: Uuid,
        tool_mode: Option<&str>,
    ) -> anyhow::Result<()> {
        if let Some(m) = tool_mode {
            if !["functions", "search", "none"].contains(&m) {
                anyhow::bail!("tool_mode must be 'functions', 'search', or 'none', got '{m}'");
            }
        }
        let result = sqlx::query("UPDATE sessions SET tool_mode = ? WHERE session_id = ?")
            .bind(tool_mode)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            anyhow::bail!("session not found: {session_id}");
        }
        Ok(())
    }

    /// Get the tool selection mode for a session. `Ok(None)` means use daemon default.
    pub async fn get_tool_mode(&self, session_id: Uuid) -> anyhow::Result<Option<String>> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT tool_mode FROM sessions WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.flatten())
    }

    /// Set the sampling temperature for a session. Valid range: 0.0–2.0.
    /// `None` clears (use daemon default).
    pub async fn set_temperature(
        &self,
        session_id: Uuid,
        temperature: Option<f64>,
    ) -> anyhow::Result<()> {
        if let Some(t) = temperature {
            if !(0.0..=2.0).contains(&t) {
                anyhow::bail!("temperature must be in 0.0..=2.0, got {t}");
            }
        }
        let result = sqlx::query("UPDATE sessions SET temperature = ? WHERE session_id = ?")
            .bind(temperature)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            anyhow::bail!("session not found: {session_id}");
        }
        Ok(())
    }

    /// Get the temperature for a session. `Ok(None)` means use daemon default.
    pub async fn get_temperature(&self, session_id: Uuid) -> anyhow::Result<Option<f64>> {
        let row: Option<Option<f64>> =
            sqlx::query_scalar("SELECT temperature FROM sessions WHERE session_id = ?")
                .bind(session_id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.flatten())
    }

    /// Get all messages for a session, ordered chronologically.
    pub async fn get_messages(&self, session_id: Uuid) -> anyhow::Result<Vec<MessageRecord>> {
                let rows: Vec<MessageRow> = sqlx::query_as(
            "SELECT id, session_id, role, content, thinking, ts, tool_name, tool_call_id
             FROM messages WHERE session_id = ? ORDER BY id ASC",
        )
        .bind(session_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, sid, role, content, thinking, ts, tool_name, tool_call_id)| MessageRecord {
                    id,
                    session_id: sid,
                    role,
                    content,
                    thinking,
                    ts,
                    tool_name,
                    tool_call_id,
                },
            )
            .collect())
    }

    /// Rename a session (force-update summary regardless of current value).
    pub async fn rename_session(&self, session_id: Uuid, name: &str) -> anyhow::Result<()> {
                sqlx::query("UPDATE sessions SET summary = ? WHERE session_id = ?")
            .bind(name)
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn store() -> (TempDir, SessionStore) {
        let tmp = TempDir::new().unwrap();
        let s = SessionStore::open(&tmp.path().join("sessions.db")).await.unwrap();
        (tmp, s)
    }

    /// `include_inactive = false` define las pestanas activas. Una sesion
    /// que se creo pero nunca recibio un mensaje NO es una pestana: queda
    /// `ended_at = NULL` para siempre y, sin este filtro, cada ventana de
    /// chat abierta sin escribir dejaba una sesion activa permanente que
    /// la hidratacion del front convertia en otra ventana al recargar.
    #[tokio::test]
    async fn active_list_excludes_sessions_without_messages() {
        let (_tmp, s) = store().await;
        let used = Uuid::new_v4();
        let empty = Uuid::new_v4();
        s.start_session_for_user(used, "default", "u1").await.unwrap();
        s.start_session_for_user(empty, "default", "u1").await.unwrap();
        s.log_message(used, "user", "hola", None).await.unwrap();

        let active = s.list_sessions_by_user("u1", false, 50).await.unwrap();
        let ids: Vec<String> = active.iter().map(|r| r.session_id.clone()).collect();
        assert!(ids.contains(&used.to_string()), "la sesion con mensaje va");
        assert!(
            !ids.contains(&empty.to_string()),
            "la sesion sin mensaje NO va: no llego a usarse"
        );
    }

    /// El filtro es solo de la lista de activas: con `include_inactive`
    /// la sesion vacia sigue existiendo y se puede recuperar.
    #[tokio::test]
    async fn include_inactive_still_returns_empty_sessions() {
        let (_tmp, s) = store().await;
        let empty = Uuid::new_v4();
        s.start_session_for_user(empty, "default", "u1").await.unwrap();

        let all = s.list_sessions_by_user("u1", true, 50).await.unwrap();
        assert!(all.iter().any(|r| r.session_id == empty.to_string()));
    }

    /// Cerrar la sesion la saca de las activas aunque tenga mensajes, y
    /// con un mensaje sola no basta para reabrirla como pestana.
    #[tokio::test]
    async fn ended_session_with_messages_is_not_active() {
        let (_tmp, s) = store().await;
        let sid = Uuid::new_v4();
        s.start_session_for_user(sid, "default", "u1").await.unwrap();
        s.log_message(sid, "user", "hola", None).await.unwrap();
        assert_eq!(s.list_sessions_by_user("u1", false, 50).await.unwrap().len(), 1);

        s.end_session(sid, Some("listo")).await.unwrap();
        assert!(s.list_sessions_by_user("u1", false, 50).await.unwrap().is_empty());
        assert_eq!(s.list_sessions_by_user("u1", true, 50).await.unwrap().len(), 1);
    }
}

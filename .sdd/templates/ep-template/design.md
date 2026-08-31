# EP-{{NNNN}} — Design

> **Status**: draft:design
> **Created**: {{YYYY-MM-DD}}

## Decisiones técnicas

[REQUIRED: Lista de decisiones con tradeoffs. Una por línea. Si la decisión
merece explicación larga, mové el detalle a un ADR (`decisions/`) y dejá
solo el resumen acá.]

| Decisión | Tradeoffs | Por qué se eligió |
|----------|-----------|-------------------|
| Usar SSE en vez de WebSocket para streaming de chat | SSE: unidireccional, simple, reconexión automática vía `EventSource`. WS: bidireccional, requiere mantener conexión por cliente. | El streaming de chat es server-pushed, el cliente solo envía al final (Enter). SSE cubre el 100% del caso. |
| `{ resultado: { text, thinking } }` en respuestas de `/v1/sessions/:id/messages` | Wrapper anidado vs flat `{ text, thinking }` | Wrapper coincide con el formato de `llmd_client::ChatResponse` para evitar transformación. |

## Contratos

[REQUIRED: APIs, schemas, eventos afectados. Pegá los snippets reales (no
"similar a X"). Esto es lo que firma los contratos.]

### HTTP

```
POST /v1/sessions/:id/messages
Content-Type: application/json

{
  "agent_id": "default",
  "text": "user message"
}
→ 200
{
  "session_id": "...",
  "agent_id": "...",
  "result": { "text": "...", "thinking": null }
}
```

### Eventos

```rust
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    SessionStarted { session_id: Uuid, agent_id: String },
    MessageReceived { session_id: Uuid, content: String },
    // ...
}
```

### Tipos TypeScript

```ts
export interface SessionStartedEvent {
  type: "session_started";  // snake_case en runtime, kebab en código
  session_id: string;
  agent_id: string;
}
```

## Dependencias

[OPTIONAL: Otras épicas, librerías, infra externa.]

- **Bloqueado por**: EP-0008 (realtime-chat-streaming — mismo flujo SSE)
- **Bloquea**: EP-0009 (voice streaming consume el mismo endpoint)
- **Librerías nuevas**: `axum-stream`, `eventsource-parser`

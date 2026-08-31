# HANDOFF — Unificación neurox (daemon + sidebar Quickshell + web)

> Estado para retomar en sesión limpia. Fecha: 2026-08-31.
> Objetivo global del usuario: **el daemon es el núcleo único**; el sidebar de
> Quickshell y el cliente web son thin clients que comparten datos (sesiones,
> providers, API keys, modelos, agentes, tools) desde el daemon, y comparten
> look & feel (el sidebar es la referencia visual; la web se alinea a él).

---

## Ubicaciones clave

| Qué | Ruta |
|-----|------|
| Daemon (Rust) | `/home/andres_fernandez/projects/neurox/daemon` |
| Cliente web (React/Vite/TS) | `/home/andres_fernandez/projects/neurox/client/web` |
| Sidebar Quickshell (config `ii`) | `/home/andres_fernandez/.config/quickshell/ii` |
| Binario daemon instalado | `~/.local/bin/neurox` (backups `neurox.bak-*`) |
| Estado persistente sidebar | `~/.local/state/quickshell/states.json` |
| Env del daemon (API keys) | `~/.config/neurox/env` (0600) |
| SVGs providers sidebar | `~/.config/quickshell/ii/assets/providers/` |
| SVGs nav sidebar | `~/.config/quickshell/ii/assets/nav/` (sessions.svg, workspace.svg) |

## Procesos en vivo (al momento del handoff)
- Daemon: corriendo, health 200 (`~/.local/bin/neurox serve`). **Simplificado** (sin db/logs/monitoreo/capabilities/metrics/memory-proxy; sin clients/tui). 0 errores, 0 warnings de compilación.
- Vite dev server: `http://localhost:5173` (HMR activo).
- Quickshell: corriendo config `ii`. Recargar con: matar el PID de `quickshell -p .../ii/shell.qml` y relanzar `setsid quickshell -p ~/.config/quickshell/ii/shell.qml`.

## IMPORTANTE: no hay git
Se eliminó todo el `.git` del proyecto y los repos remotos de GitHub (a pedido del usuario). **No hay control de versiones** — los cambios son directos en disco, sin rollback. Backups puntuales: `states.json.bak`, `neurox.bak-*`.

---

## HECHO (verificado)

### Daemon
- **Simplificado**: eliminados dominios db-CRUD, logs, observability/system-stats, prometheus metrics, capabilities, memory-proxy admin. Módulos borrados: `metrics.rs`, `capabilities.rs`, `events_log.rs`, `observability/`, `router/state/observability.rs`. `clients/` (tui+web legacy) borrado y sacado del workspace `Cargo.toml`.
- **Cleanup de código muerto**: structs legacy en `agent-lib/llm.rs`, bloque DB-CRUD en `session.rs`, funciones huérfanas (health_check_all, kill_all_persistent, etc.). 0 warnings.
- **Fix `configured`**: `catalog_is_configured_field` ahora verifica que el env var tenga valor real (antes daba true solo por tener nombre).
- **Fix DELETE sesión**: `delete_session` ahora borra la fila (nuevo `SessionStore::delete_session_row`, borra messages+session), antes solo hacía soft-close (`end_session`). Verificado: la sesión desaparece de `GET /v1/sessions`.
- **7 providers registrados** (persisten en SQLite `llm_providers`): anthropic, grok, meta, minimax, mistral, openai, openrouter. Configurados con key real: anthropic, minimax, mistral, openrouter (según el usuario).
- Test preexistente que falla (NO por estos cambios): `cancel_endpoint_marks_session_inactive` (el helper de test no registra el agente "default") y flaky `llm_admin::orchestrator::test_shutdown_all`.

### Sidebar Quickshell — conectado al daemon
- Nueva estrategia `services/ai/NeuroxApiStrategy.qml`: habla el protocolo del daemon (crea/reusa sesión, streamea `/v1/sessions/:id/messages/stream` SSE, parsea events content/thinking/tool_call/tool_result/done/error). Sesión cacheada en `/tmp/quickshell/ai/neurox_session`.
- `services/Ai.qml`: modelo "neurox" registrado (`api_format:"neurox"`, endpoint `http://127.0.0.1:7878`). `registerNeuroxModel()` fuerza neurox como default cuando el persistido no existe. Funciones añadidas:
  - `refreshNeuroxProviders()` / `neuroxProviders` (+ default provider/model)
  - `refreshNeuroxSessions()` / `neuroxSessions`, `loadNeuroxSession`, `hydrateNeuroxMessages`
  - `saveNeuroxKey(envVar,value)` → `PUT /v1/env/{key}` (body JSON en base64→curl --data @-); `clearNeuroxKey` → `DELETE /v1/env/{key}`
  - `deleteNeuroxSession`, `renameNeuroxSession`, `cancelNeuroxSession`
- `modules/common/Persistent.qml`: añadido `property string mode: "plan"` al schema `ai` (faltaba, causaba error runtime en Ai.qml:103).
- Modal de providers (`AiChatProviderModal.qml`): muestra providers del daemon con logo SVG real, badge ACTIVE, kind•model, endpoint, estado de key, campo password + guardar + limpiar. Lee de `Ai.neuroxProviders`.
- Modal de sesiones (`AiChatHistoryModal.qml`): lista sesiones del daemon con búsqueda, chips de filtro (All/Active/Closed con contadores), y por tarjeta: abrir, detener (cancel, solo activas), renombrar (inline), borrar. Ícono cloud_sync/cloud_done según estado.
- Status bar (`AiChatStatusBar.qml` + `StatusItem.qml`): ícono de sesiones usa SVG de la web (assets/nav/sessions.svg vía ColorOverlay tintado); añadido botón workspaces (assets/nav/workspace.svg) — su handler es placeholder ("coming soon").
- **Estado actual del usuario**: `states.json` → `{model: neurox, mode: build, neuroxAgent: default, temperature: 0.5}`.

### Web — alineada al sidebar
- **Providers** (`ProvidersPanel.tsx`): reconstruido con design system atómico (`Card`, `Row`, `Badge`, `IconButton`, `Input`) — NO clases custom, NO !important. Tarjeta estilo modal del sidebar: logo + nombre + badge ACTIVE (variante `.badge--active`: fondo acento sólido, texto oscuro, bold, recto), kind•model, endpoint mono, estado key, Input password + IconButton save/clear. Borde acento en activa (`.provider-card--active`). Botones de icono centrados (36px, iconos centrados vía `.icon` span flex). Input mono altura 36px. Card flex-column gap 6px. Lee providers del daemon; guarda/limpia keys vía `PUT/DELETE /v1/env`.
- **Providers ahora es vista principal** en Config (se quitaron las sub-tabs Models/Raw; solo queda providers). Se quitó el wrapper `.panel` (evita doble padding dentro de ConfigViewer). Se quitó el botón inútil "Set active".
- **Sesiones** (`SessionList` CSS en atoms.css): alineado al look del sidebar — tarjetas radius 0, búsqueda M3, chips de filtro con activo en acento sólido, botones de acción rectos, id/fecha mono. Sin !important, paleta web conservada.
- **Paleta de colores de la web: INTOCABLE** (el usuario lo dejó claro; se revirtió un intento de cambiarla). Solo se alinean bordes/estructura/componentes, NO colores. `tokens.css` mantiene `--accent: #efb05e` (ámbar).
- Bordes rectos globales: `.card`, `.input`, `.btn` tienen `border-radius: 0` en su definición base (no !important).
- `api/env.ts`: añadido `deleteEnvVar`.

---

## PENDIENTE (lo que se estaba haciendo al cortar)

### Tarea en curso: selectores de modelos y agentes en la sidebar
El usuario pidió (última instrucción antes del handoff):
1. Llevar el estilo del **dropdown de modelos de la web** (ModelSelector: grupos colapsables por provider, con logo, contador, caret) a la **sidebar**.
2. Añadir el **selector de agentes** al lado, con su estilo.
3. **Sincronizar datos**: la sidebar debe mostrar los **modelos disponibles de los providers del daemon** (`GET /v1/llm/models`, ~451 modelos, agrupar por `provider_id`) y los **agentes** (`GET /v1/agents`) — igual que la web.

**Estado**: se estaba por lanzar un subagente developer con el plan completo (fue cancelado por el usuario). PERO el `states.json` ya tiene `neuroxAgent: default`, lo que sugiere que ALGO del selector de agentes pudo haberse empezado — **verificar en sesión limpia** si `services/Ai.qml` ya tiene `currentNeuroxAgent`/`refreshNeuroxAgents`/`refreshNeuroxModels` antes de re-implementar.

**Plan detallado para retomar** (contratos del daemon verificados):
- `GET /v1/llm/models` → `{"models":[{model_id, provider_id, kind, base_url, supports_tools, capability}]}`. Agrupar por `provider_id`.
- `GET /v1/agents` → `{ephemeral_templates:[{id,...}], in_process:[], persistent:[{id,...}], running:[]}`. El id es `id`. Agente chat por defecto: `"default"`.
- `PUT /v1/llm/providers/active` con body `{"id": providerId}` para fijar el provider activo (verificar body real en `daemon/core/src/router/http.rs::set_active_llm_provider`).
- En `services/Ai.qml`: añadir `neuroxModels`/`refreshNeuroxModels()`, `neuroxAgents`/`refreshNeuroxAgents()`, `currentNeuroxAgent` (persistir en states.ai.neuroxAgent — schema en Persistent.qml puede necesitar la property `neuroxAgent`), `setNeuroxActiveModel(providerId, modelId)`. Llamar los refresh en Component.onCompleted y Config.onReadyChanged.
- `NeuroxApiStrategy.buildRequestData`: usar `Ai.currentNeuroxAgent` en vez del literal `"default"` en `agent_id`.
- UI: en `AiChat.qml` (~líneas 980-1100, dropdown que usa `root.groupedModels`), cuando `Ai.currentApiFormat==="neurox"` mostrar `Ai.neuroxModels` agrupados por provider (grupos colapsables con logo del provider + contador, items = model_id, resaltar activo con `Appearance.colors.colPrimary`). Al click → `Ai.setNeuroxActiveModel(providerId, modelId)`.
- Añadir selector de agentes al lado (dropdown que liste `Ai.neuroxAgents` por id, al elegir setea `Ai.currentNeuroxAgent`).
- Look: widgets del sidebar (RippleButton, StyledText, MaterialSymbol, Appearance, bordes rectos radius 0). Logos de provider vía `Qt.resolvedUrl("../../../assets/providers/...")` como en `AiChatProviderModal.logoForProvider`.
- Verificar: qmllint (ignorar warnings unqualified/imports), balance de llaves, probar curls. NO reiniciar quickshell (lo hace el usuario).

### Otros pendientes menores
- Botón "workspaces" del sidebar tiene handler placeholder — definir qué hace (¿vista de workspaces del daemon?).
- `MINIMAX_API_KEY` tuvo keys de prueba durante debugging; el usuario ya puso keys reales en varios providers.
- Idea futura del usuario: `neurox/design/` como fuente única de tokens/SVGs compartidos entre web y sidebar (design system). No iniciado.

---

## Reglas de trabajo aprendidas (del usuario)
- **NO cambiar la paleta de colores de la web.** Solo bordes/estructura/componentes.
- **NO usar `!important`.** Usar el design system atómico (Card/Button/Input/Badge/IconButton + clases utilitarias). Ajustar clases base o scoped, no overrides forzados.
- El **sidebar es la referencia visual**; la web se alinea a él (bordes rectos M3, badge ACTIVE con acento sólido, iconos en botones).
- La UI nueva del sidebar solo se ve en **modo neurox** (`Ai.currentApiFormat==="neurox"`), que es el default. Si "no se ve", verificar el modelo activo en states.json.
- Verificar siempre: qmllint (sidebar), tsc --noEmit (web, ignorar 2 preexistentes en ChatPanel.timeline.test.tsx), transform vía curl al dev server.

---

## UPDATE (misma sesión, tras el handoff): selectores de modelos/agentes — COMPLETADO

- **Lógica ya estaba** en `services/Ai.qml` (el subagente cancelado alcanzó a escribirla): `neuroxModels`, `groupedNeuroxModels`, `neuroxAgents`, `currentNeuroxAgent` (persistido en `states.ai.neuroxAgent`), `setNeuroxAgent`, `setNeuroxActiveModel` (PUT `/v1/llm/providers/active` body `{provider_id}`). `NeuroxApiStrategy` usa `Ai.currentNeuroxAgent` como `agent_id`.
- **FIX aplicado**: `refreshNeuroxModels` usaba el catálogo global `/v1/llm/models` que solo devolvía 3 providers (los con discovery OK). Reescrito para iterar **por provider configurado** (`GET /v1/llm/providers/{id}/models`) como la web, con **fallback al modelo configurado** cuando el discovery da vacío (caso anthropic). El endpoint por-provider devuelve `models` como **array de strings** (no objetos) — parser corregido. Encadenado: `getNeuroxProviders.onExited` → `refreshNeuroxModels()`. Verificado: anthropic 1, minimax 8, mistral 48, openrouter 395.
- **UI dropdown de agentes AÑADIDA** en `AiChat.qml` (`agentSelector` al lado del `modelSelector`, visible solo en modo neurox): popup que lista `Ai.neuroxAgents`, resalta el actual, al elegir llama `Ai.setNeuroxAgent`. qmllint OK, llaves balanceadas (347/347).
- Falta que el usuario **recargue Quickshell** para ver los cambios.
- Providers sin key (grok/meta/openai) NO aparecen en el selector (esperado: sin credenciales no hay discovery). Si el usuario quiere que aparezcan igual con su modelo configurado, ampliar el filtro en `refreshNeuroxModels` para incluir no-configurados con fallback (hoy solo itera `configured`).

---

## UPDATE 2: fixes de modelos (anthropic, letras, SplitParser)
- **Bug SplitParser**: `refreshNeuroxModels` acumulaba líneas sin `\n` (SplitParser los quita) → parse fallaba → lista vacía. Fix: parsear cada línea en `onRead` con `_acc.push()` in-place, volcar a `neuroxModels` en `onExited`.
- **Bug anthropic base_url duplicado**: el provider anthropic tenía `base_url=https://api.anthropic.com/v1` y el discovery hace `{base_url}/v1/models` → `/v1/v1/models` → 404 → 0 modelos. Fix: `PUT /v1/llm/providers/anthropic {base_url:"https://api.anthropic.com"}` (SIN /v1). Ahora trae 10 modelos reales. NOTA: la key de Anthropic debe estar asociada a un workspace (el endpoint /v1/models exige workspace-id incluso para keys "all"); el usuario generó una key con workspace default → funciona.
- **Bug de "letras en vez de modelos"**: el fallback hacía `ms=[fb]` donde `fb` ya era una lista → estructura mal; y si fb fuera string, `for mid in ms` iteraba char por char. Fix: `if not ms and isinstance(fb,list): ms=list(fb)` + serializar solo `isinstance(mid,str)`.
- Verificado: anthropic 10, minimax 8, mistral 48, openrouter 424, sin letras. qmllint OK.
- **NO se hardcodean modelos** (regla del usuario) — todos vienen del fetch por-provider al daemon. El fallback solo usa el modelo configurado del provider si el discovery da vacío.
- Pendiente: recargar Quickshell para ver los modelos en la sidebar.

---

## UPDATE 3: UNIFICACIÓN DEL CHAT + MULTI-SESIÓN CONCURRENTE (2026-08-31) ✅

Objetivo del usuario: (1) que el chat de la web y el sidebar manejen las mismas
propiedades/contrato, y (2) que el daemon pueda lanzar **múltiples sesiones
independientes y concurrentes** (ej. una en la web y otra en el sidebar a la vez).

### Causa raíz #1 — el agente `default` era un subproceso ÚNICO compartido
En `~/.config/neurox/config.yaml`, `default` estaba bajo `agents.persistent`:
un solo subproceso `agent` compartido por TODAS las sesiones (path
`send_chat_persistent`, serializado por un lock de stdin por-agente). Cuando ese
proceso moría (`Unhealthy: subprocess not running`, y `restart_policy: on-failure`
NO lo reiniciaba), **ni la web ni el sidebar recibían respuesta**, y jamás hubo
paralelismo real.

**FIX**: mover `default` a la sección `session_agents.agents` del config. Con eso,
`create_session` y `dispatch_to_agent` chequean `is_session_agent("default")`
PRIMERO y toman el path `start_for_session` → **un subproceso `agent` dedicado por
cada sesión** (mapa `SessionAgentPool.agents`, aislamiento e idle-eviction 30 min).
- `agents.persistent` quedó `[]` (ya no hay proceso compartido muerto).
- **Ruta ABSOLUTA obligatoria** en `command`: `/home/andres_fernandez/.local/bin/agent`.
  El daemon corre bajo **systemd** (`~/.config/systemd/user/neurox.service`,
  `Restart=always`, `EnvironmentFile=~/.config/neurox/env`), cuyo PATH mínimo NO
  incluye `~/.local/bin` → un `command: "agent"` a secas falla con
  `No such file or directory (os error 2)`.

### Causa raíz #2 — el agente colgaba tras streamear (nunca cerraba el turno)
`crates/agent-lib/src/llm.rs` → `chat_stream`: el loop
`while let Ok(evt) = event_rx.recv().await` solo rompía con `BEvt::Done`, pero el
backend MiniMax (y OpenAI-compat) **NUNCA emite `Done`** (solo `Content`/`Thinking`
y retorna). El `event_tx` externo no se dropeaba → el canal broadcast nunca
cerraba → `recv()` esperaba para siempre → `handle_process` no escribía su
respuesta JSON-RPC final → el daemon no emitía `Event::Done` → **no `[DONE]` →
clientes en "connecting…"/spinner infinito** (60s hasta timeout).

**FIX**: `drop(event_tx)` justo después de spawnear la tarea del backend. Así, al
terminar la tarea, su clon del sender se dropea, el canal cierra, `recv()` devuelve
`Err(Closed)`, el loop termina y el agente escribe su respuesta final.
- Recompilado: `cargo build --release -p agent` (OK). Instalado en
  `~/.local/bin/agent` (backup `agent.bak-*`, binario nuevo ~5.28 MB).
- **CUIDADO al instalar**: el binario queda "text file busy" si hay subprocesos
  `agent` vivos. Procedimiento: `systemctl --user stop neurox` →
  `kill -9` de agents huérfanos (`pgrep -x agent`) → `cp target/release/agent
  ~/.local/bin/agent` → `systemctl --user start neurox`.

### Causa raíz #3 (web) — crash del ModelSelector impedía cargar modelos
`ModelSelector.tsx:242` lanzaba `TypeError: Cannot read properties of undefined
(reading 'localeCompare')` en el `Array.sort`. El daemon devuelve
`GET /v1/llm/providers/:id/models` como **array de STRINGS** (`["MiniMax-M3", ...]`),
pero `listModels()` en `api/llm.ts` los tipaba como `DiscoveredModel[]` (objetos con
`.id`) → `m.id` undefined → `model_id` undefined → crash en el sort.

**FIX**:
- `api/llm.ts::listModels`: normaliza ambos shapes (string → `{id}`, objeto con
  `id` o `model_id`), descarta entradas sin id.
- `ModelSelector.tsx` (defensivo): el `useMemo grouped` salta entradas sin
  `model_id`/`provider_id` y usa `(x ?? "").localeCompare`.
- La web YA enviaba `{agent_id, text}` en `streamMessage` (`api/sessions.ts`) y
  parsea SSE resolviendo en `[DONE]`/`done` — contrato correcto, sin cambios.

### Contrato unificado del stream (web y sidebar idénticos)
`POST /v1/sessions/:id/messages/stream` con body **`{"agent_id": "...", "text": "..."}`**
(NO `content`). Eventos SSE: `data: {"type":"thinking"|"content"|"tool_call"|
"tool_result"|"error", ...}` y cierre por fin de conexión (+ `Event::Done` cuando el
dispatch retorna). El sidebar (`NeuroxApiStrategy`) y la web (`streamMessage`)
usan el mismo contrato.

### Verificación (todo con el daemon corregido bajo systemd)
- Stream de una sesión: termina en **1s** (antes 60s timeout). Emite thinking + content.
- **Paralelismo real**: 2 sesiones concurrentes → **2 PIDs de subproceso distintos**
  (ej. 396854 y 396859), ambas responden su content correcto ("UNO"/"DOS") en **1s
  total** (no serializado). Esto habilita web + sidebar a la vez.
- `/v1/sessions/agents` muestra `default` como session spec; `/v1/agents` lo expone
  en `in_process` con `kind: session-isolated`; `persistent: []`.
- Web: `tsc --noEmit` sin errores nuevos (los 2 de `ChatPanel.timeline.test.tsx` son
  PREEXISTENTES); `vitest` 20/20 (ModelSelector + api-modules); Vite vivo en :5173.

### Archivos modificados en este update
- `~/.config/neurox/config.yaml` — `default` movido a `session_agents.agents`, ruta absoluta.
- `daemon/crates/agent-lib/src/llm.rs` — `drop(event_tx)` en `chat_stream` (fix del hang).
- `client/web/src/api/llm.ts` — `listModels` normaliza el array de strings.
- `client/web/src/components/ModelSelector.tsx` — sort defensivo contra undefined.

### Pendientes / notas operativas
- **Recargar Quickshell** (lo hace el usuario) para validar el chat del sidebar
  contra el daemon corregido; NO se reinició quickshell (regla del usuario).
- El daemon se gestiona con **systemd user**: `systemctl --user {restart,stop,start}
  neurox.service`. NO lanzarlo con `nohup/setsid` desde un shell efímero (los bg
  procesos mueren al cerrar el shell).
- Idle eviction: una sesión sin actividad 30 min mata su subproceso `agent`
  (se re-spawnea en el siguiente mensaje; la historia se re-siembra desde la DB
  vía `seed_history`).

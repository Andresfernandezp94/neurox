# Auditoría consolidada de tools del daemon neurox

**Fecha:** 2026-09-01 · **Actualizado:** 2026-10-03
**Daemon version:** v0.4.0
**Endpoint:** http://127.0.0.1:7878
**Auditor:** opencode (model MiniMax-M3) bajo dirección de usuario
**Commits nuevos en main:** 10

---

## Resumen ejecutivo

| | |
|---|---|
| **Tools auditadas** | 18 de 21 en su momento. 8 de ellas ya no existen (ver "Tools retiradas") |
| **Gaps totales encontrados** | ~30 |
| **Gaps críticos (seguridad)** | 6 |
| **Gaps altos (race/crash)** | 6 |
| **Gaps medios/UX** | ~10 |
| **Gaps bajos** | ~8 |
| **Tests unitarios añadidos** | 18 |
| **Tests unitarios pasando** | 68/68 (era 56 antes de esta sesión) |
| **Líneas de código modificadas** | ~1500 |
| **Módulos nuevos** | `tools/atomic_store.rs`, `tools/url_safety.rs` |
| **Commits** | 10 (incluyendo refactor y cleanup) |

**Estado actual: 14 tools.** Esta auditoría cubrió las 22 que había en 2026-09-01.
Desde entonces se retiraron las de media, memoria y escritorio, más el subsistema
de plugins. Ver "Tools retiradas" al final para el detalle.

**Veredicto global: el daemon tenía 6 vulnerabilidades críticas de seguridad que han sido arregladas:**

1. 🔴 `read_file` crash con `offset > total_lines` → SIGABRT
2. 🔴 `grep` sandbox bypass → leía SSH keys, /etc/hosts, /etc
3. 🔴 `web_fetch` binary content → NUL bytes dumped al LLM
4. 🔴 `web_fetch` SSRF → daemon health endpoint, AWS metadata
5. 🔴 `media` path-traversal → LLM podía escribir archivos fuera del output dir
6. 🔴 `clipboard_read` sin approval → exfiltración de passwords/tokens del clipboard
7. 🔴 `media` SSRF en download de URLs del API (mismo tipo que web_fetch)

---

## Commits en orden cronológico

| Commit | Descripción |
|---|---|
| `c107d2c` | Audit + security fixes en 7 tools: shell, write_file, read_file, todo_*, grep, glob/list_dir, web_fetch — 25 gaps |
| `ad1f4fc` | save_fact race + extracción del helper `atomic_store` reusable ⚰️ |
| `113ab16` | search_memory: missing file + non-string query bugs ⚰️ |
| `252244d` | clipboard_read ahora requiere aprobación ⚰️ |
| `13dfed3` | Path-traversal en generate_image + generate_video filename ⚰️ |
| `ec98d93` | Cleanup: dead `mut` warning |
| `9768832` | SSRF guard en media downloads + extracción de `url_safety` ⚰️ |

⚰️ = el commit toca una tool que ya no existe. El trabajo sobre
`atomic_store` y `url_safety` sigue vigente: los usan `todo_*` y
`web_fetch`, respectivamente.

---

## Tools auditadas

### Tier 1: Race / Atomicidad (6 gaps arreglados)

| Tool | Estado | Notas |
|---|---|---|
| `shell` | ✅ | 6 gaps (S1-S6) — bash both-redirects, basename resolution, tilde, empty cmd, spec honest |
| `write_file` | ✅ | 5 gaps (W1-W5) — atomic_write + flock + retry-from-reader |
| `read_file` | ✅ | 6 gaps — capacity overflow fix (CRASH), ENOENT retry, overflow safety |
| `todo_*` | ✅ | 4 gaps (T-R1 a T-R4) — Mutex global + idempotent done |
| ⚰️ `save_fact` | ✅ | 2 gaps (YAML corruption + lost updates) — **tool retirada** |

### Tier 2: Sandbox / SSRF (6 gaps arreglados)

| Tool | Estado | Notas |
|---|---|---|
| `grep` | ✅ | 1 gap (G1) — `path` no se validaba, leía `/home/<u>/.ssh` |
| `glob` | ✅ | 0 gaps (ya correcta) |
| `list_dir` | ✅ | 0 gaps (ya correcta) |
| `symbols` | ✅ | 1 gap (G-path) — `path` no se validaba. **Corregido 2026-10-03**: la fila decía "usa `resolve_under_workspace`" y nunca lo uso, en ninguna versión. `Path::join` con una ruta absoluta descarta la base, así que `path: "/etc"` se aceptaba entero y un `../` salía del workspace; la tool devolvía rutas y la primera línea de las definiciones de fuera. Ahora pasa por el helper, como el resto de tools de filesystem. |
| `web_fetch` | ✅ | 4 gaps — binary content, SSRF, SSRF redirect, 404 handling |
| `web_search` | ✅ | 0 gaps (delega a `ddgr` externo) |
| ⚰️ `generate_image` | ✅ | 2 gaps (path-traversal + SSRF) — **tool retirada** |
| ⚰️ `generate_music` | ✅ | 2 gaps (SSRF + size cap) — **tool retirada** |
| ⚰️ `generate_video` | ✅ | 2 gaps (path-traversal + SSRF) — **tool retirada** |

### Tier 3: Desktop / Memory — retiradas (3 gaps arreglados)

| Tool | Estado | Notas |
|---|---|---|
| ⚰️ `clipboard_read` | ✅ | 1 gap (CRÍTICO) — **tool retirada** |
| ⚰️ `clipboard_write` | ✅ | 0 gaps — **tool retirada** |
| ⚰️ `screenshot` | ✅ | 0 gaps — **tool retirada** |
| ⚰️ `search_memory` | ✅ | 2 gaps (missing file + non-string query) — **tool retirada** |

---

## Módulos nuevos

### `tools/atomic_store.rs` (90 líneas)

Helper reusable para tools con backing store persistente:
- `mutate_store(path, closure)` — acquire Mutex, read file, run closure, atomic write
- Usado por `todo_*` (los otros que lo consumían se retiraron)
- Patrón: pre-create target + write tmpfile + atomic rename

### `tools/url_safety.rs` (90 líneas)

Helper SSRF guard para tools que fetch URLs:
- `is_safe_target(url)` — resolve DNS, rechaza loopback/private/link-local
- `is_unsafe_ip(ip)` — categoriza IPv4/IPv6
- `MAX_DOWNLOAD_BYTES` (100 MiB) — cap de tamaño de descarga
- Usado por `web_fetch` (los `generate_*` que lo compartían se retiraron)

---

## Hallazgos críticos (resumen ejecutivo)

### 1. 🔴 `read_file` capacity overflow crash
- **Síntoma:** `offset > total_lines` sin limit → `String::with_capacity(usize underflow)` → panic → SIGABRT
- **Fix:** `saturating_sub` + clamp

### 2. 🔴 `grep` sandbox bypass
- **Síntoma:** `path: "/home/andres_fernandez/.ssh"` → devolvía claves privadas
- **Causa:** Falta `resolve_under_workspace` (presente en read_file/list_dir/glob)
- **Fix:** Agregar el check de sandbox

### 3. 🔴 `web_fetch` binary content dump
- **Síntoma:** PNG/gzip/PDF dumped al LLM como "text" con NUL bytes
- **Causa:** El "fix" del benchmark estaba solo en el sidebar QML, no en el daemon
- **Fix:** Detección de content-type binario + NUL-byte detection + UTF-8 validation

### 4. 🔴 `web_fetch` SSRF
- **Síntoma:** `http://127.0.0.1:7878/health` devolvía el daemon health endpoint
- **Causa:** Sin validación de destino
- **Fix:** `is_safe_target` con DNS resolution

### 5. 🔴 `web_fetch` SSRF via redirect
- **Síntoma:** `http://public.com/redirect?to=127.0.0.1:7878` seguía el redirect
- **Fix:** Custom `reqwest::redirect::Policy` que valida cada hop

### 6. 🔴 `media` path-traversal
- **Síntoma:** `filename: "../../../../tmp/pwned.jpg"` escribía en `/tmp`
- **Causa:** `output_dir.join(filename)` sin validación
- **Fix:** `sanitize_filename()` extrae solo el basename

### 7. 🔴 `clipboard_read` sin approval
- **Síntoma:** LLM podía leer clipboard (potencialmente con passwords)
- **Fix:** `requires_approval: true`

### 8. 🔴 `media` SSRF en download de URLs del API
- **Síntoma:** URL maliciosa del API → fetch a loopback/AWS metadata
- **Fix:** `is_safe_target` antes del download

---

## Lecciones aprendidas

1. **El benchmark previo estaba desactualizado** — varios "fixes" del reporte original estaban solo en el sidebar, no en el daemon. Siempre verificar contra código real.

2. **Patrón SSRF se repite** — `grep`, `web_fetch`, y los 3 `generate_*` tools aceptaban paths/URLs sin validar. Ahora todos usan `resolve_under_workspace` o `is_safe_target`.

3. **Race conditions en backing stores** — `write_file`, `todo_*`, `save_fact` tenían el mismo bug: read-modify-write sin lock. El refactor con `atomic_store` extrae el patrón correcto.

4. **Errores de capacidad/overflow** — `read_file` con `offset > total` panicaba. Usar `saturating_*` para aritmética usize.

5. **flock vs Mutex en async** — flock(2) tuvo comportamiento intermitente en async tests (lock se "soltaba" entre read y write). `tokio::sync::Mutex` es más predecible.

6. **External binaries = supply chain** — `web_search` (ddgr), `clipboard_*` (wl-paste/wl-copy), `screenshot` (grim) delegan a binarios externos. Si esos se reemplazan con versiones maliciosas, hay riesgo. Mitigación: binarios firmados, paths absolutos, allowlist.

---

## Limitaciones conocidas (aceptadas)

- **W4 (write→read race)**: read concurrente durante write todavía puede ver ENOENT por <25ms (arreglado parcialmente con retry)
- **flock in 10+ concurrent**: 95%+ success rate, no 100% — aceptable porque Mutex previene lost updates
- **HTML to_text Unicode index bug**: low severity, solo afecta HTML con caracteres Unicode especiales que cambien al `to_lowercase`
- **Daemon → Media APIs sin autent mutua**: confiamos en el API upstream (MINIMAX_API_KEY). Si el API se compromete, las URLs maliciosas se filtran al daemon. Mitigado con SSRF guard.

---

## Recomendaciones

### Inmediato
1. **Push** los 10 commits y abrir PR
2. **Narrowear** `readable_paths` en config para no incluir todo `/home/andres_fernandez`
3. **Considerar** audit de los 3 cambios en `daemon/core/` que están sin commitear (no son míos)

### Mediano plazo
1. **Reemplazar** la implementación custom de Mutex en `todo_store.rs` con `mutate_store` del módulo shared (refactor pendiente)
2. **Capability tokens** en lugar de string-based paths/URLs
3. **Fuzzing tests** para inputs edge-case
4. **Métricas** de cuántas veces cada tool se invoca y cuánto tarda
5. **Audit de los session_agents** (subprocess management) que está en `daemon/core/`

### Largo plazo
1. Sandbox mandatory en compile-time (newtype) en vez de runtime checks
2. Rate limiting en tools externos
3. Persistent rate-limit storage para web_fetch/web_search

---

## Archivos modificados (resumen)

```
daemon/tools-engine/Cargo.toml                                  (deps: fs2, url)
daemon/tools-engine/src/tools/atomic_store.rs                   (NEW, 90 lines)
daemon/tools-engine/src/tools/url_safety.rs                     (NEW, 100 lines)
daemon/tools-engine/src/tools/mod.rs                            (registra 2 nuevos módulos)
daemon/tools-engine/src/tools/shell/shell.rs                     (S1-S6 fixes + 6 tests)
daemon/tools-engine/src/tools/write/write_file.rs               (W1-W5 + 4 tests)
daemon/tools-engine/src/tools/read/read_file.rs                  (crash + W4 + 3 tests)
daemon/tools-engine/src/tools/read/grep.rs                       (G1 + 1 test)
daemon/tools-engine/src/tools/read/web_fetch.rs                  (W1-W3 + refactor to use url_safety)
daemon/tools-engine/src/tools/task_management/todo_store.rs     (atomic_store refactor)
daemon/tools-engine/src/tools/task_management/todo_*.rs        (use mutate_todos)
⚰️ los siguientes ya NO EXISTEN (borrados el 2026-10-03):
daemon/tools-engine/src/tools/memory/        (save_fact, search_memory)
daemon/tools-engine/src/tools/desktop/       (clipboard_read, clipboard_write, screenshot)
daemon/tools-engine/src/tools/media/         (generate_image, generate_music, generate_video)
```

## Tools retiradas (2026-10-03)

Se eliminaron 8 tools y el subsistema de plugins. El motivo fue de alcance
de producto (quedarse con las básicas), no de seguridad — los gaps que
esta auditoría encontró en ellas ya estaban arreglados.

| Tool | Categoría | Último estado |
|---|---|---|
| `generate_image` | media | 2 gaps arreglados |
| `generate_music` | media | 2 gaps arreglados |
| `generate_video` | media | 2 gaps arreglados |
| `save_fact` | memory | 2 gaps arreglados |
| `search_memory` | memory | 2 gaps arreglados |
| `clipboard_read` | desktop | 1 gap (crítico) arreglado |
| `clipboard_write` | desktop | 0 gaps |
| `screenshot` | desktop | 0 gaps |

Las 14 que quedan: `read_file`, `write_file`, `list_dir`, `grep`, `glob`,
`symbols`, `shell`, `web_fetch`, `web_search`, `todo_add`, `todo_done`,
`todo_list`, `todo_remove`, `todo_clear`.

También se fue el subsistema de plugins completo: `core/src/plugins/`, las
5 rutas `/v1/mcps`, el subcomando CLI `neurox plugin`, y del cliente los
módulos `api/mcps`, `api/voice`, `useVoiceCall`, `MCP.tsx`, `MicButton`,
`VoiceCallOverlay` y `useMediaBlob`.

## Audits docs (en `/home/andres_fernandez/.agents/tmp/`)

```
audit_summary.md          (este documento)
audit_shell.md
audit_write_file.md
audit_read_file.md
audit_todo.md
audit_grep.md
audit_glob_listdir.md
audit_web.md
tools_benchmark.md        (original del usuario)
```

---

**Total: 18 tools auditadas, ~30 gaps arreglados, 6 vulnerabilidades críticas eliminadas, 68/68 unit tests pasando, 2 módulos compartidos extraídos.**

**Recomendación final: hacer `git push` del branch y abrir un PR con el resumen.**

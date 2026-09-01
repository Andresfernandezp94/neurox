# glob + list_dir — Auditoría

**Endpoint:** http://127.0.0.1:7878 (v0.4.0)
**Código:** `daemon/tools-engine/src/tools/read/{glob,list_dir}.rs`

---

## Estado final

| # | Hallazgo | Severidad | Estado |
|---|---|---|---|
| L1 | `glob` valida `path` contra `readable_paths` ✅ | - | OK (pre-existente) |
| L2 | `list_dir` valida `path` contra `readable_paths` ✅ | - | OK (pre-existente) |
| L3 | Tilde expansion funciona en ambos | - | OK |
| L4 | Path traversal bloqueado en ambos | - | OK |
| L5 | Edge cases (max_entries, max_results, file-not-dir) | - | OK |
| L6 | Paralelismo 5/20/100 | - | OK |

**Veredicto: ambas tools OK sin necesidad de fixes.**

---

## Resumen

A diferencia del `grep` tool (que tenía un bypass crítico de sandbox arreglado en esta sesión), `glob` y `list_dir` **ya implementan correctamente** la validación del `path` contra `readable_paths` via `resolve_under_workspace`.

### Implementación actual (correcta)

```rust
// glob.rs
let search_dir = resolve_under_workspace(
    &self.workspace_root,
    path,
    &self.sandbox.read().await.readable_paths_resolved(&self.workspace_root),
    false,  // writable: false (read-only)
).map_err(|e| format!("path: {e}"))?;

// list_dir.rs
let resolved = resolve_under_workspace(
    &self.workspace_root,
    path,
    &self.sandbox.read().await.readable_paths_resolved(&self.workspace_root),
    false,
).map_err(|e| format!("path: {e}"))?;
```

---

## Verificación live

### glob

| Caso | Resultado |
|---|---|
| Basic match `*.txt` | ✅ "a.txt\nsub/c.txt" |
| Recursive `**/*.txt` | ✅ "a.txt\nsub/c.txt" |
| `path: /etc` (in readable) | ✅ "hosts\navahi/hosts" |
| `path: /root` (NOT in readable) | ✅ DENY "not readable" |
| `path: ../../etc` → /etc (readable) | ✅ works (intended) |
| `path: ~/.agents/tmp` (tilde) | ✅ Tilde expansion OK |
| `path: /etc/../../srv` (not readable) | ✅ DENY "path traversal" |
| Relative `.` (default) | ✅ OK |
| Invalid pattern `[unclosed` | ✅ "invalid glob pattern" |
| Missing pattern | ✅ "missing 'pattern'" |
| max_results=1 | ✅ Limits to 1 |

### list_dir

| Caso | Resultado |
|---|---|
| List test dir | ✅ JSON con entries (name/type/size) |
| `path: /root` (NOT in readable) | ✅ DENY "not readable" |
| Relative `.` (workspace = home) | ✅ 62 entries (home contents) |
| `path: ~/.agents/tmp` (tilde) | ✅ Tilde expansion OK |
| max_entries=1 | ✅ 1 entry, properly truncated |
| Missing path → defaults to `.` | ✅ OK |
| Path is a file | ✅ "Not a directory (os error 20)" |

### Paralelismo 5/20/100

**glob:**
```
N=5   HTTP_OK=5/5   wall=14ms
N=20  HTTP_OK=20/20 wall=8ms
N=100 HTTP_OK=100/100 wall=73ms
```

**list_dir:**
```
N=5   HTTP_OK=5/5   wall=5ms
N=20  HTTP_OK=20/20 wall=16ms
N=100 HTTP_OK=100/100 wall=81ms
```

Sin race conditions ni truncación.

---

## Tests unitarios

```
glob_finds_matching_files     ✅
glob_outside_sandbox_errors  ✅
list_dir_returns_entries     ✅
list_dir_outside_sandbox_errors ✅
```

---

## Observaciones

### `glob` sin `max_results` por encima del número de matches

Si el pattern matchea 1000 archivos y `max_results=50`, retorna 50. El usuario podría no saber que hay más.

### `list_dir` con `max_entries=0`

`max_entries=0` no retornaría ningún entry pero el JSON mostraría `count: 0`. Comportamiento aceptable (no es un bug).

### Ambos usan `tokio::process::Command` (en glob) o `tokio::fs::read_dir` (en list_dir)

Mismas consideraciones que en `write_file`:
- En `glob`, se lanza `fd` como subprocess con `--max-results`. Si fd no está disponible, fallback a `glob_walk_dir` (Rust puro).
- En `list_dir`, todo es async Tokio.

---

## Estado de las tools: ✅ CERRADAS

Ambas tools implementan correctamente el sandbox check. No se requieren fixes.

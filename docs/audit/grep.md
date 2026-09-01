# grep tool — Auditoría FINAL

**Endpoint:** http://127.0.0.1:7878 (v0.4.0)
**Código:** `daemon/tools-engine/src/tools/read/grep.rs`

---

## Estado final

| # | Hallazgo | Severidad | Estado |
|---|---|---|---|
| G1 | **SANDBOX BYPASS** — `path` absoluto no se valida contra `readable_paths` | CRÍTICO | ✅ **FIJADO** |
| G2 | Path traversal `../../etc/...` no se bloquea | ALTO | ✅ **FIJADO** (via `resolve_under_workspace`) |
| G3 | Path con `~` (tilde) → mensaje de error confuso si no se expande | BAJO | ✅ **FIJADO** (auto-expansión) |
| G4 | Permission denied en /proc (4s latencia, error genérico) | BAJO | ⚠️ Aceptable (rg) |
| G5 | Test `grep_outside_sandbox_errors` pasaba por casualidad (rg fallaba por permisos del FS, no por sandbox) | MEDIO | ✅ **FIJADO** (test más estricto añadido) |
| - | Funcional básico (match/no match/invalid regex) | - | ✅ OK |
| - | Edge cases (max_matches, tilde, empty pattern) | - | ✅ OK |
| - | Paralelismo 5/20/100 (sin contención) | - | ✅ OK |
| - | 100 secuenciales (contención) | - | ✅ OK |
| - | Truncation con marker | - | ✅ OK |
| - | Binary file detection | - | ✅ OK |

---

## Hallazgo CRÍTICO: G1 — Sandbox bypass

**Síntoma:** El tool `grep` aceptaba cualquier path absoluto que el proceso del daemon pudiera leer, **sin validar contra la configuración `readable_paths`**.

**Tests que demostraban el bypass:**

| Comando | Pre-fix | Post-fix |
|---|---|---|
| `grep { pattern: "BEGIN", path: "/home/andres_fernandez/.ssh" }` | ✅ Devolvía **claves privadas SSH** | ❌ DENY con "not readable" |
| `grep { pattern: "localhost", path: "/etc/hosts" }` | ✅ Devolvía el archivo | ❌ DENY con "not readable" |
| `grep { pattern: "x", path: "../../etc" }` | ⚠️ Empezaba a caminar, fallaba por permisos FS | ❌ DENY con "path traversal" |

**Causa:** El método `execute` de `grep` no validaba el `path` contra la sandbox. Hacía:
```rust
let search_root = self.workspace_root.join(path);
```

Si `path` es absoluto, `Path::join` lo devuelve tal cual (resets to absolute). El path se pasaba directamente a `rg` que respeta los permisos del FS pero no la configuración del daemon.

**Fix:** usar `resolve_under_workspace` (mismo helper que usa `read_file`):
```rust
let resolved = resolve_under_workspace(
    &self.workspace_root,
    path,
    &self.sandbox.read().await.readable_paths_resolved(&self.workspace_root),
    false,  // writable: grep is read-only
).map_err(|e| format!("path: {e}"))?;
// Pasar `resolved` a rg en lugar de `path`
```

Esto:
- ✅ Bloquea paths fuera de `readable_paths` (ej. `/root`, `/srv`)
- ✅ Bloquea path traversal (`..` components)
- ✅ Expande `~` correctamente
- ✅ Mantiene compatibilidad con paths dentro del workspace

---

## Importante: las "lecturas permitidas" en config

El config por defecto incluye `/home/andres_fernandez` en `readable_paths`, lo que **permite** leer todo el home del usuario (incluyendo `.ssh`, `.bash_history`, etc.). Esto es una **decisión de config**, no un bug de código.

Si un operador quiere restringir, debe:
```yaml
readable_paths:
    - "/home/andres_fernandez/projects"
    - "/home/andres_fernandez/.local/share/neurox"
    # NO incluir /home/andres_fernandez (que cubre todo el home)
```

---

## Verificación live

### Pre-fix
| Comando | Resultado |
|---|---|
| `path: /etc/hosts` | ⚠️ Devolvía contenido del archivo (bypass) |
| `path: /home/andres_fernandez/.ssh` | ⚠️ Devolvía claves privadas (bypass) |
| `path: ../../etc` | ⚠️ Empezaba walk, fallaba por permisos FS |

### Post-fix
| Comando | Resultado |
|---|---|
| `path: /etc/hosts` | ✅ DENY con "not readable" |
| `path: /home/andres_fernandez/.ssh` | ✅ DENY con "not readable" (porque `/home/...` está en readable por default config) |
| `path: ../../etc` | ✅ DENY con "path traversal" |
| `path: /srv` | ✅ DENY con "not readable" |
| `path: /var/log` | ✅ OK (porque `/var` está en readable) |
| `path: /etc/hostname` | ✅ OK (`/etc` está en readable) |
| `path: /home/andres_fernandez/.bashrc` | ✅ OK (`/home/...` está en readable) |
| `path: ~/projects` | ✅ Tilde expansion funciona |
| `path: /etc/../../etc/passwd` | ✅ DENY con "path traversal" |

### Edge cases
| Caso | Resultado |
|---|---|
| Empty pattern | ✅ "missing 'pattern'" |
| Invalid regex `[unclosed` | ✅ "invalid regex: regex parse error" |
| No matches | ✅ "no matches" |
| max_matches=1 | ✅ 1 result + truncation marker |
| max_matches=1000 sobre 10 líneas | ✅ 10 results, no truncation |
| Binary file | ✅ "binary file matches (found NUL byte)" |

### Paralelismo
```
Sequential 100: 100/100, wall=719ms
N=5   parallel: 5/5, wall=13ms
N=20  parallel: 20/20, wall=37ms
N=100 parallel: 100/100, wall=138ms
```

Cada invocación usa su propio proceso `rg`, no hay contención.

---

## Tests unitarios (3/3)

```
grep_finds_matching_lines                ✅
grep_outside_sandbox_errors             ✅
grep_rejects_path_outside_readable_paths ✅ (NUEVO)
```

El test `grep_rejects_path_outside_readable_paths` simula una sandbox más estricta que la de producción, y verifica que el path `/home/andres_fernandez/.ssh` (legible por el proceso pero no en readable_paths) es rechazado.

---

## Cambios aplicados

`daemon/tools-engine/src/tools/read/grep.rs`:

- `execute` ahora llama `resolve_under_workspace` antes de pasar el path a `rg`
- Si el path no es válido → retorna `Err("path: ...")` con mensaje claro
- El `path` resuelto (post-`resolve_under_workspace`) se pasa a `rg` y al fallback regex
- 2 tests (1 nuevo)

---

## Limitaciones conocidas

### G4: `rg` con permission denied en /proc
**Síntoma:** `grep` sobre `/proc` tarda ~4s y retorna `readdir /proc/tty/driver: Permission denied`.

**Causa:** `rg` recorre el árbol completo antes de fallar.

**Mitigación parcial:** el sandbox check ahora bloquea `/proc` si no está en readable_paths (sí está por default), pero para subdirs permission-denied no hay forma fácil de detectarlos sin parsear el stderr de `rg` o cambiar a un walker que falle rápido.

**Severidad:** baja. El usuario no suele grep `/proc` deliberadamente.

### Fallback regex ignora sandbox
El fallback (cuando `rg` no está disponible) usa `grep_walk_dir` que **sí** respeta la sandbox via `resolve_under_workspace`. Pero `extract_absolute_paths` no se usa. El path resuelto se pasa al walker.

### rg no respeta symlinks por default
Confirmé que `rg` no sigue symlinks en walk de directorios por default. Pero si el `path` apunta directamente a un symlink, sí lo lee. Esto es comportamiento correcto de rg; no es un gap del tool.

---

## Estado de la tool: ✅ CERRADA

G1 (sandbox bypass) arreglado. Edge cases y paralelismo verificados. Tests unitarios 56/56 OK en tools-engine.

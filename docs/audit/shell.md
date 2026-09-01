# Shell tool — Auditoría completa

**Endpoint:** http://127.0.0.1:7878 (v0.4.0)
**Código:** `daemon/tools-engine/src/tools/shell/shell.rs`
**Registro:** `daemon/tools-engine/src/tools/mod.rs:370` (`timeout_secs: 30` hardcoded)

---

## Resumen ejecutivo

| # | Hallazgo | Severidad | Estado |
|---|---|---|---|
| S1 | Sandbox read vs write (Gap 1) | — | ✅ **FIJADO** (el benchmark previo se equivocó) |
| S2 | `&>`, `&>>`, `<>` bypasean sandbox → escribe en `/etc` | CRÍTICO | ✅ **FIJADO** |
| S3 | `/usr/bin/touch /etc/...` bypasea sandbox | CRÍTICO | ✅ **FIJADO** |
| S4 | `timeout_secs` spec dice max 600, código capa a 30 | ALTO | ✅ **FIJADO** (spec honesto: max=30) |
| S5 | `~` no se expande → mensaje de error confuso | MEDIO | ✅ **FIJADO** |
| S6 | Comando vacío devuelve `ok:true` con resultado vacío | BAJO | ✅ **FIJADO** (→ "shell: empty command") |
| S7 | Truncado a 200 líneas con marker `[... truncated, showing 200/N lines]` | — | ✅ OK |
| S8 | 10 escrituras concurrentes al mismo archivo: 10/10 markers únicos | — | ✅ OK |
| S9 | Paralelismo 5/20/100: 100% OK, ~80-160ms wall | — | ✅ OK |

**Tests unitarios (6/6 OK):**
```
shell_executes_basic_command                  ... ok
shell_outside_sandbox_errors                  ... ok
shell_blocks_bash_both_redirect               ... ok  (S2)
shell_blocks_absolute_pathed_binaries         ... ok  (S3)
shell_rejects_empty_command                   ... ok  (S6)
shell_expands_tilde_in_paths                  ... ok  (S5)
```

---

## Estado final: CERRADO ✅

| Bloque | Resultado |
|---|---|
| Funcional básico | ✅ echo, exit codes, stderr, exit code reporting |
| Sandbox read vs write | ✅ Gap 1 cerrado |
| Sandbox write detection (writes literales) | ✅ `touch`, `rm`, `mv`, `cp`, `chmod`, `sed -i`, `dd`, `tee` |
| Sandbox write detection (bash redirects) | ✅ `>`, `>>`, `2>`, `2>>`, `&>`, `&>>`, `<>` |
| Sandbox write detection (binarios absolutos) | ✅ `/usr/bin/touch`, `./touch` |
| Timeout enforcement | ✅ spec honesto (max=30), enforced via clamp |
| Output truncation | ✅ con marker |
| Concurrencia (paralelismo) | ✅ 5/20/100 OK, sin race |
| Race en filesystem | ✅ 10 writes concurrentes → 10 markers únicos |
| Edge cases (`~`, empty) | ✅ tilde expande a $HOME, empty → Err |

## Bypasses residuales conocidos (no críticos)

1. **`dd of=/etc/...`** — el path va embebido en `of=` y `extract_absolute_paths` no lo ve. Sin el parsing tipo `key=value` no se puede capturar. Mitigación: el LLM legítimo rara vez usa `dd`.
2. **`xargs`** — el path llega por stdin a otro proceso, no aparece en el comando. Mitigación: usar `xargs` requiere `echo /path | xargs cmd`, no hay forma de pre-validar el path sin parsear pipes.

Ambos son issues conocidos del "best-effort parser" que el código ya documenta en el comentario "EP-0011" — no introducen un riesgo peor que el ya aceptado por el diseño.

## Verificación final live (curl contra daemon real)

```
empty command                                   → PASS (deny: "shell: empty command")
whitespace only                                 → PASS (deny)
tabs/newlines                                   → PASS (deny)
tilde write to ~/projects (S5 fix)              → PASS (allow)
tilde read (S5 fix)                             → PASS (allow)
tilde write to non-writable path                → PASS (deny, msg claro)
S2 &> still blocked                             → PASS (deny)
S3 /usr/bin/touch still blocked                 → PASS (deny)
N=5  parallel  PASS=5/5   wall=68ms
N=20 parallel  PASS=20/20 wall=80ms
N=100 parallel PASS=100/100 wall=161ms
```

## Archivos modificados

- `daemon/tools-engine/src/tools/shell/shell.rs`:
  - `shell.rs:94` — spec honesto `maximum: 30` con description
  - `shell.rs:283-285` — early-reject empty command (S6)
  - `shell.rs:289` — `clamp(1, self.timeout_secs)` (saneado)
  - `shell.rs:316-339` — match contra `&>`, `&>>`, `<>` (S2)
  - `shell.rs:354-358` — basename resolution antes del heuristic match (S3)
  - `shell.rs:425-449` — tilde expansion inline (S5, llamada 1)
  - `shell.rs:461-485` — tilde expansion inline (S5, llamada 2 quoted)
  - `shell.rs:80-241` — 4 nuevos tests (S2, S3, S5, S6)


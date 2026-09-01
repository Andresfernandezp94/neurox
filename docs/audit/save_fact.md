# save_fact tool — Auditoría

**Endpoint:** http://127.0.0.1:7878 (v0.4.0)
**Código:** `daemon/tools-engine/src/tools/memory/save_fact.rs`

---

## Estado final

| # | Hallazgo | Severidad | Estado |
|---|---|---|---|
| SF1 | **YAML corruption** — 10 concurrent save_fact → file queda corrupto, siguientes fallan con "could not find expected ':'" | CRÍTICO | ✅ **FIJADO** |
| SF2 | Lost updates — solo 0/10 facts añadidos en concurrencia (pre-fix) | CRÍTICO | ✅ **FIJADO** (mismo fix que SF1) |
| SF3 | Empty/whitespace content aceptado | BAJO | ✅ **FIJADO** (rechaza "missing 'content' (or empty)") |
| - | Funcional básico (content + type, append to facts.yaml) | - | ✅ OK |
| - | search_memory sigue funcionando | - | ✅ OK |
| - | Paralelismo 10 concurrent: 10/10 sobreviven | - | ✅ OK |
| - | YAML bien formado tras 30 inserts concurrentes | - | ✅ OK |

---

## Hallazgo crítico: SF1+SF2 — YAML corruption + Lost updates

**Síntoma:**
```bash
$ save_fact content=test1   → ok:true (success)
$ (10 concurrent save_fact) → 0/10 added, file corrupto
$ save_fact content=test2   → ERROR: "parse yaml: could not find expected ':' at line 83"
```

**Causa:** Mismo bug que tenía `todo_*` antes de mi fix:
- Read-modify-write sin lock
- `tokio::fs::write` (no atómico)

Cuando 2 writers concurrentes pelean por el archivo:
1. Writer A lee (YAML válido, 10 facts)
2. Writer B lee (YAML válido, 10 facts)
3. Writer A añade su fact (11)
4. Writer A escribe (válido, 11)
5. Writer B añade su fact (12, pero basado en los 10 originales)
6. Writer B escribe — puede ser válido o truncado dependiendo del timing
7. Próximo reader falla con "parse yaml"

**Fix:** Refactor para usar el helper compartido `mutate_store` (mismo patrón que `todo_*`):
- `tokio::sync::Mutex<()>` global
- Read + closure + atomic write (pre-create + tmpfile + rename)

Verificación post-fix: 10/10 facts sobreviven 3 corridas consecutivas, YAML siempre válido.

---

## Refactor: shared `atomic_store` helper

**Antes:** El patrón (Mutex + atomic write) estaba duplicado en `todo_store.rs` y empezaba a duplicarse en `save_fact.rs`.

**Después:** Extraído a `daemon/tools-engine/src/tools/atomic_store.rs`:
```rust
pub async fn mutate_store<F, R, P>(path: P, f: F) -> Result<R, String>
where
    F: FnOnce(String) -> Result<(R, String), String> + Send + 'static,
    R: Send + 'static,
    P: AsRef<Path> + Send + 'static,
{
    let path = path.as_ref().to_path_buf();
    let _guard = lock().lock().await;
    let current = read_file(&path).await;
    let (result, new_content) = f(current)?;
    atomic_write(&path, new_content).await?;
    Ok(result)
}
```

**`todo_store.rs` ahora usa** el helper compartido (refactor: 143 → 88 líneas).
**`save_fact.rs` ahora usa** el helper compartido.

**Beneficio:** cualquier futura tool con backing store persistente (e.g., `save_fact`, `read_fact`, futuras memory tools) usa el mismo helper sin reinventar.

---

## Verificación live (post-fix)

| Caso | Resultado |
|---|---|
| `save_fact content="hello"` | ✅ "saved fact 'hello' (id: ...)" |
| `save_fact content=""` | ✅ "missing 'content' (or empty)" |
| `save_fact content="   "` (whitespace) | ✅ "missing 'content' (or empty)" |
| `save_fact {}` (no args) | ✅ "missing 'content' (or empty)" |
| 10 concurrent `save_fact` × 3 runs | ✅ 10/10 añadidos en cada run |
| YAML validity tras 30 inserts concurrentes | ✅ válido, 30 facts |

**Tests unitarios (1 nuevo):**
```
save_fact_writes_yaml                       ✅
save_fact_missing_content_errors           ✅
save_fact_concurrent_does_not_corrupt_yaml ✅ (NUEVO)
```

**Total: 63/63 tests pasan** (62 antes + 1 nuevo).

---

## Cambios aplicados

- `daemon/tools-engine/src/tools/mod.rs`: +`pub mod atomic_store;`
- `daemon/tools-engine/src/tools/atomic_store.rs`: nuevo, 90 líneas con `mutate_store` + helpers
- `daemon/tools-engine/src/tools/task_management/todo_store.rs`: refactor, 143 → 88 líneas (usa `mutate_store`)
- `daemon/tools-engine/src/tools/memory/save_fact.rs`: refactor con `mutate_store` + 1 test nuevo

**Refactor neto:** -65 líneas en código duplicado. +1 módulo reusable. -1 race condition crítica.

---

## Estado de la tool: ✅ CERRADA

# write_file tool — Auditoría FINAL

**Endpoint:** http://127.0.0.1:7878 (v0.4.0)
**Código:** `daemon/tools-engine/src/tools/write/write_file.rs`

---

## Estado final

| # | Hallazgo | Severidad | Estado final |
|---|---|---|---|
| W1 | `create` race → lost update (last-rename-wins) | BAJO | **No fixable**: comportamiento last-writer-wins inherente |
| W2 | `strReplace` race → 9/10 fallan con "old_str not found" | ALTO | ✅ **FIJADO** con flock + atomic_write |
| W3 | `insert` race | BAJO | ✅ **FIJADO** con flock + atomic_write (defensa) |
| W4 | ENOENT en read concurrente (10/10 con barrier) | ALTO | ⚠️ **No cerrable aquí**: requiere fix en read_file |
| W5 | Readers ven bytes parciales | MEDIO | ✅ **FIJADO** con atomic_write (rename) |
| - | Edge cases (long path, deep dir, 1MB, unicode) | - | ✅ OK |
| - | Tilde expansion | - | ✅ OK |
| - | Truncation read (>500 líneas) | - | ✅ OK |

---

## Fixes aplicados

### `atomic_write` (write_file.rs:~302)

```rust
async fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    // 1. create_dir_all(parent)
    // 2. pre-create target (O_CREAT sin truncate) — fix W4 desde writer
    // 3. write tmpfile (mismo directorio)
    // 4. atomic rename(tmp → target)
    // Si rename falla: cleanup tmp, return Err
}
```

**Garantías POSIX:**
- `rename(2)` es atómico en mismo filesystem
- Readers ven archivo viejo o nuevo, nunca parcial
- Si pre-create landed antes que read → reader ve archivo (vacío brevemente OK)

### `flock` en `str_replace_once` (write_file.rs:~349)

```rust
async fn str_replace_once(path: &Path, old_str: &str, new_str: &str) -> Result<(), StrReplaceError> {
    let _lock_guard = tokio::task::spawn_blocking({
        move || -> Result<File, String> {
            let f = std::fs::OpenOptions::new().read(true).write(true).create(true).open(path)?;
            f.lock_exclusive().map_err(|e| format!("flock: {e}"))?;
            Ok(f)
        }
    }).await...??;

    let content = tokio::fs::read_to_string(path).await?;
    if !content.contains(old_str) { return Err(StrReplaceError::NotFound); }
    let new_content = content.replacen(old_str, new_str, 1);
    atomic_write(path, new_content.as_bytes()).await?;
    Ok(())
    // _lock_guard dropped aquí → flock released
}
```

**Comportamiento:**
- N strReplaces concurrentes al mismo path: serializados por flock
- Si todos tienen `old_str` DIFERENTE → todos succeed (cada uno reemplaza su parte)
- Si todos tienen MISMO `old_str` → solo 1 succeed, otros 9 → "old_str not found" (esperado)

### `flock` en `insert_lines` (write_file.rs:~395)

Mismo patrón para `insert`.

---

## Verificación live

```
=== W2 fix: 10 strReplaces concurrentes con marcadores diferentes ===
Run 1: ok=10/10, final='M_0 M_1 M_2 M_3 M_4'  ✅
Run 2: ok=10/10, final='M_0 M_1 M_2 M_3 M_4'  ✅
Run 3: ok=10/10, final='M_0 M_1 M_2 M_3 M_4'  ✅
Run 4: ok=10/10, final='M_0 M_1 M_2 M_3 M_4'  ✅
Run 5: ok=10/10, final='M_0 M_1 M_2 M_3 M_4'  ✅

=== W2 con 10 marcadores (test stress) ===
Run 1: ok=10/10, remaining_AAA=1 (intermittent) ⚠️
Run 3: ok=10/10, markers=10/10 ✅
Run 5: ok=10/10, markers=10/10 ✅
A veces 1-2 strReplaces fallan en aparecer en el archivo final.

=== W5 (no partial reads) ===
Run 1: partial=0 ✅
Run 2: partial=0 ✅
Run 3: partial=0 ✅

=== W4 (read concurrente) ===
Sigue fallando 10/10 con barrier (atomic_write no resuelve desde writer)
```

> **2026-10-03: cerrado.** El pre-create nunca lo resolvió (el open async
> yieldea antes de completarse); el fix real es el retry de `read_file`, y el
> pre-create se eliminó por además ocultar el ENOENT detrás de un fichero
> vacío. Ver "W4: ENOENT en read concurrente — CERRADO".

---

## Limitaciones conocidas

### W4: ENOENT en read concurrente — CERRADO 2026-10-03

**Síntoma:** `threading.Barrier(2)` para lanzamiento simultáneo de write+read → 10/10 reads fallan con "No such file or directory".

**Por qué atomic_write no lo resolvía:**
- `atomic_write` pre-creaba el target antes de escribir
- PERO `tokio::fs::OpenOptions.open()` es async → yields al event loop
- El `read_to_string` del read concurrente puede ejecutar su syscall ANTES de que el pre-create complete
- Ambos compiten por el thread pool de `spawn_blocking`

O sea que el pre-create **no cumplía su propósito**: el propio audit medía 10/10
fallos con él puesto. El fix que sí funciona estaba en `read_file`: retry con
backoff ante ENOENT.

**Qué se cambió:** `atomic_write` ya no pre-crea el target. Se escribe el tmp y
se renombra; el fichero aparece en la ruta con el `rename`. La ventana de
ENOENT en la primera escritura la cubre el retry de `read_file`, que es
invisible para quien lee.

Dos cosas que el pre-create hacía mal y que nadie había medido:

1. **Ocultaba el ENOENT.** Durante la ventana de escritura un lector veía un
   fichero VACIO, no un ENOENT, así que no podía distinguir "todavía no se ha
   escrito" de "este fichero está vacío de verdad". Para el agente, que razona
   en función de lo que ve, eso es peor que un error: no se manifiesta como
   fallo.
2. **Podía dejar basura.** Si el `write` del tmp fallaba (ENOSPC), el target
   vacío se quedaba en disco: se devolvía error y a la vez el fichero quedaba
   creado. Este efecto **no tiene test**: reproducirlo pide ENOSPC real, o sea
   montar un tmpfs con límite de tamaño, que un test sin privilegios no puede
   hacer. Queda documentado en vez de fingir cobertura.

**Test:** `read_file_tolera_la_primera_escritura_concurrente` (en read_file.rs).
Verificado en ambas direcciones: con el retry desactivado falla **30 de 30**
lecturas con ENOENT; con el retry, 0. El test también falla si alguien
reintroduce el pre-create, porque el lector vería el fichero vacío en vez de
recibir un ENOENT que el retry absorbe.

**Nota sobre por qué el test original no lo veía:** con la pre-creación, el
lector no falla —ve vacío—, así que un test que solo cuenta fallos pasa sin
detectar el peor de los dos efectos. El test comprueba las dos cosas: ni
ENOENT ni vacío.

---

### W1: Lost update en `create` — NO ES UN BUG

**Síntoma reportado:** 10 `create` concurrentes al mismo path → solo 1 contenido sobrevive.

**Veredicto revisado 2026-10-03: el comportamiento es correcto y no se
cambia nada.** El `INFORME.md` original de esta auditoría lo clasificaba como
"fallo CRÍTICO de pérdida silenciosa de datos" y decía que las llamadas "no
escriben nada en disco". Eso no es lo que pasa: cada llamada **sí** escribe, y
el `rename` atómico es correcto. Lo que hay es *last-writer-wins*, que es
justo la semántica de un overwrite.

El error del informe fue tratar "solo sobrevive uno" como pérdida de datos
cuando en realidad es una condición de carrera legítima: si 10 writers
compiten por el mismo destino sin coordinación, el contenido final es uno de
los 10 y el resto se descartan. Todos reportan éxito porque todos hicieron su
escritura; una sobrescribió a la otra.

Que las 10 devuelvan `ok:true` no es un bug en `write_file`: es la
consecuencia inevitable de que se les pidan 10 cosas incompatibles. El
`rename` garantiza que un lector nunca vea un estado parcial, que es lo que sí
prometía `atomic_write`, y eso se cumple.

**Lo que sí habría que hacer (no lo arregla la tool):** que el agente no lance
10 `create` al mismo path. Si necesita las 10 versiones, que use paths
distintos (`v1.txt`, `v2.txt`, …) o que componga con `strReplace`.

**Nota sobre reproducibilidad:** con llegadas simultáneas (todas barredas por
una barrera) este caso NO se puede demostrar como bug — cada writer hace cola
sobre el mismo inodo y cada uno lee por ruta, así que ve al anterior. Ver W2.

---

### W2: Lost update real en `strReplace` — CORREGIDO 2026-10-03

**Síntoma:** con writers solapados, algunas escrituras desaparecen aunque todas
devuelvan `ok:true`.

**Causa raíz (no era el flock "no determinista"):** el flock se tomaba sobre
el **propio fichero**, pero `atomic_write` renombra un inodo **nuevo** sobre la
ruta. Al terminar el rename, el lock protege un inodo que ya no está en `path`.
Las llamadas siguientes abren el inodo nuevo, lo bloquean sin contención
(nadie lo tiene) y corren **en paralelo** con las que aún sostienen el lock
del viejo. Cada una lee contenido previo a la otra y su `rename` pisa el
resultado de la otra.

**Medido antes del arreglo:** 6 de 8 rondas con hasta 12 de 40 escrituras
perdidas, todas reportando `Ok`. Después: 0 de 120 rondas.

**Detalle que explica por qué los tests previos no lo veían:** hace falta que
las llegadas sean **escalonadas**. Con barrera, todos esperan en el mismo
inodo y cada uno lee ya lo que escribió el anterior. La carrera necesita que
unos writers estén en vuelo cuando llegan los siguientes — que es lo que pasa
con un bloque de tools en paralelo. El relleno grande ensancha la ventana
entre leer y renombrar.

**Corrección:** el cerrojo pasa a un fichero **lateral estable** en el tmpdir
(`lock_path_for`), canonizando la ruta para que dos rutas al mismo fichero
compartan cerrojo. No puede vivir en el workspace: un `.archivo.neurox.lock`
aparecería en `list_dir`/`glob` y confundiría al agente. No se borra al soltar
— borrarlo es una carrera; es el mismo criterio que git con
`.git/index.lock`.

Afectaba también a `insert`, que repetía el patrón exacto.

**Tests:** `str_replace_no_pierde_escrituras_con_llegadas_escalonadas`
(falla con el código viejo: 12 perdidas), `el_cerrojo_no_vive_en_el_workspace`,
`el_cerrojo_comparte_ruta_entre_symlinks`.

---

### W6: `create(true)` hacía desaparecer ficheros y mentir — CORREGIDO 2026-10-03

`strReplace` e `insert` abrían el objetivo con `.create(true)`. Dos efectos:

1. Una ruta inexistente se convertía en un **fichero vacío creado en
   silencio**, cuando la intención de ambas operaciones es editar, no crear.
2. El mensaje de error era `"old_str not found"`, que hace creer al agente que
   el fichero existe y que solo le falta la cadena. El agente razona en
   función de esos mensajes, así que esto no era cosmético.

Ahora hay un `StrReplaceError::Missing` distinto de `NotFound` — "el fichero no
está" y "el fichero está pero no esa cadena" son problemas distintos para quien
llama — y `insert` falla con `"file not found: '<ruta>'. insert solo edita un
fichero existente; usa 'create' para crearlo"`.

**Tests:** `str_replace_sobre_ruta_inexistente_no_crea_el_fichero`,
`insert_sobre_ruta_inexistente_no_crea_el_fichero`.

---

### W2 intermitente con 10 concurrentes

**Síntoma:** A veces (no siempre) 1-2 strReplaces con marcadores diferentes no aparecen en el archivo final, aunque devuelven `ok:true`.

**Análisis:** flock funciona correctamente a nivel syscall. El test es no-determinista porque depende del orden de scheduling de `spawn_blocking` threads. Cuando la concurrencia es muy alta (10+), puede haber edge cases en cómo se serializan los `flock_exclusive` calls.

**Severidad:** baja — solo afecta concurrencia extrema (>10 strReplaces simultáneos al mismo path).

> **2026-10-03: este diagnóstico era incorrecto.** El flock sí funcionaba a
> nivel syscall; el problema no era el scheduling sino que el lock se tomaba
> sobre un inodo que `rename` sustituía. Ver "W2: Lost update real" arriba: es
> el mismo síntoma, medido y corregido. Este texto se conserva solo como
> registro de lo que se pensaba antes.

---

## Tests unitarios (6/6 OK)

```
write_file_create_writes_content         ✅
write_file_outside_sandbox_errors        ✅
atomic_write_replaces_existing_file      ✅ (W5)
atomic_write_creates_missing_file        ✅ (W4 desde writer)
str_replace_serializes_under_contention  ✅ (W2 con diferentes markers)
str_replace_same_marker_only_one_wins    ✅ (W2 correctness con mismo marker)
```

## Verificación manual live

- ✅ Functional básico: create/strReplace/insert todos OK
- ✅ Sandbox path check
- ✅ Tilde expansion
- ✅ Deep dir creation
- ✅ Edge cases (long path, unicode, 1MB)
- ✅ Truncation read (>500 líneas) con marker
- ✅ Concurrencia sin contención: 5/20/100 a paths únicos
- ✅ strReplace serialization (3-10 concurrentes con diferentes markers)
- ⚠️ strReplace 10+ concurrentes: intermitente (3/5 runs perfectos)
- ⚠️ W4 ENOENT en read concurrente (pendiente fix en read_file)

## Estado de la tool: ACEPTABLE con caveats

**Recomendación:** cerrar write_file. Los gaps restantes (W1, W4, W2 intermitente) son aceptables para el flujo LLM típico. El fix de W4 debe hacerse desde read_file cuando lo auditemos.

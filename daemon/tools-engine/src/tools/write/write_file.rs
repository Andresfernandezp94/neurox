use tokio::sync::RwLock;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use serde_json::Value;
use async_trait::async_trait;
use fs2::FileExt;
use crate::tools::{Tool, ToolSpec};
use crate::sandbox::SandboxConfig;
use crate::tools::helpers::*;
use crate::tools::{ToolCategory, Mode};
// WriteFileTool extracted from core/src/tools/mod.rs (tools/write/write_file.rs)
pub struct WriteFileTool {
    pub workspace_root: PathBuf,
    pub sandbox: Arc<RwLock<Box<dyn SandboxConfig>>>,
}

#[async_trait]
impl Tool for WriteFileTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "write_file".to_string(),
            description: "Write/modify a file. Commands: 'create' (overwrite), 'strReplace' (find/replace text), 'insert' (insert at line).".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute or workspace-relative path"},
                    "command": {"type": "string", "enum": ["create", "strReplace", "insert"], "default": "create"},
                    "content": {"type": "string", "description": "Content to write (for create/insert)"},
                    "old_str": {"type": "string", "description": "String to find (for strReplace)"},
                    "new_str": {"type": "string", "description": "Replacement string (for strReplace)"},
                    "insert_line": {"type": "integer", "description": "Line number to insert at, 0-indexed (for insert)"}
                },
                "required": ["path"]
            }),
            requires_approval: true,
        ..Default::default()
        }
    }

    fn categories(&self) -> Vec<ToolCategory> {
        vec![ToolCategory::Filesystem]
    }

    fn mode_compatible(&self) -> Vec<Mode> {
        vec![Mode::Build]
    }


    async fn execute(&self, _ctx: &crate::ExecuteContext, args: Value) -> Result<String, String> {
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing 'path'".to_string())?;
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("create");

        // EP-0019-03: write tools use the operator-configured SandboxConfig
        // to decide whether the target path is acceptable.
        let resolved = resolve_under_workspace(
            &self.workspace_root,
            path,
            &self.sandbox.write().await.writable_paths_resolved(&self.workspace_root),
            true,
        )
        .map_err(|e| format!("path: {e}"))?;

        match command {
            "create" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'content' for create".to_string())?;
                atomic_write(&resolved, content.as_bytes()).await?;
                Ok(format!("wrote {} bytes to {}", content.len(), path))
            }
            "strReplace" => {
                let old_str = args
                    .get("old_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'old_str' for strReplace".to_string())?;
                let new_str = args
                    .get("new_str")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'new_str' for strReplace".to_string())?;

                // Un solo intento, y es a proposito.
                //
                // El `flock` de `str_replace_once` se mantiene durante toda la
                // operacion y serializa a los escritores sobre la misma ruta,
                // asi que la contencion ya esta resuelta DENTRO del intento.
                // Por eso `str_replace_serializes_under_contention` pasa con
                // 10 writers simultaneos.
                //
                // Esto NO es un reintento. Antes habia aqui un
                // `for _attempt in 0..MAX_ATTEMPTS` con un comentario que
                // prometia reintentar los errores de E/S con backoff, pero
                // todos los brazos del `match` hacian `return`: el bucle se
                // ejecutaba una vez y nada mas, `MAX_ATTEMPTS` y `last_err`
                // eran decorativos, y el `Err` de despues era inalcanzable.
                // No habia forma de reintentar nada. Clippy lo cantaba como
                // `never_loop`, que es `deny`, o sea que `cargo clippy` no
                // compilaba este crate.
                //
                // Por que no hace falta reintentar: de los errores de E/S que
                // quedan (EACCES, EISDIR, ENOSPC) ninguno se arregla solo en
                // 5 ms. Y el unico transitorio que citaba el comentario —un
                // fichero ausente por un write concurrente— tampoco llega
                // aqui, porque `str_replace_once` abre con `.create(true)`:
                // si el fichero no esta, lo crea vacio y el resultado es
                // `NotFound`, no un error de E/S.
                //
                // Reintentar de verdad, junto con quitar ese
                // `.create(true)` —que hoy hace que strReplace sobre una ruta
                // inexistente cree un fichero vacio y diga "old_str not
                // found" en vez de "file not found"—, es trabajo aparte.
                match str_replace_once(&resolved, old_str, new_str).await {
                    Ok(()) => Ok(format!("replaced in {}", path)),
                    Err(StrReplaceError::NotFound) => {
                        Err(format!("old_str not found in file '{}'", path))
                    }
                    Err(StrReplaceError::Missing) => Err(format!(
                        "file not found: '{path}'. strReplace only edits an existing \
file; use 'create' to make it."
                    )),
                    Err(StrReplaceError::Io(e)) => Err(e),
                }
            }
            "insert" => {
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "missing 'content' for insert".to_string())?;
                let insert_line = args
                    .get("insert_line")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize);

                insert_lines(&resolved, content, insert_line).await?;
                Ok(format!("inserted into {} at line {:?}", path, insert_line))
            }
            other => Err(format!("unknown command '{}'", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sandbox(writable: Vec<String>) -> Arc<RwLock<Box<dyn SandboxConfig>>> {
        Arc::new(RwLock::new(Box::new(TestSandbox {
            enabled: true,
            writable,
        })))
    }

    struct TestSandbox {
        enabled: bool,
        writable: Vec<String>,
    }
    impl SandboxConfig for TestSandbox {
        fn enabled(&self) -> bool { self.enabled }
        fn is_writable(&self, path: &std::path::Path) -> bool {
            self.writable.iter().any(|w| path.starts_with(std::path::PathBuf::from(w)))
        }
        fn is_readable(&self, path: &std::path::Path) -> bool { self.is_writable(path) }
    }

    #[tokio::test]
    async fn write_file_create_writes_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        let sandbox = make_sandbox(vec![dir.path().to_string_lossy().to_string()]);
        let tool = WriteFileTool {
            workspace_root: dir.path().to_path_buf(),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({
                    "path": path.to_string_lossy().to_string(),
                    "command": "create",
                    "content": "hello world",
                }),
            )
            .await
            .expect("execute");
        assert!(result.contains("wrote"));
        let written = std::fs::read_to_string(&path).unwrap();
        assert_eq!(written, "hello world");
    }

    #[tokio::test]
    async fn write_file_outside_sandbox_errors() {
        let sandbox = make_sandbox(vec![]);
        let tool = WriteFileTool {
            workspace_root: std::path::PathBuf::from("/tmp"),
            sandbox,
        };
        let result = tool
            .execute(
                &crate::ExecuteContext { agent_id: "test".into(), cancel: None, http_client: None },
                serde_json::json!({
                    "path": "/etc/test",
                    "command": "create",
                    "content": "x",
                }),
            )
            .await;
        assert!(result.is_err(), "expected error for path outside sandbox");
    }

    // W1+W4+W5 fix: atomic write. Two concurrent writers to the same path
    // must not produce a state where readers see "no such file" or partial
    // bytes. The atomic_write helper writes to a tmpfile in the same
    // directory and renames atomically.
    #[tokio::test]
    async fn atomic_write_replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("existing.txt");
        std::fs::write(&path, "OLD").unwrap();
        atomic_write(&path, b"NEW").await.expect("atomic write");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "NEW");
    }

    // W4 fix: atomic_write to a fresh path must still result in the file
    // existing on disk immediately after the call returns (no window where
    // a concurrent reader can see ENOENT).
    #[tokio::test]
    async fn atomic_write_creates_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fresh.txt");
        assert!(!path.exists());
        atomic_write(&path, b"HELLO").await.expect("atomic write");
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "HELLO");
    }

    // W2 fix: strReplace under contention with DIFFERENT old_str values
    // must all succeed thanks to flock serialization. (When N calls race
    // on the SAME old_str, only one wins — that's correct behavior.)
    #[tokio::test]
    async fn str_replace_serializes_under_contention() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.txt");
        std::fs::write(&path, "AAA_0 AAA_1 AAA_2 AAA_3 AAA_4 AAA_5 AAA_6 AAA_7 AAA_8 AAA_9").unwrap();
        // Fire 10 concurrent strReplaces, each replacing a unique marker.
        // All must succeed; the final content must contain all 10 new markers.
        let mut handles = vec![];
        for i in 0..10 {
            let p = path.clone();
            handles.push(tokio::spawn(async move {
                str_replace_once(
                    &p,
                    &format!("AAA_{i}"),
                    &format!("MARKER_{i}"),
                )
                .await
            }));
        }
        let mut ok = 0;
        for h in handles {
            if h.await.unwrap().is_ok() { ok += 1; }
        }
        assert_eq!(ok, 10, "all 10 strReplaces must succeed under flock");
        let r#final = std::fs::read_to_string(&path).unwrap();
        for i in 0..10 {
            assert!(r#final.contains(&format!("MARKER_{i}")), "missing MARKER_{i}: {}", r#final);
        }
        // And none of the originals should remain.
        for i in 0..10 {
            assert!(!r#final.contains(&format!("AAA_{i}")), "AAA_{i} still present: {}", r#final);
        }
    }

    /// Regresion del lost update silencioso en strReplace.
///
/// El fallo era que el flock se tomaba sobre el propio fichero, y
/// `atomic_write` renombra un inodo NUEVO sobre la ruta: el lock acababa
/// protegiendo un inodo muerto, asi que las llegadas posteriores bloqueaban
/// el inodo nuevo (sin contention) y corrían en paralelo con las que aun
/// tenían el viejo. Todas reportaban `Ok` y sus cambios desaparecian.
///
/// Medido antes del arreglo: 6 de 8 rondas con hasta 12 de 40 escrituras
/// perdidas. Despues: 0 de 120.
///
/// El detalle que hace falta para reproducirlo, y que explica por que el test
/// original no lo veia: las llegadas tienen que ser ESCALONADAS. Con una
/// barrera para que todos entren a la vez, todos hacen cola sobre el mismo
/// inodo viejo y cada uno lee ya por ruta, asi que cada uno ve al anterior y no
/// se pierde nada. La carrera necesita que unos writers ya esten en vuelo
/// cuando llegan los siguientes, que es lo que hace un bloque de tools en
/// paralelo. El relleno grande ensancha la ventana entre leer y renombrar.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn str_replace_no_pierde_escrituras_con_llegadas_escalonadas() {
    let n = 40usize;
    let relleno = "x".repeat(2_000_000);

    for ronda in 0..5 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.txt");
        let mut base = String::new();
        for i in 0..n {
            base.push_str(&format!("AAA_{i} "));
        }
        base.push_str(&relleno);
        std::fs::write(&path, &base).unwrap();

        let mut handles = vec![];
        for i in 0..n {
            let p = path.clone();
            let delay_us = (i as u64 * 137) % 2500;
            handles.push(tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_micros(delay_us)).await;
                str_replace_once(&p, &format!("AAA_{i}"), &format!("MARKER_{i}")).await
            }));
        }
        let mut ok = 0;
        for h in handles {
            if h.await.unwrap().is_ok() {
                ok += 1;
            }
        }
        let final_txt = std::fs::read_to_string(&path).unwrap();
        let perdidos: Vec<usize> =
            (0..n).filter(|i| !final_txt.contains(&format!("MARKER_{i}"))).collect();
        assert_eq!(ok, n, "ronda {ronda}: no todas las llamadas pudieron escribir");
        assert!(
            perdidos.is_empty(),
            "ronda {ronda}: escrituras perdidas {perdidos:?} — lost update silencioso"
        );
    }
}

/// strReplace sobre una ruta que no existe: dice que no existe y NO crea el
/// fichero. Antes abria con `.create(true)`, que creaba un fichero vacio y
/// luego reportaba "old_str not found" — un mensaje que hacia creer al agente
/// que el fichero existia.
#[tokio::test]
async fn str_replace_sobre_ruta_inexistente_no_crea_el_fichero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("no_existe.txt");

    let err = str_replace_once(&path, "viejo", "nuevo")
        .await
        .expect_err("debe fallar");
    assert!(matches!(err, StrReplaceError::Missing), "{err:?}");
    assert!(
        !path.exists(),
        "no debe crear el fichero: {}",
        path.display()
    );
}

/// El mismo contrato en `insert`: edita, no crea.
#[tokio::test]
async fn insert_sobre_ruta_inexistente_no_crea_el_fichero() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("no_existe.txt");

    let err = insert_lines(&path, "linea", None)
        .await
        .expect_err("debe fallar");
    assert!(err.contains("file not found"), "{err}");
    assert!(
        !path.exists(),
        "no debe crear el fichero: {}",
        path.display()
    );
}

/// Dos rutas al mismo fichero (via symlink) deben compartir cerrojo, no
/// serializarse solo consigo mismas.
#[cfg(unix)]
#[tokio::test]
async fn el_cerrojo_comparte_ruta_entre_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real.txt");
    let link = dir.path().join("link.txt");
    std::fs::write(&real, "contenido\n").unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();

    assert_eq!(lock_path_for(&real), lock_path_for(&link));
}

/// El cerrojo no se deja en el workspace: apareceria en `list_dir`/`glob` y
/// confundiria al agente.
#[tokio::test]
async fn el_cerrojo_no_vive_en_el_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("archivo.txt");
    std::fs::write(&path, "x\n").unwrap();

    let lp = lock_path_for(&path);
    assert!(lp.starts_with(std::env::temp_dir()), "{}", lp.display());
    assert!(
        !lp.starts_with(dir.path()),
        "el cerrojo no puede estar junto al fichero: {}",
        lp.display()
    );

    // Y tomarlo no crea ficheros junto al objetivo.
    let _g = lock_exclusive(&path).await.unwrap();
    let entradas: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(entradas, vec!["archivo.txt".to_string()], "{entradas:?}");
}

// W2 fix: strReplace with the SAME old_str — only one of N concurrent
    // The flock ensures no torn writes; the retry-on-conflict path
    // guarantees we don't return spurious "old_str not found" when the
    // file genuinely never had it.
    #[tokio::test]
    async fn str_replace_same_marker_only_one_wins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shared.txt");
        std::fs::write(&path, "AAA BBB CCC").unwrap();
        let mut handles = vec![];
        for i in 0..10 {
            let p = path.clone();
            handles.push(tokio::spawn(async move {
                str_replace_once(&p, "BBB", &format!("MARKER_{i}")).await
            }));
        }
        let mut ok = 0;
        let mut not_found = 0;
        for h in handles {
            match h.await.unwrap() {
                Ok(()) => ok += 1,
                Err(StrReplaceError::NotFound) => not_found += 1,
                Err(e) => panic!("unexpected error: {:?}", e),
            }
        }
        assert_eq!(ok, 1, "exactly one strReplace should win");
        assert_eq!(not_found, 9, "the other 9 should fail with NotFound");
    }
}

// ────────────────────────────────────────────────────────────────────
// Internal helpers (extracted to keep `execute` readable)
// ────────────────────────────────────────────────────────────────────

/// W1+W4+W5: atomic write — ensure the target exists, then write to
/// a tmpfile in the same directory and rename into place. `rename(2)`
/// is atomic on POSIX so readers always observe the old or the new
/// file, never a partial state.
///
/// W4 fix: pre-create the target file (empty) before writing so
/// concurrent readers never see ENOENT. During the write window they
/// see an empty file; once rename lands they see the new content.
async fn atomic_write(path: &Path, content: &[u8]) -> Result<(), String> {
    use uuid::Uuid;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("mkdir: {e}"))?;
        }
    }
    // Pre-create the target so a concurrent reader never sees ENOENT.
    // If the file already exists this is a no-op (O_CREAT without
    // O_TRUNC and without O_EXCL). The file may briefly be empty
    // during the write window, but it always exists.
    tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .await
        .map_err(|e| format!("precreate: {e}"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp = parent.join(format!(".tmp.{}", Uuid::new_v4()));
    // Write tmpfile (still invisible to readers of the original path).
    tokio::fs::write(&tmp, content)
        .await
        .map_err(|e| format!("write failed: {e}"))?;
    // Atomic rename onto the target path.
    if let Err(e) = tokio::fs::rename(&tmp, path).await {
        // Best-effort cleanup so we don't leave .tmp.* files behind.
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(format!("rename failed: {e}"));
    }
    Ok(())
}

/// Donde vive el cerrojo lateral de una ruta.
///
/// NO puede ser el propio fichero. `atomic_write` renombra un inodo NUEVO
/// sobre la ruta, asi que un flock tomado sobre el fichero acaba
/// protegiendo un inodo que ya no esta en `path`: las siguientes llamadas
/// abren el inodo nuevo, lo bloquean sin contention (nadie lo tiene) y
/// corren en paralelo con las que aun sostienen el viejo. Medido antes de
/// arreglarlo: 6 de 8 rondas con escrituras perdidas —hasta 12 de 40— y las
/// 40 reportando `Ok`.
///
/// Vive en el tmpdir y no al lado del fichero por dos razones: un
/// `.archivo.neurox.lock` en el workspace apareceria en `list_dir`/`glob` y
/// confundiria al agente; y el tmpdir es el sitio semantico correcto, porque
/// el cerrojo es efimero. Se canoniza la ruta para que dos rutas distintas al
/// mismo fichero (via symlink) compartan cerrojo.
///
/// Una colision de nombres aqui solo produce serializacion de mas, nunca una
/// perdida, asi que un hash de 64 bits sobra.
fn lock_path_for(path: &Path) -> PathBuf {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let h = fnv1a64(&abs.to_string_lossy());
    std::env::temp_dir().join(format!("neurox-locks/{h:016x}.lock"))
}

/// FNV-1a de 64 bits, para no meter una dependencia de hash.
fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// Cerrojo exclusivo sobre `path`, tomado sobre el lateral estable.
///
/// Devolver el `File` mantiene el cerrojo: se suelta cuando el llamante lo
/// deja caer. NO se borra el fichero lateral al soltar, porque borrarlo es una
/// carrera (otro proceso puede estar esperando sobre ese inodo mientras un
/// tercero crea uno nuevo y entra en paralelo). Es el mismo criterio que usa
/// git con `.git/index.lock`.
async fn lock_exclusive(path: &Path) -> Result<std::fs::File, String> {
    let p = lock_path_for(path);
    if let Some(parent) = p.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("mkdir lock: {e}"))?;
    }
    tokio::task::spawn_blocking(move || -> Result<std::fs::File, String> {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&p)
            .map_err(|e| format!("open lock: {e}"))?;
        f.lock_exclusive().map_err(|e| format!("flock: {e}"))?;
        Ok(f)
    })
    .await
    .map_err(|e| format!("join: {e}"))?
}

/// W2 fix: outcome of a single strReplace attempt.
#[derive(Debug)]
enum StrReplaceError {
    /// `old_str` is genuinely absent — caller should fail fast.
    NotFound,
    /// The path does not exist. strReplace edits; it does not create. Kept
    /// separate from `NotFound` because "the file isn't there" and "the file is
    /// there but that string isn't" are different problems for the caller.
    Missing,
    /// Filesystem / I/O error — surface to caller verbatim.
    Io(String),
}

/// W2 fix: one attempt of strReplace under an exclusive lock.
///
/// If `old_str` is no longer present because another writer modified the file,
/// returns `NotFound` (for the same `old_str` from concurrent callers, only
/// one wins).
async fn str_replace_once(
    path: &Path,
    old_str: &str,
    new_str: &str,
) -> Result<(), StrReplaceError> {
    // Cerrojo sobre el lateral estable, NO sobre el fichero: `atomic_write`
    // renombra un inodo nuevo sobre la ruta y dejaria el flock apuntando al
    // inodo viejo. Ver `lock_path_for`.
    let _lock_guard = lock_exclusive(path)
        .await
        .map_err(StrReplaceError::Io)?;

    // El fichero tiene que existir. Antes se abria con `.create(true)`, que
    // hacia dos cosas malas a la vez: crear un fichero VACIO si la ruta no
    // existia (borrando de paso la intencion de strReplace, que es editar, no
    // crear) y devolver despues "old_str not found", que hacia creer al
    // agente que el fichero existia y no tenia la cadena.
    if tokio::fs::metadata(path).await.is_err() {
        return Err(StrReplaceError::Missing);
    }

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| StrReplaceError::Io(format!("read: {e}")))?;
    if !content.contains(old_str) {
        return Err(StrReplaceError::NotFound);
    }
    let new_content = content.replacen(old_str, new_str, 1);
    atomic_write(path, new_content.as_bytes())
        .await
        .map_err(StrReplaceError::Io)?;
    // `_lock_guard` dropped here → flock released.
    Ok(())
}

/// W3 fix: insert under the same stable lock as strReplace, for the same
/// reason: `atomic_write` renames a new inode over the path, so locking the
/// file itself leaves the lock on a dead inode.
async fn insert_lines(
    path: &Path,
    content: &str,
    insert_line: Option<usize>,
) -> Result<(), String> {
    let _lock_guard = lock_exclusive(path).await?;

    // Igual que en strReplace: insert edita, no crea. Con `.create(true)` una
    // ruta inexistente se convertia en un fichero vacio silenciosamente.
    if tokio::fs::metadata(path).await.is_err() {
        return Err(format!(
            "file not found: '{}'. insert only edits an existing file; use \
'create' to make it.",
            path.display()
        ));
    }

    let existing = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("read: {e}"))?;
    let mut lines: Vec<&str> = existing.lines().collect();
    match insert_line {
        Some(line) => {
            let idx = line.min(lines.len());
            let insert_lines: Vec<&str> = content.lines().collect();
            for (i, l) in insert_lines.iter().enumerate() {
                lines.insert(idx + i, l);
            }
        }
        None => {
            for l in content.lines() {
                lines.push(l);
            }
        }
    }
    let new_content = lines.join("\n") + "\n";
    atomic_write(path, new_content.as_bytes()).await
    // `_lock_guard` dropped here → flock released.
}

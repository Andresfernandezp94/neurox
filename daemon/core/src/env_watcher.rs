//! Env-file watchdog (EP-0018-05).
//!
//! Background task that polls `~/.config/neurox/env` (or wherever
//! `NEUROX_ENV_FILE` points) and propagates changes to the daemon
//! process environment via `std::env::set_var` / `remove_var`.
//!
//! Why polling and not inotify:
//! - The file lives at a path we control; the overhead of a 5-second
//!   poll is negligible (the file is rarely written to).
//! - Avoids adding `notify` as a dep just for one feature.
//! - Works uniformly on macOS dev hosts and Linux prod hosts.
//!
//! Scope:
//! - **In-process** consumers (default, tools) see
//!   the new value immediately on the next `std::env::var(...)` call.
//! - **Subprocesses spawned after the change** inherit the new env
//!   automatically (fork copies the process env).
//! - **Subprocesses spawned before the change** keep their frozen env
//!   until restarted — restarting the orchestrator-managed services is
//!   a separate concern (the operator can `POST /start` to recycle them).
//! - **External components** read `EnvironmentFile`
//!   only at `daemon-reload + restart`. They are NOT notified by this
//!   watcher (out of scope for the MVP).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::environments::env_file_path;

/// Default poll interval. 5s is responsive enough for ops (typing in the
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Background watcher loop. Spawn via `tokio::spawn`. Returns only if the
/// file path resolution fails at startup (e.g. `HOME` unset) — otherwise
/// it runs forever, logging diffs on every change.
/// Run the env-file watchdog loop. `on_change` is called (await) whenever
/// the snapshot of `~/.config/neurox/env` differs from the previous one
/// (so consumers like the LLM catalog cache can be invalidated).
pub async fn watch_loop<F, Fut>(on_change: F)
where
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    let path = match env_file_path().canonicalize() {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "env_watcher: could not resolve env file path; watcher disabled");
            return;
        }
    };
    info!(
        path = %path.display(),
        poll_secs = POLL_INTERVAL.as_secs(),
        "env_watcher: started"
    );

    let mut ticker = interval(POLL_INTERVAL);
    // First tick fires immediately; we don't want that — wait for the
    // first interval before reading.
    ticker.tick().await;

    let mut cache: BTreeMap<String, String> = read_kv(&path).unwrap_or_default();
    // Propagate the initial snapshot to the process env so providers that
    // declare `api_key_env = "<key>"` are marked as configured on the first
    // catalog build (without waiting for the env file to change).
    if !cache.is_empty() {
        let initial_changed = apply_diff(&BTreeMap::new(), &cache, &path);
        if initial_changed {
            on_change().await;
        }
    }

    loop {
        ticker.tick().await;
        if tick(&mut cache, &path) {
            on_change().await;
        }
    }
}

/// Parse the env file into a sorted map of KEY=value pairs. Lines that
/// don't match the `^[A-Z][A-Z0-9_]*=` pattern (comments, blanks) are
/// ignored.
fn read_kv(path: &Path) -> std::io::Result<BTreeMap<String, String>> {
    let content = std::fs::read_to_string(path)?;
    let mut out = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let k = k.trim();
            if crate::environments::validate_key(k).is_ok() {
                out.insert(k.to_string(), v.to_string());
            }
        }
    }
    Ok(out)
}

/// Apply the diff between the previous snapshot and the new one. Sets
/// new keys. **Does NOT remove keys** — see `EP-2026-09-02` notes below.
///
/// EP-2026-09-02: auto-removal was a footgun. Two problems:
///   1. In a multi-threaded process (tokio) `std::env::remove_var` is
///      marked `unsafe` since Rust 1.65 because libc's unsetenv modifies
///      a per-libc copy of the environ array that `std::env::var` later
///      reads. The kernel-managed `/proc/<pid>/environ` keeps the
///      original, but the daemon's libc view is left permanently
///      corrupted — there's no safe way to re-sync without a restart.
///   2. Any partial write to the env file (PUT /v1/env/:key, manual
///      edit, accidental overwrite) would silently nuke every key not
///      in the new file. Operators only learned about it via 401s and
///      the chat model selector going blank.
///
/// The canonical way to remove a key is `DELETE /v1/env/:key`, which
/// already handles the lifecycle explicitly. The watcher is now
/// strictly additive.
fn apply_diff(
    prev: &BTreeMap<String, String>,
    next: &BTreeMap<String, String>,
    path: &Path,
) -> bool {
    let mut changed = false;
    // Added or updated keys.
    for (k, v) in next {
        if prev.get(k) != Some(v) {
            info!(
                key = %k,
                path = %path.display(),
                "env_watcher: propagated env var to daemon process"
            );
            std::env::set_var(k, v);
            changed = true;
        }
    }
    changed
}

// NOTA: `touch_after_write()` fue eliminado.
//
// Abria el archivo sin escribir nada y su propio comentario decia que el
// mtime "no se consulta": el watcher diffea contenido, no timestamps. Era un
// no-op disfrazado de "nudge al poll".
//
// Los callers (PUT/DELETE /v1/env/:key) lo invocaban esperando acelerar la
// propagacion. No aceleraba nada: la escritura ya es visible para el proximo
// `read_kv`, y el plazo real es POLL_INTERVAL (5s). Los callers dejaron de
// llamarlo y el PUT sigue siendo correcto sin el.
//
// Se elimina en vez de dejar un stub porque un stub con el mismo nombre
// compila igual y sugiere que la funcion existe. Si hace falta un nudge real,
// tiene que empujar la escritura, no tocar el mtime.

/// Test-only helper: synchronously apply a snapshot (used by the
/// integration test to bypass the polling interval).
pub fn apply_snapshot(snapshot: &BTreeMap<String, String>) {
    let Some(path) = env_file_path().canonicalize().ok() else {
        return;
    };
    let prev = read_kv(&path).unwrap_or_default();
    apply_diff(&prev, snapshot, &path);
    // Note: this is intentionally best-effort — production code uses the
    // polling watcher. Tests use `apply_snapshot` to skip the wait.
}

/// Un tick del loop: lee el archivo, aplica el diff contra `cache` y
/// devuelve el `cache` nuevo.
///
/// Se extrae del `loop` para poder testearlo sin esperar 5s por caso. La
/// firma con `&mut BTreeMap` es lo que hace testeable el bug que tenía el
/// loop original: que `cache` quedara en un estado distinto del aplicado.
fn tick(cache: &mut BTreeMap<String, String>, path: &Path) -> bool {
    match read_kv(path) {
        Ok(next) => {
            let changed = apply_diff(cache, &next, path);
            // `cache` tiene que quedar en `next`, NO en una re-lectura: si
            // el archivo cambia entre la lectura y esta asignacion, el
            // watcher queda desincronizado (el `cache` reflejaria un estado
            // que nunca se aplico al proceso) y el proximo diff compara
            // contra algo que el proceso no tiene.
            *cache = next;
            changed
        }
        Err(e) => {
            // No se toca `cache` en error de lectura. Reemplazarla por un
            // vacio haria que al volver a leer se re-aplicara todo, lo que
            // dispara un rebuild de catalogo por un error de I/O pasajero.
            debug!(error = %e, path = %path.display(), "env_watcher: read failed");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// `std::env::set_var` muta el entorno del PROCESO, compartido por todos
    /// los tests del binario. Sin este lock, dos tests que usen la misma key
    /// se pisan y el fallo aparece en el test que no lo.proto.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    struct Fixture {
        dir: std::path::PathBuf,
    }

    impl Fixture {
        /// Archivo de env + `NEUROX_ENV_FILE` apuntando a él. Cada test
        /// tiene el suyo para no pisar el archivo real del daemon.
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "np-watcher-{tag}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let file = dir.join("env");
            std::fs::write(&file, "").unwrap();
            std::env::set_var("NEUROX_ENV_FILE", &file);
            Self { dir }
        }

        fn path(&self) -> std::path::PathBuf {
            self.dir.join("env")
        }

        /// Escribe el archivo como lo haría el operador a mano.
        fn write(&self, contents: &str) {
            std::fs::write(self.path(), contents).unwrap();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::env::remove_var("NEUROX_ENV_FILE");
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn tick_propagates_a_new_key_to_the_process() {
        let _g = lock();
        let fx = Fixture::new("new-key");
        fx.write("NP_WATCH_A=hello\n");

        let mut cache = BTreeMap::new();
        assert!(
            tick(&mut cache, &fx.path()),
            "new key must count as changed"
        );
        assert_eq!(std::env::var("NP_WATCH_A").ok().as_deref(), Some("hello"));
        std::env::remove_var("NP_WATCH_A");
    }

    #[test]
    fn second_tick_without_change_reports_no_change() {
        let _g = lock();
        let fx = Fixture::new("no-change");
        fx.write("NP_WATCH_B=v1\n");

        let mut cache = BTreeMap::new();
        assert!(tick(&mut cache, &fx.path()));
        // Segundo tick, archivo idéntico: no hay nada que aplicar. Si
        // reportara cambio, cada poll dispararía un rebuild de catálogo.
        assert!(!tick(&mut cache, &fx.path()));
        std::env::remove_var("NP_WATCH_B");
    }

    #[test]
    fn tick_detects_a_changed_value() {
        let _g = lock();
        let fx = Fixture::new("changed");
        fx.write("NP_WATCH_C=v1\n");

        let mut cache = BTreeMap::new();
        assert!(tick(&mut cache, &fx.path()));

        fx.write("NP_WATCH_C=v2\n");
        assert!(tick(&mut cache, &fx.path()));
        assert_eq!(std::env::var("NP_WATCH_C").ok().as_deref(), Some("v2"));
        std::env::remove_var("NP_WATCH_C");
    }

    /// El bug que tenía el loop original: re-leía el archivo para actualizar
    /// `cache`, así que `cache` podía quedar en un estado que nunca se
    /// aplicó al proceso.
    #[test]
    fn cache_stays_in_sync_with_what_was_applied() {
        let _g = lock();
        let fx = Fixture::new("sync");
        fx.write("NP_WATCH_D=v1\n");

        let mut cache = BTreeMap::new();
        tick(&mut cache, &fx.path());

        // Lo que quedó en `cache` tiene que ser exactamente lo que el
        // proceso tiene. Con la re-lectura anterior, `cache` podía
        // desincronizarse y el próximo diff comparaba contra un estado
        // que el proceso nunca tuvo.
        let applied = cache.get("NP_WATCH_D").cloned();
        assert_eq!(applied.as_deref(), Some("v1"));
        assert_eq!(
            std::env::var("NP_WATCH_D").ok().as_deref(),
            applied.as_deref()
        );
        std::env::remove_var("NP_WATCH_D");
    }

    #[test]
    fn a_change_written_between_ticks_is_not_missed() {
        // Con el bug del desync: `cache` reflejaba la re-lectura (v2), el
        // proceso tenía v1. Al siguiente cambio a v3, el diff comparaba
        // v2 contra v3 y aplicaba v3 — funcionaba por casualidad. Pero si
        // el archivo volvía a v1, `cache` ya decía v2, `next` decía v1 y
        // aplicaba v1 sin avisar del todo el vaivén. Acá se verifica el
        // camino feliz: todo cambio real se aplica.
        let _g = lock();
        let fx = Fixture::new("between");
        fx.write("NP_WATCH_E=v1\n");

        let mut cache = BTreeMap::new();
        tick(&mut cache, &fx.path());
        fx.write("NP_WATCH_E=v2\n");
        assert!(tick(&mut cache, &fx.path()));
        assert_eq!(std::env::var("NP_WATCH_E").ok().as_deref(), Some("v2"));
        std::env::remove_var("NP_WATCH_E");
    }

    #[test]
    fn read_error_keeps_cache_intact() {
        let _g = lock();
        let fx = Fixture::new("read-error");
        fx.write("NP_WATCH_F=v1\n");

        let mut cache = BTreeMap::new();
        tick(&mut cache, &fx.path());
        let before = cache.clone();

        // Archivo que no existe: `read_kv` falla.
        std::fs::remove_file(fx.path()).unwrap();
        assert!(!tick(&mut cache, &fx.path()));
        // `cache` intacto: si se hubiera vaciado, al volver el archivo
        // todas las keys se verían como nuevas y se re-aplicarían, con un
        // rebuild de catálogo de yapa por un error de I/O pasajero.
        assert_eq!(cache, before);
        std::env::remove_var("NP_WATCH_F");
    }

    #[test]
    fn read_kv_skips_comments_blanks_and_invalid_names() {
        let _g = lock();
        let fx = Fixture::new("parse");
        fx.write("# comentario\n\nNP_WATCH_OK=1\nlower=skip\nNP-WITH-DASH=skip\n2LEADING=skip\n");
        let map = read_kv(&fx.path()).unwrap();
        assert!(map.contains_key("NP_WATCH_OK"));
        assert!(!map.contains_key("lower"));
        assert!(!map.contains_key("NP-WITH-DASH"));
        assert!(!map.contains_key("2LEADING"));
    }

    #[test]
    fn apply_diff_does_not_remove_keys() {
        // Documenta la decisión de diseño: `remove_var` es `unsafe` desde
        // Rust 1.65 porque libc mantiene su propia copia de `environ`. El
        // watcher es aditivo y borrar es un acto explícito vía
        // DELETE /v1/env/:key.
        let _g = lock();
        std::env::set_var("NP_WATCH_KEEP", "still-here");
        let path = std::path::PathBuf::from("/nonexistent/env");
        let prev = BTreeMap::from([("NP_WATCH_KEEP".to_string(), "still-here".to_string())]);
        let next = BTreeMap::new(); // todo "borrado"
        let _ = apply_diff(&prev, &next, &path);
        // Sigue presente: el watcher no quita.
        assert_eq!(
            std::env::var("NP_WATCH_KEEP").ok().as_deref(),
            Some("still-here")
        );
        std::env::remove_var("NP_WATCH_KEEP");
    }
}

//! Workspaces: entornos aislados con su propia raiz y su propio sandbox.
//!
//! Un workspace es un directorio mas un sandbox. Es lo que el operador ve
//! en la tab General de la vista Workspace: lo agrega, lo edita y lo
//! removes, y cada agente desplegado adentro trabaja con los permisos de
//! ese entorno y no con los de otro.
//!
//! ## Donde queda el aislamiento
//!
//! El subproceso del agente nunca ve el workspace: las tools se ejecutan en
//! el proceso del daemon (`router::handle_session_tool_call`). Asi que el
//! aislamiento es 100% de este lado, y se resuelve en un solo punto:
//!
//! 1. La sesion sabe que workspace tiene (`sessions.workspace_id`).
//! 2. Al despachar un mensaje, el daemon resuelve ese workspace a un
//!    `WorkspaceScope` con la raiz ya resuelta y el sandbox de ESE
//!    workspace, y lo mete en el `ExecuteContext`.
//! 3. Los tools leen el par del contexto en vez de su par propio, que queda
//!    como default. Ver `tools_engine::ExecuteContext::scope`.
//!
//! No se reconstruye el registro de tools por workspace: hay un solo
//! `ToolRegistry` y los tool_calls se resuelven por nombre contra el.
//!
//! ## Por que el sandbox viaja en un Arc por workspace
//!
//! Con un unico Arc compartido, dos workspaces con permisos distintos
//! serian indistinguibles para el tool y cambiar uno pisaria al otro. Es el
//! mismo bug que hacia que configurar el sandbox desde la UI no llegara a
//! las tools.
//!
//! ## Persistencia
//!
//! SQLite con el patron del resto del daemon (`CREATE TABLE IF NOT EXISTS`
//! idempotente, no hay directorio de migraciones). No se toca `config.yaml`,
//! asi que los comentarios del archivo del operador se conservan.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::SandboxConfig;

/// Estado de un workspace. El daemon no usa esto para decidir nada: es la
/// etiqueta que muestra la UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceStatus {
    Active,
    Paused,
    Draft,
}

impl WorkspaceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkspaceStatus::Active => "active",
            WorkspaceStatus::Paused => "paused",
            WorkspaceStatus::Draft => "draft",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "paused" => WorkspaceStatus::Paused,
            "draft" => WorkspaceStatus::Draft,
            _ => WorkspaceStatus::Active,
        }
    }
}

/// Un workspace persistido.
///
/// Los paths del sandbox se guardan SIN resolver, con los placeholders
/// intactos (`${workspace}`, `${home}`). `${workspace}` se expande contra la
/// raiz de ESTE workspace cuando el tool lo lee, no contra el root global:
/// en un sandbox por workspace el placeholder tiene que significar el
/// entorno que lo declaro.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Directorio raiz del workspace.
    pub root: String,
    pub status: WorkspaceStatus,
    pub icon: Option<String>,
    /// Workspace por defecto cuando una sesion no pide uno. A lo sumo uno
    /// tiene el flag.
    pub is_default: bool,
    pub sandbox: SandboxConfig,
    pub created_at: String,
    pub updated_at: String,
}

impl WorkspaceRecord {
    /// Sandbox listo para meter en un `Box<dyn SandboxConfig>`.
    ///
    /// Los placeholders no se expanden aca: se resuelven en la lectura, con
    /// el root del workspace. Ver `resolve_one`.
    pub fn sandbox_config(&self) -> SandboxConfig {
        self.sandbox.clone()
    }

    /// `true` si `path` cae dentro del root del workspace.
    pub fn owns(&self, path: &Path) -> bool {
        path.starts_with(Path::new(&self.root))
    }
}

/// Lo que se puede cambiar de un workspace. `None` = no tocar.
///
/// Los defaults salen de `WorkspaceRecord`, asi que un PATCH parcial no
/// pisa lo que no menciona.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorkspacePatch {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub root: Option<String>,
    pub status: Option<WorkspaceStatus>,
    pub icon: Option<Option<String>>,
    pub is_default: Option<bool>,
    pub sandbox_enabled: Option<bool>,
    pub sandbox_writable_paths: Option<Vec<String>>,
    pub sandbox_readable_paths: Option<Vec<String>>,
    pub sandbox_max_recursion_depth: Option<usize>,
}

pub struct WorkspaceStore {
    pool: sqlx::SqlitePool,
}

const SELECT_COLS: &str = "id, name, description, root, status, icon, is_default, \
     sandbox_enabled, sandbox_writable, sandbox_readable, sandbox_depth, created_at, updated_at";

type Row = (
    String,             // id
    String,             // name
    Option<String>,     // description
    String,             // root
    String,             // status
    Option<String>,     // icon
    i64,                // is_default
    i64,                // sandbox_enabled
    String,             // sandbox_writable (JSON)
    String,             // sandbox_readable (JSON)
    i64,                // sandbox_depth
    String,             // created_at
    String,             // updated_at
);

impl WorkspaceStore {
    pub async fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = sqlx::SqlitePool::connect(&url).await?;
        sqlx::query("PRAGMA journal_mode=WAL").execute(&pool).await.ok();
        sqlx::query("PRAGMA synchronous=NORMAL").execute(&pool).await.ok();
        Self::ensure_table(&pool).await?;
        Ok(Self { pool })
    }

    /// Tabla de workspaces. Idempotente, igual que el resto del daemon.
    ///
    /// Los paths del sandbox van como JSON en TEXT. Son listas cortas y se
    /// leen siempre enteras, asi que una columna JSON no compra nada frente
    /// a una tabla con una fila por path, que ademas habria que mantener
    /// sincronizada con el DELETE.
    pub async fn ensure_table(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id               TEXT PRIMARY KEY,
                name             TEXT NOT NULL,
                description      TEXT,
                root             TEXT NOT NULL,
                status           TEXT NOT NULL DEFAULT 'active',
                icon             TEXT,
                is_default       INTEGER NOT NULL DEFAULT 0,
                sandbox_enabled  INTEGER NOT NULL DEFAULT 1,
                sandbox_writable TEXT NOT NULL DEFAULT '[]',
                sandbox_readable TEXT NOT NULL DEFAULT '[]',
                sandbox_depth    INTEGER NOT NULL DEFAULT 10,
                created_at       TEXT NOT NULL,
                updated_at       TEXT NOT NULL
            )",
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Id libre a partir del nombre, igual que el slug que ya usaba la UI
    /// mock (`name.toLowerCase().replace(/[^a-z0-9-]+/g, "-")`).
    async fn unique_id(&self, name: &str) -> anyhow::Result<String> {
        let base: String = name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let mut slug: String = base
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        if slug.is_empty() {
            slug = "workspace".to_string();
        }
        let mut candidate = slug.clone();
        let mut n = 2;
        while self.exists(&candidate).await? {
            candidate = format!("{slug}-{n}");
            n += 1;
        }
        Ok(candidate)
    }

    async fn exists(&self, id: &str) -> anyhow::Result<bool> {
        let found: Option<String> =
            sqlx::query_scalar("SELECT id FROM workspaces WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(found.is_some())
    }

    pub async fn list(&self) -> anyhow::Result<Vec<WorkspaceRecord>> {
        // `SELECT_COLS` es una constante de compilacion, no input de usuario: el
        // `AssertSqlSafe` de sqlx 0.9 es el puente declarado para eso.
        let sql =
            sqlx::AssertSqlSafe(format!("SELECT {SELECT_COLS} FROM workspaces ORDER BY is_default DESC, name ASC"));
        let rows: Vec<Row> = sqlx::query_as(sql).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(row_to_record).collect())
    }

    pub async fn get(&self, id: &str) -> anyhow::Result<Option<WorkspaceRecord>> {
        let sql =
            sqlx::AssertSqlSafe(format!("SELECT {SELECT_COLS} FROM workspaces WHERE id = ?"));
        let row: Option<Row> = sqlx::query_as(sql).bind(id).fetch_optional(&self.pool).await?;
        Ok(row.map(row_to_record))
    }

    pub async fn create(
        &self,
        name: &str,
        root: &str,
        patch: &WorkspacePatch,
    ) -> anyhow::Result<WorkspaceRecord> {
        let id = self.unique_id(name).await?;
        let now = chrono::Utc::now().to_rfc3339();
        let sandbox = patch_sandbox(&SandboxConfig::default(), patch);
        sqlx::query(
            "INSERT INTO workspaces
             (id, name, description, root, status, icon, is_default,
              sandbox_enabled, sandbox_writable, sandbox_readable, sandbox_depth,
              created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(patch.description.clone().flatten())
        .bind(root)
        .bind(patch.status.unwrap_or(WorkspaceStatus::Active).as_str())
        .bind(patch.icon.clone().flatten())
        .bind(patch.is_default.unwrap_or(false) as i64)
        .bind(sandbox.enabled as i64)
        .bind(serde_json::to_string(&sandbox.writable_paths)?)
        .bind(serde_json::to_string(&sandbox.readable_paths)?)
        .bind(sandbox.max_recursion_depth as i64)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        if patch.is_default == Some(true) {
            self.clear_other_defaults(&id).await?;
        }
        self.get(&id).await?.ok_or_else(|| anyhow::anyhow!("workspace creado pero no se lee: {id}"))
    }

    /// PATCH parcial. Solo toca las columnas presentes en el patch.
    pub async fn update(&self, id: &str, patch: &WorkspacePatch) -> anyhow::Result<WorkspaceRecord> {
        let Some(current) = self.get(id).await? else {
            anyhow::bail!("workspace not found: {id}");
        };
        let sandbox = patch_sandbox(&current.sandbox, patch);
        let name = patch.name.clone().unwrap_or(current.name);
        let description = match &patch.description {
            Some(v) => v.clone(),
            None => current.description,
        };
        let root = patch.root.clone().unwrap_or(current.root);
        let status = patch.status.unwrap_or(current.status);
        let icon = match &patch.icon {
            Some(v) => v.clone(),
            None => current.icon,
        };
        let is_default = patch.is_default.unwrap_or(current.is_default);

        sqlx::query(
            "UPDATE workspaces SET
                name = ?, description = ?, root = ?, status = ?, icon = ?, is_default = ?,
                sandbox_enabled = ?, sandbox_writable = ?, sandbox_readable = ?,
                sandbox_depth = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(name)
        .bind(description)
        .bind(root)
        .bind(status.as_str())
        .bind(icon)
        .bind(is_default as i64)
        .bind(sandbox.enabled as i64)
        .bind(serde_json::to_string(&sandbox.writable_paths)?)
        .bind(serde_json::to_string(&sandbox.readable_paths)?)
        .bind(sandbox.max_recursion_depth as i64)
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;

        if patch.is_default == Some(true) {
            self.clear_other_defaults(id).await?;
        }
        self.get(id).await?.ok_or_else(|| anyhow::anyhow!("workspace no leido tras update: {id}"))
    }

    /// Borra el registro. NO borra el directorio: el directorio es del
    /// usuario y puede tener su trabajo adentro. Remover un workspace
    /// revoca el acceso, no destruye nada.
    pub async fn delete(&self, id: &str) -> anyhow::Result<()> {
        let result = sqlx::query("DELETE FROM workspaces WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            anyhow::bail!("workspace not found: {id}");
        }
        Ok(())
    }

    /// Deja un solo default.
    async fn clear_other_defaults(&self, keep: &str) -> anyhow::Result<()> {
        sqlx::query("UPDATE workspaces SET is_default = 0 WHERE id != ?")
            .bind(keep)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_record(r: Row) -> WorkspaceRecord {
    let json_list = |s: &str| -> Vec<String> {
        serde_json::from_str(s).unwrap_or_default()
    };
    WorkspaceRecord {
        id: r.0,
        name: r.1,
        description: r.2,
        root: r.3,
        status: WorkspaceStatus::parse(&r.4),
        icon: r.5,
        is_default: r.6 != 0,
        sandbox: SandboxConfig {
            enabled: r.7 != 0,
            writable_paths: json_list(&r.8),
            readable_paths: json_list(&r.9),
            max_recursion_depth: r.10.max(1) as usize,
        },
        created_at: r.11,
        updated_at: r.12,
    }
}

/// Aplica el patch sobre el sandbox existente.
fn patch_sandbox(base: &SandboxConfig, patch: &WorkspacePatch) -> SandboxConfig {
    SandboxConfig {
        enabled: patch.sandbox_enabled.unwrap_or(base.enabled),
        writable_paths: patch
            .sandbox_writable_paths
            .clone()
            .unwrap_or_else(|| base.writable_paths.clone()),
        readable_paths: patch
            .sandbox_readable_paths
            .clone()
            .unwrap_or_else(|| base.readable_paths.clone()),
        max_recursion_depth: patch
            .sandbox_max_recursion_depth
            .unwrap_or(base.max_recursion_depth)
            .max(1),
    }
}

// ─── Registro runtime ───────────────────────────────────────────────────────

/// Estado vivo de los workspaces: un `Arc` de sandbox por id.
///
/// Se mantiene aparte del store porque el `Arc` es lo que comparten los
/// tools: el store guarda la fila, el registro guarda la celda que se
/// escribe. Recargar desde el store al arrancar.
pub struct WorkspaceRegistry {
    sandboxes: RwLock<HashMap<String, Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>>>>,
    /// Sandbox global, el que aplica cuando la sesion no pide workspace.
    /// El mismo `Arc` que el engine y el `WorkspaceLayer`.
    global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>>,
}

impl WorkspaceRegistry {
    pub fn new(global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>>) -> Self {
        Self {
            sandboxes: RwLock::new(HashMap::new()),
            global,
        }
    }

    /// Carga los sandboxes desde el store. Reemplaza el mapa entero, asi
    /// que un workspace borrado del store no deja una celda viva.
    pub async fn load_from(&self, store: &WorkspaceStore) -> anyhow::Result<()> {
        let records = store.list().await?;
        let mut map = HashMap::new();
        for r in records {
            map.insert(r.id.clone(), sandbox_arc(r.sandbox_config()));
        }
        *self.sandboxes.write().await = map;
        Ok(())
    }

    /// Sandbox vivo de un workspace, construyolo si no estaba.
    pub async fn sandbox_for(&self, id: &str) -> Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> {
        if let Some(a) = self.sandboxes.read().await.get(id) {
            return a.clone();
        }
        let nuevo = sandbox_arc(SandboxConfig::default());
        self.sandboxes.write().await.insert(id.to_string(), nuevo.clone());
        nuevo
    }

    /// Reemplaza el sandbox de un workspace. Es lo que hace
    /// `PUT /v1/workspaces/:id/sandbox`.
    pub async fn set_sandbox(
        &self,
        id: &str,
        cfg: SandboxConfig,
    ) -> Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> {
        let nuevo = sandbox_arc(cfg);
        self.sandboxes.write().await.insert(id.to_string(), nuevo.clone());
        nuevo
    }

    /// Saca el workspace del registro. La celda puede seguir viva si un
    /// scope ya la habia clonado, asi que el sandbox se relaja a "nada
    /// permitido" antes de soltarlo: no alcanza con sacar la referencia.
    pub async fn remove(&self, id: &str) {
        let removed = self.sandboxes.write().await.remove(id);
        if let Some(cell) = removed {
            let mut guard = cell.write().await;
            *guard = Box::new(DenyAll);
        }
    }

    pub async fn ids(&self) -> Vec<String> {
        self.sandboxes.read().await.keys().cloned().collect()
    }

    pub fn global_sandbox(&self) -> Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> {
        self.global.clone()
    }
}

fn sandbox_arc(cfg: SandboxConfig) -> Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> {
    Arc::new(RwLock::new(Box::new(cfg) as Box<dyn tools_engine::SandboxConfig>))
}

/// Sandbox que no deja pasar nada.
///
/// Se usa al remover un workspace: el `Arc` ya clonado puede sobrevivir en
/// un scope en vuelo, y sin esto seguiria autorizando con los permisos
/// viejos de un workspace que el operador ya borro.
struct DenyAll;

impl tools_engine::SandboxConfig for DenyAll {
    fn enabled(&self) -> bool {
        true
    }
    fn is_writable(&self, _p: &Path) -> bool {
        false
    }
    fn is_readable(&self, _p: &Path) -> bool {
        false
    }
    fn readable_paths_resolved(&self, _ws: &Path) -> Vec<PathBuf> {
        Vec::new()
    }
    fn writable_paths_resolved(&self, _ws: &Path) -> Vec<PathBuf> {
        Vec::new()
    }
}

/// Resuelve el workspace de una sesion a un scope efectivo.
///
/// Sin workspace, o con un id que no existe, cae al sandbox global con el
/// root global: es el comportamiento de antes y no debe cambiar por un
/// id mal escrito.
pub async fn resolve_scope(
    store: &WorkspaceStore,
    registry: &WorkspaceRegistry,
    workspace_id: Option<&str>,
    global_root: &Path,
) -> tools_engine::WorkspaceScope {
    let Some(id) = workspace_id else {
        return tools_engine::WorkspaceScope {
            id: None,
            root: global_root.to_path_buf(),
            sandbox: registry.global_sandbox(),
        };
    };
    let found = store.get(id).await.ok().flatten();
    match found {
        Some(rec) => tools_engine::WorkspaceScope {
            id: Some(rec.id.clone()),
            root: PathBuf::from(&rec.root),
            sandbox: registry.sandbox_for(id).await,
        },
        None => tools_engine::WorkspaceScope {
            id: None,
            root: global_root.to_path_buf(),
            sandbox: registry.global_sandbox(),
        },
    }
}

#[cfg(test)]
mod tests {
    //! El store es CRUD sobre SQLite. Lo que importa de verdad es que dos
    //! workspaces no compartan celda de sandbox: si compartieran, cambiar
    //! los permisos de uno cambiaria los del otro en caliente.

    use super::*;
    use crate::config::SandboxConfig;

    async fn store() -> (tempfile::TempDir, WorkspaceStore) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("t.db");
        (dir, WorkspaceStore::open(&db).await.unwrap())
    }

    fn patch_readable(paths: &[&str]) -> WorkspacePatch {
        WorkspacePatch {
            sandbox_readable_paths: Some(paths.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn create_lista_y_trae_el_workspace() {
        let (_d, s) = store().await;
        let creado = s
            .create("Sixbell", "/tmp/sixbell", &patch_readable(&["/tmp/sixbell"]))
            .await
            .unwrap();
        assert_eq!(creado.id, "sixbell", "el id sale del nombre, en minuscula");
        assert_eq!(creado.name, "Sixbell");
        assert_eq!(creado.sandbox.readable_paths, vec!["/tmp/sixbell"]);

        let todos = s.list().await.unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(s.get("sixbell").await.unwrap().unwrap().id, "sixbell");
        assert!(s.get("nope").await.unwrap().is_none());
    }

    /// Dos workspaces con el mismo nombre no se pisan el id.
    #[tokio::test]
    async fn nombres_repetidos_generan_ids_distintos() {
        let (_d, s) = store().await;
        let a = s.create("Personal", "/tmp/a", &WorkspacePatch::default()).await.unwrap();
        let b = s.create("Personal", "/tmp/b", &WorkspacePatch::default()).await.unwrap();
        assert_eq!(a.id, "personal");
        assert_eq!(b.id, "personal-2");
        assert_eq!(s.list().await.unwrap().len(), 2);
    }

    /// El PATCH es parcial: lo que no menciona no se toca.
    #[tokio::test]
    async fn patch_parcial_no_pisa_los_otros_campos() {
        let (_d, s) = store().await;
        s.create(
            "Sixbell",
            "/tmp/sixbell",
            &WorkspacePatch {
                description: Some(Some("equipo".into())),
                sandbox_readable_paths: Some(vec!["/tmp/sixbell".into()]),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let upd = s
            .update(
                "sixbell",
                &WorkspacePatch {
                    name: Some("Sixbell renombrado".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(upd.name, "Sixbell renombrado");
        assert_eq!(upd.description.as_deref(), Some("equipo"), "no se pierde");
        assert_eq!(upd.sandbox.readable_paths, vec!["/tmp/sixbell"], "no se pierde");
    }

    #[tokio::test]
    async fn delete_saca_la_fila_y_no_falla_dos_veces() {
        let (_d, s) = store().await;
        s.create("Sixbell", "/tmp/sixbell", &WorkspacePatch::default()).await.unwrap();
        s.delete("sixbell").await.unwrap();
        assert!(s.get("sixbell").await.unwrap().is_none());
        // El segundo delete tiene que fallar, no dejar un fantasma.
        assert!(s.delete("sixbell").await.is_err());
    }

    /// Solo un default a la vez.
    #[tokio::test]
    async fn el_default_es_unico() {
        let (_d, s) = store().await;
        s.create(
            "A",
            "/tmp/a",
            &WorkspacePatch { is_default: Some(true), ..Default::default() },
        )
        .await
        .unwrap();
        s.create(
            "B",
            "/tmp/b",
            &WorkspacePatch { is_default: Some(true), ..Default::default() },
        )
        .await
        .unwrap();

        let defaults: Vec<_> = s
            .list()
            .await
            .unwrap()
            .into_iter()
            .filter(|w| w.is_default)
            .collect();
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].id, "b");
    }

    /// Cada workspace tiene su PROPIA celda. Es el punto del asunto: con
    /// un Arc compartido, cambiar el sandbox de uno cambiaria el del otro.
    #[tokio::test]
    async fn cada_workspace_tiene_su_propia_celda_de_sandbox() {
        let (_d, s) = store().await;
        let global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> =
            Arc::new(RwLock::new(Box::new(SandboxConfig::default())));
        let reg = WorkspaceRegistry::new(global.clone());
        reg.load_from(&s).await.unwrap();

        s.create(
            "A",
            "/tmp/a",
            &patch_readable(&["${workspace}"]),
        )
        .await
        .unwrap();
        s.create("B", "/tmp/b", &patch_readable(&["/tmp/b"])).await.unwrap();
        reg.load_from(&s).await.unwrap();

        let celda_a = reg.sandbox_for("a").await;
        let celda_b = reg.sandbox_for("b").await;
        assert!(
            !Arc::ptr_eq(&celda_a, &celda_b),
            "A y B no pueden compartir la celda"
        );
        assert!(!Arc::ptr_eq(&celda_a, &global), "tampoco la global");

        // A alcanza solo su root; B, el suyo.
        let ra = celda_a.read().await.readable_paths_resolved(Path::new("/tmp/a"));
        let rb = celda_b.read().await.readable_paths_resolved(Path::new("/tmp/b"));
        assert_eq!(ra, vec![PathBuf::from("/tmp/a")], "`${{workspace}}` es el root propio");
        assert_eq!(rb, vec![PathBuf::from("/tmp/b")]);
    }

    /// Cambiar el sandbox de un workspace no toca el del otro.
    #[tokio::test]
    async fn cambiar_un_sandbox_no_mueve_el_otro() {
        let (_d, s) = store().await;
        let global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> =
            Arc::new(RwLock::new(Box::new(SandboxConfig::default())));
        let reg = WorkspaceRegistry::new(global.clone());
        s.create("A", "/tmp/a", &patch_readable(&["/tmp/a"])).await.unwrap();
        s.create("B", "/tmp/b", &patch_readable(&["/tmp/b"])).await.unwrap();
        reg.load_from(&s).await.unwrap();

        reg.set_sandbox(
            "a",
            SandboxConfig { readable_paths: vec!["/tmp/a2".into()], ..Default::default() },
        )
        .await;

        let ra = reg.sandbox_for("a").await.read().await.readable_paths_resolved(Path::new("/tmp/a"));
        let rb = reg.sandbox_for("b").await.read().await.readable_paths_resolved(Path::new("/tmp/b"));
        assert_eq!(ra, vec![PathBuf::from("/tmp/a2")], "A cambio");
        assert_eq!(rb, vec![PathBuf::from("/tmp/b")], "B no se movio");
    }

    /// Al remover un workspace, la celda que alguien ya tiene clonada deja
    /// de autorizar. Sin relajar la celda a "nada permitido", un scope en
    /// vuelo seguiría con los permisos de un workspace ya borrado.
    #[tokio::test]
    async fn remover_relaja_la_celda_ya_clonada() {
        let (_d, s) = store().await;
        let global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> =
            Arc::new(RwLock::new(Box::new(SandboxConfig::default())));
        let reg = WorkspaceRegistry::new(global.clone());
        s.create("A", "/tmp/a", &patch_readable(&["/tmp/a"])).await.unwrap();
        reg.load_from(&s).await.unwrap();

        let clonada = reg.sandbox_for("a").await;
        assert_eq!(clonada.read().await.readable_paths_resolved(Path::new("/tmp/a")).len(), 1);

        reg.remove("a").await;
        assert!(
            clonada.read().await.readable_paths_resolved(Path::new("/tmp/a")).is_empty(),
            "la celda en vuelo tiene que dejar de autorizar"
        );
    }

    /// Sin workspace, o con un id que no existe, cae al global. Un id mal
    /// escrito no puede dejar al agente sin permisos ni tirarlo.
    #[tokio::test]
    async fn scope_sin_workspace_cae_al_global() {
        let (_d, s) = store().await;
        let global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> = Arc::new(RwLock::new(
            Box::new(SandboxConfig { readable_paths: vec!["/home".into()], ..Default::default() }),
        ));
        let reg = WorkspaceRegistry::new(global.clone());

        let sc = resolve_scope(&s, &reg, None, Path::new("/root")).await;
        assert_eq!(sc.id, None);
        assert_eq!(sc.root, PathBuf::from("/root"));
        assert!(Arc::ptr_eq(&sc.sandbox, &global));

        let sc = resolve_scope(&s, &reg, Some("no-existe"), Path::new("/root")).await;
        assert_eq!(sc.id, None, "un id desconocido no inventa un workspace");
        assert_eq!(sc.root, PathBuf::from("/root"));
    }

    /// Con workspace conocido, el scope trae el root de ESE workspace, no
    /// el global.
    #[tokio::test]
    async fn scope_con_workspace_trae_su_root() {
        let (_d, s) = store().await;
        let global: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>> =
            Arc::new(RwLock::new(Box::new(SandboxConfig::default())));
        let reg = WorkspaceRegistry::new(global.clone());
        s.create("Sixbell", "/srv/sixbell", &patch_readable(&["${workspace}"])).await.unwrap();
        reg.load_from(&s).await.unwrap();

        let sc = resolve_scope(&s, &reg, Some("sixbell"), Path::new("/root")).await;
        assert_eq!(sc.id.as_deref(), Some("sixbell"));
        assert_eq!(sc.root, PathBuf::from("/srv/sixbell"));
        assert_eq!(
            sc.sandbox.read().await.readable_paths_resolved(&sc.root),
            vec![PathBuf::from("/srv/sixbell")],
            "en un sandbox por workspace, `${{workspace}}` es el root de ese workspace"
        );
    }
}

/// Las dos piezas de workspaces juntas: persistencia y estado vivo.
///
/// Se guarda en `AppState.workspaces` como `Option`. Si no se cablea, la
/// API responde 503 y toda sesion cae al sandbox global, que es el
/// comportamiento de antes. Asi los tests que arman un `AppState` a mano
/// no quedan rotos.
#[derive(Clone)]
pub struct WorkspacesLayer {
    pub store: Arc<WorkspaceStore>,
    pub registry: Arc<WorkspaceRegistry>,
}

impl WorkspacesLayer {
    /// Abre el store contra el mismo archivo SQLite que el resto del
    /// daemon y carga los sandboxes al registro.
    pub async fn open(
        db_path: &Path,
        global_sandbox: Arc<RwLock<Box<dyn tools_engine::SandboxConfig>>>,
    ) -> anyhow::Result<Self> {
        let store = Arc::new(WorkspaceStore::open(db_path).await?);
        let registry = Arc::new(WorkspaceRegistry::new(global_sandbox));
        registry.load_from(&store).await?;
        Ok(Self { store, registry })
    }
}

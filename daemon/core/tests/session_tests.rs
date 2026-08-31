use std::path::PathBuf;
use uuid::Uuid;

use neurox::session::{SessionRecord, SessionStore};

fn temp_db_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "neurox-test-{}-{}.db",
        std::process::id(),
        nanos
    ))
}

#[tokio::test]
async fn session_store_open_creates_db_file() {
    let path = temp_db_path();
    let _store = SessionStore::open(&path)
        .await
        .expect("open should succeed");
    assert!(path.exists(), "db file should exist at {path:?}");
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn session_store_creates_tables() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();

    let sid = Uuid::new_v4();
    store.start_session(sid, "default").await.unwrap();
    store.log_message(sid, "user", "hi", None).await.unwrap();
    store.log_message(sid, "assistant", "hello!", None).await.unwrap();
    store.end_session(sid, Some("greeting")).await.unwrap();

    let list = store.list_sessions(10).await.unwrap();
    assert_eq!(list.len(), 1);
    let s: &SessionRecord = &list[0];
    assert_eq!(s.agent_id, "default");
    assert_eq!(s.summary.as_deref(), Some("greeting"));
    assert!(s.ended_at.is_some());
    assert!(!s.started_at.is_empty());

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn multiple_sessions_in_order() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();

    let s1 = Uuid::new_v4();
    let s2 = Uuid::new_v4();
    let s3 = Uuid::new_v4();

    store.start_session(s1, "default").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    store.start_session(s2, "default").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    store.start_session(s3, "default").await.unwrap();

    let list = store.list_sessions(10).await.unwrap();
    assert_eq!(list.len(), 3);
    // Newest first
    assert_eq!(list[0].session_id, s3.to_string());
    assert_eq!(list[2].session_id, s1.to_string());

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn list_respects_limit() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();
    for _ in 0..5 {
        store.start_session(Uuid::new_v4(), "default").await.unwrap();
    }
    let list = store.list_sessions(2).await.unwrap();
    assert_eq!(list.len(), 2);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn end_session_with_no_summary_works() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();
    let sid = Uuid::new_v4();
    store.start_session(sid, "default").await.unwrap();
    store.end_session(sid, None).await.unwrap();
    let list = store.list_sessions(1).await.unwrap();
    assert!(list[0].ended_at.is_some());
    assert!(list[0].summary.is_none());
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn open_twice_preserves_data() {
    let path = temp_db_path();
    {
        let store = SessionStore::open(&path).await.unwrap();
        let sid = Uuid::new_v4();
        store.start_session(sid, "default").await.unwrap();
        store.log_message(sid, "user", "persisted", None).await.unwrap();
    }
    {
        let store = SessionStore::open(&path).await.unwrap();
        let list = store.list_sessions(10).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].agent_id, "default");
    }
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn set_and_get_model_persists() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();
    let sid = Uuid::new_v4();
    store.start_session(sid, "default").await.unwrap();

    // Sin modelo seteado → None (usa default del daemon)
    assert_eq!(store.get_model(sid).await.unwrap(), None);

    store
        .set_model(sid, "openrouter", "anthropic/claude-3.5-sonnet")
        .await
        .unwrap();
    let got = store.get_model(sid).await.unwrap();
    assert_eq!(
        got,
        Some((
            "openrouter".to_string(),
            "anthropic/claude-3.5-sonnet".to_string()
        ))
    );

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn set_model_updates_existing_session() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();
    let sid = Uuid::new_v4();
    store.start_session(sid, "default").await.unwrap();

    store
        .set_model(sid, "minimax", "MiniMax-M2.7")
        .await
        .unwrap();
    store
        .set_model(sid, "anthropic", "claude-3-haiku")
        .await
        .unwrap();

    let got = store.get_model(sid).await.unwrap();
    assert_eq!(
        got,
        Some(("anthropic".to_string(), "claude-3-haiku".to_string()))
    );

    // list_sessions también expone los campos
    let list = store.list_sessions(10).await.unwrap();
    assert_eq!(list[0].provider_id.as_deref(), Some("anthropic"));
    assert_eq!(list[0].model.as_deref(), Some("claude-3-haiku"));

    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn set_model_on_missing_session_errors() {
    let path = temp_db_path();
    let store = SessionStore::open(&path).await.unwrap();
    let sid = Uuid::new_v4();
    let err = store.set_model(sid, "minimax", "x").await.unwrap_err();
    assert!(err.to_string().contains("session not found"));
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn migration_adds_columns_to_existing_db() {
    let path = temp_db_path();
    // Crear DB vieja SIN columnas provider_id/model
    {
        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        sqlx::query(
            "CREATE TABLE sessions (
                session_id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                summary TEXT
             )",
        )
        .execute(&pool)
        .await
        .unwrap();
    }
    // Reabrir con SessionStore → migración agrega columnas
    let store = SessionStore::open(&path).await.unwrap();
    let sid = Uuid::new_v4();
    store.start_session(sid, "default").await.unwrap();
    store
        .set_model(sid, "openai_compat", "gpt-4o")
        .await
        .unwrap();
    assert_eq!(
        store.get_model(sid).await.unwrap(),
        Some(("openai_compat".to_string(), "gpt-4o".to_string()))
    );
    let _ = std::fs::remove_file(&path);
}

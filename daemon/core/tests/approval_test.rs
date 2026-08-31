//! E2E tests for the approval flow.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

use neurox::approval::{ApprovalDecision, ApprovalManager};

#[tokio::test]
async fn approve_then_resolve() {
    let mgr = Arc::new(ApprovalManager::new(Duration::from_secs(5)));
    let mgr_clone = mgr.clone();

    let requester = tokio::spawn(async move {
        let (id, decision) = mgr_clone
            .request(
                uuid::Uuid::new_v4(),
                "shell".into(),
                serde_json::json!({"cmd": "rm -rf /"}),
                None,
            )
            .await;
        (id, decision)
    });

    // Give request a moment to register
    tokio::time::sleep(Duration::from_millis(50)).await;
    let pending = mgr.list().await;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["tool"], "shell");

    // Approve it
    let approval_id = pending[0]["id"]
        .as_str()
        .unwrap()
        .parse::<uuid::Uuid>()
        .unwrap();
    assert!(mgr.respond(approval_id, ApprovalDecision::Approve).await);

    let (id, decision) = requester.await.unwrap();
    assert_eq!(id, approval_id);
    assert!(matches!(decision, ApprovalDecision::Approve));
    assert_eq!(mgr.list().await.len(), 0);
}

#[tokio::test]
async fn deny_path() {
    let mgr = Arc::new(ApprovalManager::new(Duration::from_secs(5)));
    let mgr_clone = mgr.clone();

    let requester = tokio::spawn(async move {
        mgr_clone
            .request(
                uuid::Uuid::new_v4(),
                "shell".into(),
                serde_json::json!({}),
                None,
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    let pending = mgr.list().await;
    let id = pending[0]["id"]
        .as_str()
        .unwrap()
        .parse::<uuid::Uuid>()
        .unwrap();
    assert!(mgr.respond(id, ApprovalDecision::Deny).await);

    let (_, decision) = requester.await.unwrap();
    assert!(matches!(decision, ApprovalDecision::Deny));
}

#[tokio::test]
async fn timeout_defaults_to_deny() {
    let mgr = Arc::new(ApprovalManager::new(Duration::from_millis(200)));

    let start = Instant::now();
    let (_, decision) = mgr
        .request(
            uuid::Uuid::new_v4(),
            "shell".into(),
            serde_json::json!({}),
            None,
        )
        .await;
    let elapsed = start.elapsed();

    assert!(matches!(decision, ApprovalDecision::Deny));
    assert!(elapsed >= Duration::from_millis(200));
    assert!(elapsed < Duration::from_secs(2));
}

#[tokio::test]
async fn respond_nonexistent_returns_false() {
    let mgr = ApprovalManager::new(Duration::from_secs(5));
    assert!(
        !mgr.respond(uuid::Uuid::new_v4(), ApprovalDecision::Approve)
            .await
    );
}

#[tokio::test]
async fn cancel_session_denies_pending() {
    let mgr = Arc::new(ApprovalManager::new(Duration::from_secs(60)));
    let session_id = uuid::Uuid::new_v4();
    let other_session = uuid::Uuid::new_v4();

    let m1 = mgr.clone();
    let sid1 = session_id;
    let _h1 = tokio::spawn(async move {
        m1.request(sid1, "shell".into(), serde_json::json!({}), None)
            .await
    });
    let m2 = mgr.clone();
    let sid2 = other_session;
    let _h2 = tokio::spawn(async move {
        m2.request(sid2, "shell".into(), serde_json::json!({}), None)
            .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(mgr.list().await.len(), 2);

    mgr.cancel_session(session_id).await;
    assert_eq!(mgr.list().await.len(), 1);
}

#[tokio::test]
async fn cancel_all_denies_everything() {
    let mgr = Arc::new(ApprovalManager::new(Duration::from_secs(60)));
    for _ in 0..5 {
        let m = mgr.clone();
        let _h = tokio::spawn(async move {
            m.request(
                uuid::Uuid::new_v4(),
                "shell".into(),
                serde_json::json!({}),
                None,
            )
            .await
        });
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(mgr.list().await.len(), 5);
    mgr.cancel_all().await;
    assert_eq!(mgr.list().await.len(), 0);
}

#[tokio::test]
async fn list_includes_age_ms() {
    let mgr = Arc::new(ApprovalManager::new(Duration::from_secs(60)));
    let m = mgr.clone();
    let _h = tokio::spawn(async move {
        m.request(
            uuid::Uuid::new_v4(),
            "tool".into(),
            serde_json::json!({}),
            None,
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    let pending = mgr.list().await;
    assert!(pending[0]["age_ms"].as_u64().unwrap() >= 50);
    mgr.cancel_all().await;
}

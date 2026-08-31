use std::path::PathBuf;

use neurox::config::{
    AgentsConfig, EphemeralAgentSpec, PersistentAgentSpec, RestartPolicy, SandboxConfig,
    SessionAgentsConfig,
};
use neurox::protocols::{ProtocolKind, TransportKind};
use neurox::registry::Registry;

fn persistent(id: &str) -> PersistentAgentSpec {
    PersistentAgentSpec {
        id: id.to_string(),
        kind: neurox::config::AgentKind::Subprocess {
            command: "/bin/echo".to_string(),
            args: vec!["hello".to_string()],
            env: Default::default(),
        },
        protocol: ProtocolKind::JsonRpc,
        transport: TransportKind::Stdio,
        restart_policy: RestartPolicy::OnFailure,
        depends_on: vec![],
        requires_approval: vec![],
        approval_timeout_secs: 60,
        llm: None,
        system_prompt: None,
    }
}

fn ephemeral(id: &str) -> EphemeralAgentSpec {
    EphemeralAgentSpec {
        id: id.to_string(),
        kind: neurox::config::AgentKind::Subprocess {
            command: "/bin/echo".to_string(),
            args: vec![],
            env: Default::default(),
        },
        protocol: ProtocolKind::JsonRpc,
        transport: TransportKind::Stdio,
        requires_approval: vec![],
    }
}

#[tokio::test]
async fn empty_registry_has_no_agents() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    assert_eq!(r.list_persistent().await.len(), 0);
    assert_eq!(r.list_ephemeral_templates().await.len(), 0);
}

#[tokio::test]
async fn register_persistent_then_list() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    r.register_persistent(persistent("a")).await;
    r.register_persistent(persistent("b")).await;

    let list = r.list_persistent().await;
    assert_eq!(list.len(), 2);
    let ids: Vec<_> = list.iter().map(|s| s.id.clone()).collect();
    assert!(ids.contains(&"a".to_string()));
    assert!(ids.contains(&"b".to_string()));
}

#[tokio::test]
async fn register_overwrites_existing_persistent() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    r.register_persistent(persistent("a")).await;
    r.register_persistent(persistent("a")).await;
    assert_eq!(r.list_persistent().await.len(), 1);
}

#[tokio::test]
async fn get_persistent_returns_some_when_exists() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    r.register_persistent(persistent("default")).await;
    let got = r.get_persistent("default").await;
    assert!(got.is_some());
    assert_eq!(got.unwrap().id, "default");
}

#[tokio::test]
async fn get_persistent_returns_none_when_missing() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    assert!(r.get_persistent("ghost").await.is_none());
}

#[tokio::test]
async fn remove_persistent_succeeds_when_exists() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    r.register_persistent(persistent("a")).await;
    assert!(r.remove_persistent("a").await);
    assert!(r.get_persistent("a").await.is_none());
}

#[tokio::test]
async fn remove_persistent_returns_false_when_missing() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    assert!(!r.remove_persistent("ghost").await);
}

#[tokio::test]
async fn persistent_and_ephemeral_are_independent() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    r.register_persistent(persistent("p1")).await;
    r.register_ephemeral_template(ephemeral("e1")).await;

    assert_eq!(r.list_persistent().await.len(), 1);
    assert_eq!(r.list_ephemeral_templates().await.len(), 1);
    assert!(r.get_persistent("e1").await.is_none());
    assert!(r.get_ephemeral_template("p1").await.is_none());
}

#[tokio::test]
async fn load_from_config_populates_registry() {
    let r = Registry::new(PathBuf::from("/tmp/nonexistent.yaml"));
    let cfg = neurox::config::CoreConfig {
        bind_addr: "127.0.0.1:7878".into(),
        db_path: std::path::PathBuf::from("/tmp/x.db"),
        log_level: "info".into(),
        agents: AgentsConfig {
            persistent: vec![persistent("default"), persistent("memory")],
            ephemeral_templates: vec![ephemeral("researcher")],
        },
        api_token: None,
        tls: None,
        spawner_concurrency: 4,
        in_process: vec![],
        services: vec![],
        plugins_registry: None,
        llm: neurox::config::LlmConfig::default(),
        sandbox: SandboxConfig::default(),
        session_agents: SessionAgentsConfig::default(),
        auth: neurox::config::AuthConfigSection::default(),
    };
    r.load_from_config(&cfg).await.unwrap();

    assert_eq!(r.list_persistent().await.len(), 2);
    assert_eq!(r.list_ephemeral_templates().await.len(), 1);
    assert!(r.get_persistent("default").await.is_some());
    assert!(r.get_ephemeral_template("researcher").await.is_some());
}

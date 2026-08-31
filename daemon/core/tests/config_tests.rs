use neurox::config::{AgentKind, RestartPolicy};

#[test]
fn core_config_default_is_sane() {
    let cfg = neurox::config::CoreConfig::default();
    assert!(cfg.bind_addr.starts_with("127.0.0.1"));
    assert_eq!(cfg.log_level, "info");
    assert!(cfg.agents.persistent.is_empty());
    assert!(cfg.agents.ephemeral_templates.is_empty());
}

#[test]
fn restart_policy_serializes_as_kebab_case() {
    let p = RestartPolicy::OnFailure;
    let yaml = serde_yml::to_string(&p).unwrap();
    assert!(yaml.contains("on-failure"));

    let p = RestartPolicy::Always;
    let yaml = serde_yml::to_string(&p).unwrap();
    assert!(yaml.contains("always"));
}

#[test]
fn restart_policy_deserializes_from_kebab_case() {
    let p: RestartPolicy = serde_yml::from_str("on-failure").unwrap();
    assert!(matches!(p, RestartPolicy::OnFailure));

    let p: RestartPolicy = serde_yml::from_str("always").unwrap();
    assert!(matches!(p, RestartPolicy::Always));
}

#[test]
fn full_yaml_parses_into_core_config() {
    let yaml = r#"
bind_addr: "0.0.0.0:9000"
log_level: "debug"
agents:
  persistent:
    - id: default
      kind: subprocess
      command: "/usr/bin/default"
      args: ["--model", "x"]
      protocol: json-rpc
      transport: stdio
      restart_policy: on-failure
  ephemeral_templates:
    - id: worker
      kind: subprocess
      command: "/usr/bin/worker"
      protocol: json-rpc
      transport: stdio
"#;
    let cfg: neurox::config::CoreConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(cfg.bind_addr, "0.0.0.0:9000");
    assert_eq!(cfg.log_level, "debug");
    assert_eq!(cfg.agents.persistent.len(), 1);
    let p = &cfg.agents.persistent[0];
    assert_eq!(p.id, "default");
    match &p.kind {
        AgentKind::Subprocess { command, args, env } => {
            assert_eq!(command, "/usr/bin/default");
            assert_eq!(args, &vec!["--model".to_string(), "x".to_string()]);
            assert!(env.is_empty());
        }
    }
    assert_eq!(cfg.agents.ephemeral_templates.len(), 1);
}

#[test]
fn core_config_load_returns_default_when_file_missing() {
    let path = std::path::Path::new("/tmp/definitely_does_not_exist_neurox_test.yaml");
    let cfg = neurox::config::CoreConfig::load(path).unwrap();
    assert_eq!(cfg.log_level, "info");
}

#[test]
fn agent_kind_subprocess_roundtrips() {
    let kind = AgentKind::Subprocess {
        command: "foo".into(),
        args: vec!["a".into(), "b".into()],
        env: Default::default(),
    };
    let json = serde_json::to_string(&kind).unwrap();
    let back: AgentKind = serde_json::from_str(&json).unwrap();
    match back {
        AgentKind::Subprocess { command, args, env } => {
            assert_eq!(command, "foo");
            assert_eq!(args, vec!["a".to_string(), "b".to_string()]);
            assert!(env.is_empty());
        }
    }
}

#[test]
fn spawner_concurrency_default_is_8() {
    let cfg = neurox::config::CoreConfig::default();
    assert_eq!(cfg.spawner_concurrency, 8);
}

#[test]
fn spawner_concurrency_loads_from_yaml() {
    let yaml = "spawner_concurrency: 16\n";
    let cfg: neurox::config::CoreConfig = serde_yml::from_str(yaml).unwrap();
    assert_eq!(cfg.spawner_concurrency, 16);
}

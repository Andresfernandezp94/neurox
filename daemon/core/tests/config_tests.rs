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

/// El example es la puerta de entrada de una instalación: alguien que
/// clona el repo lo copia a ~/.config/neurox/config.yaml. Si queda atrás
/// del config real, instala un daemon sin providers declarados ni auth
/// configurada, y el síntoma es "no me funciona el chat" sin pista.
///
/// Este test ata el example al struct real: si mañana se agrega un campo
/// obligatorio o se renombra una sección, falla acá y no en la máquina de
/// quien instaló.
#[test]
fn config_example_parses_into_core_config() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.yaml");
    let cfg = neurox::config::CoreConfig::load(&path)
        .expect("config.example.yaml must parse into CoreConfig");

    assert_eq!(cfg.bind_addr, "127.0.0.1:7878");
    assert!(
        !cfg.llm.providers.is_empty(),
        "the example must declare providers, otherwise a clean install has none"
    );
    assert!(
        cfg.llm.providers.iter().all(|p| p.api_key_env.is_some()),
        "every provider in the example should name its api_key_env"
    );
}

/// Los providers del example deben coincidir con los que el daemon puede
/// resolver: cada `kind` tiene su propio endpoint por defecto, asi que un
/// kind invalido no se descubre en elCatalogo.
#[test]
fn config_example_providers_have_valid_kinds() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config.example.yaml");
    let cfg = neurox::config::CoreConfig::load(&path).unwrap();

    for p in &cfg.llm.providers {
        assert!(
            !p.id.is_empty(),
            "provider with empty id would break `default_provider` lookups"
        );
        assert!(
            !p.base_url.starts_with("https://") || p.kind != neurox::config::LlmProviderKind::Anthropic,
            "anthropic expects https://api.anthropic.com"
        );
    }
}

/// La forma estricta `env:VAR` entre llaves y signo de peso que no existe
/// debe hacer fallar la carga del config.
///
/// El síntoma que viene de expanding a vacío: el daemon levanta, el config
/// parsea, y el primer mensaje de chat falla con un 401 que no señala la
/// causa. El error de arranque dice exactamente qué falta.
#[test]
fn load_fails_when_required_env_var_is_missing() {
    std::env::remove_var("NEURO_TEST_REQ_MISSING");
    let dir = std::env::temp_dir().join(format!("neurox-cfg-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(
        &path,
        "bind_addr: \"127.0.0.1:7878\"\nlog_level: \"info\"\n\
         session_agents:\n  agents:\n    default:\n      command: \"/bin/true\"\n",
    )
    .unwrap();

    // Con la var presente, carga bien.
    std::env::set_var("NEURO_TEST_REQ_MISSING", "valor");
    let yaml = std::fs::read_to_string(&path).unwrap().replace(
        "bind_addr",
        &format!("extra: \"${{env:NEURO_TEST_REQ_MISSING}}\"\nbind_addr"),
    );
    std::fs::write(&path, &yaml).unwrap();
    assert!(
        neurox::config::CoreConfig::load(&path).is_ok(),
        "config with a satisfied env ref must load"
    );

    // Sin la var, falla nombrando la variable.
    std::env::remove_var("NEURO_TEST_REQ_MISSING");
    let err = neurox::config::CoreConfig::load(&path)
        .expect_err("missing env ref must fail the load");
    let msg = err.to_string();
    assert!(
        msg.contains("NEURO_TEST_REQ_MISSING"),
        "error must name the missing var, got: {msg}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// La forma laxa `${VAR}` no debe romper la carga: existe para
/// placeholders opcionales.
#[test]
fn load_okays_when_lax_var_is_missing() {
    std::env::remove_var("NEURO_TEST_LAX_OPTIONAL");
    let dir = std::env::temp_dir().join(format!("neurox-cfg-lax-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.yaml");
    std::fs::write(
        &path,
        "bind_addr: \"127.0.0.1:${NEURO_TEST_LAX_OPTIONAL}\"\nlog_level: \"info\"\n",
    )
    .unwrap();

    // Carga: la forma laxa expande a vacío sin error.
    assert!(neurox::config::CoreConfig::load(&path).is_ok());
    std::fs::remove_dir_all(&dir).ok();
}

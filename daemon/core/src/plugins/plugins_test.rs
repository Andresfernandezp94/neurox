// Tests for the plugin system. EP-0005.

use tempfile::TempDir;

use crate::plugins::{
    verify, PluginManifest, PluginVersion, Registry, RegistryEntry, RegistryVersion,
};

fn build_manifest() -> PluginManifest {
    PluginManifest {
        name: "tui".into(),
        version: "0.1.0".into(),
        description: "TUI client".into(),
        author: "Tester".into(),
        license: Some("MIT".into()),
        repo: "Andresfernandezp94/neurox-plugin-tui".into(),
        tags: vec!["tui".into()],
        min_core_version: Some("0.1.0".into()),
        max_core_version: None,
        entry: "bin/run".into(),
        capabilities: vec![],
        tools: vec![], // EP-0005 added this field; helper was never updated
        native_libs: vec![],
        resources: Default::default(),
    }
}

#[test]
fn manifest_roundtrip() {
    let m = build_manifest();
    let json = serde_json::to_string(&m).unwrap();
    let back: PluginManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "tui");
    assert_eq!(back.version, "0.1.0");
    assert_eq!(back.entry_path(), "bin/run");
}

#[test]
fn manifest_default_entry_path() {
    let mut m = build_manifest();
    m.entry = "".into(); // empty to test default
    let m = PluginManifest {
        entry: m.entry,
        // re-construct with sensible defaults
        ..build_manifest()
    };
    assert_eq!(m.entry_path(), "bin/run");
}

#[test]
fn sha256_verification_passes() {
    let bytes = b"hello world";
    let hash = verify::sha256_hex(bytes);
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );
    verify::verify_sha256(bytes, &hash).unwrap();
}

#[test]
fn sha256_verification_fails_on_mismatch() {
    let result = verify::verify_sha256(
        b"hello",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(result.is_err());
}

#[test]
fn registry_resolve_latest() {
    let reg = Registry {
        version: 1,
        plugins: std::collections::BTreeMap::from([(
            "tui".to_string(),
            RegistryEntry {
                repo: "some/repo".into(),
                description: "desc".into(),
                versions: std::collections::BTreeMap::from([
                    (
                        "0.1.0".into(),
                        RegistryVersion {
                            artifact: "file:///tmp/tui-0.1.0.tar.gz".into(),
                            sha256: "abc".into(),
                            gpg_signature: None,
                            gpg_key_fingerprint: None,
                            min_core_version: None,
                            max_core_version: None,
                        },
                    ),
                    (
                        "0.2.0".into(),
                        RegistryVersion {
                            artifact: "file:///tmp/tui-0.2.0.tar.gz".into(),
                            sha256: "def".into(),
                            gpg_signature: None,
                            gpg_key_fingerprint: None,
                            min_core_version: None,
                            max_core_version: None,
                        },
                    ),
                ]),
            },
        )]),
    };
    let (name, version, _rv) = reg.resolve("tui", "latest").unwrap();
    assert_eq!(name, "tui");
    assert_eq!(version, "0.2.0");
}

#[test]
fn registry_resolve_explicit_version() {
    let reg = Registry {
        version: 1,
        plugins: std::collections::BTreeMap::from([(
            "tui".to_string(),
            RegistryEntry {
                repo: "some/repo".into(),
                description: "desc".into(),
                versions: std::collections::BTreeMap::from([(
                    "0.1.0".into(),
                    RegistryVersion {
                        artifact: "file:///tmp/tui-0.1.0.tar.gz".into(),
                        sha256: "abc".into(),
                        gpg_signature: None,
                        gpg_key_fingerprint: None,
                        min_core_version: None,
                        max_core_version: None,
                    },
                )]),
            },
        )]),
    };
    let (_, version, _) = reg.resolve("tui", "0.1.0").unwrap();
    assert_eq!(version, "0.1.0");
}

#[test]
fn registry_resolve_unknown_plugin() {
    let reg = Registry {
        version: 1,
        plugins: Default::default(),
    };
    assert!(reg.resolve("nope", "latest").is_err());
}

#[test]
fn manager_installs_from_file_url() {
    let tmp = TempDir::new().unwrap();
    let plugins_dir = tmp.path().join("plugins");
    let cache_dir = tmp.path().join("cache");
    let staging = tmp.path().join("staging");
    std::fs::create_dir_all(&staging).unwrap();

    // Build a tiny plugin artifact.
    let plugin_dir = staging.join("tui");
    std::fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    std::fs::write(
        plugin_dir.join("manifest.json"),
        serde_json::to_string(&build_manifest()).unwrap(),
    )
    .unwrap();
    std::fs::write(plugin_dir.join("bin/run"), "#!/bin/bash\necho ok\n").unwrap();
    let artifact_path = tmp.path().join("tui-0.1.0.tar.gz");
    let bytes = build_tarball(&plugin_dir);
    let sha = verify::sha256_hex(&bytes);
    std::fs::write(&artifact_path, &bytes).unwrap();

    let registry = Registry {
        version: 1,
        plugins: std::collections::BTreeMap::from([(
            "tui".to_string(),
            RegistryEntry {
                repo: "some/repo".into(),
                description: "TUI".into(),
                versions: std::collections::BTreeMap::from([(
                    "0.1.0".into(),
                    RegistryVersion {
                        artifact: format!("file://{}", artifact_path.display()),
                        sha256: sha,
                        gpg_signature: None,
                        gpg_key_fingerprint: None,
                        min_core_version: None,
                        max_core_version: None,
                    },
                )]),
            },
        )]),
    };

    let manager = crate::plugins::PluginManager::new(plugins_dir.clone(), cache_dir);
    let info = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(manager.install(&registry, "tui", "0.1.0"))
        .unwrap();
    assert_eq!(info.name, "tui");
    assert_eq!(info.version, "0.1.0");

    // Plugin should be listed.
    let list = manager.list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "tui");

    // Manifest should be readable.
    let m = manager.read_manifest("tui").unwrap();
    assert_eq!(m.entry_path(), "bin/run");

    // Entry binary should exist.
    assert!(plugins_dir.join("tui/bin/run").is_file());

    // Remove.
    manager.remove("tui").unwrap();
    assert!(!plugins_dir.join("tui").exists());
}

fn build_tarball(src: &std::path::Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut bytes);
        let gz = flate2::write::GzEncoder::new(cursor, flate2::Compression::default());
        let mut tar = tar::Builder::new(gz);
        tar.append_dir_all(".", src).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    bytes
}

#[test]
fn plugin_version_struct() {
    let v = PluginVersion {
        semver: "1.2.3".into(),
        min_core_version: Some("0.1.0".into()),
        max_core_version: Some("0.2.0".into()),
    };
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains("\"semver\":\"1.2.3\""));
}

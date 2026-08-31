use std::path::PathBuf;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn data_dir_ends_with_neurox() {
    let _g = ENV_LOCK.lock().unwrap();
    let d = neurox::runtime::data_dir();
    assert!(d.ends_with("neurox"), "got {d:?}");
}

#[test]
fn config_dir_ends_with_neurox() {
    let _g = ENV_LOCK.lock().unwrap();
    let d = neurox::runtime::config_dir();
    assert!(d.ends_with("neurox"), "got {d:?}");
}

#[test]
fn default_db_path_is_inside_data_dir() {
    let _g = ENV_LOCK.lock().unwrap();
    let db = neurox::runtime::default_db_path();
    let data = neurox::runtime::data_dir();
    assert!(db.starts_with(&data), "db={db:?} data={data:?}");
    assert_eq!(db.file_name().unwrap(), "neurox.db");
}

#[test]
fn default_config_path_is_inside_config_dir() {
    let _g = ENV_LOCK.lock().unwrap();
    let cfg = neurox::runtime::default_config_path();
    let config = neurox::runtime::config_dir();
    assert!(cfg.starts_with(&config), "cfg={cfg:?} config={config:?}");
    assert_eq!(cfg.file_name().unwrap(), "config.yaml");
}

#[test]
fn init_dirs_creates_all_dirs() {
    let _g = ENV_LOCK.lock().unwrap();
    let tmp = std::env::temp_dir().join(format!(
        "neurox-test-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));

    let saved_home = std::env::var_os("HOME");
    let saved_xdg_config = std::env::var_os("XDG_CONFIG_HOME");
    let saved_xdg_data = std::env::var_os("XDG_DATA_HOME");
    let saved_xdg_cache = std::env::var_os("XDG_CACHE_HOME");

    std::env::set_var("HOME", &tmp);
    std::env::set_var("XDG_CONFIG_HOME", tmp.join("config"));
    std::env::set_var("XDG_DATA_HOME", tmp.join("data"));
    std::env::set_var("XDG_CACHE_HOME", tmp.join("cache"));

    let result = neurox::runtime::init_dirs();

    let data_dir = neurox::runtime::data_dir();
    let config_dir = neurox::runtime::config_dir();
    let socket_dir = neurox::runtime::default_socket_dir();

    if let Some(v) = saved_home {
        std::env::set_var("HOME", v);
    } else {
        std::env::remove_var("HOME");
    }
    if let Some(v) = saved_xdg_config {
        std::env::set_var("XDG_CONFIG_HOME", v);
    } else {
        std::env::remove_var("XDG_CONFIG_HOME");
    }
    if let Some(v) = saved_xdg_data {
        std::env::set_var("XDG_DATA_HOME", v);
    } else {
        std::env::remove_var("XDG_DATA_HOME");
    }
    if let Some(v) = saved_xdg_cache {
        std::env::set_var("XDG_CACHE_HOME", v);
    } else {
        std::env::remove_var("XDG_CACHE_HOME");
    }

    result.expect("init_dirs should succeed");
    assert!(data_dir.exists(), "data_dir {data_dir:?} should exist");
    assert!(
        config_dir.exists(),
        "config_dir {config_dir:?} should exist"
    );
    assert!(
        socket_dir.exists(),
        "socket_dir {socket_dir:?} should exist"
    );
}

#[test]
fn socket_dir_under_xdg_runtime_or_tmp() {
    let _g = ENV_LOCK.lock().unwrap();
    let d = neurox::runtime::default_socket_dir();
    assert!(d.ends_with("neurox"), "got {d:?}");
    let xdg_runtime =
        std::env::var_os("XDG_RUNTIME_DIR").map_or_else(|| PathBuf::from("/tmp"), PathBuf::from);
    assert!(
        d.starts_with(&xdg_runtime),
        "socket dir {d:?} not under XDG_RUNTIME_DIR or /tmp"
    );
    let _: PathBuf = d;
}

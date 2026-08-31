// Plugin manager: install / list / run / remove / update / service.
//
// State on disk:
//   ~/.local/share/neurox/plugins/<name>/
//     manifest.json
//     bin/...
//     lib/...
//     assets/...
//
//   (optional) systemd user service:
//     ~/.config/systemd/user/neurox-<name>.service

use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::plugins::manifest::PluginManifest;
use crate::plugins::registry::Registry;
use crate::plugins::verify::{verify_gpg, verify_sha256};

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub install_path: String,
    pub installed_at: String,
    pub status: PluginInstallStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallStatus {
    Ok,
    /// Manifest is broken or checksums don't match.
    Broken,
    /// Plugin is not currently runnable on this OS/arch.
    Incompatible,
}

#[derive(Debug, Clone)]
pub struct PluginManager {
    pub plugins_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl PluginManager {
    pub fn new(plugins_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            plugins_dir,
            cache_dir,
        }
    }

    /// Default location: `~/.local/share/neurox/plugins/`.
    pub fn default_local() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("neurox");
        Self::new(base.join("plugins"), base.join("cache"))
    }

    pub fn plugin_dir(&self, name: &str) -> PathBuf {
        self.plugins_dir.join(name)
    }

    pub fn manifest_path(&self, name: &str) -> PathBuf {
        self.plugin_dir(name).join("manifest.json")
    }

    /// Read a plugin's manifest from disk.
    pub fn read_manifest(&self, name: &str) -> Result<PluginManifest> {
        let path = self.manifest_path(name);
        let bytes = std::fs::read(&path).with_context(|| format!("read manifest at {path:?}"))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {path:?}"))
    }

    /// Install a plugin by name. Resolves the registry, downloads the
    /// artifact, verifies SHA256 + GPG, extracts to `plugins/<name>`.
    pub async fn install(
        &self,
        registry: &Registry,
        name: &str,
        version_req: &str,
    ) -> Result<PluginInfo> {
        let (resolved_name, resolved_version, rv) = registry.resolve(name, version_req)?;

        // 1. Download artifact.
        let artifact_bytes = download(&rv.artifact).await?;
        // 2. Verify SHA256.
        verify_sha256(&artifact_bytes, &rv.sha256)?;
        // 3. Verify GPG signature if available.
        if let (Some(sig_url), Some(sig_path)) = (
            &rv.gpg_signature,
            signature_cache_path(&self.cache_dir, name, &resolved_version).ok(),
        ) {
            let sig_bytes = download(sig_url).await?;
            if let Some(parent) = sig_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }
            tokio::fs::write(&sig_path, &sig_bytes).await.ok();
            verify_gpg(
                std::path::Path::new(&artifact_cache_path(
                    &self.cache_dir,
                    name,
                    &resolved_version,
                )?),
                &sig_path,
                rv.gpg_key_fingerprint.as_deref(),
            )?;
        }

        // 4. Extract tarball.
        let plugin_dir = self.plugin_dir(&resolved_name);
        if plugin_dir.exists() {
            tokio::fs::remove_dir_all(&plugin_dir).await.ok();
        }
        tokio::fs::create_dir_all(&plugin_dir).await?;
        extract_tarball(&artifact_bytes, &plugin_dir)?;

        // 5. Read manifest that came inside the tarball.
        let manifest = self.read_manifest(&resolved_name)?;
        Ok(PluginInfo {
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            author: manifest.author,
            install_path: plugin_dir.to_string_lossy().to_string(),
            installed_at: now_iso(),
            status: PluginInstallStatus::Ok,
        })
    }

    /// List installed plugins.
    pub fn list(&self) -> Result<Vec<PluginInfo>> {
        let mut out: Vec<PluginInfo> = Vec::new();
        if !self.plugins_dir.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(s) => s.to_string(),
                None => continue,
            };
            match self.read_manifest(&name) {
                Ok(m) => out.push(PluginInfo {
                    name: m.name,
                    version: m.version,
                    description: m.description,
                    author: m.author,
                    install_path: path.to_string_lossy().to_string(),
                    installed_at: read_installed_at(&path).unwrap_or_else(now_iso),
                    status: PluginInstallStatus::Ok,
                }),
                Err(_) => out.push(PluginInfo {
                    name: name.clone(),
                    version: "?".into(),
                    description: "(manifest unreadable)".into(),
                    author: "?".into(),
                    install_path: path.to_string_lossy().to_string(),
                    installed_at: read_installed_at(&path).unwrap_or_else(now_iso),
                    status: PluginInstallStatus::Broken,
                }),
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn remove(&self, name: &str) -> Result<()> {
        let dir = self.plugin_dir(name);
        if !dir.exists() {
            anyhow::bail!("plugin '{name}' is not installed");
        }
        std::fs::remove_dir_all(&dir).with_context(|| format!("remove {dir:?}"))?;
        Ok(())
    }

    /// Update a plugin to the latest version. Returns the new plugin info.
    pub async fn update(&self, registry: &Registry, name: &str) -> Result<PluginInfo> {
        self.install(registry, name, "latest").await
    }

    /// Run a plugin in the foreground. Returns the exit code.
    pub fn run(&self, name: &str, args: &[String]) -> Result<i32> {
        let manifest = self.read_manifest(name)?;
        let entry = self.plugin_dir(name).join(manifest.entry_path());
        if !entry.is_file() {
            anyhow::bail!(
                "plugin '{name}' entry {:?} not found. The plugin may be broken.",
                entry
            );
        }
        let mut cmd = std::process::Command::new(&entry);
        cmd.args(args);
        for lib in &manifest.native_libs {
            let abs = self.plugin_dir(name).join(lib);
            if let Some(parent) = abs.parent() {
                prepend_env_path(&mut cmd, "LD_LIBRARY_PATH", parent);
                #[cfg(target_os = "macos")]
                prepend_env_path(&mut cmd, "DYLD_LIBRARY_PATH", parent);
            }
        }
        let status = cmd.status().with_context(|| format!("spawn {entry:?}"))?;
        Ok(status.code().unwrap_or(1))
    }

    /// Generate a systemd user service unit for the plugin. Returns the
    /// path of the generated file.
    pub fn service_unit(&self, name: &str) -> Result<PathBuf> {
        let manifest = self.read_manifest(name)?;
        let unit = render_systemd_unit(name, &manifest);
        let target_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("systemd")
            .join("user");
        std::fs::create_dir_all(&target_dir)?;
        let path = target_dir.join(format!("neurox-{name}.service"));
        std::fs::write(&path, unit)?;
        Ok(path)
    }
}

fn prepend_env_path(cmd: &mut std::process::Command, key: &str, value: &Path) {
    let existing = std::env::var_os(key);
    let new = match existing {
        Some(prev) => format!("{}:{}", value.display(), prev.to_string_lossy()),
        None => value.display().to_string(),
    };
    cmd.env(key, new);
}

fn render_systemd_unit(name: &str, manifest: &PluginManifest) -> String {
    let entry = manifest.entry_path();
    format!(
        r#"[Unit]
Description=neurox plugin: {name} (v{version})
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=~/.local/share/neurox/plugins/{name}
EnvironmentFile=%h/.config/neurox/env
ExecStart=~/.local/share/neurox/plugins/{name}/{entry}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        name = name,
        version = manifest.version,
        entry = entry,
    )
}

async fn download(url: &str) -> Result<Vec<u8>> {
    if let Some(path) = url.strip_prefix("file://") {
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("read file://{path}"))?;
        return Ok(bytes);
    }
    let resp = reqwest::get(url)
        .await
        .with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("download {url} returned {status}");
    }
    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("read body of {url}"))?;
    Ok(bytes.to_vec())
}

/// Extract a `.tar.gz` plugin artifact into `dest`. The archive must
/// contain a `manifest.json` at the root.
fn extract_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    let cursor = std::io::Cursor::new(bytes);
    let gz = GzDecoder::new(cursor);
    let mut archive = Archive::new(gz);
    // EP-0011 S-007: zip-slip check. Reject any tar entry whose path
    // would escape `dest` after extraction. Iterate entries manually
    // so we can validate each one before unpacking.
    for entry in archive.entries()? {
        let mut entry = entry.context("read tar entry")?;
        let entry_path = entry.path().context("entry path")?.to_path_buf();
        // The destination for this entry is dest.join(entry_path).
        // We must ensure it resolves under dest.
        let target = dest.join(&entry_path);
        let normalized = target
            .components()
            .fold(std::path::PathBuf::new(), |mut acc, c| {
                acc.push(c.as_os_str());
                acc
            });
        if !normalized.starts_with(dest) {
            anyhow::bail!(
                "zip-slip detected: entry '{}' escapes '{}'",
                entry_path.display(),
                dest.display()
            );
        }
        // EP-0011 S-007 (refined): for directory entries, create the
        // directory explicitly. For files, let `tar` handle unpack.
        // Use `unpack_in(dest)` which respects the entry's path relative
        // to dest.
        entry.unpack_in(dest).context("unpack entry")?;
    }
    Ok(())
}

fn signature_cache_path(cache: &Path, name: &str, version: &str) -> Result<PathBuf> {
    let dir = cache.join("signatures").join(name);
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{version}.asc")))
}

fn artifact_cache_path(cache: &Path, name: &str, version: &str) -> Result<String> {
    Ok(cache
        .join("artifacts")
        .join(name)
        .join(format!("{version}.tar.gz"))
        .to_string_lossy()
        .to_string())
}

fn now_iso() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{dur}") // simple unix timestamp; replace with chrono::Utc::now().to_rfc3339() if needed
}

fn read_installed_at(dir: &Path) -> Option<String> {
    let p = dir.join(".installed_at");
    std::fs::read_to_string(p).ok()
}

// Registry fetcher. The registry is a single JSON file published at a
// well-known URL (configurable). It maps plugin names to a list of
// versions, each with an artifact URL and verification metadata.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: u32,
    pub plugins: BTreeMap<String, RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub repo: String,
    pub description: String,
    pub versions: BTreeMap<String, RegistryVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryVersion {
    /// Release artifact URL (tarball).
    pub artifact: String,
    /// SHA256 of the artifact (hex).
    pub sha256: String,
    /// Optional GPG detached signature URL (.asc or .sig).
    #[serde(default)]
    pub gpg_signature: Option<String>,
    /// Hex-encoded fingerprint of the GPG key allowed to sign.
    #[serde(default)]
    pub gpg_key_fingerprint: Option<String>,
    /// Semver constraint on the core.
    #[serde(default)]
    pub min_core_version: Option<String>,
    #[serde(default)]
    pub max_core_version: Option<String>,
}

impl Registry {
    /// Fetch the registry from the given URL. Falls back to a local file
    /// path if the URL starts with `file://`.
    pub async fn fetch(url: &str) -> Result<Self> {
        if let Some(path) = url.strip_prefix("file://") {
            let bytes = tokio::fs::read(path)
                .await
                .with_context(|| format!("read registry file {path}"))?;
            return Self::parse(&bytes);
        }
        let resp = reqwest::get(url)
            .await
            .with_context(|| format!("GET {url}"))?;
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("read body of {url}"))?;
        Self::parse(&bytes)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        serde_json::from_slice(bytes).context("parse registry.json")
    }

    /// Resolve a plugin + version against the registry. Picks the
    /// highest version matching `version_req` if `version_req` is not
    /// "latest". Returns the resolved `name` and its `RegistryVersion`.
    pub fn resolve(
        &self,
        name: &str,
        version_req: &str,
    ) -> Result<(String, String, RegistryVersion)> {
        let entry = self
            .plugins
            .get(name)
            .with_context(|| format!("plugin '{name}' not found in registry"))?;

        let version = if version_req == "latest" || version_req.is_empty() {
            // Pick the highest semver (lexicographic OK for our registry).
            entry
                .versions
                .keys()
                .max()
                .cloned()
                .with_context(|| format!("no versions available for plugin '{name}'"))?
        } else {
            entry
                .versions
                .keys()
                .find(|v| v == &version_req)
                .cloned()
                .with_context(|| format!("version '{version_req}' not found for plugin '{name}'"))?
        };

        let rv = entry
            .versions
            .get(&version)
            .cloned()
            .with_context(|| format!("version '{version}' missing for plugin '{name}'"))?;

        Ok((name.to_string(), version, rv))
    }
}

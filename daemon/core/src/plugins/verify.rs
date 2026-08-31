// Verification of downloaded artifacts. SHA256 + GPG detached signature.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    out.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn sha256_hex_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("read {path:?}"))?;
    Ok(sha256_hex(&bytes))
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<()> {
    let actual = sha256_hex(bytes);
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        anyhow::bail!("SHA256 mismatch: expected {expected_hex}, got {actual}");
    }
}

/// Optional GPG verification. EP-0011 S-008: hard-fail if a signature is
/// declared but `gpg`/`gpg2` is not on PATH (was: soft-fail with warning).
pub fn verify_gpg(artifact: &Path, signature: &Path, fingerprint: Option<&str>) -> Result<()> {
    use std::process::Command;
    let gpg = which("gpg").or_else(|| which("gpg2"));
    let gpg = match gpg {
        Some(p) => p,
        None => {
            // EP-0011 S-008: hard-fail when gpg is missing. The previous
            // behavior of silently accepting unsigned plugins was a
            // supply-chain attack vector.
            anyhow::bail!(
                "gpg/gpg2 not found on PATH but plugin has signature; install gpg or remove the signature requirement"
            );
        }
    };

    let mut cmd = Command::new(&gpg);
    cmd.arg("--verify").arg(signature).arg(artifact);
    if let Some(fp) = fingerprint {
        // EP-0011 S-008: verify fingerprint against trust store at
        // ~/.config/neurox/plugin-trust.json. If the fingerprint isn't
        // in the trust list, reject the install.
        let home = std::env::var("HOME").unwrap_or_default();
        let trust_path = std::path::PathBuf::from(&home).join(".config/neurox/plugin-trust.json");
        if let Ok(content) = std::fs::read_to_string(&trust_path) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(arr) = parsed.get("trusted_keys").and_then(|v| v.as_array()) {
                    let trusted: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    if !trusted.contains(&fp) {
                        anyhow::bail!(
                            "fingerprint '{}' not in trust store ({}); add to {}",
                            fp,
                            trust_path.display(),
                            trusted.join(", ")
                        );
                    }
                }
            }
        }
        cmd.env("GNUPGHOME", std::env::temp_dir().join("neurox-gpg"));
    }
    let status = cmd
        .status()
        .with_context(|| format!("failed to invoke {}", gpg))?;
    if !status.success() {
        anyhow::bail!("gpg verification failed: {status}");
    }
    Ok(())
}

fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

# Finding: First-run admin password leaked to journald via stderr

## Metadata

- ID: SEC-0001-2026-002
- Severity: Critical
- CVSS-equivalent: 9.8
- STRIDE: I,R
- OWASP ASVS: v5.0.0-2.10.4
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `daemon/core/src/main.rs`
- Line: 511-518 (eprintln!("    password: {admin_pwd}"))
- Component: daemon (first-run bootstrap)
- Commit: current main

## Description

When the daemon starts for the first time and bootstraps the admin user,
it prints the generated admin password to **stderr** via `eprintln!`. On
Arch Linux (and most systemd-based distros), stderr is captured by
systemd-journald and persisted indefinitely. The password can later
be read by anyone with `journalctl` access (the user themself, root,
or any user in the `systemd-journal` group).

The exact log line from this machine:

```
ago 11 13:26:03 archlinux neurox[5558]:   [auth] FIRST-RUN bootstrap
ago 11 13:26:03 archlinux neurox[5558]:   Created initial admin user. Credentials:
ago 11 13:26:03 archlinux neurox[5558]:     username: admin
ago 11 13:26:03 archlinux neurox[5558]:     password: 9ty58S9m@dcfAkqU
```

The password `9ty58S9m@dcfAkqU` is now permanently recoverable.

## Impact

Whoever can read journalctl on this host can recover the bootstrap admin
password. On a single-user machine that's just the owner; on a shared
machine or in a containerized deployment it can leak to other admins.

The bootstrap admin has **full RBAC privileges** (admin role), so the
stolen password is equivalent to full takeover of the daemon's auth.

## Proof of Concept

```bash
journalctl --user -u neurox.service --since "2026-08-01" \
  | grep -E "password|admin"
```

**Expected output when the bug is present**:
```
ago 11 13:26:03 archlinux neurox[5558]:     password: 9ty58S9m@dcfAkqU
```

## Remediation

Print the password to a one-shot **file descriptor** that the user
manually reads (e.g. a FIFO that lives only during first boot), or
write it to a mode-0600 file in `~/.config/neurox/` and instruct the
user to `cat` it once and delete it. Do NOT use `eprintln!`.

```diff
--- a/daemon/core/src/main.rs
+++ b/daemon/core/src/main.rs
-        eprintln!("    username: admin");
-        eprintln!("    password: {admin_pwd}");
-        eprintln!("  ⚠️  Save this password now. It will NOT be shown again.");
-        eprintln!("  Change it immediately: PATCH /v1/users/me/password");
+        // Write the password to a private file the user must read manually.
+        let path = std::env::var_os("HOME")
+            .map(PathBuf::from)
+            .unwrap_or_default()
+            .join(".config/neurox/bootstrap_password");
+        std::fs::write(&path, format!("{admin_pwd}\n"))?;
+        #[cfg(unix)]
+        { let _ = std::os::unix::fs::PermissionsExt::set_mode(
+            &std::fs::metadata(&path)?.permissions(), 0o600); }
+        eprintln!("Bootstrap admin password written to {} (mode 0600).", path.display());
+        eprintln!("Read it, save it externally, then DELETE the file.");
```

Also: rotate the existing admin password immediately (`PATCH /v1/users/me/password`).

## References

- `daemon/core/src/main.rs:511-518`
- `journalctl --user -u neurox.service` shows the historic leak
## Regression Test

```bash
# 1. After fix, verify no password line in journald after a fresh first-run
journalctl --user -u neurox.service --since "today" | grep -E "password: "
[ $? -ne 0 ] || { echo "FAIL: password leaked"; exit 1; }
echo "OK: no password in journald"
```


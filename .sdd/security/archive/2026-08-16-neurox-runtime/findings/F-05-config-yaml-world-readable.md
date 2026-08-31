# Finding: `config.yaml` and backups world-readable (mode 0644)

## Metadata

- ID: SEC-0001-2026-005
- Severity: Medium
- CVSS-equivalent: 5.5
- STRIDE: I
- OWASP ASVS: v5.0.0-3.4.1
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `~/.config/neurox/config.yaml`
- Line: n/a (file mode)
- Component: daemon config
- Commit: n/a (filesystem)

Also: 3 `config.yaml.bak-*` files in the same directory.

## Description

`~/.config/neurox/config.yaml` has mode `0644` (world-readable). The
file contains runtime configuration that may include URLs, ports,
JWT paths, observability toggles, and other operational data. Three
backup files (`config.yaml.bak-20260811-135454`,
`config.yaml.bak-20250815-004329`,
`config.yaml.bak-pre-step3-20260815-192931`) are also world-readable
and may contain older secrets (e.g. a `jwt_secret` literal if it was
ever pasted in before being moved to a separate file).

Contrast with the correctly-protected `env`, `users.json`, and
`jwt_secret` files in the same directory, which are `0600`. This
inconsistency is almost certainly due to the editor/backup tool
leaving the new file world-readable and leaving stale `.bak` files.

## Impact

- `config.yaml` contents leaked to any local user (paths, ports,
  auth toggles, feature flags).
- Backup `.bak` files may contain **historical secrets** (e.g. an old
  JWT secret literal, an old api_token). Even if the live `jwt_secret`
  file has been rotated, the backup still holds the old one.

## Proof of Concept

```bash
ls -la ~/.config/neurox/config.yaml*
# -rw-r--r-- ... config.yaml
# -rw-r--r-- ... config.yaml.bak-20260811-135454
# -rw-r--r-- ... config.yaml.bak-20260815-004329
# -rw-r--r-- ... config.yaml.bak-pre-step3-20260815-192931

# From any other user:
/usr/bin/cat ~/.config/neurox/config.yaml
/usr/bin/cat ~/.config/neurox/config.yaml.bak-pre-step3-20260815-192931
```

**Expected output when the bug is present**: file contents visible.

## Remediation

1. `chmod 600 ~/.config/neurox/config.yaml*` (sets all matches).
2. **Delete** the `.bak-*` files after verifying the live `config.yaml`
   is correct — they should never be left behind.
3. Add a CI/lint rule: any `~/.config/neurox/*` file MUST be `0600`.
4. Configure the editor (if `vim`, set `backupmode` to a private
   location) to avoid creating world-readable backups in the config
   dir.

```bash
chmod 600 ~/.config/neurox/config.yaml ~/.config/neurox/*.bak-*
# After verifying config.yaml is correct:
rm ~/.config/neurox/config.yaml.bak-*
```

## References

- `~/.config/neurox/config.yaml` (current + 3 backups)
- Compared with `env`, `jwt_secret`, `users.json` which are `0600` ✓
## Regression Test

```bash
test "$(stat -c '%a' ~/.config/neurox/config.yaml)" = "600" || exit 1
ls ~/.config/neurox/*.bak-* 2>/dev/null | grep -q . && { echo "FAIL: stale backups"; exit 1; }
echo "OK"
```


# Finding: SQLite databases world-readable (mode 0644)

## Metadata

- ID: SEC-0001-2026-004
- Severity: Medium
- CVSS-equivalent: 5.5
- STRIDE: I
- OWASP ASVS: v5.0.0-3.4.1
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: all 5 active SQLite DBs
- Line: n/a
- Component: workspace (databases)
- Commit: n/a (file mode is filesystem-level)

| DB | Path | Size | Mode |
|---|---|---:|---|
| neurox.db | `~/.local/share/neurox/` | 1.1 MB | 0644 |
| logs.sqlite | `~/.local/share/neurox/` | 20 KB | 0644 |
| memory.db | `~/.local/share/memoryd/` | 1.5 MB | 0644 |
| workspace-1.db | `~/.local/share/memoryd/workspaces/` | 222 KB | 0644 |
| llmd.db | `~/.local/share/llmd/` | 20 KB | 0644 |

## Description

All 5 active SQLite databases have permissions `0644` (world-readable).
On a multi-user system, any local user can:

- Read `memory.db` → all memories + embeddings (BLOBs) + content_hashes.
- Read `neurox.db` → all sessions + all messages (including LLM
  responses that may have echoed user input).
- Read `logs.sqlite` → operational logs.
- Read `llmd.db` → LLM provider configuration.

The user-facing data includes personal notes, embedded vectors
(irreversibly leaked content), chat history with the agent, and
provider URLs/models. While this is a local-machine risk, it crosses
the trust boundary assumed by single-user workstations when other
users share the box.

## Impact

- **Confidentiality**: any local user can read all stored memories and
  sessions. Embeddings are also leaked (these are derived from text,
  so they reveal semantic content).
- **Integrity**: any local user can write to the DBs (0644 is
  world-writable on the group too? — no, 0644 = rw-r--r--, but ANY
  user can copy/modify the file and replace it with a tampered
  version that the daemon would happily read).

## Proof of Concept

```bash
# From any local user (not just andres_fernandez):
sqlite3 ~/.local/share/memoryd/memory.db \
  "SELECT id, category, substr(content, 1, 80) FROM memories LIMIT 3;"
```

**Expected output when the bug is present**:
```
ff3d8525-...|fact|Las skills de opencode en ~/.config/opencode/skills/...
fda15fea-...|fact|EP-0026-UX fix (2026-08-15): app shell ahora cubre 100dvh
```

## Remediation

Set permissions to `0600` on all DB files. Add to `~/.config/`
permissions enforcement and ensure any new DBs inherit `0600`.

```bash
chmod 600 ~/.local/share/neurox/*.db
chmod 600 ~/.local/share/memoryd/*.db
chmod 600 ~/.local/share/memoryd/workspaces/*.db
chmod 600 ~/.local/share/llmd/*.db
```

In Rust code (sqlx), set `PRAGMA` or post-creation chmod:
```rust
// After opening, restrict the file mode:
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(&db_path, perms)?;
}
```

Also: add a CI check that `stat -c '%a'` returns `600` for any DB
under `~/.local/share/{neurox,memoryd,llmd}/`.

## References

- `mcps/memory/memoryd/src/core/store/sqlite_store.rs` — where DBs are
  opened; no `set_permissions` call after creation.
- `daemon/core/src/session.rs:62` — same pattern for `neurox.db`.
## Regression Test

```bash
# After chmod + code fix, verify mode on a freshly-created DB
rm /tmp/test.db
(some code path that creates the DB)
test "$(stat -c '%a' /tmp/test.db)" = "600" || { echo "FAIL"; exit 1; }
echo "OK"
```


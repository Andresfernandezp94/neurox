# Tools Audit

This directory contains the audit work for the daemon's native tools.

## Contents

- `SUMMARY.md` — Top-level summary of the audit: tools covered, gaps
  found, fixes applied, recommended next steps.
- `shell.md` — Audit of the `shell` tool (sandbox read vs write, redirects).
- `write_file.md` — Audit of `write_file` (atomicity, race conditions).
- `read_file.md` — Audit of `read_file` (capacity overflow crash, ENOENT retry).
- `todo.md` — Audit of `todo_add` / `todo_done` / `todo_remove` / `todo_clear` / `todo_list`
  (lost-update race when concurrent).
- `grep.md` — Audit of `grep` (sandbox bypass on the `path` argument).
- `glob_listdir.md` — Audit of `glob` and `list_dir` (no gaps — already correct).
- `web.md` — Audit of `web_fetch` and `web_search` (SSRF, binary content).
- `save_fact.md` — Audit of `save_fact` (YAML corruption under concurrency).
- `verify_all_tools.py` — End-to-end functional verification harness
  (57 tests, 22 tools + parallel sanity). Run against a live daemon.

## How to run the verification

```bash
# Mint a JWT
TOKEN=$(python3 -c "import base64, json, time, uuid; \
  print(__import__('jwt').encode(\
    {'sub': '6c60c37c-b94c-482b-ba38-01e108c17de9', \
     'username': 'admin', 'role': 'Admin', \
     'iat': int(time.time()), 'exp': int(time.time()) + 24*3600}, \
    base64.b64decode(open('/home/andres_fernandez/.config/neurox/jwt_secret').read().split('\"secret\": \"')[1].split('\"')[0]), \
    algorithm='HS256'))")

# Run
python3 docs/audit/verify_all_tools.py
# (the script mints its own token from ~/.config/neurox/jwt_secret)
```

## Headline numbers

- 22 tools audited
- 18 with at least one gap
- ~32 gaps total
- 7 critical security vulnerabilities fixed
- 68/68 unit tests passing (from 56 before)
- 2 shared modules extracted: `tools/atomic_store.rs`, `tools/url_safety.rs`

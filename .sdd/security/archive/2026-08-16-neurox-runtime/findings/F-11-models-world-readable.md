# Finding: GGUF models world-readable (1.5 GB total in `~/models/`)

## Metadata

- ID: SEC-0001-2026-006
- Severity: Low
- STRIDE: I
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `~/models/`
- Component: model storage
- Commit: n/a (filesystem)

## Description

The GGUF model files are world-readable (`-rw-r--r--`):

- `embeddinggemma-300m-q8_0.gguf` (313 MB)
- `bge-large-en-v1.5-q8_0.gguf` (638 MB)
- `bge-m3-q8_0.gguf` (581 MB)
- `chat/` directory (other chat models)

These are inference models (not secret per se — anyone can download
the public GGUF from Hugging Face). However:

- They cost ~1.5 GB on disk.
- An attacker with local access can copy them off the box without
  hitting any quota/permission.
- They are intellectual property of the original model authors;
  the license may not permit unrestricted redistribution by
  unrelated parties.

## Impact

Low. The models are publicly downloadable. The only "leak" is
local-only access on a multi-user box.

## Proof of Concept

```bash
ls -la ~/models/*.gguf
# -rw-r--r-- ... embeddinggemma-300m-q8_0.gguf
# -rw-r--r-- ... bge-large-en-v1.5-q8_0.gguf
# -rw-r--r-- ... bge-m3-q8_0.gguf
```

## Remediation

```bash
chmod 600 ~/models/*.gguf
chmod 700 ~/models/chat/
```

If `llama-server` needs to read them, the user running it (the
`andres_fernandez` user) can still read them; world-read is removed.

## References

- `~/models/` (file permissions)
- llama-server docs
## Regression Test

```bash
test "$(stat -c '%a' ~/models/embeddinggemma-300m-q8_0.gguf)" = "600" \
  || { echo "FAIL"; exit 1; }
```


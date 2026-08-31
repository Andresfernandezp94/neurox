# Finding: Embedding cache leak via embedding model inversion (theoretical)

## Metadata

- ID: SEC-0001-2026-006
- Severity: Low
- STRIDE: I
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `mcps/memory/memoryd/src/core/store/sqlite_store.rs`
- Table: `embedding_cache` (BLOB column `vector`)
- Component: memoryd
- Commit: current main

## Description

The `embedding_cache` table stores the vector representations of all
memories as BLOBs (768-dim float32 vectors from `embeddinggemma-300m`).
While the cache is local-only (mode 0644 — see F-04), the vectors
themselves are reversible: with enough vectors, an attacker can use
embedding inversion attacks to reconstruct approximate text from
the vectors.

This is a **theoretical** finding — embedding inversion is an active
research area and not practical against modern models without
substantial compute. However, it is worth noting for a high-value
target (e.g. someone storing personal notes or business secrets as
memories).

## Impact

- Embeddings leak semantic content even when the original text is
  encrypted at rest.
- Combined with F-04 (world-readable DBs), an attacker with local
  access can recover text from vectors.

## Proof of Concept

No immediate PoC — this is a theoretical risk. Documented as a
"watchlist" finding for future review.

```bash
# Verify the existence and size of the cache:
sqlite3 ~/.local/share/memoryd/memory.db \
  "SELECT count(*), sum(length(vector)) FROM embedding_cache;"
```

## Remediation

1. **Encrypt the DB at rest** — `sqlcipher` integration or filesystem
   encryption (LUKS).
2. **Hash-then-embed**: store the original text hash alongside the
   vector; if you don't need to invert, don't.
3. **Reduce dimensionality**: 768-dim is more invertible than 128-dim
   or 64-dim. If recall quality allows, project down.

## References

- Paper: "Information Leakage in Embedding Models" (Carlini et al.,
  2020) and follow-ups.
- `mcps/memory/memoryd/src/core/store/sqlite_store.rs` — BLOB storage
## Regression Test

N/A — theoretical finding. Add to a "watchlist" doc.


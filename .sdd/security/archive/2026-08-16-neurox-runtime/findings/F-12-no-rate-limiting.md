# Finding: No rate limiting on daemon or most MCPs (only memoryd has one)

## Metadata

- ID: SEC-0001-2026-006
- Severity: Low
- STRIDE: D
- OWASP ASVS: v5.0.0-11.1.4
- NIST SSDF: PS.3.2
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- Component: daemon, llmd, voiced, clickup-d, playwright-d
- Commit: current main

## Description

A repo-wide search for `rate.?limit|throttle|GovernorLayer|tower-governor`
found references only in:

- `mcps/memory/memoryd/src/main.rs` — `RateLimiter::new` (memoryd
  internal rate limit)
- `daemon/agents/agent/src/llm.rs` — comment about handling HTTP 429
  from the LLM provider (outgoing, not incoming)

No rate limiting on:

- The daemon itself (`daemon/core/src/router/`) — any client on
  loopback can spam `/v1/sessions`, `/v1/mcps`, etc.
- voiced — `/voice/ws` (per F-08, also public WS)
- llmd — the shell tool (per F-06, can spawn `/bin/sh -c <anything>`)
- clickup-d — REST proxy to ClickUp
- playwright-d — browser automation (expensive operations)

## Impact

A local attacker (or prompt-injected LLM) can DoS the daemon by
issuing many concurrent requests. The shell tool + no rate limit =
amplification vector if F-06 is exploited.

## Proof of Concept

```bash
# Loop 1000 session creations in 1s:
for i in $(seq 1 1000); do
  curl -s -X POST http://127.0.0.1:7878/v1/sessions \
    -H "Authorization: Bearer neurox-memory-dev-2026" &
done; wait
```

**Expected output when the bug is present**: daemon accepts all 1000
requests, spawns 1000 subprocess agents, exhausts memory.

## Remediation

Add a rate limit middleware (e.g. `tower-governor`) to the daemon
and to voiced (especially because `/voice/ws` is unauthenticated —
see F-03).

```rust
// In daemon/core/src/router/mod.rs:
use tower_governor::{GovernorLayer, GovernorConfig};
let governor = GovernorLayer {
    config: Box::leak(Box::new(GovernorConfig::default())),
};
api_routes = api_routes.layer(governor);
```

For voiced (WS), rate-limit at the connection level: `max N concurrent
WS per IP`.

## References

- `mcps/memory/memoryd/src/main.rs` (memoryd is the only one with a
  rate limiter — copy that pattern)
- tower-governor crate
## Regression Test

```bash
# Spam test: 200 requests in 1 second should hit 429
ab -n 200 -c 50 http://127.0.0.1:7878/v1/system/stats
# Should see ~50 of them return 429
```


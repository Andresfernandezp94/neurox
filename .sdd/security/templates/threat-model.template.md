# Threat Model: [REQUIRED: feature or component name]

## Metadata

- Threat model ID: [REQUIRED: TM-YYYY-NNN]
- Date: [REQUIRED: YYYY-MM-DD]
- Mode: threat-model
- Performed by: [REQUIRED: agent or human]
- Related proposal: [OPTIONAL: link to proposal.md if this is part of a larger change]

## Context

[REQUIRED: what is being modeled. The feature, the component, the change. 1-3 paragraphs.]

Example: This threat model covers the new WebSocket endpoint `/v1/agents/<id>/messages/stream` that will allow real-time streaming of agent responses to clients.

## Architecture

[REQUIRED: components + data flows + trust boundaries. ASCII art preferred.]

```
[Browser/Client]
     │ HTTPS
     ▼
[API Gateway] ── AuthLayer ──> [Agent Daemon]
                                    │
                                    │ WebSocket
                                    ▼
                                [LLM Provider]
                                    │
                                    │ HTTPS
                                    ▼
                                [External API]
```

## Trust Boundaries

[REQUIRED: explicit list of trust boundaries]

1. **Internet ↔ API Gateway**: clients on the public internet
2. **API Gateway ↔ Agent Daemon**: internal VPC
3. **Agent Daemon ↔ LLM Provider**: external service
4. **Agent Daemon ↔ Local SQLite**: local disk

## Assets

[REQUIRED: what's being protected]

- User credentials (JWT tokens)
- Agent conversation history (privacy)
- LLM API keys (cost, abuse)
- System shell access (RCE potential)

## STRIDE Analysis

### Data flow 1: Browser → API Gateway (HTTPS)

| STRIDE | Amenaza | Mitigación |
|---|---|---|
| S | [REQUIRED] | [REQUIRED o "no mitigado"] |
| T | [REQUIRED] | [REQUIRED] |
| R | [REQUIRED] | [REQUIRED] |
| I | [REQUIRED] | [REQUIRED] |
| D | [REQUIRED] | [REQUIRED] |
| E | [REQUIRED] | [REQUIRED] |

### Data flow 2: API Gateway → Agent Daemon (WebSocket)

| STRIDE | Amenaza | Mitigación |
|---|---|---|
| S | ... | ... |
| T | ... | ... |
| R | ... | ... |
| I | ... | ... |
| D | ... | ... |
| E | ... | ... |

[Repeat for each data flow]

## Threats Identified

[REQUIRED: aggregate threats with severity]

| ID | Threat | STRIDE | Severity | Mitigation |
|---|---|---|---|---|
| TM-NNN | [description] | [S\|T\|...] | [Critical\|High\|...] | [how to mitigate] |
| TM-NNN | ... | ... | ... | ... |

## Security Decisions

[REQUIRED: explicit decisions made during this threat model]

- **D1**: [decision] — rationale
- **D2**: [decision] — rationale

## Out of Scope

[REQUIRED: explicit list of what this threat model does NOT cover]

- Authentication of LLM provider (assumed secure)
- Network-level DDoS (handled by API Gateway)
- Physical security of servers

## Validation

Before implementing:

- [ ] All STRIDE threats identified have a mitigation OR a documented "accepted risk"
- [ ] Critical/High threats have specific mitigations
- [ ] Decision log captures the "why" for each design choice
- [ ] `bash security/bin/validate-threat-model.sh <this-file>.md` exits 0

## References

- security/lib/stride-explained.md — STRIDE categories explained
- security/lib/owasp-asvs-mini.md — OWASP ASVS for API security
- security/process.md — overall process

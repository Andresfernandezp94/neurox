# Audit Report: [REQUIRED: same scope as proposal]

## Metadata

- Audit ID: [REQUIRED: YYYY-MM-DD-<scope>]
- Completed: [REQUIRED: YYYY-MM-DD]
- Performed by: [REQUIRED: agent or human name]

## Executive Summary

[REQUIRED: 3-5 sentences for stakeholders. State the key risk and the recommended immediate action. No jargon.]

**TL;DR**: [REQUIRED: one-sentence summary]

## Methodology

[REQUIRED: 1 paragraph. Which layers were covered, which tools were used, what was in scope. Link to design.md for details.]

## Findings Summary

[REQUIRED: table with all findings, sorted by severity descending]

| ID | Severity | CVSS | STRIDE | Title | Status |
|---|---|---|---|---|---|
| [REQUIRED: SEC-XXXX-YYYY-NNN] | [REQUIRED: Critical\|High\|Medium\|Low] | [REQUIRED: 0.0-10.0] | [REQUIRED: S\|T\|R\|I\|D\|E] | [REQUIRED: title] | [REQUIRED: Open\|Confirmed\|Fixed] |
| ... | ... | ... | ... | ... | ... |

## Findings by Severity

### Critical (N)

[REQUIRED: list of critical findings, with one-line description + link to detailed file]

### High (N)

[REQUIRED: same format]

### Medium (N)

[REQUIRED: same format]

### Low (N)

[REQUIRED: same format]

## Findings by STRIDE Category

[REQUIRED: aggregate view by STRIDE category, helps identify systemic issues]

| STRIDE | # findings | Notable examples |
|---|---|---|
| S (Spoofing) | [N] | [example] |
| T (Tampering) | [N] | [example] |
| R (Repudiation) | [N] | [example] |
| I (Info Disclosure) | [N] | [example] |
| D (DoS) | [N] | [example] |
| E (Elevation) | [N] | [example] |

## What Was NOT Found (positive)

[REQUIRED: explicit list of things that were checked but came up clean. This builds confidence.]

- ✅ No hardcoded secrets in code (gitleaks clean)
- ✅ No SQL injection (sqlx with prepared statements throughout)
- ✅ Path traversal mitigated (`resolve_under_workspace()`)
- ✅ Auth HTTP uses timing-safe comparison (`subtle::ConstantTimeEq`)
- ✅ Bash tool requires approval (`requires_approval: true`)

## Detailed Findings

[REQUIRED: link to each finding file]

- [F-01-...](findings/F-01-...md) — Critical
- [F-02-...](findings/F-02-...md) — High
- [F-03-...](findings/F-03-...md) — Medium
- [F-04-...](findings/F-04-...md) — Low
- ...

## Out of Scope (considered, not findings)

[REQUIRED: things that looked like findings but aren't, with reasoning]

- **[Theoretical issue]**: [why it's not a finding, e.g., "requires local access"]

## Immediate Actions Required

[REQUIRED: list of Critical/High findings that need action NOW]

1. **[SEC-XXXX-YYYY-NNN]** — [title]
   - Mitigation: [concrete action]
   - Owner: [name]
   - Deadline: [date]

## Recommendations (Roadmap)

[REQUIRED: prioritized list of follow-up actions]

1. [First priority — usually Critical]
2. [Second priority]
3. [Backlog]

## Metrics

[REQUIRED: track these for future trend analysis]

- Total findings: [N]
- Critical: [N]
- High: [N]
- Medium: [N]
- Low: [N]
- Coverage: [% of layers covered]
- Time spent: [hours]

## References

- security/templates/finding.template.md — finding template used
- security/lib/severity-matrix.md — severity classification
- security/lib/stride-explained.md — STRIDE categories
- security/lib/owasp-asvs-mini.md — ASVS mapping reference
- security/lib/nist-ssdf-mini.md — SSDF mapping reference

## Changelog

| Date | Change |
|---|---|
| [REQUIRED: YYYY-MM-DD] | Initial report |

# Finding: [REQUIRED: short descriptive title]

## Metadata

> Todos los campos marcados REQUIRED deben estar. El validator fallará si falta alguno.

- ID: [REQUIRED: SEC-NNNN-YYYY-NNN]   <!-- Generar con: bash security/bin/new-finding.sh -->
- Severity: [REQUIRED: Critical|High|Medium|Low]   <!-- Validar con: bash security/bin/score-severity.sh -->
- CVSS-equivalent: [REQUIRED: 0.0-10.0]
- STRIDE: [REQUIRED: S|T|R|I|D|E]   <!-- Puede ser múltiple separado por coma: S,T,I -->
- OWASP ASVS: [OPTIONAL: v5.0.0-X.Y.Z]
- NIST SSDF: [OPTIONAL: PO|PS|PW|RV.N.M]
- Discovered: [REQUIRED: YYYY-MM-DD]
- Discovered by: [REQUIRED: agent-name or human-name]
- Status: [REQUIRED: Open|Confirmed|Fixed|WontFix|FalsePositive]

## Location

- File: [REQUIRED: path/to/file.ext]
- Line: [REQUIRED: line_number]
- Component: [REQUIRED: which component/repo is affected]
- Commit: [OPTIONAL: git commit hash where it exists]

## Description

[REQUIRED: 2-5 sentences explaining what the vulnerability is, in plain language. No jargon.]

## Impact

[REQUIRED: 2-5 sentences explaining what could happen if this is exploited. Be concrete about who/what is affected.]

## Proof of Concept

```bash
[REQUIRED: executable command that demonstrates the vulnerability]
```

**Expected output when the bug is present**:
```
[REQUIRED: what the command should output when the bug exists]
```

## Remediation

[REQUIRED: concrete code/config change to fix. Be specific. Reference the exact file:line to change.]

Example structure:
```diff
- old code
+ new code
```

## Regression Test

```bash
[REQUIRED: how to verify the fix works. Should fail before fix, pass after.]
```

**Expected output after fix**:
```
[REQUIRED: what the test should show when the fix is in place]
```

## References

- [OPTIONAL: links to docs, CVEs, commits, related findings]
- [OPTIONAL: related findings in this audit (e.g., SEC-0001-2026-002)]

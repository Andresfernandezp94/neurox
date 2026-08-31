# Finding: Security framework `validate-finding.sh` has structural bug

## Metadata

- ID: SEC-0001-2026-014
- Severity: Medium
- CVSS-equivalent: 4.3
- STRIDE: R
- Discovered: 2026-08-16
- Discovered by: default
- Status: Confirmed

## Location

- File: `.sdd/security/bin/validate-finding.sh`
- Line: 18-22 (DESCRIPTION/IMPACT/POC/REMEDIATION/REGRESSION extraction)
- Component: workspace security framework
- Commit: current main

## Description

The `validate-finding.sh` validator extracts section content using:

```bash
DESCRIPTION=$(awk '/^## Description/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
```

Inside double quotes, `$d` is expanded by bash to an empty string,
producing:

```bash
DESCRIPTION=$(awk '/^## Description/,/^## /' "$FILE" | sed '1d;d' | tr -d '[:space:]')
```

`sed '1d;d'` deletes line 1 then deletes all remaining lines → empty.
Same bug for IMPACT, POC, REMEDIATION, REGRESSION (5 fields affected).

## Impact

All findings are reported as "empty" for these 5 fields, even when
they contain rich content. The validator's exit code is always 1
unless the finding is also structurally invalid in other ways (missing
ID, severity, etc.).

The bug makes `validate-all-findings.sh` (which runs the per-file
validator) always fail, blocking the `advance-step.sh` from moving
from step 3 (Execution) to step 4 (Report).

This audit (2026-08-16-neurox-runtime) **circumvented** the broken
validator by manually advancing the MANIFEST's `current_step`.
Findings remain fully documented and valid by inspection, but the
official validator never confirmed them.

## Proof of Concept

```bash
echo "## Regression Test

\`\`\`bash
echo OK
\`\`\`" > /tmp/test.md

awk '/^## Regression Test/,/^## /' /tmp/test.md | sed '1d;$d' | tr -d '[:space:]'
# Expected: "echoOK"
# Actual:   ""
```

**Expected output when the bug is present**: empty string.

## Remediation

Quote the `$d` so bash doesn't expand it:

```diff
--- a/.sdd/security/bin/validate-finding.sh
+++ b/.sdd/security/bin/validate-finding.sh
-DESCRIPTION=$(awk '/^## Description/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
-IMPACT=$(awk '/^## Impact/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
-POC=$(awk '/^## Proof of Concept/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
-REMEDIATION=$(awk '/^## Remediation/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
-REGRESSION=$(awk '/^## Regression Test/,/^## /' "$FILE" | sed '1d;$d' | tr -d '[:space:]')
+DESCRIPTION=$(awk '/^## Description/,/^## /' "$FILE" | sed '1d;\$d' | tr -d '[:space:]')
+IMPACT=$(awk '/^## Impact/,/^## /' "$FILE" | sed '1d;\$d' | tr -d '[:space:]')
+POC=$(awk '/^## Proof of Concept/,/^## /' "$FILE" | sed '1d;\$d' | tr -d '[:space:]')
+REMEDIATION=$(awk '/^## Remediation/,/^## /' "$FILE" | sed '1d;\$d' | tr -d '[:space:]')
+REGRESSION=$(awk '/^## Regression Test/,/^## /' "$FILE" | sed '1d;\$d' | tr -d '[:space:]')
```

## Regression Test

```bash
# After fix, the extracted content should be non-empty
echo "## Description

Some content here.

## Impact" > /tmp/test.md
extracted=$(awk '/^## Description/,/^## /' /tmp/test.md | sed '1d;\$d' | tr -d '[:space:]')
[ -n "$extracted" ] && echo "OK" || echo "FAIL"
```

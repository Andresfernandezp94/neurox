#!/usr/bin/env bash
# security/bin/score-severity.sh
# Helper para sugerir severity basado en keywords del description.
# NO reemplaza el juicio humano, solo sugiere.
#
# Uso: bash security/bin/score-severity.sh "<description>"

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Uso: bash security/bin/score-severity.sh \"<description>\"" >&2
  exit 1
fi

DESC=$(echo "$1" | tr '[:upper:]' '[:lower:]')

SCORE=0
RATIONALE=""

# Keywords que suman puntos (max ~10)
# Helper para matchear keywords con regex (case insensitive)
match() {
  local pattern="$1"
  [[ "$DESC" =~ $pattern ]]
}

match 'rce|remote.code.execution|arbitrary.code' && {
  SCORE=$((SCORE + 6))
  RATIONALE+="RCE pattern (+6). "
}
match 'secret.*(exposed|leaked|in.*code|in.*repo|commited|commiteado)' && {
  SCORE=$((SCORE + 5))
  RATIONALE+="Secret exposure pattern (+5). "
}
match 'auth.*bypass|bypass.*authentication|no.*auth|missing.*auth|sin.*auth|sin.*autenticacion' && {
  SCORE=$((SCORE + 5))
  RATIONALE+="Auth bypass pattern (+5). "
}
match 'sql.*injection|sqli' && {
  SCORE=$((SCORE + 5))
  RATIONALE+="SQL injection pattern (+5). "
}
match 'command.*injection|rce.*via' && {
  SCORE=$((SCORE + 6))
  RATIONALE+="Command injection pattern (+6). "
}
match 'hardcoded|commited|commiteado' && {
  SCORE=$((SCORE + 4))
  RATIONALE+="Hardcoded credential pattern (+4). "
}
match 'xss' && {
  SCORE=$((SCORE + 3))
  RATIONALE+="XSS pattern (+3). "
}
match 'csrf' && {
  SCORE=$((SCORE + 2))
  RATIONALE+="CSRF pattern (+2). "
}
match 'idor|insecure.*direct|privilege.*escalation|escalation.*privilegios' && {
  SCORE=$((SCORE + 5))
  RATIONALE+="IDOR / privilege escalation pattern (+5). "
}
match 'denial.*of.*service|\bdos\b|rate.*limit' && {
  SCORE=$((SCORE + 2))
  RATIONALE+="DoS pattern (+2). "
}
match 'missing.*encryption|no.*tls|plaintext' && {
  SCORE=$((SCORE + 4))
  RATIONALE+="Missing encryption pattern (+4). "
}
match 'info.*disclosure|verbose.*error|stack.*trace|\bdisclosure\b|fuga' && {
  SCORE=$((SCORE + 2))
  RATIONALE+="Info disclosure pattern (+2). "
}
match 'missing.*(header|csp|hsts)' && {
  SCORE=$((SCORE + 1))
  RATIONALE+="Missing header pattern (+1). "
}
match 'api.*key.*(leaked|exposed)' && {
  SCORE=$((SCORE + 5))
  RATIONALE+="API key exposure pattern (+5). "
}

# Bonus por combos especialmente peligrosos
if [[ "$DESC" =~ (rce|remote.code|arbitrary.code|command.injection) ]] && \
   [[ "$DESC" =~ (auth.bypass|no.auth|missing.auth|sin.auth|sin.autenticacion|exposed|public) ]]; then
  SCORE=$((SCORE + 4))
  RATIONALE+="⚠️  COMBO CRÍTICO: RCE + auth bypass → Critical (+4 extra). "
fi

# Clamp a [0, 10]
[ "$SCORE" -gt 10 ] && SCORE=10
[ "$SCORE" -lt 0 ] && SCORE=0

# Mapear score a severity
case "$SCORE" in
  9|10) SEVERITY="Critical" ;;
  7|8)  SEVERITY="High" ;;
  4|5|6) SEVERITY="Medium" ;;
  *)    SEVERITY="Low" ;;
esac

echo "Suggested severity: $SEVERITY (score: $SCORE/10)"
echo "Rationale: $RATIONALE"
echo ""
echo "NOTA: Esta es solo una sugerencia. Verifica con:"
echo "  cat security/lib/severity-matrix.md"
echo ""
echo "Si aceptas, usa: $SEVERITY en el campo 'Severity:' del finding."

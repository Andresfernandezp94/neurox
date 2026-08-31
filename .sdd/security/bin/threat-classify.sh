#!/usr/bin/env bash
# security/bin/threat-classify.sh
# Helper para sugerir categorías STRIDE basadas en keywords del description.
# NO reemplaza el juicio humano, solo sugiere.
#
# Uso: bash security/bin/threat-classify.sh "<description>"

set -euo pipefail

if [ $# -ne 1 ]; then
  echo "Uso: bash security/bin/threat-classify.sh \"<description>\"" >&2
  exit 1
fi

DESC=$(echo "$1" | tr '[:upper:]' '[:lower:]')

CLASSIFIED=""
RATIONALE=""

# Spoofing
if [[ "$DESC" =~ (spoof|impersonat|forg|credential.stuff|phish|token.replay|jwt.none) ]]; then
  CLASSIFIED+="S,"
  RATIONALE+="S (Spoofing) — keywords match impersonation/forgery. "
fi

# Tampering
if [[ "$DESC" =~ (tamper|sql.inject|path.traversal|command.inject|xss|deserial|unauthorized.modif|manipulat|integrity) ]]; then
  CLASSIFIED+="T,"
  RATIONALE+="T (Tampering) — keywords match data modification. "
fi

# Repudiation
if [[ "$DESC" =~ (repud|deny.action|no.audit.log|missing.log|no.timestamp|non.repud) ]]; then
  CLASSIFIED+="R,"
  RATIONALE+="R (Repudiation) — keywords match action denial / missing audit. "
fi

# Information Disclosure
if [[ "$DESC" =~ (info.disclos|leak|expos|verbose|stack.trace|directory.list|plaintext|unencrypted|no.tls|missing.encryption) ]]; then
  CLASSIFIED+="I,"
  RATIONALE+="I (Information Disclosure) — keywords match data exposure. "
fi

# Denial of Service
if [[ "$DESC" =~ (denial.of.service|dos|amplif|redos|catastrophic|backtrack|resource.exhaust|rate.limit|zip.bomb|mem.leak|infinite.loop) ]]; then
  CLASSIFIED+="D,"
  RATIONALE+="D (DoS) — keywords match availability impact. "
fi

# Elevation of Privilege
if [[ "$DESC" =~ (privilege.escalat|idor|insecure.direct|vertical.escalat|horizontal.escalat|rce|shell.access|admin.access|unauthorized.access) ]]; then
  CLASSIFIED+="E,"
  RATIONALE+="E (Elevation of Privilege) — keywords match privilege escalation. "
fi

# Si no se clasificó nada, default a I (info disclosure) como fallback
if [ -z "$CLASSIFIED" ]; then
  CLASSIFIED="I"
  RATIONALE="No specific STRIDE keyword matched. Defaulted to I (Info Disclosure) as safest fallback."
fi

# Limpiar la última coma
CLASSIFIED=$(echo "$CLASSIFIED" | sed 's/,$//')

echo "Suggested STRIDE: $CLASSIFIED"
echo "Rationale: $RATIONALE"
echo ""
echo "NOTA: Esta es solo una sugerencia. Verifica con:"
echo "  cat security/lib/stride-explained.md"
echo ""
echo "Si aceptas, usa: STRIDE: $CLASSIFIED en el finding."

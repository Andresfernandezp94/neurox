# Disclosure Policy — neurox

> **Cómo manejar un reporte de vulnerabilidad recibido externamente.**
> Basado en ISO/IEC 29147 + CISA CVD Process + Google Project Zero policy.

## Overview

neurox acepta reportes de vulnerabilidades de seguridad de la comunidad. Este documento define cómo se manejan esos reportes — desde la recepción hasta la disclosure pública.

## Canales de recepción

### Canal primario (preferido)
- **Email**: andresfernandezp94@gmail.com
- **PGP**: (pendiente — agregar key cuando se configure)
- **Asunto**: incluir `[SECURITY]` al inicio

### Canales secundarios
- **GitHub Security Advisories**: https://github.com/Andresfernandezp94/neurox/security/advisories (cuando esté habilitado)
- **Issue tracker público**: solo para problemas NO sensibles. Reportes de seguridad NUNCA por issue público.

## Qué incluir en el reporte

Para acelerar el triage, el reporte debe incluir:

1. **Título descriptivo** (1 línea)
2. **Descripción técnica** (qué es la vulnerabilidad)
3. **Pasos para reproducir** (PoC ejecutable)
4. **Impacto** (qué se ve afectado)
5. **Versiones afectadas** (qué commit/tag/release)
6. **Versiones donde NO ocurre** (si lo probaste)
7. **Tu nombre/handle** (opcional — podés ser anónimo)
8. **Disclosure timeline preferido** (si tenés restricciones)

### Plantilla sugerida

```
Asunto: [SECURITY] <título corto>

## Resumen
<1-2 oraciones>

## Detalle técnico
<descripción técnica>

## Pasos para reproducir
1. <paso 1>
2. <paso 2>
3. <paso 3>

## Resultado esperado
<qué pasa cuando seguís los pasos>

## Impacto
<qué se ve afectado>

## Versiones afectadas
- <commit hash>
- <tag>
- <release>

## Versiones donde NO ocurre
- <commit hash>
- <tag>

## Información del reporter
- Nombre/handle: <opcional>
- Email de contacto: <opcional>
- Quiere crédito en advisory público: sí/no

## Timeline preferido
<si tenés restricciones (ej: "necesito disclosure antes de X fecha")>
```

## Triage y respuesta

### SLA de respuesta inicial

| Severidad estimada | SLA de primera respuesta |
|---|---|
| Critical | < 24 horas |
| High | < 3 días |
| Medium | < 7 días |
| Low | < 14 días |

### Proceso interno

1. **Recepción** — Email llega a `andresfernandezp94@gmail.com`
2. **Acknowledge** — Responder al reporter confirmando recepción (dentro del SLA)
3. **Triage** — Evaluar severidad con `bash security/bin/score-severity.sh`
4. **Reproducir** — Validar que el reporte es legítimo (NO es un false positive)
5. **Asignar** — Si es válido, crear finding en `security/archive/<fecha>-<nombre>/findings/F-NN-*.md`
6. **Coordinar** — Comunicar al reporter el plan de remediación
7. **Remediar** — sixbell-developer crea el fix
8. **Validar** — Verificar que el fix resuelve el issue
9. **Disclosure** — Coordinar fecha de disclosure pública con el reporter

## Disclosure timeline (default)

Basado en Google Project Zero + ZDI:

- **Reporte recibido** → Acknowledge + triage (1-7 días según severidad)
- **Trabajo en fix** → hasta 90 días desde el reporte
- **Pre-disclosure** → Notificar al reporter 7 días antes de la fecha de disclosure
- **Disclosure pública** → Después del fix + 7 días de gracia (para que los usuarios actualicen)
- **Advisory GitHub** → Publicado en la fecha de disclosure

### Excepciones al timeline

| Situación | Disclosure inmediata |
|---|---|
| Explotación activa observada | Sin esperar fix |
| Reporter no responde por 60 días | Disclosure unilateral |
| Reporter pide disclosure inmediata | Sin esperar 90 días |

## Crédito al reporter

Si el reporter quiere crédito:
- En advisory público: incluir nombre/handle (a elección del reporter)
- En CHANGELOG del repo: incluir nombre en la versión del fix
- En GitHub Security Advisories: incluir como "Credits"

Si el reporter prefiere anónimo:
- No incluir en advisory
- Agradecer en privado

## Lo que NO hacemos

- **No** contactamos a law enforcement contra reporters de buena fe
- **No** demandamos a reporters que respetan este policy
- **No** pagamos bounties (neurox es open source no comercial)
- **No** publicamos datos del reporter sin consentimiento
- **No** usamos canales inseguros (Discord, Twitter, etc.) para discutir vulnerabilidades

## Safe harbor

Reporter de buena fe que:
- Respeta este policy
- No accede a data de otros usuarios
- No degrada el servicio
- Reporta de inmediato si descubre data sensible

...está protegido de acciones legales.

## Out of scope

Estos issues NO califican para disclosure coordinado:

- Self-XSS
- CSRF en endpoints sin state-changing operations
- Missing security headers sin impacto demostrable
- Rate limiting ausente sin evidencia de abuse
- Issues en versiones EOL (deprecated)
- Issues en forks no mantenidos por el owner
- Bugs que requieren acceso físico

## Template de advisory público

```markdown
# Security Advisory: <título>

**Severity**: <Critical|High|Medium|Low>
**CVSS**: <score>
**Affected versions**: <rango>
**Patched in**: <versión>
**Disclosed**: <YYYY-MM-DD>

## Summary
<1-2 oraciones>

## Details
<descripción técnica>

## Impact
<qué se ve afectado>

## Reproduction
<PoC>

## Remediation
<cómo actualizar / qué fix se aplicó>

## Credits
<reporter (si quiere crédito)>

## References
- <links a commits, advisories, etc.>
```

## Schema del proceso (para `bin/disclosure.sh`)

```
1. Validar formato del reporte (template match)
2. Asignar ID único: SEC-DISC-YYYY-NNN
3. Crear directorio: security/archive/<fecha>-<slug>/
4. Inicializar finding con severidad estimada
5. Enviar email de acknowledge al reporter
6. Esperar triage humano
7. Si válido → flujo normal de audit mode
8. Si no válido → enviar email de "no es un finding" + cerrar
```

## Referencias

- ISO/IEC 29147 (Vulnerability Disclosure): https://www.iso.org/standard/72311.html
- ISO/IEC 30111 (Vulnerability Handling): https://www.iso.org/standard/69725.html
- CISA CVD: https://www.cisa.gov/resources-tools/programs/coordinated-vulnerability-disclosure-program
- Google Project Zero policy: https://googleprojectzero.blogspot.com/p/disclosure-deadlines-and-bug-bounties.html
- CERT Guide to CVD: https://certcc.github.io/CERT-Guide-to-CVD/

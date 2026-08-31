# STRIDE Explained — neurox

> **Cómo clasificar un threat con STRIDE.**
> Para clasificar automáticamente: `bash security/bin/threat-classify.sh "<description>"`

## Qué es STRIDE

STRIDE es un modelo de Microsoft (1999) que categoriza amenazas en 6 tipos. Cada letra = una propiedad de seguridad que se puede violar.

## Las 6 categorías

### S — Spoofing (Suplantación de identidad)
**Pregunta clave**: ¿Se puede hacer pasar por otro usuario/sistema?

**Ejemplos**:
- Login sin verificar credenciales
- Token JWT sin firma
- IP spoofing
- DNS spoofing
- Replay attack

**Contramedidas típicas**: autenticación, firma digital, MFA, certificate pinning.

### T — Tampering (Manipulación)
**Pregunta clave**: ¿Se puede modificar data sin autorización?

**Ejemplos**:
- SQL injection
- Path traversal
- Modificar cookies/headers
- Modificar data en tránsito sin TLS
- Modificar binarios en disco

**Contramedidas típicas**: firma, hash, MAC, TLS, validación de input, parameterized queries.

### R — Repudiation (Repudio)
**Pregunta clave**: ¿Se puede negar una acción sin evidencia?

**Ejemplos**:
- Logs sin timestamps
- Logs que el atacante puede borrar
- Acciones sin audit trail
- Falta de non-repudiation en transacciones

**Contramedidas típicas**: audit logs inmutables, timestamps firmados, append-only logs.

### I — Information Disclosure (Divulgación de información)
**Pregunta clave**: ¿Se filtra data que no debería?

**Ejemplos**:
- Mensajes de error verbose
- Stack traces en responses
- Headers que exponen versiones
- Data en logs
- Data en backups sin cifrar
- Secrets en código/repos

**Contramedidas típicas**: cifrado, minimización de data, generic error messages, secret management.

### D — Denial of Service (Denegación de servicio)
**Pregunta clave**: ¿Se puede tirar el servicio?

**Ejemplos**:
- Sin rate limiting
- Resource exhaustion
- Algoritmo O(n²) con input controlado
- ReDoS (regex catastrophic backtracking)
- Zip bomb
- Billion laughs

**Contramedidas típicas**: rate limiting, timeouts, circuit breakers, async processing, resource limits.

### E — Elevation of Privilege (Escalación de privilegios)
**Pregunta clave**: ¿Se pueden escalar permisos?

**Ejemplos**:
- IDOR (Insecure Direct Object Reference)
- Vertical privilege escalation (user → admin)
- Horizontal privilege escalation (user1 → user2)
- Container escape
- SQL injection que permite UNION SELECT con admin
- Path traversal que accede a `/etc/shadow`

**Contramedidas típicas**: autorización por rol, validación de ownership, principio de least privilege, sandboxing.

## Cómo usar STRIDE en un threat model

1. **Diagrama el sistema** — componentes, data flows, trust boundaries
2. **Para cada data flow**, preguntá las 6 preguntas STRIDE
3. **Anotá las amenazas encontradas** en el template
4. **Priorizá por impacto** (combiná con severity-matrix.md)
5. **Diseñá contramedidas** para cada amenaza

## Ejemplo completo

**Sistema**: `neuro-pro serve --bind 0.0.0.0:7878` con WS endpoint `/v1/commands`

| Categoría | ¿Amenaza existe? | Razón |
|---|---|---|
| **S** | ✅ Sí | Atacante puede conectarse sin auth y pretender ser Andrés |
| **T** | ✅ Sí | Atacante puede enviar `start_agent` con config maliciosa |
| **R** | ⚠️ Parcial | Hay logs pero atacante puede acceder y modificar si escala |
| **I** | ✅ Sí | `/v1/brain/status` filtra modelo + tokens sin auth |
| **D** | ✅ Sí | Atacante puede hacer `cancel_session` en sesiones activas |
| **E** | ✅ Sí | Atacante puede hacer `approval_response: approve` para auto-aprobar bash commands → RCE |

**Conclusión**: el finding es **Critical** porque múltiples categorías STRIDE están afectadas, con vector de ataque Network + complejidad Low.

## Schema (para usar en templates)

```yaml
stride: E  # una de: S | T | R | I | D | E
```

Si el finding afecta múltiples categorías, listar todas separadas por coma: `S,T,I`

## Atajos rápidos

| Si el finding es... | Probable STRIDE |
|---|---|
| Auth bypass | S |
| SQL/command injection | T, E |
| IDOR | E |
| Missing rate limit | D |
| Hardcoded secret | I |
| Log tampering | R, T |
| Missing TLS | T, I |

## Referencias

- Microsoft original: https://learn.microsoft.com/en-us/azure/security/develop/threat-modeling-tool
- OWASP: https://owasp.org/www-community/Threat_Modeling
- Shostack's book: https://shostack.org/books/threat-modeling-book

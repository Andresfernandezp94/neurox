# Example Findings — neurox

> **5 ejemplos reales de findings bien estructurados.**
> Para usar como referencia cuando generes un nuevo finding.

## Ejemplo 1: Critical (RCE combinado)

```markdown
# Finding: WS sin auth permite auto-aprobar bash commands → RCE workstation

## Metadata
- ID: SEC-0001-2026-001
- Severity: Critical
- CVSS-equivalent: 9.5
- STRIDE: S,T,R,I,D,E
- OWASP ASVS: v5.0.0-13.2.1, v5.0.0-4.1.1
- NIST SSDF: PW.7.2, RV.1.1
- Discovered: 2026-08-08
- Discovered by: EVA (auditoría 2026-08-08)

## Location
- File: core/src/router/ws.rs
- Line: 114-125
- Component: neuro-pro daemon (WS endpoint /v1/commands)

## Description
El endpoint WebSocket `/v1/commands` no valida el Bearer token en el handshake. Combinado con el daemon bindeado a `0.0.0.0:7878` y la existencia del bash tool (que requiere approval), un atacante en la LAN puede auto-aprobar approvals de bash commands del agente EVA, resultando en ejecución remota de código arbitrario en la workstation de Andrés.

## Impact
- **RCE workstation** del usuario que corre el daemon
- Acceso a todos los archivos en el HOME directory
- Acceso a secrets en `~/.config/neuro-pro/env`
- Capacidad de suplantar al agente y aprobar acciones futuras

## Proof of Concept
```bash
# Desde CUALQUIER dispositivo en 192.168.1.x
python3 -c "
import asyncio, json, websockets
async def t():
    async with websockets.connect('ws://victim:7878/v1/commands') as ws:
        await ws.send(json.dumps({'type': 'approval_response', 'id': '<uuid>', 'decision': 'approve'}))
asyncio.run(t())
"
```

**Expected output** (cuando hay un bash approval pending):
```
{"type":"approval_resolved","id":"<uuid>","decision":"approve"}
```
→ El bash command del agente se ejecuta inmediatamente.

## Remediation
1. Cambiar bind: `sed -i 's/--bind 0.0.0.0:7878/--bind 127.0.0.1:7878' ~/.config/systemd/user/neuro-pro.service`
2. Aplicar AuthLayer al upgrade de WS (ver hallazgo F-02)

## Regression Test
```bash
# Después del fix:
python3 -c "
import asyncio, json, websockets
async def t():
    try:
        async with websockets.connect('ws://192.168.1.5:7878/v1/commands') as ws:
            await ws.send(json.dumps({'type': 'list_agents'}))
            print(await ws.recv())
    except Exception as e:
        print(f'Expected: connection refused or 401 ({e})')
asyncio.run(t())
"
```
→ Debe fallar (connection refused) o retornar 401.

## References
- 2026-08-08-security-audit/findings/F-01-ws-sin-auth-bind-lan.md
- OWASP API Security Top 10 — API1:2023 Broken Object Level Authorization
```

## Ejemplo 2: High (hardcoded secret)

```markdown
# Finding: API key commiteada en código

## Metadata
- ID: SEC-0001-2026-002
- Severity: High
- CVSS-equivalent: 7.5
- STRIDE: I
- OWASP ASVS: v5.0.0-7.2.1
- NIST SSDF: PW.7.2
- Discovered: 2026-08-08
- Discovered by: gitleaks (CI scan)

## Location
- File: examples/agents-with-tls.yaml
- Line: 15
- Component: neuro-pro (ejemplo de config)

## Description
El archivo `examples/agents-with-tls.yaml` contiene un valor placeholder que, si se reemplaza accidentalmente con una API key real y se commitea, expone el secret en el historial de git.

## Impact
- Si se commitea una API key real, queda en git history para siempre (incluso si se borra después)
- Cualquiera con acceso al repo (incluso forks) puede extraerla
- Costos no autorizados si la key es de un servicio pago (AWS, OpenAI, etc.)

## Proof of Concept
```bash
grep -rn "API_KEY\|SECRET\|TOKEN" examples/agents-with-tls.yaml
```

**Expected output**:
```
15:  api_key: "change-me-to-something-random-and-secret"
```

## Remediation
1. Mover el secret a env var
2. Documentar en README cómo configurar
3. Agregar `examples/agents-with-tls.yaml` a gitleaks allowlist (NO al código real)

## Regression Test
```bash
bash security/bin/gitleaks.sh --no-git examples/
```
→ Debe retornar 0 leaks (excepto los en allowlist).

## References
- gitleaks docs: https://github.com/gitleaks/gitleaks
```

## Ejemplo 3: Medium (missing rate limit)

```markdown
# Finding: Endpoint de auth sin rate limit

## Metadata
- ID: SEC-0002-2026-001
- Severity: Medium
- CVSS-equivalent: 5.3
- STRIDE: D, S
- OWASP ASVS: v5.0.0-4.3.1
- NIST SSDF: PW.7.2
- Discovered: 2026-08-15
- Discovered by: sixbell-researcher

## Location
- File: neuro-pro-plugin-voice/src/main.rs
- Line: 78-81
- Component: voice plugin

## Description
El endpoint `/voice/start` no tiene rate limiting. Un atacante puede hacer miles de requests por segundo para crear miles de sesiones, agotando memoria del proceso.

## Impact
- DoS por memory exhaustion
- Cada sesión consume ~1KB de memoria (UUID + estado)
- 10K requests = 10MB, suficiente para empezar a notar lag

## Proof of Concept
```bash
for i in {1..10000}; do
  curl -s -X POST http://victim:9998/voice/start -d '{}' &
done
wait
ps aux | grep voiced | awk '{print $6}'  # RSS en KB
```

**Expected output**:
```
<user>  <pid>  <defunct>  50000  # memoria crece linealmente con # requests
```

## Remediation
Agregar `governor::RateLimiter` con 10 requests/min por IP:
```rust
use governor::{Quota, RateLimiter};
let limiter = RateLimiter::direct(Quota::per_minute(NonZeroU32::new(10).unwrap()));
```

## Regression Test
```bash
for i in {1..100}; do curl -s -X POST http://localhost:9998/voice/start; done | wc -l
# Debe retornar ~10 (las primeras 10), el resto 429 Too Many Requests
```

## References
- OWASP API Security Top 10 — API4:2023 Unrestricted Resource Consumption
```

## Ejemplo 4: Low (verbose errors)

```markdown
# Finding: Error messages exponen stack traces en producción

## Metadata
- ID: SEC-0003-2026-001
- Severity: Low
- CVSS-equivalent: 3.1
- STRIDE: I
- OWASP ASVS: v5.0.0-8.3.1
- NIST SSDF: PW.7.2
- Discovered: 2026-08-20
- Discovered by: sixbell-developer (peer review)

## Location
- File: neuro-pro-plugin-voice/src/voice_session.rs
- Line: 100-102
- Component: voice plugin

## Description
Cuando el bash command del usuario falla, el handler retorna el stderr completo al cliente, incluyendo paths absolutos del filesystem.

## Impact
- Information disclosure: paths internos del sistema
- Facilita reconocimiento para un atacante que ya tiene acceso

## Proof of Concept
```bash
# Trigger an error by sending invalid bash
curl -X POST http://victim:9998/voice/start
# Then trigger a bash command that fails:
```

**Expected output** (problemático):
```
{"error":"exec failed: /home/andres_fernandez/.cargo/bin/some-tool: No such file or directory"}
```

## Remediation
Reemplazar por mensaje genérico + log interno:
```rust
send_json(&mut ws_tx, &ServerMsg::Error {
    message: "Internal error".into(),  // genérico al cliente
}).await;
tracing::error!(error = %e, "internal error");  // detalle al log
```

## Regression Test
```bash
# Después del fix, el response no debe incluir paths absolutos
curl ... | jq -e '.message | test("/home/") | not'
```

## References
- CWE-209: https://cwe.mitre.org/data/definitions/209.html
```

## Ejemplo 5: Theoretical (sin PoC, no reportar como finding pero documentar)

> **NOTA**: Este NO debe ir en `findings/`. Va en una sección "out of scope" del reporte.

```markdown
## Out of scope (considerados pero descartados)

### ReDoS potencial en bash tool
El bash tool acepta cualquier comando. Si un usuario pasa un comando con regex vulnerable (ej: `rg "^(a+)+$"`), podría causar catastrophic backtracking.

**Por qué no es finding**: requiere que el usuario (con acceso legítimo) ejecute el comando. No es exploitable por un atacante externo. Documentado para awareness.
```

## Cómo usar estos ejemplos

Cuando generes un nuevo finding:
1. Mirá el ejemplo más cercano a tu finding (mismo severity o misma STRIDE)
2. Copiá la estructura
3. Rellená los campos con la data específica
4. Validá con `bash security/bin/validate-finding.sh <tu-finding>.md`
5. Si falla, corregí y re-valida

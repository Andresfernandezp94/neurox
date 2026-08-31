# Constitution — neurox ecosystem

> Principios globales que gobiernan TODOS los repos del ecosistema neurox.
> Cada sub-repo hereda estos principios. En caso de conflicto, esta constitution gana.

## Producto

- **Nombre**: neurox
- **Descripción**: Plataforma de orquestación de agentes IA, headless, Linux-first, extensible por plugins
- **Owner**: Andrés Fernández
- **Source of Truth**: Este `.sdd/` (neurox workspace raíz)

## Principios Globales

### G1. Plugin-first extensibility

El core es mínimo. Toda funcionalidad no-esencial vive en plugins que se
registran dinámicamente. Los plugins declaran sus capacidades (tools + skills)
y el daemon las propaga al agente en runtime.

### G2. Contracts over coupling

Los repos se comunican por contratos HTTP explícitos, no por dependencias de
código. Los contratos están documentados en `changes/EP-0001-daemon-mcps/specs/`
y validados por schemas JSON en cada `manifest.json`.

### G3. Each repo owns its lifecycle

Cada repo tiene su propio CI, releases, versioning, y puede deployarse
independientemente. El workspace neurox sincroniza versiones compatibles
vía git submodules.

### G4. SSD hierarchy (single source of truth)

- `neurox/.sdd/` → **ÚNICO** SOT del workspace (governance, constitución, vocabulario, contratos cross-repo, decisiones, épicas)
- Sub-repos **NO** tienen `.sdd/` propio (EP-0009: el SOT local era duplicación que generaba drift)
- Un ADR local NO puede contradecir la constitución global

### G5. Daemon-first architecture

El daemon (`neurox`) es la única fuente de verdad para estado de agentes,
sesiones, tools, y plugins. Los clientes son thin (HTTP/WS, no persisten estado).
Los plugins son procesos separados que se registran en el daemon.

### G6. Security by default

- Secrets en env vars, nunca en config files ni logs
- Bind a 127.0.0.1 por default (no expuesto a red)
- Verificación SHA256 + GPG para plugins
- Rate limiting por default en plugins
- `cargo audit` en CI de todos los repos Rust

### G7. Dynamic over static

- Tools se registran dinámicamente (no hardcodeadas)
- Skills se inyectan por plugin (no compiladas en el default_agent)
- Módulos se habilitan/deshabilitan en runtime (no rebuild)
- Config se parchea via API (no restart)

### G8. Test everything, verify always

- Todo repo Rust tiene CI con: fmt + clippy + test + build + audit
- Tests incluyen unit + integration + E2E
- Clippy con `-D warnings` (zero tolerance)
- Coverage se mide y reporta

### G9. Documentation lives in the SOT

- README.md en cada sub-repo (template fijo — solo link al SOT y al índice de docs)
- CHANGELOG.md: **solo en meta-repo**. Sub-repos no tienen (EP-0009: historial vive en `git log` + tags)
- SECURITY.md: **solo en `.sdd/security/`**. Sub-repos no tienen (disclosure policy única)
- Docs de módulo (`docs/<module>/`) viven solo en meta-repo (EP-0008)
- Contratos inter-repo documentados en `.sdd/changes/EP-NNNN/specs/`

### G12. Source of Truth > Code > Doc (jerarquía documental)

**Regla dura**: cualquier cambio (mayor o menor) debe propagar al nivel siguiente en el mismo PR.

```
SOT (.sdd/)         ← define CÓMO se trabaja
   ↓
Código (sub-repos)   ← implementa lo que el SOT dice
   ↓
Doc (docs/<module>/) ← describe lo que el código hace
```

**Consecuencias**:
- Cambio en código → actualizar `docs/<module>/` correspondiente en el mismo PR
- Cambio en API pública → actualizar `docs/<module>/api.md`
- Cambio en arquitectura → actualizar `docs/<module>/architecture.md`
- Cambio en convención (naming, prefix, status) → bump en `.sdd/INDEXING.md` (changelog) + regenerar `.sdd/DOCS-INDEX.md` si corresponde
- PR sin doc-update = PR incompleto (no auto-merge)

Ver `.sdd/methodologies/method-08-doc-update-mandatory.md` para el workflow concreto.

### G13. Naming canónico: agent_id único = `default`

El único `agent_id` válido en el workspace es **`default`** (lowercase, sin underscore, sin caracteres especiales).

**Reglas**:
- ID técnico en código, JSON, YAML, TOML: `"default"`
- Display name en UI: `"Default"` (CapitalCase, no es ID)
- Directorio template: `agents/Default/` (filesystem convention, no es ID)
- Constantes Rust: `DEFAULT_AGENT_ID = "default"`
- Endpoints HTTP: `/v1/default/...` (no `/v1/default_agent/...`)
- **NO usar**: `adan`, `eva`, `neurox_default`, `default_agent`, ni nombres propios
- **NO usar**: `MiAgente` como placeholder (era didáctico, ahora inconsistente)

**Consecuencias**:
- Cualquier agente registrado con otro ID es un error de configuración
- El lint (`bin/lint.sh` Checks 10/11/12) detecta IDs no canónicos
- Ver [EP-0009](../changes/EP-0009-purge-non-default-agent-names/) para la historia completa del rename

### G10. Linux-first, cross-platform future

Linux es el target primario. macOS/Windows son roadmap futuro con
instaladores nativos por plataforma (.pkg, .msi). No se sacrifica
la experiencia Linux por compatibilidad prematura.

### G11. Identity isolation (EP-0004)

Cada agent con `session_id` propio carga su system prompt + facts +
skills desde su propio `--identity-dir` (filesystem separado). El daemon
nunca mezcla identidades. El `tools_allowlist` por agent limita qué
tools se exponen al LLM.

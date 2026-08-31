# Buenas Prácticas de Documentación

> Cómo escribir documentación que **no se desactualice** y que **la gente realmente lea**.
>
> Versión: 2026-08-08 | Status: draft

## Principios

1. **Documentación viva**: si cambia el código, cambia la doc en el mismo commit
2. **Source of Truth (SSD)**: una sola fuente de verdad por concepto, linkear desde otros lugares
3. **Escaneable**: estructura clara con headers, tablas, listas — no párrafos largos
4. **Ejecutable**: los ejemplos deben poderse copiar y correr
5. **Bilingüe selectivo**: español narrativo + inglés técnico (código, comandos)

## Estructura estándar de un README

```markdown
# <Project Name>

One-line description (50-80 chars).

## What
<1-2 párrafos: qué es, qué problema resuelve>

## Why
<Por qué existe, qué problema resuelve>

## How
<Instalación, uso, ejemplos. Ejecutable.>

## Status
| Aspect | Status |
|--------|--------|
| Version | X.Y.Z |
| Tests | N passing |
| CI | green/red |
| SSD | ✅/⚠️/❌ |

## Links
- [SSD del repo](../INDEXING.md)
- [Entry point del workspace](../../README.md)
```

## Reglas de escritura

### Headers

- `# H1` solo una vez (el título del archivo)
- `## H2` para secciones principales
- `### H3` para subsecciones
- No más de 3 niveles de profundidad (si necesitás más, dividí el archivo)

### Listas

- **Ordenadas** (`1. 2. 3.`) para pasos
- **Sin orden** (`-`) para items
- Items cortos: máximo 1 línea
- Si el item tiene sub-texto, usar sub-bullet

### Tablas

- Solo cuando hay 3+ columnas comparables
- Headers claros y cortos
- Alineación: texto a la izquierda, números a la derecha
- No tablas anidadas

### Código

- Especificar lenguaje: ` ```rust `, ` ```bash `
- Máximo ~20 líneas inline; si es más, link a archivo
- Comandos completos con cwd si importa
- Output esperado cuando aplique

### Links

- **Relativos** dentro del mismo repo: `[method-01](./method-01-flujo-de-trabajo.md)`
- **Absolutos** para cross-repo: `[ADR-0001](../decisions/ADR-0001-multi-agent-subprocess-architecture.md)`
- Texto del link descriptivo, no "click aquí"

### Tablas de estado

| Símbolo | Significado |
|---------|-------------|
| ✅ | Done / Working / Verified |
| ⚠️ | In progress / Partial / Warning |
| ❌ | Broken / Missing / Deprecated |
| 🟢 | Healthy |
| 🟡 | Caution |
| 🔴 | Critical / Blocked |

## Anti-patterns

- ❌ README que dice "este proyecto hace cosas" — ser específico
- ❌ TOCs largos sin estructura real
- ❌ "TODO: documentar" — hacerlo o abrir issue
- ❌ Capturas de pantalla en lugar de texto
- ❌ Links rotos (verificar con `markdown-link-check`)
- ❌ ASCII art innecesario
- ❌ Documentación duplicada en 2+ lugares (DRY)
- ❌ Documentación sin autor/fecha
- ❌ Markdown generado desde código sin revisar

## Lifecycle de documentos

| Estado | Cuándo | Acción |
|--------|--------|--------|
| `draft` | Recién creado, no revisado | Iterar |
| `review` | Primera versión completa | Pedir review |
| `active` | Aprobado y útil | Mantener al día |
| `superseded` | Reemplazado por otro doc | Marcar como tal, link al nuevo |
| `archived` | Ya no aplica, pero se conserva | Mover a `.sdd/archived/` |

## Templates

Ver `~/Proyectos/neurox/repos/neuro-pro/.sdd/templates/` (en el SSoT del root):
- `proposal.md` — para nueva épica
- `design.md` — decisiones técnicas + contratos
- `tasks.md` — plan de implementación por fases
- `requirements.md` — criterios de aceptación para specs

## Convenciones de archivos

- **Nombre**: `kebab-case.md` (sin mayúsculas, sin espacios)
- **Extensión**: siempre `.md`
- **Encoding**: UTF-8 sin BOM
- **Line endings**: LF (no CRLF)
- **Trailing newline**: requerido al final
- **Max width**: 100 columnas (soft), 120 (hard)

## Estructura del SSD

```
.sdd/
├── README.md              ← estructura y workflow OpenSpec
├── INDEX.md               ← entry point del repo (qué es, status, épicas activas)
├── CONSTITUTION.md        ← Project Charter (principios inmutables)
├── GLOSSARY.md            ← vocabulario canónico
├── ecosystem-map.md       ← arquitectura y contratos cross-repo
├── testing.md             ← convenciones de testing
├── decisions/             ← ADRs (inmutables)
│   └── README.md          ← índice de ADRs
├── changes/               ← épicas en propuesta
│   ├── README.md
│   └── EP-NNNN-<nombre>/
│       ├── proposal.md
│       ├── design.md
│       ├── tasks.md
│       └── specs/
│           └── EP-NNNN-NN-<repo>/
│               └── requirements.md
├── templates/              ← plantillas para nuevos artefactos
├── scripts/close-epic.sh  ← automatiza archivar épicas
├── CHECKLIST.md           ← workflow para cerrar épica
├── follow-ups.md          ← bugs/mejoras pendientes
├── CHANGELOG → /CHANGELOG.md
└── archived/              ← épicas cerradas (inmutable)
    ├── README.md
    ├── REGISTRO.md
    └── EP-NNNN-<nombre>/
```

## Verificación de calidad

Antes de mergear un PR con cambios de docs:

- [ ] Links verificados (no rotos)
- [ ] Tablas bien formadas (renderizan en GitHub)
- [ ] Code blocks con lenguaje especificado
- [ ] Headers jerárquicos sin saltar niveles
- [ ] Sin contenido en párrafos >5 líneas (romper en listas/sub-headers)
- [ ] Sin TODOs/FIXMEs sin ticket
- [ ] Sin caracteres CJK (verificado por `no-cjk` lint)
- [ ] Sin secretos hardcoded (verificado por `no-secrets` lint)

## Referencias

- [Flujo de trabajo](./method-01-flujo-de-trabajo.md)
- [Buenas prácticas de código](./method-03-buenas-practicas-codigo.md)
- SSD del workspace: `~/Proyectos/neurox/repos/neuro-pro/.sdd/`
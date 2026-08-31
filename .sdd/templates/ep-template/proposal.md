# EP-0000 — Plantilla canónica para crear una nueva épica

> **Status**: template
> **Created**: 2026-08-15
> **Updated**: 2026-08-15
> **Owner**: @Andres
> **Note**: Esta NO es una épica real. Es la **plantilla** que sirve de guía al
> crear una nueva EP. Reemplazá el slug y los placeholders cuando la copies
> a `changes/EP-NNNN-<slug>/`. Ver "Cómo usar esta plantilla" abajo.

## Cómo usar esta plantilla

1. **NO edites este archivo** — es la referencia. Si querés mejorarla, abrí
   un cambio al template explícitamente.
2. **No crees archivos a mano** — usá `bash bin/start-epic.sh <slug-kebab>`
   desde `~/Proyectos/neurox/.sdd/`. El script crea `proposal.md`,
   `design.md`, `tasks.md` y `specs/EP-NNNN-01-<repo>/requirements.md` con
   los placeholders genéricos.
3. **Esta plantilla existe** porque los placeholders del script son
   demasiado escuetos (`[REQUIRED: 1-3 párrafos.]`) y no enseñan **qué
   escribir**. Copiá esta estructura al crear tu EP y reemplazá cada
   `{{PLACEHOLDER}}` por el contenido real.
4. **Convención snake_case** en nombres de campos JSON / eventos del
   daemon (ver `CONSTITUTION.md` y EP-0008 preamble).

---

# EP-{{NNNN}} — {{Título descriptivo}}

> **Status**: draft:proposal
> **Created**: {{YYYY-MM-DD}}
> **Updated**: {{YYYY-MM-DD}}
> **Owner**: {{@nombre}}
> **Reviewers**: [{{@user1}}, {{@user2}}]
> **Related**:
>   - {{link a otra épica relacionada — opcional}}
>   - {{ADR relacionado — opcional}}

## Problema

[REQUIRED: 1-3 párrafos. Responde:]

- **¿Qué problema resolvemos?** — describe el dolor concreto
- **¿Por qué importa?** — cuál es el costo de no resolverlo
- **¿Cuál es el trigger / contexto?** — por qué ahora

Ejemplo de buena apertura:
> El daemon serializa los mensajes de error de los plugins con
> `serde(rename_all = "snake_case")` en `daemon/core/src/events.rs:7`,
> pero la SPA los lee con keys en PascalCase (ej. `SessionStarted` en
> vez de `session_started`). Cada vez que un evento nuevo se agrega al
> daemon, la SPA rompe silenciosamente porque el `switch` exhaustivo
> falla el typecheck.

## Solución propuesta

[REQUIRED: Alto nivel. NO código todavía. Explica:]

- Cómo se resuelve el problema a nivel arquitectura
- Qué componentes cambian y en qué orden
- Qué tradeoffs se tomaron (con justificación)
- Cómo se deploya / se rota

Si tiene varios caminos posibles, listalos y marcá el recomendado.

## Alternativas consideradas

[OPTIONAL: Lista de alternativas rechazadas y por qué. Es útil porque
previene que la próxima persona re-proponga algo similar.]

## Criterios de aceptación

[REQUIRED: Lista de condiciones que marcan esta épica como done.
Cada criterio debe ser verificable objetivamente — no "el código está
limpio" sino "el endpoint POST /v1/foo retorna 200 cuando recibe { bar: 1 }".]

Ejemplo:
- [ ] Endpoint `POST /v1/foo` acepta `{ bar: string }` y retorna `200`
- [ ] El log muestra `[foo] bar=…` en formato JSON estructurado
- [ ] El test E2E `foo.test.ts` pasa en CI
- [ ] ADR-0009 publicado con la decisión de usar `serde_json::Value`

## Riesgos

[OPTIONAL: Lista de riesgos conocidos + mitigación. Mejor explícito que
descubrir el riesgo tarde.]

- **R1**: <descripción> → **Mitigación**: <cómo>
- **R2**: ...

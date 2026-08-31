# Workflow de Pull Requests

> Cómo abrir, revisar y mergear PRs en el workspace `neurox`. **Aplica a todos los repos.**
>
> Versión: 2026-08-08 | Status: draft

## Anatomía de un buen PR

### Título

Formato Conventional Commits:

```
<type>(<scope>): <short description>

<type>: feat | fix | docs | refactor | test | chore | perf
<scope>: nombre del módulo / área afectada
<short description>: imperativo presente, max 72 chars
```

**Ejemplos buenos**:
- `feat(memory): add rate limiting to embeddings endpoint`
- `fix(client): handle SSE disconnect gracefully`
- `docs(ssd): add EP-0016 to REGISTRO`
- `chore(deps): bump tokio to 1.41`

**Ejemplos malos**:
- ❌ `Update` (no dice qué)
- ❌ `fix bug` (qué bug?)
- ❌ `WIP: still working on this` (no va al main)
- ❌ `feat: add new feature.` (con punto, no imperativo)

### Descripción

Usar template:

```markdown
## What

<1-3 oraciones: qué cambia este PR>

## Why

<Por qué: qué problema resuelve, qué necesidad satisface>

## Changes

- <bullet 1>
- <bullet 2>
- <bullet 3>

## Tests

- [ ] <test 1>
- [ ] <test 2>

## Verification

<Cómo verificar manualmente>

## Linked épica

- [EP-NNNN — título](../archived/EP-NNNN-...) (si aplica)
```

### Tamaño

- **Ideal**: < 400 líneas de diff
- **Máximo aceptable**: 800 líneas
- **Más de 800**: dividir en PRs más pequeños (a menos que sea refactor mecánico)

## Antes de abrir el PR

```bash
# 1. Lint pasa
bash scripts/lint.sh

# 2. Tests pasan
cargo test --workspace --exclude neuro-pro-gtk-overlay
# o
pnpm exec vitest --run

# 3. Build limpio
cargo build --release
# o
pnpm build

# 4. Diff contra main
git diff main...HEAD --stat
# (verificar que el diff sea razonable)

# 5. Self-review
git diff main...HEAD
# (leer tu propio código como si fueras el reviewer)

# 6. Branch actualizado
git fetch origin
git rebase origin/main
```

## Abrir el PR

```bash
git push -u origin <branch-name>

gh pr create \
  --base main \
  --head <branch-name> \
  --title "<conventional commit title>" \
  --body "$(cat <<'EOF'
<template del body>
EOF
)"
```

O desde la web: https://github.com/<org>/<repo>/compare/main...<branch>

## Review

### Para el autor

- **Asignar reviewers**: 1 mínimo, 2 preferentemente para cambios grandes
- **Asignar labels**: `bug`, `enhancement`, `docs`, `breaking-change`
- **Linked issue**: si hay issue, referenciarlo en el body
- **Screenshots**: si hay cambios visuales

### Para el reviewer

- **Responder en 24h** (ideal: 4h si no es grande)
- **Comentar específico**: por línea si es posible
- **Marcar blocking vs nit**: usar prefix `[blocking]` o `[nit]`
- **Aprobar solo cuando está listo**: no por cortesía
- **Pedir cambios no es failure**: es parte del proceso

### Qué buscar al revisar

1. **Correctness**: ¿hace lo que dice el PR?
2. **Tests**: ¿hay tests para los cambios? ¿cubren edge cases?
3. **Naming**: ¿los nombres son claros?
4. **DRY sin over-engineering**: ¿hay duplicación obvia? ¿hay abstracción prematura?
5. **Errores**: ¿se manejan correctamente? ¿hay silent fallbacks?
6. **Seguridad**: ¿hay secrets hardcoded? ¿validación de input?
7. **Performance**: ¿hay loops O(n²)? ¿queries N+1?
8. **Documentación**: ¿el SSD/CHANGELOG se actualizó?

## Merge

### Cuándo mergear

- ✅ CI pasa (todos los checks)
- ✅ Al menos 1 aprobación (2 para breaking changes)
- ✅ Sin comentarios `[blocking]` sin resolver
- ✅ Branch actualizado con main (no conflictos)
- ✅ Conversación resuelta (todos los threads)

### Cómo mergear

**Squash and merge** (default):
- Mantiene main con un commit por feature
- El PR description se convierte en el commit message
- El branch se borra automáticamente

**Rebase and merge** (para PRs con commits lógicos):
- Mantiene los commits individuales
- Útil cuando los commits tienen valor histórico

**NO usar**: regular merge (genera merge commits que ensucian el history)

### Después de mergear

```bash
# Volver a main
git switch main
git pull --rebase

# Borrar branch local
git branch -d <branch-name>

# Verificar working tree limpio
git status
```

## PRs bloqueados / Stuck

Si un PR está bloqueado > 3 días:
1. Ping al reviewer
2. Si sigue bloqueado, pingar a otro reviewer
3. Si hay conflicto técnico, abrir un ADR o issue
4. Si el PR es muy grande, dividirlo

## Anti-patterns

- ❌ PRs sin descripción ("fix bug" no cuenta)
- ❌ PRs con 50+ archivos cambiados sin justificación
- ❌ Self-merge sin review
- ❌ Force-push a un branch que está siendo revisado
- ❌ Mezclar cambios de formato con cambios funcionales
- ❌ Mergear con checks pendientes (a menos que sean triviales y conocidos)
- ❌ PRs con commits de "fix typo" o "address review" — squash los en local antes de pushear

## Referencias

- [Flujo de trabajo](./method-01-flujo-de-trabajo.md)
- [Checklist de release](./method-05-checklist-release.md)
- [Convenciones de git](./method-06-convenciones-git.md)
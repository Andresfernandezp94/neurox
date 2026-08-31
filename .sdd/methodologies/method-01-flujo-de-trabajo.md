# Flujo de Trabajo en `neurox`

> Metodología operativa. **Cómo trabajar día a día** en cualquier repo del workspace.
>
> Versión: 2026-08-08 | Status: draft

## Princípios rectores

1. **SSD es ley**: la documentación en `.sdd/` (Source of Truth) gana sobre el código y sobre la conversación.
2. **Multi-tenancy**: cada repo es independiente pero debe mantener su `.sdd/` propio y respetar el del entry point.
3. **Idempotencia**: las herramientas y scripts deben poderse correr múltiples veces sin efectos colaterales.
4. **Cascade de constraints**: el `CONSTITUTION.md` raíz gana → `CONSTITUTION.md` local del repo → código.

## Workflow diario

### 1. Elegir el repo correcto

```bash
# Entry point del workspace
cat ~/Proyectos/neurox/repos/neuro-pro/.sdd/INDEX.md

# Ver estado por repo (sin tocar nada)
cd ~/Proyectos/neurox/repos/<repo> && git status && git log --oneline -5
```

### 2. Identificar el SSD del repo

| Repo | `.sdd/` | Ubicación |
|------|---------|-----------|
| `neuro-pro` | ✅ completo | `repos/neuro-pro/.sdd/` (entry point) |
| `neuro-pro-plugin-memory` | ✅ INDEX | `repos/neuro-pro-plugin-memory/.sdd/` |
| `neuro-pro-plugin-tui` | ✅ completo | `repos/neuro-pro-plugin-tui/.sdd/` (working tree) |
| `neuro-pro-plugin-voice` | ✅ INDEX | `repos/neuro-pro-plugin-voice/.sdd/` |
| `neuro-pro-plugin-cli` | ✅ INDEX | `repos/neuro-pro-plugin-cli/.sdd/` |
| `neuro-pro-plugin-gui` | ✅ INDEX | `repos/neuro-pro-plugin-gui/.sdd/` |
| `neuro-pro-registry` | ✅ INDEX | `repos/neuro-pro-registry/.sdd/` |

### 3. Crear branch para trabajar

```bash
git switch -c feat/<descripcion-corta>
# o
git switch -c fix/<issue-id>-<descripcion>
# o
git switch -c docs/<descripcion>
```

**Prefijos semánticos** (Conventional Commits style):
- `feat/` — nueva feature
- `fix/` — bug fix
- `docs/` — solo documentación
- `refactor/` — refactor sin cambio funcional
- `test/` — solo tests
- `chore/` — mantenimiento (deps, configs)

### 4. Hacer cambios siguiendo los principios del SSD

Antes de tocar código, **leer el `CONSTITUTION.md` del repo y la propuesta de la épica** (si existe).

### 5. Antes de commitear

```bash
# 1. Ver lo que cambió
git status
git diff

# 2. Lint (si el repo tiene scripts/lint.sh)
bash scripts/lint.sh

# 3. Tests
cargo test --workspace --exclude neuro-pro-gtk-overlay
# o
pnpm exec vitest --run

# 4. Build
cargo build --release
# o
pnpm build
```

### 6. Commit con Conventional Commits

```bash
git add <files>
git commit -m "<type>(<scope>): <short description>

<longer description if needed>

Refs: <issue/PR/archivo>
"
```

**Ejemplos válidos**:
```
feat(memory): add rate limiting to embeddings endpoint
fix(client): handle SSE disconnect gracefully
docs(ssd): add EP-0016 to REGISTRO
chore(deps): bump tokio to 1.41
```

### 7. Push y abrir PR

```bash
git push -u origin <branch-name>
gh pr create --base main --head <branch-name> --title "..." --body "..."
```

### 8. Review y merge

- **Reviewer**: al menos 1 aprobación (preferentemente 2 para cambios grandes)
- **CI**: todos los checks deben pasar
- **Merge**: squash merge (mantiene main limpio)
- **Borrar branch** después de merge

## Reglas del workspace

### Numeración de épicas

- **Continua, correlativa, sin gaps**
- Verificar último número en `~/Proyectos/neurox/repos/neuro-pro/.sdd/archived/REGISTRO.md`
- Formato: `EP-NNNN-<nombre-corto-kebab-case>`
- Próxima libre: ver último entry

### Naming de branches

- Máximo 60 caracteres
- Solo kebab-case
- Sin tildes, sin caracteres especiales
- Sin prefijos redundantes (`feat-feat-` mal)

### Mensajes de commit

- Línea subject: máximo 72 caracteres
- Imperativo presente ("add" no "added")
- Sin punto final
- Body opcional: wrap a 72 columnas
- Referenciar issue/PR/archivo cuando aplique

### Tags

- Formato: `vX.Y.Z` (semver)
- Solo en main después de merge
- Pre-release: `vX.Y.Z-rc.N`
- Build artifacts: tag → release → binarios

## Estados válidos del workspace

| Estado | Significado | Acción |
|--------|-------------|--------|
| `clean` | Sin cambios sin commitear | OK para trabajar |
| `modified` | Cambios sin stagear | `git add` o `git restore` |
| `ahead` | Commits sin pushear | `git push` |
| `behind` | Commits en remote no mergeados | `git pull --rebase` |
| `diverged` | Both ahead y behind | `git pull --rebase` o merge |

## Comandos útiles

```bash
# Ver estado agregado de todos los repos
for r in ~/Proyectos/neurox/repos/*/; do
  echo "=== $(basename $r) ==="
  git -C "$r" status --short --branch
done

# Sync todos los repos con su remote
for r in ~/Proyectos/neurox/repos/*/; do
  git -C "$r" fetch origin 2>/dev/null
  echo "$(basename $r): $(git -C "$r" rev-list --count HEAD..@{u} 2>/dev/null || echo 'no upstream') ahead, $(git -C "$r" rev-list --count @{u}.. 2>/dev/null || echo 'no upstream') behind"
done
```

## Anti-patterns

- ❌ Commitear sin tests (excepto `docs/` o `chore/`)
- ❌ Mensajes vagos ("fix bug", "update", "wip")
- ❌ Mezclar cambios no relacionados en un commit
- ❌ Mergear sin review aprobado
- ❌ Force-push a main (nunca)
- ❌ Force-push a un branch compartido sin coordinar
- ❌ Crear archivos `.md` sin referenciarlos en el SSD

## Referencias

- [Buenas prácticas de documentación](./method-02-buenas-practicas-documentacion.md)
- [Buenas prácticas de código](./method-03-buenas-practicas-codigo.md)
- [Workflow de PRs](./method-04-workflow-prs.md)
- [Checklist de release](./method-05-checklist-release.md)
- [Convenciones de git](./method-06-convenciones-git.md)
- SSD del entry point: `~/Proyectos/neurox/repos/neuro-pro/.sdd/INDEX.md`
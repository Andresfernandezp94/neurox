# Convenciones de Git

> Reglas para usar git en el workspace `neurox`. Aplican a todos los submodules.
>
> Versión: 2026-08-08 | Status: draft

## Configuración global

```bash
# Identidad (una sola vez por máquina)
git config --global user.name "Andrés Fernández"
git config --global user.email "andres.fernandez@sixbell.com"

# Editor
git config --global core.editor "vim"  # o nano, code, etc

# Push solo de la rama actual (evita push accidental de todo)
git config --global push.default simple

# Default branch
git config --global init.defaultBranch main

# Rebase por defecto al hacer pull
git config --global pull.rebase true

# Autocorrect
git config --global help.autocorrect 10

# Diff con colores
git config --global color.ui auto
```

## Workflow

### Branches

- **main**: branch protegido, requiere PR
- **feat/<descripcion>**: nueva feature
- **fix/<issue-id>-<descripcion>**: bug fix
- **docs/<descripcion>**: solo docs
- **chore/<descripcion>**: mantenimiento
- **refactor/<descripcion>**: refactor sin cambio funcional
- **test/<descripcion>**: solo tests

**Reglas**:
- Sin mayúsculas, sin tildes, sin caracteres especiales
- kebab-case
- Máximo 60 caracteres
- Sin prefijos redundantes (`feat-feat-` mal)

### Commits

Ver [Buenas prácticas de código](./method-03-buenas-practicas-codigo.md#commits).

Formato:
```
<type>(<scope>): <subject>

<body>

<footer>
```

**Type** (Conventional Commits):
- `feat`: nueva feature
- `fix`: bug fix
- `docs`: solo docs
- `style`: formato sin cambio lógico
- `refactor`: refactor
- `test`: tests
- `chore`: mantenimiento
- `perf`: performance

**Scope**: módulo o área afectada (opcional pero recomendado).

**Subject**:
- Imperativo presente ("add" no "added")
- Minúscula
- Sin punto final
- Max 72 chars

**Body** (opcional):
- Wrap a 72 cols
- Explica el "qué" y el "por qué"
- Separa del subject con línea en blanco

**Footer** (opcional):
- `Refs: #123` para issues
- `BREAKING CHANGE: <description>` para breaking changes
- `Co-authored-by: Name <email>` para pair programming

### Tags

- Formato: `vX.Y.Z` (semver)
- Annotated tags: `git tag -a vX.Y.Z -m "message"`
- Solo en main después de merge
- No mover ni borrar después de publicar (rompe consumidores)

### Remotes

- `origin` → GitHub
- `upstream` → original (solo para forks)
- `local` → opcional, para branches de experimentación

## Comandos comunes

### Ver estado

```bash
# Estado del working tree
git status

# Diff no stageado
git diff

# Diff stageado
git diff --cached

# Diff contra main
git diff main...HEAD

# Ver log
git log --oneline -20
git log --oneline --graph --decorate --all -20  # visual

# Buscar en el log
git log --grep="pattern" --oneline
git log -S "string" --oneline  # pickaxe
```

### Trabajar con branches

```bash
# Crear y switchear
git switch -c feat/nueva-feature

# Switchear a branch existente
git switch main

# Borrar branch local
git branch -d feat/vieja

# Borrar branch remote
git push origin --delete feat/vieja

# Listar branches
git branch -a
```

### Trabajar con cambios

```bash
# Staghear
git add <file>
git add .  # todo

# Commitear
git commit -m "feat: add X"

# Modificar último commit (solo si NO está pusheado)
git commit --amend
git commit --amend --no-edit  # mantener mensaje

# Soft reset (mantiene cambios staged)
git reset --soft HEAD~1

# Hard reset (DESTRUCTIVO — solo local, no pusheado)
git reset --hard HEAD~1
```

### Sincronizar

```bash
# Fetch + rebase
git fetch origin
git rebase origin/main

# O todo junto
git pull --rebase

# Push
git push
git push -u origin <branch>  # primera vez
git push --force-with-lease  # después de rebase
```

### Stash

```bash
# Guardar cambios sin commitear
git stash push -m "wip"

# Ver stashes
git stash list

# Aplicar último stash
git stash pop

# Aplicar stash específico
git stash apply stash@{2}

# Borrar stash
git stash drop stash@{0}
```

### Inspeccionar

```bash
# Ver un commit
git show <sha>

# Ver archivo en un commit específico
git show <sha>:<file>

# Buscar en el código
git grep "pattern"

# Quién modificó una línea
git blame <file>
```

## Submodules

Este workspace usa git submodules (`neurox` → cada repo).

### Clonar

```bash
# Clonar con submodules
git clone --recursive git@github.com:Andresfernandezp94/neurox.git

# O si ya clonaste sin --recursive
git submodule update --init --recursive
```

### Trabajar en un submodule

```bash
# Entrar al submodule
cd repos/neuro-pro

# Cambios dentro del submodule se commitean NORMALMENTE
git switch -c feat/x
# ... hacer cambios ...
git commit -m "..."
git push

# Volver al root y sincronizar el submodule
cd ../..
git add repos/neuro-pro  # marca el nuevo commit
git commit -m "chore(submodule): bump neuro-pro to v0.X.Y"
```

### Peligros

⚠️ **`git reset --hard` desde el root puede borrar trabajo del submodule**:
- El `reset --hard` borra los cambios en el working tree del root
- Los submodules tienen su propio git state
- Si haces reset --hard en el root sin hacer commit de los cambios del submodule, **se pierden**

**Nota**: la regla "nunca `git reset --hard` desde una rama incompleta
de submodule" está documentada narrativamente en la memoria del agente
EVA (`~/.agents/EVA/memoria/learning.md`), que es filesystem del agente,
no del repo. No se referencia como link desde el SOT porque ese path
no es estable cross-agent.

## Anti-patterns

- ❌ **Force-push a main** (rompe historial para todos)
- ❌ **Force-push a un branch compartido** sin coordinar
- ❌ **`git reset --hard` en código sin commitear** (se pierde)
- ❌ **Commits con archivos grandes** (> 1 MB) sin Git LFS
- ❌ **Commits con secrets** hardcoded (usar env vars)
- ❌ **Commits con caracteres CJK** (revisar con `no-cjk` lint)
- ❌ **Mezclar cambios no relacionados** en un commit
- ❌ **Mensajes vagos** ("fix", "update", "wip")
- ❌ **Force-push sin `--force-with-lease`** (puede pisar trabajo de otros)

## Hooks (opcional)

Para reforzar convenciones localmente, configurar git hooks en `~/.gitconfig`:

```ini
[core]
    hooksPath = ~/.gitconfig/hooks
```

Y crear `~/.gitconfig/hooks/pre-commit`:

```bash
#!/usr/bin/env bash
# Pre-commit hook: corre el linter antes de commitear

REPO_ROOT="$(git rev-parse --show-toplevel)"

# Si hay scripts/lint.sh, correrlo
if [ -f "$REPO_ROOT/scripts/lint.sh" ]; then
    bash "$REPO_ROOT/scripts/lint.sh" || {
        echo "Lint failed. Commit aborted."
        exit 1
    }
fi

# Verificar CJK en archivos stageados
if git diff --cached --name-only | xargs rg -lP '[一-鿿　-〿぀-ゟ゠-ヿ가-힯]' 2>/dev/null; then
    echo "CJK characters found in staged files. Commit aborted."
    exit 1
fi
```

## Referencias

- [Flujo de trabajo](./method-01-flujo-de-trabajo.md)
- [Buenas prácticas de código](./method-03-buenas-practicas-codigo.md)
- [Workflow de PRs](./method-04-workflow-prs.md)
- [Checklist de release](./method-05-checklist-release.md)
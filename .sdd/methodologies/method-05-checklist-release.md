# Checklist de Release

> Pasos a verificar antes de taggear una versión y publicar un release.
>
> Versión: 2026-08-08 | Status: draft

## Pre-release (T-1 día)

### Código

- [ ] Todos los PRs planeados mergeados a main
- [ ] Branch actualizado con todos los cambios
- [ ] `bash scripts/lint.sh --strict` pasa (0 errors, 0 warnings)
- [ ] `cargo test --workspace --exclude neuro-pro-gtk-overlay --all-targets` pasa
- [ ] `cargo build --release` sin warnings
- [ ] Versión sincronizada en `Cargo.toml` + `manifest.json` (para plugins)
- [ ] No hay `TODO`/`FIXME` críticos en código de producción

### Documentación

- [ ] `CHANGELOG.md` actualizado con la nueva versión
- [ ] `README.md` actualizado (si hubo cambios de uso)
- [ ] `.sdd/INDEX.md` actualizado (status, versión, repo status)
- [ ] `.sdd/archived/REGISTRO.md` actualizado si se cerró alguna épica
- [ ] `.sdd/changes/` épica movida a `archived/` con `bash scripts/close-epic.sh`
- [ ] No hay referencias rotas a archivos movidos

### Tests manuales

- [ ] Smoke test del binario principal: `./target/release/<bin> --version`
- [ ] Smoke test del endpoint principal: `curl http://127.0.0.1:<port>/health`
- [ ] Si hay CLI: probar los subcommands principales
- [ ] Si hay GUI: navegar a las páginas principales y verificar render
- [ ] Si hay plugin: verificar que se registra en el daemon (`GET /v1/plugins`)

### Seguridad

- [ ] No hay secrets hardcodeados (lint `no-secrets` pasa)
- [ ] No hay caracteres CJK en código (lint `no-cjk` pasa)
- [ ] Versiones de deps sin CVEs críticos (`cargo audit` y/o `pnpm audit`)
- [ ] Si hay breaking change, marcado en CHANGELOG con `**BREAKING**`

## Release (T-0)

### Build

```bash
# 1. Build release
cargo build --release

# 2. Generar tarball
VERSION="0.X.Y"
BIN_NAME="neuro-pro"  # o el nombre del binario
tar czf "${BIN_NAME}-${VERSION}-x86_64-linux.tar.gz" \
    -C target/release "${BIN_NAME}"

# 3. SHA256
sha256sum "${BIN_NAME}-${VERSION}-x86_64-linux.tar.gz" > \
    "${BIN_NAME}-${VERSION}-x86_64-linux.tar.gz.sha256"
cat "${BIN_NAME}-${VERSION}-x86_64-linux.tar.gz.sha256"
```

### Tag

```bash
# 1. Tag
git tag -a "v${VERSION}" -m "Release v${VERSION}

<summary of changes>

See CHANGELOG.md for full list."

# 2. Push tag
git push origin "v${VERSION}"
```

### Release en GitHub

```bash
gh release create "v${VERSION}" \
    --title "v${VERSION} — <title>" \
    --notes-file RELEASE_NOTES.md \
    --target main \
    "${BIN_NAME}-${VERSION}-x86_64-linux.tar.gz" \
    "${BIN_NAME}-${VERSION}-x86_64-linux.tar.gz.sha256"
```

O desde la web: https://github.com/<org>/<repo>/releases/new?tag=v<VERSION>

### Para plugins: actualizar registry

Si es un plugin (`neuro-pro-plugin-*`):

1. Editar `~/Proyectos/neurox/repos/neuro-pro-registry/registry.json`
2. Agregar nueva entrada con:
   ```json
   "<plugin>": {
     "repo": "Andresfernandezp94/<plugin>",
     "version": "0.X.Y",
     "sha256": "<hash from step above>"
   }
   ```
3. PR al registry
4. Después de merge, tag del registry mismo

## Post-release (T+1)

- [ ] Verificar que el release es visible en GitHub
- [ ] Verificar que la CI del tag pasó (release workflow)
- [ ] Verificar que el binario descargable es el correcto
- [ ] Si es plugin: verificar que `neuro-pro install <plugin>` descarga la nueva versión
- [ ] Anunciar en canal interno (Slack, etc) si aplica
- [ ] Cerrar el milestone en GitHub si aplica

## Rollback (si algo sale mal)

```bash
# Borrar release en GitHub
gh release delete "v${VERSION}" --yes

# Borrar tag local y remote
git tag -d "v${VERSION}"
git push origin --delete "v${VERSION}"

# Revertir merge en main (si aplica)
git revert -m 1 <merge-commit-sha>
git push origin main

# Re-publicar versión anterior como latest
```

## Plantilla de RELEASE_NOTES.md

```markdown
# v0.X.Y — <title>

<1-2 sentence summary>

## What's New

- <feature 1>
- <feature 2>

## Bug Fixes

- <fix 1>
- <fix 2>

## Breaking Changes

- <breaking change 1>

## Upgrade Notes

<steps to upgrade from previous version>

## Full Changelog

See `git log` for the complete list of changes.
```

## Versiones semánticas

Seguir [SemVer](https://semver.org/):
- **MAJOR** (X.0.0): breaking changes
- **MINOR** (0.X.0): nueva feature, backward-compatible
- **PATCH** (0.0.X): bug fixes, backward-compatible

**Pre-release**: `0.X.Y-rc.N` para release candidates, `0.X.Y-beta.N` para betas.

## Anti-patterns

- ❌ Tag sin CHANGELOG entry
- ❌ Release sin binario attached
- ❌ SHA256 incorrecto o faltante
- ❌ Tag movido o borrado después de publicar (rompe consumidores)
- ❌ Mezclar breaking changes en patch release
- ❌ Release sin testing manual previo

## Referencias

- [Flujo de trabajo](./method-01-flujo-de-trabajo.md)
- [Workflow de PRs](./method-04-workflow-prs.md)
- [Convenciones de git](./method-06-convenciones-git.md)
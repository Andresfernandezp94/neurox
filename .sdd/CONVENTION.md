# SSoT — Convention: `@/` path reference

## What `@/` means

`@/` in any markdown file in this SSD is a **convention** that
resolves to:

> The root of the workspace that contains this `.sdd/`.

In this repository, the workspace root is `~/Proyectos/neurox/`,
so `@/.sdd/archived/REGISTRO.md` refers to
`~/Proyectos/neurox/.sdd/archived/REGISTRO.md`.

## Why use it

- **Portable**: the path is the same on every machine, regardless of
  where the user clones the workspace or what their home directory is.
- **Readable**: `@/.sdd/...` is easier to scan than
  `../../../../.sdd/...` (the relative path the symlinks themselves
  need) or `~/Proyectos/neurox/.sdd/...` (the absolute path).
- **Tool-agnostic**: any renderer (GitHub, IDE, docs site) can replace
  `@/` with the workspace root, or just leave it as a stable symbol.

## Where it applies

- All `.md` files inside `.sdd/` (this workspace root).
- All `.md` files inside `<sub-repo>/.sdd/` (the repo's governance
  docs reference `@/...` for cross-repo paths to the workspace root).

The symlinks **do not** use `@/` — they must contain a real path the
kernel can resolve. Symlinks keep `../../../../.sdd/...` so they
work regardless of which user or which directory layout the repo is
checked out under.

## Mapping

| `@/` reference | Real path |
|---|---|
| `@/.sdd/INDEX.md` | `~/Proyectos/neurox/.sdd/INDEX.md` |
| `@/.sdd/archived/REGISTRO.md` | `~/Proyectos/neurox/.sdd/archived/REGISTRO.md` |
| `@/daemon/.sdd/INDEX.md` | `~/Proyectos/neurox/daemon/.sdd/INDEX.md` |
| `@/daemon/core/src/lib.rs` | `~/Proyectos/neurox/daemon/core/src/lib.rs` |
| `@/client/web/src/App.tsx` | `~/Proyectos/neurox/client/web/src/App.tsx` |
| `@/mcps/memory/memoryd/src/main.rs` | `~/Proyectos/neurox/mcps/memory/memoryd/src/main.rs` |

## Editor support

Most editors have a "go to file" command that accepts paths
relative to the workspace root (e.g. `Cmd-P` in VSCode with the
`@` prefix). If your editor doesn't, the mapping table above is
authoritative.

## Note on the legacy `repos/` layout

Older commits referenced `@/repos/<name>/...`. That layout was
abandoned — sub-repos now live at `@/<sub-repo>/` directly (e.g.
`@/daemon/`, `@/mcps/voice/`). Anything still pointing at `repos/`
is stale and should be updated.

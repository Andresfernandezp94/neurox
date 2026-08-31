# Contribuir a neurox

## Estructura del monorepo

```
neurox/
├── daemon/       ← núcleo Rust (binario `neurox` + `agent` + tools-engine)
├── client/web/   ← cliente web (React + Vite + TS)
├── agents/       ← plantillas de agentes (JSON + Markdown)
├── docs/         ← documentación de producto
└── .sdd/         ← Source of Truth: gobernanza, ADRs, metodología
```

## Metodología

El proyecto usa **Spec-Driven Development** (`.sdd/`). Antes de trabajar:

1. Leé [`.sdd/README.md`](./.sdd/README.md) (entry point).
2. Revisá el [glosario canónico](./.sdd/GLOSSARY.md) — usá esos términos.
3. Para features nuevas, seguí el flujo de épicas (`.sdd/bin/start-epic.sh`).

## Desarrollo del daemon (Rust)

```bash
cd daemon
cargo build                 # debug
cargo build --release       # release
cargo test                  # tests
cargo clippy                # lint (config en clippy.toml)
cargo fmt                    # formato (config en rustfmt.toml)
```

Al cambiar el binario `agent`, reinstalá siguiendo el procedimiento de
[`docs/architecture.md`](./docs/architecture.md#9-operación) (parar el daemon,
matar subprocesos `agent`, copiar, reiniciar).

## Desarrollo del cliente web (React)

```bash
cd client/web
npm install
npm run dev          # dev server con HMR (http://localhost:5173)
npm run build        # tsc --noEmit + vite build
npm test             # vitest
```

## Convenciones

- **Rust**: seguí `rustfmt.toml` y `clippy.toml`. Sin `unwrap()` en paths de
  producción; propagá errores con `?` / `anyhow`.
- **TypeScript**: sin `any` gratuito; tipá las respuestas de la API. No uses
  `!important` en CSS — usá el design system atómico.
- **Modelos LLM**: nunca los hardcodees; obtenelos del daemon por proveedor.
- **Secretos**: jamás commitees API keys. Viven en `~/.config/neurox/env`.

## Seguridad

- No introduzcas dependencias de nombre sospechoso (typosquatting).
- No agregues endpoints que expongan el daemon fuera de `127.0.0.1` sin
  discutirlo antes.
- Reportá vulnerabilidades siguiendo `.sdd/security/`.

## Commits y PRs

- Commits pequeños y descriptivos.
- No pushees directo a `main` sin revisión, salvo acuerdo explícito.
- Describí en el PR: qué cambia, qué se probó, y qué quedó pendiente.

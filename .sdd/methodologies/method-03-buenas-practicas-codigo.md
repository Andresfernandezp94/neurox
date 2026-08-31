# Buenas Prácticas de Código

> Reglas y patrones que aplican a todo código del workspace. **Stack-agnostic** en la mayoría, con secciones específicas para Rust y TypeScript.
>
> Versión: 2026-08-08 | Status: draft

## Principios universales

1. **Readability counts more than cleverness**: el código se lee 10x más de lo que se escribe
2. **Explicit is better than implicit**: comportamiento visible en el código, no oculto en magia
3. **Small functions, single responsibility**: cada función hace una cosa
4. **Types are documentation**: el compilador enforce contratos
5. **Fail fast**: errores al inicio, no al final
6. **DRY (con moderación)**: duplicación es mejor que acoplamiento malo
7. **YAGNI**: no agregues features que no necesitas hoy

## Naming

### Variables

- **Nombres descriptivos**: `user_count` mejor que `n`
- **Sin abreviaciones crípticas**: `request_id` no `req_id`
- **Sin prefijos redundantes**: `name` no `str_name` (en Python), `count` no `iCount` (en Java)
- **Booleanos con prefijo**: `is_active`, `has_children`, `should_retry`

### Funciones

- **Verbos**: `fetch_user`, `parse_response`, `validate_input`
- **Sin prefijos redundantes**: `delete()` no `delete_record()` (en Rust)
- **Mismo nivel de abstracción**: una función no mezcla "fetch" + "validate" + "save"

### Tipos y clases

- **Sustantivos**: `User`, `Request`, `Session` (no `UserManager`, `RequestHandler`)
- **Sin sufijos redundantes**: en Rust `User` no `UserStruct`
- **Singular vs plural**: `Vec<User>` no `Users`

### Constantes

- **SCREAMING_SNAKE_CASE** (Rust) o `UPPER_SNAKE` (TS)
- **Agrupadas por dominio**: `MAX_RETRIES`, `DEFAULT_TIMEOUT`
- **Sin prefijos redundantes**: en Rust, no es necesario `const PI: f64` → `const PI_F64: f64`

## Funciones

### Tamaño

- **Ideal**: 5-30 líneas
- **Máximo**: 50 líneas (refactorizar si excede)
- **Excepción justificada**: parsers, funciones matemáticas

### Parámetros

- **Máximo 4-5 parámetros** (usar struct si más)
- **Orden**: required primero, opcionales después
- **Sin booleanos de control**: en vez de `process(data, true, false)`, usar `process_verbose(data)` o struct con flags

### Returns

- **Un solo return type** (no tuplas con significados implícitos)
- **Errores como Result**, no panic
- **Documentar comportamiento de error** en rustdoc/jsdoc

## Errores

### Rust

```rust
// ❌ Mal
fn fetch_user(id: u32) -> User {
    // ...
    user.unwrap()  // panic en producción
}

// ✅ Bien
fn fetch_user(id: u32) -> Result<User, FetchError> {
    // ...
    user.ok_or(FetchError::NotFound(id))
}
```

```rust
// ❌ Mal
fn parse(input: &str) -> i32 {
    input.parse().unwrap_or(0)  // silenciar el error
}

// ✅ Bien
fn parse(input: &str) -> Result<i32, ParseIntError> {
    input.parse()
}
```

### TypeScript

```typescript
// ❌ Mal
function fetchUser(id: number): User {
    // ...
    return data;  // what if data is undefined?
}

// ✅ Bien
function fetchUser(id: number): Promise<User> {
    return fetch(`/api/users/${id}`)
        .then(r => r.json())
        .catch(e => { throw new FetchError(e) });
}
```

## Comentarios

### Cuándo comentar

- **"Por qué"**, no "qué" (el código ya dice qué)
- **Workarounds** (con link al issue)
- **Decisiones no obvias** (por qué X y no Y)
- **TODOs con ticket**: `// TODO(#123): improve this`

### Cuándo NO comentar

- ❌ Código autoexplicativo: `i += 1  // increment i`
- ❌ Nombres descriptivos: `let count = 0  // counter`
- ❌ Código que cambió: comentarios outdated son peor que nada

### Formato

```rust
// Single-line comment in Rust uses //
/// Doc comment for public items
/// Goes on triple-slash lines
/// Markdown supported

// Block comments for inline
/*
   Use for temporary notes or complex explanations
*/
```

```typescript
// Single-line
// Multi-line uses multiple // lines

/**
 * JSDoc for public APIs
 * @param id - user identifier
 * @returns the user object
 */
```

## Testing

Ver `CONVENTION.md` y los README de cada módulo en `docs/<module>/`. Las
convenciones de testing se mantienen en cada módulo; no hay un archivo
central de testing cross-repo (todavía).

Quick rules:
- **Unit tests**: funciones puras, helpers
- **Integration**: endpoints, WS handlers
- **Naming**: `it("does X when Y")`
- **Coverage target**: 70%+ para código de negocio, 90%+ para parsers

## Anti-patterns universales

- ❌ **Magic numbers**: usar constantes con nombres
- ❌ **Deep nesting**: refactorizar con early return
- ❌ **God objects**: clases que hacen todo
- ❌ **Copy-paste sin abstracción**: si copias 2 veces, abstraé; si 3, es obligatorio
- ❌ **Premature optimization**: medir antes de optimizar
- ❌ **Cargo culting**: hacer cosas porque "se hacen", no porque entiendas por qué

## Específico de Rust

Ver la constitución del workspace en `CONSTITUTION.md` y el README del
módulo daemon en `docs/daemon/README.md`. Las convenciones específicas
de Rust viven en `daemon/clippy.toml` y `daemon/rustfmt.toml`.

## Específico de TypeScript / React

- **Functional components** + hooks (no class components)
- **Props explícitas** con interface o type
- **No `any`**: usar `unknown` y validar
- **Strict mode**: `"strict": true` en tsconfig
- **ESLint + Prettier** siempre activos

## Linters

Todo repo debe tener un linter local. Ver `scripts/lint.sh` en cada repo para los lints específicos:

| Lint | Severidad | Aplica a |
|------|-----------|----------|
| no-cjk | error | todo |
| no-secrets | error | todo |
| version-drift | error | plugins (manifest vs Cargo) |
| ssd-index-present | error | todo repo del workspace |
| no-todo-unlinked | warning | código + markdown |
| no-unwrapped | warning | Rust production code |
| bin-run-executable | error | plugins con bin/run |
| readme-stale | warning | todo |

## Referencias

- [Flujo de trabajo](./method-01-flujo-de-trabajo.md)
- [Buenas prácticas de documentación](./method-02-buenas-practicas-documentacion.md)
- [Workflow de PRs](./method-04-workflow-prs.md)
- Constitución del workspace: [`CONSTITUTION.md`](../CONSTITUTION.md)
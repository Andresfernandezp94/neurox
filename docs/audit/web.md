# web_fetch + web_search — Auditoría

**Endpoint:** http://127.0.0.1:7878 (v0.4.0)
**Código:** `daemon/tools-engine/src/tools/read/{web_fetch,web_search}.rs`

---

## Estado final — web_fetch

| # | Hallazgo | Severidad | Estado |
|---|---|---|---|
| W1 | **BINARIO DUMPED** — PNG, gzip, PDF retornados como "text" con bytes NUL | CRÍTICO | ✅ **FIJADO** (NUL detection + content-type check) |
| W2 | **SSRF** — `http://127.0.0.1:7878/health` accesible (loopback) | CRÍTICO | ✅ **FIJADO** (SSRF guard con DNS resolution) |
| W2-redir | **SSRF via redirect** — `?url=...` hacia loopback | CRÍTICO | ✅ **FIJADO** (redirect policy) |
| W3 | 404 returns `ok:true` con result vacío (engañoso) | ALTO | ✅ **FIJADO** (error en 4xx/5xx) |
| W4 | `file://` scheme rejection | BAJO | ✅ OK (pre-existente) |
| W5 | HTML parsing works | - | ✅ OK |
| W6 | Large response truncation (16K) | - | ✅ OK |
| W7 | 30s timeout enforcement | - | ✅ OK |
| W8 | Selective mode con search_terms | - | ✅ OK |
| W9 | HTTP → HTTPS redirects (legítimos) | - | ✅ OK |

## Estado final — web_search

| # | Hallazgo | Severidad | Estado |
|---|---|---|---|
| S1 | Empty query rejected | - | ✅ OK |
| S2 | Special chars / long query | - | ✅ OK |
| S3 | Nonsense query returns empty array | - | ✅ OK |
| S4 | Real search returns results | - | ✅ OK |
| - | Dependencia externa ddgr (DDG puede rate-limitar) | INFO | ⚠️ Aceptable |

---

## Hallazgos críticos

### W1: Binary content dumped to LLM

**Pre-fix:**
```bash
$ web_fetch url=https://httpbin.org/image/png
→ ok:true, result:"�PNG\r\n\u001a\n\u0000\u0000\u0000\rIHDR..."  (raw PNG bytes)
```

**Causa:** `web_fetch` usaba `resp.text().await` que falla en binario, pero el código devolvía el error en vez de tratarlo bien. Y el benchmark claim dijo que el fix estaba aplicado al sidebar pero NO al daemon.

**Post-fix:**
- Detecta `content-type` binario (image/*, application/pdf, application/zip, etc.) → rechaza
- Lee bytes y busca NUL en los primeros 8KB → si encuentra, rechaza
- Si no es UTF-8 válido → rechaza

Mensaje al LLM: `"fetched body is binary (NUL byte found within first 8192 bytes, total N bytes). web_fetch only returns text."`

### W2: SSRF (Server-Side Request Forgery)

**Pre-fix:**
```bash
$ web_fetch url=http://127.0.0.1:7878/health
→ ok:true, result:'{"auth_required":true,"service":"neurox",...}'  (daemon health leaked!)
```

**Causa:** `web_fetch` confiaba solo en que el URL fuera http/https, sin verificar el destino.

**Post-fix:** función `is_safe_target(url)` que:
1. Parsea el URL
2. Verifica scheme = http|https
3. Resuelve el host a IPs via DNS
4. Rechaza si CUALQUIER IP resuelta es:
   - Loopback (127.0.0.0/8, ::1)
   - Privada (10/8, 172.16/12, 192.168/16, fc00::/7)
   - Link-local (169.254/16, fe80::/10) — **CRÍTICO para AWS metadata**
   - Unspecified (0.0.0.0, ::)
   - Broadcast (255.255.255.255)

### W2-redir: SSRF via redirect

**Pre-fix:**
```bash
$ web_fetch url=http://httpbin.org/redirect-to?url=http://127.0.0.1:7878/health
→ ok:true, result:'{"auth_required":true,...}'  (BYPASS via redirect!)
```

**Causa:** `is_safe_target` solo validaba la URL inicial. Los redirects se seguían ciegamente.

**Post-fix:** `reqwest::redirect::Policy::custom` que valida CADA URL de redirect con la misma función `is_safe_target`. Si un redirect es unsafe, aborta y devuelve error.

### W3: 404 returns ok:true

**Pre-fix:**
```bash
$ web_fetch url=https://httpbin.org/status/404
→ ok:true, result:""  (engañoso para el LLM)
```

**Post-fix:** Verifica `resp.status().is_client_error() || is_server_error()` y retorna `Err("http 404 Not Found")`.

---

## Verificación live

| Comando | Pre-fix | Post-fix |
|---|---|---|
| `web_fetch url=https://httpbin.org/image/png` | ⚠️ dumped binary | ✅ "binary content-type 'image/png'..." |
| `web_fetch url=https://httpbin.org/gzip` | ⚠️ dumped binary | ✅ "fetched body is binary (NUL byte found...)" |
| `web_fetch url=http://127.0.0.1:7878/health` | ⚠️ SSRF to daemon | ✅ "refusing to fetch private/loopback address" |
| `web_fetch url=http://httpbin.org/redirect-to?...127.0.0.1...` | ⚠️ SSRF via redirect | ✅ "redirect blocked: refusing to fetch private/loopback" |
| `web_fetch url=http://169.254.169.254/` | ⚠️ SSRF AWS metadata | ✅ "refusing to fetch private/loopback" |
| `web_fetch url=http://10.0.0.1/` | ⚠️ SSRF private | ✅ blocked |
| `web_fetch url=https://httpbin.org/status/404` | ⚠️ ok:true empty | ✅ "http 404 Not Found" |
| `web_fetch url=https://example.com` | ✅ OK | ✅ OK |
| `web_fetch url=file:///etc/passwd` | ✅ rejected | ✅ "scheme not allowed: file" |
| `web_fetch url=javascript:alert(1)` | ✅ rejected | ✅ "scheme not allowed: javascript" |
| `web_fetch url=not-a-url` | ⚠️ "builder error" | ✅ "invalid url: relative URL without a base" |

**Paralelismo 5/20/100:**
```
N=5   HTTP_OK=5/5   wall=280ms
N=20  HTTP_OK=20/20 wall=308ms
N=100 HTTP_OK=100/100 wall=482ms
```

---

## Cambios aplicados

### `daemon/tools-engine/src/tools/read/web_fetch.rs`

**Nuevas funciones:**
- `is_safe_target(url)` — parsea + valida scheme + resuelve DNS + verifica IPs
- `is_unsafe_ip(ip)` — categoriza IPv4/IPv6 (loopback, private, link-local, etc.)

**`execute` modificado:**
- `is_safe_target(url)?` antes de hacer cualquier request (W2)
- Client usa `redirect::Policy::custom` que valida cada redirect (W2-redir)
- Verifica `status.is_client_error() || is_server_error()` (W3)
- Lista negra de `content-type` binarios antes de leer body (W1)
- Lee como bytes y busca NUL en primeros 8KB (W1)
- Valida UTF-8 (W1)

### `daemon/tools-engine/Cargo.toml`

- Añadido `url = "2"` para parsing de URLs

### Tests añadidos (6)

```
safe_target_blocks_loopback       ✅
safe_target_blocks_private        ✅
safe_target_blocks_non_http       ✅
safe_target_allows_public         ✅
safe_target_rejects_invalid_url   ✅
unsafe_ip_categorization          ✅
```

**Total: 62/62 tests pasan** (56 anteriores + 6 nuevos).

---

## web_search — sin gaps encontrados

- ✅ Empty query rejected
- ✅ Special chars (quotes, backslashes) work
- ✅ Long query handled (ddgr limits)
- ✅ Nonsense query returns `[]`
- ✅ Real query returns JSON results
- ⚠️ External dependency (ddgr) puede rate-limitar — issue externo, no del daemon

---

## Estado de las tools: ✅ CERRADAS

`web_fetch` y `web_search` ambas cerradas. Los 3 gaps críticos (binary content, SSRF loopback, SSRF redirect) están arreglados. El gap menor (404) también.

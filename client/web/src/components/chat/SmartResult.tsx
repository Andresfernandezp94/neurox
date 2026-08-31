// SmartResult — renderer for tool results that picks a specialised
// view based on the content shape (JSON, diff, media, plain text).
//
// EP-0024-UX. The tool result is a free-form string but in practice
// it falls into a few common shapes that benefit from specialised
// rendering:
//   - JSON object / array       → pretty-printed with key:value colouring
//   - Unified diff               → `+` green, `-` red, context muted
//   - Image generation output    → inline <img> with auth-aware fetch
//   - Audio generation output    → inline <audio> with auth-aware fetch
//   - Video generation output    → inline <video> with auth-aware fetch
//   - Shell command output       → monospace plain text
//   - everything else            → monospace plain text with token colouring
//
// EP-2026-08-19 (media fix): media used to render as `<img src="/v1/files/...">`,
// which broke in two ways: (1) the browser cannot send the Bearer token
// on <img> requests, so a daemon with `auth_required: true` returned 401;
// (2) the URL concatenated the empty VITE_API_BASE with a path starting
// in `/`, producing a double-slash URL that the Vite dev proxy collapsed
// to a single slash, turning the absolute file path into a relative one
// (and 404-ing on the daemon). The fix is to fetch the media with the
// Bearer header and feed the response blob into a `blob:` object URL via
// `useMediaBlob`. That works in dev and prod, with or without auth.

import { Highlight, themes } from "prism-react-renderer";
import { ToolCodeBlock } from "../../shared/components/molecules/ToolCodeBlock";
import { useMediaBlob } from "./useMediaBlob";

type ResultKind =
  | "json"
  | "diff"
  | "media-image"
  | "media-audio"
  | "media-video"
  | "text";

const MEDIA_EXTS = {
  image: ["jpg", "jpeg", "png", "gif", "webp", "svg"],
  audio: ["mp3", "wav", "ogg", "m4a", "flac"],
  video: ["mp4", "webm", "mov"],
} as const;

function extOf(path: string): string | null {
  const i = path.lastIndexOf(".");
  if (i < 0) return null;
  return path.slice(i + 1).toLowerCase();
}

function detectResultKind(tool: string, output: string): ResultKind {
  const trimmed = output.trim();

  // EP-2026-08-19: media detection ANTES del JSON check.
  //
  // El backend suele emitir un JSON tipo
  //   {"ok": true, "path": "/home/user/img.png"}
  // como tool_result de generate_image. Si chequeamos JSON primero,
  // lo renderizamos como bloque JSON y el usuario nunca ve la imagen
  // — solo "Image saved · /path/..." y un dump de `{"ok": true, ...}`.
  //
  // extractMediaPath() busca paths dentro del string sin importar que
  // esté entre comillas JSON (el regex excluye `"` y `'`), así que
  // funciona igual sobre JSON crudo.
  //
  // El daemon emite `generate_audio` (ver core/src/capabilities.rs:213)
  // pero algunas versiones/configs lo nombran `generate_music`. Aceptamos
  // ambos como sinónimos.
  const MEDIA_TOOLS: Record<
    "media-image" | "media-audio" | "media-video",
    readonly string[]
  > = {
    "media-image": ["generate_image"],
    "media-audio": ["generate_music", "generate_audio"],
    "media-video": ["generate_video"],
  };
  const mediaPath = extractMediaPath(output);
  const mediaExt = mediaPath ? extOf(mediaPath) : null;
  if (mediaExt) {
    if (
      MEDIA_TOOLS["media-image"].includes(tool) &&
      MEDIA_EXTS.image.includes(mediaExt as never)
    ) {
      return "media-image";
    }
    if (
      MEDIA_TOOLS["media-audio"].includes(tool) &&
      MEDIA_EXTS.audio.includes(mediaExt as never)
    ) {
      return "media-audio";
    }
    if (
      MEDIA_TOOLS["media-video"].includes(tool) &&
      MEDIA_EXTS.video.includes(mediaExt as never)
    ) {
      return "media-video";
    }
  }

  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      JSON.parse(trimmed);
      return "json";
    } catch {
      /* fall through */
    }
  }
  const lines = trimmed.split("\n");
  if (lines.length >= 3) {
    const hunkOrFileHeader = lines.filter(
      (l) =>
        /^@@/.test(l) ||
        /^(\+\+\+|---) /.test(l) ||
        /^[+-]{2} /.test(l),
    ).length;
    const diffLike = lines.filter((l) => /^[+\-@]/.test(l)).length;
    if (
      hunkOrFileHeader >= 1 &&
      diffLike >= 3 &&
      diffLike / lines.length >= 0.3
    ) {
      return "diff";
    }
  }
  return "text";
}

function extractMediaPath(output: string): string | null {
  const m = output.match(
    /([/~][^\s'"\n]+\.(?:jpg|jpeg|png|gif|webp|svg|mp3|wav|ogg|m4a|flac|mp4|webm|mov))/i,
  );
  return m && m[1] ? m[1] : null;
}

// ─── Rich Text Tokeniser ─────────────────────────────────────────────────────

type RichKind =
  | "url"
  | "path"
  | "ts-iso"
  | "ts-date"
  | "ts-time"
  | "level"
  | "number"
  | "hex"
  | "ip"
  | "text";

const RICH_PATTERNS: { kind: RichKind; re: RegExp }[] = [
  { kind: "url", re: /https?:\/\/[^\s<>"'`)]+/ },
  { kind: "path", re: /(?:^|[\s"'`(,=])\/(?:home|tmp|var|etc|usr|opt|root|[a-zA-Z0-9_.\-]+\/)+[a-zA-Z0-9_.\-]+/ },
  { kind: "ts-iso", re: /\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}(?::\d{2})?(?:\.\d+)?(?:Z|[+-]\d{2}:?\d{2})?/ },
  { kind: "ts-date", re: /\b\d{4}-\d{2}-\d{2}\b/ },
  { kind: "ts-time", re: /\b\d{2}:\d{2}:\d{2}\b/ },
  { kind: "level", re: /\b(?:FATAL|CRITICAL|ERROR|WARN(?:ING)?|INFO|DEBUG|TRACE)\b/ },
  { kind: "ip", re: /\b(?:\d{1,3}\.){3}\d{1,3}\b/ },
  { kind: "hex", re: /#[0-9a-fA-F]{3,8}\b/ },
  { kind: "number", re: /\b\d+(?:\.\d+)?\b/ },
];

function tokenizeRich(text: string): { kind: RichKind; text: string }[] {
  const out: { kind: RichKind; text: string }[] = [];
  let i = 0;
  while (i < text.length) {
    let matched = false;
    for (const { kind, re } of RICH_PATTERNS) {
      re.lastIndex = i;
      const m = re.exec(text);
      if (m && m.index === i) {
        out.push({ kind, text: m[0] });
        i += m[0].length;
        matched = true;
        break;
      }
    }
    if (!matched) {
      const last = out[out.length - 1];
      const ch = text[i] ?? "";
      if (last && last.kind === "text") last.text += ch;
      else out.push({ kind: "text", text: ch });
      i++;
    }
  }
  const collapsed: { kind: RichKind; text: string }[] = [];
  for (const tok of out) {
    const prev = collapsed[collapsed.length - 1];
    if (prev && prev.kind === "text" && tok.kind === "text") prev.text += tok.text;
    else collapsed.push({ ...tok });
  }
  return collapsed;
}

function RichTextBlock({ text }: { text: string }) {
  const tokens = tokenizeRich(text);
  return (
    <pre
      className="chat__tool-result-inline chat__tool-result-inline--rich"
      data-testid="smart-result-rich"
    >
      {tokens.map((tok, i) => {
        const level =
          tok.kind === "level"
            ? tok.text.toLowerCase().startsWith("err") ||
              tok.text === "FATAL" ||
              tok.text === "CRITICAL"
              ? "error"
              : tok.text.toLowerCase().startsWith("warn")
              ? "warning"
              : tok.text === "INFO"
              ? "info"
              : tok.text === "DEBUG" || tok.text === "TRACE"
              ? tok.text.toLowerCase()
              : undefined
            : undefined;
        const dataAttr = level ? { "data-level": level } : {};
        return (
          <span
            key={i}
            className={`rich-token rich-token--${tok.kind}`}
            {...dataAttr}
          >
            {tok.text}
          </span>
        );
      })}
    </pre>
  );
}

function JsonBlock({ raw }: { raw: string }) {
  let pretty: string;
  try {
    pretty = JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    pretty = raw;
  }
  return (
    <ToolCodeBlock
      code={pretty}
      language="json"
      className="chat__tool-result-inline chat__tool-result-inline--json"
      testId="smart-result-json"
    />
  );
}

function detectLanguageFromDiff(raw: string): string {
  const m = raw.match(/^[+\-]{3} (\S+)/m);
  if (!m) return "text";
  const path = m[1] ?? "";
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  const map: Record<string, string> = {
    py: "python",
    pyi: "python",
    js: "javascript",
    mjs: "javascript",
    cjs: "javascript",
    ts: "typescript",
    tsx: "tsx",
    jsx: "jsx",
    rs: "rust",
    go: "go",
    sh: "bash",
    bash: "bash",
    zsh: "bash",
    json: "json",
    yaml: "yaml",
    yml: "yaml",
    toml: "toml",
    md: "markdown",
    mdx: "markdown",
    html: "markup",
    htm: "markup",
    xml: "markup",
    css: "css",
    scss: "scss",
    sass: "sass",
    java: "java",
    kt: "kotlin",
    swift: "swift",
    rb: "ruby",
    php: "php",
    c: "c",
    h: "c",
    cpp: "cpp",
    cc: "cpp",
    hpp: "cpp",
    sql: "sql",
    zig: "zig",
  };
  return map[ext] ?? "text";
}

function HighlightedLine({ code, language }: { code: string; language: string }) {
  return (
    <Highlight theme={themes.vsDark} code={code} language={language}>
      {({ tokens, getLineProps, getTokenProps }) => {
        const lineTokens = tokens[0] ?? [];
        return (
          <span {...getLineProps({ line: lineTokens, className: "chat__diff-line-content" })}>
            {lineTokens.map((token, key) => (
              <span key={key} {...getTokenProps({ token })} />
            ))}
          </span>
        );
      }}
    </Highlight>
  );
}

type DiffRowKind = "add" | "del" | "ctx" | "meta" | "hunk";

function DiffViewer({ raw }: { raw: string }) {
  const lines = raw.split("\n");
  const language = detectLanguageFromDiff(raw);

  let oldLine = 0;
  let newLine = 0;

  const rows: { kind: DiffRowKind; old: string; new: string; content: string }[] = [];

  for (const line of lines) {
    if (/^@@/.test(line)) {
      const m = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (m) {
        oldLine = parseInt(m[1] ?? "1", 10);
        newLine = parseInt(m[2] ?? "1", 10);
      }
      rows.push({ kind: "hunk", old: "", new: "", content: line });
    } else if (/^(\+\+\+|---) /.test(line)) {
      rows.push({ kind: "meta", old: "", new: "", content: line.slice(4) });
    } else if (line.startsWith("+")) {
      rows.push({
        kind: "add",
        old: "",
        new: String(newLine++),
        content: line.slice(1),
      });
    } else if (line.startsWith("-")) {
      rows.push({
        kind: "del",
        old: String(oldLine++),
        new: "",
        content: line.slice(1),
      });
    } else {
      const body = line.startsWith(" ") ? line.slice(1) : line;
      rows.push({
        kind: "ctx",
        old: String(oldLine++),
        new: String(newLine++),
        content: body,
      });
    }
  }

  return (
    <div className="chat__diff-viewer" data-testid="smart-result-diff">
      {rows.map((row, i) => (
        <div
          key={i}
          className={`chat__diff-row chat__diff-row--${row.kind}`}
        >
          <span className="chat__diff-num chat__diff-num--old">{row.old}</span>
          <span className="chat__diff-num chat__diff-num--new">{row.new}</span>
          <span className="chat__diff-content">
            {row.kind === "meta" || row.kind === "hunk" ? (
              <span className="chat__diff-raw">{row.content}</span>
            ) : (
              <HighlightedLine code={row.content} language={language} />
            )}
          </span>
        </div>
      ))}
    </div>
  );
}

function MediaBlock({
  kind,
  output,
}: {
  kind: "media-image" | "media-audio" | "media-video";
  output: string;
}) {
  const path = extractMediaPath(output);
  const filename = path ? path.split("/").pop() ?? path : "";
  const isImage = kind === "media-image";
  const isVideo = kind === "media-video";
  const label = isImage
    ? "Image saved"
    : isVideo
    ? "Video saved"
    : "Audio saved";

  // EP-2026-08-19: auth-aware fetch via useMediaBlob. The browser
  // cannot add headers to <img>/<video>/<audio> requests, so we go
  // through fetch() and a blob: object URL. The path we pass to the
  // hook is the API path with the absolute file path URL-encoded into
  // it so the request lands on the daemon's /v1/files handler.
  //
  // Bug visto el 2026-08-19: `path.split("/").join("/")` sobre
  // "/home/user/img.png" devuelve "/home/user/img.png" (con `/` al
  // inicio porque el primer elemento del split es `""`). Concatenado
  // con `/v1/files/` daba `/v1/files//home/user/img.png` con doble
  // slash, que el daemon rechaza con 404 — el MediaBlock quedaba
  // eternamente en `loading` o `error` sin mostrar la imagen.
  // Fix: filtrar los segmentos vacíos del split antes del join, y
  // agregar `/` entre `/v1/files` y el path encoded.
  const apiPath = path
    ? `/v1/files/${path
        .split("/")
        .map((seg) => encodeURIComponent(seg))
        .filter((seg) => seg.length > 0)
        .join("/")}`
    : null;
  const { url, status, error } = useMediaBlob(apiPath, !!path);

  return (
    <div
      className="chat__tool-result-inline chat__tool-result-inline--media"
      data-testid="smart-result-media"
      data-kind={kind}
    >
      <div className="chat__media-label">
        {label}
        {path && (
          <>
            {" · "}
            <code className="chat__media-path" title={path}>
              {path}
            </code>
            {/* EP-2026-08-19: el botón ↓ download SIEMPRE se muestra
                cuando hay path, no solo cuando url está listo. Si el
                fetch inline falló (auth, daemon config, etc.) el
                usuario igualmente puede abrir el archivo en otra
                pestaña usando `apiPath` directamente — el browser
                agregará la cookie de sesión si existe, y sino la
                URL es clara sobre qué pidió. */}
            {apiPath && (
              <a
                className="chat__media-download"
                href={apiPath}
                target="_blank"
                rel="noreferrer"
                aria-label={`Open ${filename}`}
                data-testid="smart-result-media-open"
              >
                ↗ open
              </a>
            )}
            {url && (
              <a
                className="chat__media-download"
                href={url}
                download={filename}
                aria-label={`Download ${filename}`}
                data-testid="smart-result-media-download"
              >
                ↓ download
              </a>
            )}
          </>
        )}
      </div>
      {!path ? (
        <div
          className="chat__media-thumb chat__media-thumb--missing"
          aria-label="Media preview unavailable"
        >
          <span className="chat__media-thumb-glyph">🎵</span>
          <span className="chat__media-thumb-hint">path not found in output</span>
        </div>
      ) : status === "error" ? (
        <div
          className="chat__media-thumb chat__media-thumb--missing"
          aria-label="Media preview failed"
          data-testid="smart-result-media-error"
        >
          <span className="chat__media-thumb-glyph">⚠</span>
          <span className="chat__media-thumb-hint">
            preview unavailable{error ? ` (${error})` : ""}
          </span>
        </div>
      ) : status === "ready" && url ? (
        isImage ? (
          <img
            src={url}
            alt={filename || "generated image"}
            className="chat__media-thumb"
            loading="lazy"
            data-testid="smart-result-image"
          />
        ) : isVideo ? (
          <video
            controls
            preload="metadata"
            className="chat__media-video"
            data-testid="smart-result-video"
          >
            <source src={url} />
            Your browser does not support inline video.
          </video>
        ) : (
          <audio
            controls
            preload="none"
            className="chat__media-audio"
            data-testid="smart-result-audio"
          >
            <source src={url} />
            Your browser does not support inline audio.
          </audio>
        )
      ) : (
        <div
          className="chat__media-thumb chat__media-thumb--loading"
          aria-label="Loading media"
          data-testid="smart-result-media-loading"
        >
          <span className="chat__media-thumb-glyph">⏳</span>
          <span className="chat__media-thumb-hint">loading…</span>
        </div>
      )}
    </div>
  );
}

export function SmartResult({ tool, output }: { tool: string; output: string | undefined }) {
  const text = output ?? "";
  const kind = detectResultKind(tool, text);
  switch (kind) {
    case "json":
      return <JsonBlock raw={text} />;
    case "diff":
      return <DiffViewer raw={text} />;
    case "media-image":
    case "media-audio":
    case "media-video":
      return <MediaBlock kind={kind} output={text} />;
    default:
      return <RichTextBlock text={text} />;
  }
}

/**
 * Build a one-line preview of a tool result for the toggle button.
 * Naively taking the first line breaks JSON outputs (the first line
 * of pretty-printed JSON is just `{`), so for parseable JSON we
 * single-line stringify. For everything else we concatenate the
 * first two short lines so multi-line outputs aren't truncated to a
 * lone character.
 */
export function buildResultPreview(result: string): string {
  const trimmed = result.trim();
  if (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  ) {
    try {
      return JSON.stringify(JSON.parse(trimmed));
    } catch {
      /* fall through */
    }
  }
  const lines = result.split("\n");
  const first = lines[0] ?? "";
  if (first.length < 30 && lines.length > 1) {
    return (first + " " + lines[1]).slice(0, 120);
  }
  return first.slice(0, 80);
}

export { detectResultKind };

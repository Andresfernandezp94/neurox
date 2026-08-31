/**
 * ToolCodeBlock — syntax-highlighted code block for chat tool output.
 *
 * Wraps `Highlight` from prism-react-renderer so args / responses inside
 * the chat panel render with proper token coloring AND respect real
 * line breaks from JSON.stringify(..., null, 2).
 *
 * EP-0026-UX: replaces the bare <pre> at:
 *   - ChatPanel.tsx:678 (tool args)
 *   - ChatPanel.tsx:384 (JsonBlock — tool response)
 *
 * Tokens are themed against project CSS variables (--accent, --text-*)
 * so dark / light modes flip automatically.
 */
import { Highlight, type PrismTheme } from "prism-react-renderer";

export interface ToolCodeBlockProps {
  code: string;
  /** Language id accepted by prism-react-renderer. Defaults to "json". */
  language?: string;
  /** Class to apply to the <pre> — preserves tool-block / tool-result-inline classes. */
  className?: string;
  /** Test id for selectors. */
  testId?: string;
}

/**
 * Project-aware theme. Keys (property) → accent. Strings → green.
 * Numbers / booleans → cyan. Punctuation → secondary text.
 */
const toolCodeBlockTheme: PrismTheme = {
  plain: {
    color: "var(--text-primary)",
    backgroundColor: "transparent",
  },
  styles: [
    { types: ["property", "operator", "tag"], style: { color: "var(--accent)" } },
    { types: ["string", "char"], style: { color: "#a3e635" } },
    { types: ["number", "boolean"], style: { color: "#67e8f9" } },
    { types: ["punctuation", "keyword", "selector"], style: { color: "var(--text-secondary)" } },
    { types: ["comment"], style: { color: "var(--text-muted)", fontStyle: "italic" } },
    { types: ["function", "method", "class-name"], style: { color: "#fbbf24" } },
  ],
};

export function ToolCodeBlock({
  code,
  language = "json",
  className,
  testId,
}: ToolCodeBlockProps) {
  return (
    <Highlight code={code} language={language} theme={toolCodeBlockTheme}>
      {({ className: hlClass, style, tokens, getLineProps, getTokenProps }) => (
        <pre
          className={`${hlClass} ${className ?? ""}`}
          // Force transparent background so the <pre> stays flush against
          // the chat panel surface (no double-fill with prism's default).
          style={{ ...style, background: "transparent" }}
          data-testid={testId ?? "tool-code-block"}
        >
          {tokens.map((line, i) => {
            const lineProps = getLineProps({ line, key: i });
            return (
              <div {...lineProps} key={i}>
                {line.map((token, j) => {
                  const tokenProps = getTokenProps({ token, key: j });
                  return <span {...tokenProps} key={j} />;
                })}
              </div>
            );
          })}
        </pre>
      )}
    </Highlight>
  );
}
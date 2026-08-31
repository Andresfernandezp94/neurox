// Markdown — safe markdown rendering with GFM (tables, strikethrough, etc).
// Sanitizes by default (no raw HTML allowed).
//
// Tablas: las envolvemos en <div class="table-wrap"> con overflow-x:auto
// para que no rompan el viewport cuando el contenido es más ancho que el
// contenedor (mismo patrón que ya se usa para <pre> en organisms.css).

import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MarkdownProps {
  children: string;
}

export function Markdown({ children }: MarkdownProps) {
  return (
    <div className="markdown">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        components={{
          a: ({ href, children }) => (
            <a href={href} target="_blank" rel="noopener noreferrer">
              {children}
            </a>
          ),
          code: ({ className, children, ...rest }) => {
            const isInline = !className;
            if (isInline) {
              return <code className="inline-code">{children}</code>;
            }
            return (
              <code className={className} {...rest}>
                {children}
              </code>
            );
          },
          pre: ({ children }) => <pre className="code-block">{children}</pre>,
          table: ({ children }) => (
            <div className="table-wrap">
              <table>{children}</table>
            </div>
          ),
        }}
      >
        {children}
      </ReactMarkdown>
    </div>
  );
}
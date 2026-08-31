// Tests de los componentes ArgsBlock / ResultBlock.
//
// Cubren el render DOM (no solo el registry): cómo se ven las labels,
// el status badge, y los renderers custom (shell, grep, etc).
//
// IMPORTANTE: en JSX los atributos string ("foo\nbar") NO procesan
// secuencias de escape — pasan la cadena literal. Para strings con
// caracteres especiales (newlines, tabs, etc.) hay que usar
// expression containers (`output={"foo\nbar"}`) o template literals
// (`output={`foo\nbar`}`).

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

// Mockear useToolsSchema para que no haga fetch real en tests.
vi.mock("./useToolsSchema", () => ({
  useToolsSchema: () => ({ labelMap: {}, loadedAt: null, loaded: true }),
}));

import { ArgsBlock } from "./ArgsBlock";
import { ResultBlock } from "./ResultBlock";

describe("ArgsBlock — render de args con labels legibles", () => {
  it("muestra cada key conocida como par label:valor", () => {
    render(
      <ArgsBlock
        tool="shell"
        args={{ cmd: "ls -la", cwd: "/tmp", timeout: 5000 }}
      />,
    );
    expect(screen.getByText("Command")).toBeInTheDocument();
    expect(screen.getByText("ls -la")).toBeInTheDocument();
    expect(screen.getByText("CWD")).toBeInTheDocument();
    expect(screen.getByText("/tmp")).toBeInTheDocument();
    expect(screen.getByText("Timeout")).toBeInTheDocument();
    // duración usa toLocaleString — formato depende del locale del runner,
    // matcheamos por prefijo numérico + sufijo "ms"
    expect(
      screen.getByText(/^[\d,.\s]+ms$/),
    ).toBeInTheDocument();
  });

  it("formatea boolean como ✓ / �", () => {
    const { container } = render(
      <ArgsBlock tool="read_file" args={{ path: "/etc/hosts" }} />,
    );
    expect(container.textContent).toContain("/etc/hosts");
    expect(screen.getByText("Path")).toBeInTheDocument();
  });

  it("cae al JSON cuando ninguna key matchea el schema del tool", () => {
    render(
      <ArgsBlock
        tool="shell"
        args={{ foobar: 1, baz: "x" }}
      />,
    );
    // Sin labels conocidos → bloque JSON.
    expect(screen.getByTestId("tool-args-json")).toBeInTheDocument();
  });

  it("renderiza como null si args no es un objeto", () => {
    const { container } = render(
      <ArgsBlock tool="shell" args="not an object" />,
    );
    expect(container.firstChild).toBeNull();
    const { container: c2 } = render(<ArgsBlock tool="shell" args={null} />);
    expect(c2.firstChild).toBeNull();
  });

  it("los args desconocidos aparecen en un details colapsable aparte", () => {
    render(
      <ArgsBlock
        tool="shell"
        args={{ cmd: "ls", foobar: 123 }}
      />,
    );
    expect(screen.getByText("Command")).toBeInTheDocument();
    expect(screen.getByText(/more param/i)).toBeInTheDocument();
  });

  it("content largo se muestra colapsado con un toggle para expandir", () => {
    const longContent = "x".repeat(500);
    render(
      <ArgsBlock
        tool="write_file"
        args={{ path: "/tmp/a.txt", content: longContent }}
      />,
    );
    // El preview debe estar truncado, pero hay un <details> para abrir.
    const details = screen.getByTestId("tool-args-multiline-content");
    expect(details.tagName.toLowerCase()).toBe("details");
  });
});

describe("ResultBlock — status badge + body", () => {
  it("interrupted → badge sin body", () => {
    render(<ResultBlock tool="shell" output={undefined} interrupted />);
    const status = screen.getByTestId("tool-result-interrupted");
    expect(status).toBeInTheDocument();
    expect(status.textContent).toMatch(/interrupted/i);
  });

  it("shell ok (exit_code=0) → badge ✓ ok + cuerpo con stdout", () => {
    render(
      <ResultBlock
        tool="shell"
        output={JSON.stringify({
          exit_code: 0,
          stdout: "hello world",
          stderr: "",
        })}
      />,
    );
    expect(screen.getByTestId("tool-result")).toBeInTheDocument();
    const status = screen.getByTestId("tool-result-status");
    expect(status.className).toMatch(/ok/);
    expect(status.textContent).toContain("ok");
    expect(screen.getByTestId("tool-shell-stdout")).toBeInTheDocument();
    expect(screen.getByTestId("tool-shell-stdout").textContent).toContain(
      "hello world",
    );
  });

  it("shell fail (exit_code=2) → badge ✗ fail + muestra stderr", () => {
    render(
      <ResultBlock
        tool="shell"
        output={JSON.stringify({
          exit_code: 2,
          stdout: "",
          stderr: "permission denied",
        })}
      />,
    );
    const status = screen.getByTestId("tool-result-status");
    expect(status.className).toMatch(/fail/);
    expect(screen.getByTestId("tool-shell-stderr")).toBeInTheDocument();
    expect(screen.getByTestId("tool-shell-stderr").textContent).toContain(
      "permission denied",
    );
  });

  it("grep vacío → badge ∅ empty", () => {
    render(<ResultBlock tool="grep" output="" />);
    const status = screen.getByTestId("tool-result-status");
    expect(status.className).toMatch(/empty/);
  });

  it("grep con matches → lista renderizada con file:line", () => {
    const output = JSON.stringify({
      matches: [
        { file: "foo.ts", line: 12, text: "fn chat()" },
        { file: "bar.ts", line: 34, text: "fn chatPanel()" },
      ],
    });
    render(<ResultBlock tool="grep" output={output} />);
    expect(screen.getByText("foo.ts:12")).toBeInTheDocument();
    expect(screen.getByText("bar.ts:34")).toBeInTheDocument();
    expect(screen.getByText("fn chat()")).toBeInTheDocument();
  });

  it("glob → lista de paths", () => {
    render(<ResultBlock tool="glob" output={"a.txt\nb.txt\nc.txt"} />);
    expect(screen.getByText("a.txt")).toBeInTheDocument();
    expect(screen.getByText("b.txt")).toBeInTheDocument();
    expect(screen.getByText("c.txt")).toBeInTheDocument();
  });

  it("glob con 1 sola línea → cae al SmartResult (no vale la pena custom)", () => {
    // Con <2 líneas el registry devuelve null → fallback.
    render(<ResultBlock tool="glob" output={"single.txt"} />);
    // El SmartResult es el body por default.
    expect(screen.getByTestId("tool-result")).toBeInTheDocument();
  });

  it("tool desconocido → cae al SmartResult sin romper", () => {
    // No debe tirar, debe mostrar algún body.
    render(
      <ResultBlock
        tool="some_random_plugin"
        output={"{\"ok\": true, \"data\": 123}"}
      />,
    );
    expect(screen.getByTestId("tool-result")).toBeInTheDocument();
  });
});

import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { createRef } from "react";
import { Input } from "./Input";

describe("Input", () => {
  it("renders an input element", () => {
    render(<Input aria-label="username" />);
    expect(screen.getByLabelText("username")).toBeInTheDocument();
  });

  it("defaults to type=text", () => {
    render(<Input aria-label="username" />);
    expect(screen.getByLabelText("username")).toHaveAttribute("type", "text");
  });

  it("forwards ref to underlying input", () => {
    const ref = createRef<HTMLInputElement>();
    render(<Input ref={ref} aria-label="username" />);
    expect(ref.current).toBeInstanceOf(HTMLInputElement);
  });

  it("can focus via ref", () => {
    const ref = createRef<HTMLInputElement>();
    render(<Input ref={ref} aria-label="username" />);
    ref.current?.focus();
    expect(ref.current).toHaveFocus();
  });

  it("calls onChange", () => {
    const handle = vi.fn();
    render(<Input onChange={handle} aria-label="username" />);
    fireEvent.change(screen.getByLabelText("username"), {
      target: { value: "abc" },
    });
    expect(handle).toHaveBeenCalled();
  });

  it("respects disabled", () => {
    render(<Input disabled aria-label="username" />);
    expect(screen.getByLabelText("username")).toBeDisabled();
  });

  it("applies invalid modifier", () => {
    render(<Input invalid aria-label="username" />);
    const wrapper = screen.getByLabelText("username").parentElement;
    expect(wrapper).toHaveClass("input-wrapper--invalid");
    expect(screen.getByLabelText("username")).toHaveClass("input--invalid");
  });

  it("applies size class for sm", () => {
    render(<Input size="sm" aria-label="username" />);
    expect(screen.getByLabelText("username")).toHaveClass("input--sm");
  });

  it("merges custom className", () => {
    render(<Input className="extra" aria-label="username" />);
    const input = screen.getByLabelText("username");
    expect(input).toHaveClass("extra");
    expect(input).toHaveClass("input");
  });

  it("renders rightAdornment", () => {
    render(
      <Input rightAdornment={<span>kg</span>} aria-label="weight" />,
    );
    expect(screen.getByText("kg")).toBeInTheDocument();
  });
});

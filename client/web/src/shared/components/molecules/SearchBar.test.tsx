import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SearchBar } from "./SearchBar";

describe("SearchBar", () => {
  it("renders an input", () => {
    render(<SearchBar value="" onChange={() => {}} />);
    expect(screen.getByRole("searchbox")).toBeInTheDocument();
  });

  it("uses default placeholder", () => {
    render(<SearchBar value="" onChange={() => {}} />);
    expect(screen.getByRole("searchbox")).toHaveAttribute(
      "placeholder",
      "Search…",
    );
  });

  it("uses custom placeholder", () => {
    render(
      <SearchBar value="" onChange={() => {}} placeholder="Find users" />,
    );
    expect(screen.getByRole("searchbox")).toHaveAttribute(
      "placeholder",
      "Find users",
    );
  });

  it("displays current value", () => {
    render(<SearchBar value="foo" onChange={() => {}} />);
    expect(screen.getByRole("searchbox")).toHaveValue("foo");
  });

  it("calls onChange when typing", () => {
    const handle = vi.fn();
    render(<SearchBar value="" onChange={handle} />);
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "abc" },
    });
    expect(handle).toHaveBeenCalledWith("abc");
  });

  it("does not show clear button when value is empty", () => {
    render(<SearchBar value="" onChange={() => {}} onClear={() => {}} />);
    expect(
      screen.queryByRole("button", { name: "Clear search" }),
    ).not.toBeInTheDocument();
  });

  it("shows clear button when value is non-empty and onClear provided", () => {
    render(
      <SearchBar value="x" onChange={() => {}} onClear={() => {}} />,
    );
    expect(
      screen.getByRole("button", { name: "Clear search" }),
    ).toBeInTheDocument();
  });

  it("calls onClear when clear button clicked", () => {
    const handle = vi.fn();
    render(
      <SearchBar value="x" onChange={() => {}} onClear={handle} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    expect(handle).toHaveBeenCalledTimes(1);
  });

  it("uses custom aria-label", () => {
    render(
      <SearchBar
        value=""
        onChange={() => {}}
        ariaLabel="Find sessions"
      />,
    );
    expect(screen.getByRole("searchbox")).toHaveAttribute(
      "aria-label",
      "Find sessions",
    );
  });

  it("merges custom className", () => {
    const { container } = render(
      <SearchBar value="" onChange={() => {}} className="extra" />,
    );
    expect(container.firstChild).toHaveClass("extra");
    expect(container.firstChild).toHaveClass("search-bar");
  });
});

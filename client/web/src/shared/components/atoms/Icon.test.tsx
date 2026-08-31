import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Icon } from "./Icon";

describe("Icon", () => {
  it("renders the named icon", () => {
    const { container } = render(<Icon name="IconMenu" />);
    const svg = container.querySelector("svg");
    expect(svg).toBeInTheDocument();
  });

  it("is decorative by default (aria-hidden)", () => {
    const { container } = render(<Icon name="IconMenu" />);
    const wrapper = container.querySelector(".icon");
    expect(wrapper).toHaveAttribute("aria-hidden", "true");
  });

  it("becomes an img with aria-label when title is provided", () => {
    render(<Icon name="IconMenu" title="Open menu" />);
    expect(screen.getByRole("img", { name: "Open menu" })).toBeInTheDocument();
  });

  it("applies size via inline style", () => {
    const { container } = render(<Icon name="IconMenu" size="lg" />);
    const wrapper = container.querySelector(".icon") as HTMLElement;
    expect(wrapper.style.width).toBe("1.25rem");
    expect(wrapper.style.height).toBe("1.25rem");
  });

  it("applies color via inline style", () => {
    const { container } = render(
      <Icon name="IconMenu" color="var(--accent)" />,
    );
    const wrapper = container.querySelector(".icon") as HTMLElement;
    expect(wrapper.style.color).toBe("var(--accent)");
  });

  it("merges custom className", () => {
    const { container } = render(<Icon name="IconMenu" className="extra" />);
    expect(container.querySelector(".icon.extra")).toBeInTheDocument();
  });

  it("supports many icon names", () => {
    const names = ["IconMenu", "IconChat", "IconSun", "IconMoon", "IconCheck"];
    for (const name of names) {
      const { unmount, container } = render(<Icon name={name as never} />);
      expect(container.querySelector("svg")).toBeInTheDocument();
      unmount();
    }
  });
});

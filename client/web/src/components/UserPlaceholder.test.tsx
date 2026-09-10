// Tests para UserPlaceholder — avatar + nombre + rol + footer opcional.
//
// EP-0025: el item de "Pantalla completa" vivía en un dropdown que fue
// retirado del componente (el fullscreen ahora está en el chat footer).
// Los testids `user-menu*` ya no existen; los tests cubren el card.

import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";

const authMock = vi.fn();
vi.mock("../hooks/useAuth", () => ({
  useAuth: () => authMock(),
}));

import { UserPlaceholder } from "./UserPlaceholder";

describe("UserPlaceholder", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
    authMock.mockReturnValue({
      user: { username: "andres", role: "admin" },
      clear: vi.fn(),
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("renderiza avatar + nombre + rol del usuario", () => {
    render(<UserPlaceholder />);
    expect(screen.getByTestId("user-avatar")).toBeInTheDocument();
    expect(screen.getByTestId("user-info")).toBeInTheDocument();
    expect(screen.getByText("andres")).toBeInTheDocument();
    expect(screen.getByText("admin")).toBeInTheDocument();
  });

  it("renderiza el footer que recibe como prop (ej: logout)", () => {
    render(<UserPlaceholder footer={<button>logout</button>} />);
    expect(
      screen.getByRole("button", { name: "logout" }),
    ).toBeInTheDocument();
  });

  it("aplica avatarClassName extra al user-card", () => {
    const { container } = render(
      <UserPlaceholder avatarClassName="conn-avatar conn-avatar--ok" />,
    );
    expect(container.querySelector(".conn-avatar--ok")).not.toBeNull();
  });

  it("sin usuario cae a inicial U", () => {
    authMock.mockReturnValue({ user: null, clear: vi.fn() });
    render(<UserPlaceholder />);
    expect(screen.getByText("U")).toBeInTheDocument();
  });
});
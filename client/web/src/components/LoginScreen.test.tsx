// Tests for LoginScreen (EP-0007 F7-F8 — JWT user/password form).
//
// Cobertura:
// - Renderiza form con username + password inputs + submit
// - Submit vacío → deshabilitado
// - Submit con creds válidas → llama login() y setSession()
// - 401 → muestra "Credenciales inválidas"
// - Toggle mostrar/ocultar password
// - Footer explica dónde se guarda el token
// - Enter en username → focus password

import { describe, expect, it, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, cleanup, waitFor } from "@testing-library/react";
import { LoginScreen } from "../components/LoginScreen";
import * as authApi from "../api/auth";
import { ApiError } from "../api/client";

const setSessionMock = vi.fn();

vi.mock("../hooks/useAuth", () => ({
  useAuth: () => ({ setSession: setSessionMock }),
}));

vi.mock("../shared/hooks/useTheme", () => ({
  useTheme: () => ({ mode: "dark", setMode: vi.fn() }),
}));

describe("LoginScreen", () => {
  beforeEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders form with username + password inputs and submit button", () => {
    render(<LoginScreen />);
    expect(screen.getByTestId("login-username")).toBeTruthy();
    expect(screen.getByTestId("login-password")).toBeTruthy();
    expect(screen.getByTestId("login-submit")).toBeTruthy();
    expect(screen.getByTestId("login-submit")).toBeDisabled();
  });

  it("submit is enabled only when both username and password are non-empty", () => {
    render(<LoginScreen />);
    const username = screen.getByTestId("login-username");
    const password = screen.getByTestId("login-password");
    const submit = screen.getByTestId("login-submit");

    fireEvent.change(username, { target: { value: "admin" } });
    expect(submit).toBeDisabled();

    fireEvent.change(password, { target: { value: "secret123" } });
    expect(submit).not.toBeDisabled();
  });

  it("submits valid creds and calls login() then setSession()", async () => {
    vi.spyOn(authApi, "login").mockResolvedValue({
      token: "jwt-abc",
      user: {
        id: "u-1",
        username: "admin",
        role: "Admin",
        created_at: "2026-08-11T00:00:00Z",
        last_login_at: null,
      },
    });

    render(<LoginScreen />);
    fireEvent.change(screen.getByTestId("login-username"), {
      target: { value: "admin" },
    });
    fireEvent.change(screen.getByTestId("login-password"), {
      target: { value: "secret" },
    });
    fireEvent.click(screen.getByTestId("login-submit"));

    await waitFor(() => {
      expect(authApi.login).toHaveBeenCalledWith("admin", "secret");
    });
    await waitFor(
      () => {
        expect(setSessionMock).toHaveBeenCalledWith(
          "jwt-abc",
          expect.objectContaining({ username: "admin" }),
        );
      },
      { timeout: 1000 },
    );
  });

  it("shows 'Credenciales inválidas' on 401 from daemon", async () => {
    vi.spyOn(authApi, "login").mockRejectedValue(
      new ApiError(401, "/v1/auth/login", { error: "invalid credentials" }, "Unauthorized"),
    );

    render(<LoginScreen />);
    fireEvent.change(screen.getByTestId("login-username"), {
      target: { value: "admin" },
    });
    fireEvent.change(screen.getByTestId("login-password"), {
      target: { value: "wrong" },
    });
    fireEvent.click(screen.getByTestId("login-submit"));

    await waitFor(() => {
      expect(screen.getByRole("alert")).toHaveTextContent(
        /Credenciales inválidas/i,
      );
    });
  });

  it("toggles password visibility", () => {
    render(<LoginScreen />);
    const password = screen.getByTestId("login-password") as HTMLInputElement;
    expect(password.type).toBe("password");

    const showToggle = screen.getByLabelText(/Mostrar contraseña/i);
    fireEvent.click(showToggle);
    expect(password.type).toBe("text");

    const hideToggle = screen.getByLabelText(/Ocultar contraseña/i);
    fireEvent.click(hideToggle);
    expect(password.type).toBe("password");
  });

  it("renders footer explaining where the token lives", () => {
    render(<LoginScreen />);
    expect(screen.getByText(/sessionStorage/i)).toBeTruthy();
  });

  it("Enter on username focuses password", () => {
    render(<LoginScreen />);
    const username = screen.getByTestId("login-username");
    fireEvent.change(username, { target: { value: "admin" } });
    fireEvent.keyDown(username, { key: "Enter" });
    expect(document.activeElement).toBe(screen.getByTestId("login-password"));
  });
});

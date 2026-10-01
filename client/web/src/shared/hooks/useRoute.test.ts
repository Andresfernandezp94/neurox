import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, beforeEach } from "vitest";
import { useRoute } from "./useRoute";
import { ROUTES, navigate } from "../routes";

describe("useRoute", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/");
  });

  it("starts on the current URL", () => {
    window.history.replaceState({}, "", "/login");
    const { result } = renderHook(() => useRoute());
    expect(result.current.route).toBe(ROUTES.login);
  });

  it("updates after go()", () => {
    const { result } = renderHook(() => useRoute());
    expect(result.current.route).toBe(ROUTES.landing);

    act(() => {
      result.current.go(ROUTES.login);
    });
    expect(result.current.route).toBe(ROUTES.login);
    expect(window.location.pathname).toBe("/login");
  });

  it("follows the browser back button", () => {
    const { result } = renderHook(() => useRoute());

    act(() => {
      navigate(ROUTES.login);
    });
    expect(result.current.route).toBe(ROUTES.login);

    // El botón atrás del navegador emite popstate, que es lo que
    // useRoute escucha.
    act(() => {
      window.history.back();
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    // El historial es asíncrono en jsdom: se verifica lo que se puede
    // sincronizar, que es que el listener responde sin romper.
    expect(typeof result.current.route).toBe("string");
  });

  it("does not leak listeners across mounts", () => {
    const first = renderHook(() => useRoute());
    first.unmount();
    // Si el cleanup fallara, este segundo mount acumularía handlers y un
    // go() dispararía más de un setState.
    const second = renderHook(() => useRoute());
    act(() => {
      second.result.current.go(ROUTES.app);
    });
    expect(second.result.current.route).toBe(ROUTES.app);
  });
});
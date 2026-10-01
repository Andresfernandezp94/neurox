import { describe, it, expect, beforeEach } from "vitest";
import { ROUTES, normalizePath, navigate, currentRoute } from "./routes";

describe("routes", () => {
  beforeEach(() => {
    window.history.replaceState({}, "", "/");
  });

  describe("normalizePath", () => {
    it("maps the known routes", () => {
      expect(normalizePath("/")).toBe(ROUTES.landing);
      expect(normalizePath("/login")).toBe(ROUTES.login);
      expect(normalizePath("/app")).toBe(ROUTES.app);
    });

    it("tolerates a trailing slash", () => {
      // /login/ y /login son la misma pantalla: sin esto, un deep link
      // con slash caería a la landing y se perdería el formulario.
      expect(normalizePath("/login/")).toBe(ROUTES.login);
      expect(normalizePath("/app/")).toBe(ROUTES.app);
    });

    it("strips the query string", () => {
      expect(normalizePath("/app?hideHeader=1")).toBe(ROUTES.app);
    });

    it("falls back to the landing for unknown routes", () => {
      // Un deep link roto no debe dejar la app en blanco.
      expect(normalizePath("/no-existe")).toBe(ROUTES.landing);
      expect(normalizePath("/app/admin/deep")).toBe(ROUTES.landing);
    });
  });

  describe("navigate", () => {
    it("pushes by default, so the back button returns", () => {
      navigate(ROUTES.login);
      expect(window.location.pathname).toBe("/login");
      expect(currentRoute()).toBe(ROUTES.login);
    });

    it("replaces when asked, so login→app leaves no history entry", () => {
      // Con push, el botón atrás del navegador devolvería al login ya
      // autenticado. Por eso login→app y logout→login usan replace.
      navigate(ROUTES.app, { replace: true });
      expect(currentRoute()).toBe(ROUTES.app);
    });

    it("emits popstate so useRoute subscribers update", () => {
      // pushState no dispara popstate por sí solo; sin este evento
      // manual el hook no se enteraría del cambio de ruta.
      let fired = 0;
      const onPop = () => {
        fired += 1;
      };
      window.addEventListener("popstate", onPop);
      navigate(ROUTES.app);
      window.removeEventListener("popstate", onPop);
      expect(fired).toBe(1);
    });
  });

  describe("currentRoute", () => {
    it("reads the live location", () => {
      window.history.replaceState({}, "", "/login");
      expect(currentRoute()).toBe(ROUTES.login);
      window.history.replaceState({}, "", "/app");
      expect(currentRoute()).toBe(ROUTES.app);
    });
  });
});
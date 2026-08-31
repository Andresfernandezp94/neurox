// Tests del MediaBlock — verifica que la URL del fetch al daemon
// se construye correctamente.
//
// EP-2026-08-19: bug visto en producción. El apiPath tenía un
// `//` doble (path "/home/user/img.png" + prefijo "/v1/files/"
// daba "/v1/files//home/user/img.png"). El daemon rechazaba con 404
// y la imagen no se mostraba.

import { render, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

// Mockear useToolsSchema para no hacer fetch de /v1/tools en tests.
vi.mock("./useToolsSchema", () => ({
  useToolsSchema: () => ({ labelMap: {}, loadedAt: null, loaded: true }),
}));

// Mockear client.ts para que getToken devuelva algo y buildApiUrl
// no tire.
vi.mock("../../../api/client", async () => {
  const actual = await vi.importActual<typeof import("../../../api/client")>(
    "../../../api/client",
  );
  return {
    ...actual,
    getApiBase: () => "",
    getToken: () => "test-token",
    buildApiUrl: (p: string) => p,
  };
});

import { ResultBlock } from "./ResultBlock";

describe("MediaBlock — apiPath construido sin double slash", () => {
  const originalFetch = global.fetch;

  afterEach(() => {
    global.fetch = originalFetch;
  });

  it("path absoluto típico → /v1/files/home/user/img.png (un solo slash)", async () => {
    let fetchedUrl: string | null = null;
    global.fetch = vi.fn(async (url: RequestInfo | URL) => {
      fetchedUrl = String(url);
      // Devolver un blob fake de 1x1 PNG.
      return new Response(
        new Blob([new Uint8Array([137, 80, 78, 71])], { type: "image/png" }),
        {
          status: 200,
          headers: { "Content-Type": "image/png" },
        },
      );
    }) as unknown as typeof fetch;

    render(
      <ResultBlock
        tool="generate_image"
        output={"Image saved at /home/user/img.png"}
      />,
    );

    await waitFor(() => {
      expect(fetchedUrl).not.toBeNull();
    });

    expect(fetchedUrl).toBe("/v1/files/home/user/img.png");
    // explícitamente: NO debe tener `//`
    expect(fetchedUrl).not.toMatch(/\/\//);
  });

  it("path con espacios se rompe por la limitación del regex de extracción (no new feature)", async () => {
    // NOTA: el regex de extractMediaPath no soporta paths con espacios
    // (solo captura hasta el primer whitespace). Este test documenta
    // esa limitación — la URL final termina siendo solo el último
    // segmento. Cuando se arregle el regex (e.g. usar una heurística
    // más laxa), este test se actualiza.
    let fetchedUrl: string | null = null;
    global.fetch = vi.fn(async (url: RequestInfo | URL) => {
      fetchedUrl = String(url);
      return new Response(new Blob(), { status: 200 });
    }) as unknown as typeof fetch;

    render(
      <ResultBlock
        tool="generate_image"
        output={"saved at /tmp/Imagenes del usuario/img.png"}
      />,
    );

    await waitFor(() => expect(fetchedUrl).not.toBeNull());
    // Solo el último segmento matchea (no el path completo).
    expect(fetchedUrl).toBe("/v1/files/img.png");
    expect(fetchedUrl).not.toMatch(/\/\//);
  });

  it("path con segmentos que se repiten → colapsa los vacíos", async () => {
    let fetchedUrl: string | null = null;
    global.fetch = vi.fn(async (url: RequestInfo | URL) => {
      fetchedUrl = String(url);
      return new Response(new Blob(), { status: 200 });
    }) as unknown as typeof fetch;

    render(
      <ResultBlock
        tool="generate_image"
        output={"saved at //double//slashes/img.png"}
      />,
    );

    await waitFor(() => expect(fetchedUrl).not.toBeNull());
    // El filtro de vacíos colapsa //double//slashes → double/slashes
    expect(fetchedUrl).toBe("/v1/files/double/slashes/img.png");
    expect(fetchedUrl).not.toMatch(/\/\//);
  });

  it("path en home → encoded, sin doble slash", async () => {
    let fetchedUrl: string | null = null;
    global.fetch = vi.fn(async (url: RequestInfo | URL) => {
      fetchedUrl = String(url);
      return new Response(new Blob(), { status: 200 });
    }) as unknown as typeof fetch;

    render(
      <ResultBlock
        tool="generate_audio"
        output={"Audio saved at /tmp/sound.mp3"}
      />,
    );

    await waitFor(() => expect(fetchedUrl).not.toBeNull());
    expect(fetchedUrl).toBe("/v1/files/tmp/sound.mp3");
  });

  it("tool_result como JSON con `path` → image preview, no JSON dump", async () => {
    // EP-2026-08-19: bug visto en producción. El daemon emite el
    // tool_result como JSON:
    //   {"ok": true, "path": "/home/user/img.png"}
    // Antes este caso caía al JsonBlock y el usuario veía un dump
    // de JSON sin la imagen. Ahora la detección de media tiene
    // prioridad y se renderiza como <img>.
    let fetchedUrl: string | null = null;
    global.fetch = vi.fn(async (url: RequestInfo | URL) => {
      fetchedUrl = String(url);
      return new Response(new Blob(), { status: 200 });
    }) as unknown as typeof fetch;

    render(
      <ResultBlock
        tool="generate_image"
        output={`{"ok": true, "path": "/home/user/img.png"}`}
      />,
    );

    await waitFor(() => expect(fetchedUrl).not.toBeNull());
    expect(fetchedUrl).toBe("/v1/files/home/user/img.png");
    // Esperar a que el estado async del useMediaBlob ponga el
    // <img> en el DOM (status=ready → render de la imagen).
    await waitFor(() => {
      expect(document.querySelector("img")).toBeTruthy();
    });
    // NO debe haber un JsonBlock (test-id smart-result-json).
    expect(document.querySelector('[data-testid="smart-result-json"]')).toBeNull();
  });

  it("si el fetch inline falla, el botón 'open' sigue disponible para abrir el archivo", async () => {
    // El botón "↗ open" apunta a /v1/files/{path} directamente. Es
    // un fallback útil si el fetch inline falla (auth, daemon
    // config, etc.) — el usuario puede abrir el archivo en otra
    // pestaña sin perder el contexto del chat.
    global.fetch = vi.fn(async () =>
      new Response("not found", { status: 404 }),
    ) as unknown as typeof fetch;

    render(
      <ResultBlock
        tool="generate_image"
        output={`Image saved at /home/user/img.png`}
      />,
    );

    await waitFor(() => {
      expect(
        document.querySelector('[data-testid="smart-result-media-error"]'),
      ).toBeTruthy();
    });
    const openLink = document.querySelector(
      '[data-testid="smart-result-media-open"]',
    ) as HTMLAnchorElement | null;
    expect(openLink).toBeTruthy();
    expect(openLink?.getAttribute("href")).toBe(
      "/v1/files/home/user/img.png",
    );
    expect(openLink?.getAttribute("target")).toBe("_blank");
  });
});

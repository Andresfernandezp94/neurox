import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useBootGate } from "./useBootGate";

// El gate importa isMockMode de mockData. Por defecto false, así que
// ejercitamos el camino real (con red).
vi.mock("../shared/mock/mockData", () => ({
  isMockMode: () => false,
}));

describe("useBootGate", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("keeps the loader while the store is not ready", () => {
    const { result } = renderHook(() => useBootGate({ ready: false, minMs: 100, maxMs: 1000 }));
    expect(result.current.booting).toBe(true);
  });

  it("keeps the loader until the minimum elapses, even if ready", () => {
    // Listo antes de minMs: el loader no debe parpadear.
    const { result } = renderHook(() => useBootGate({ ready: true, minMs: 500, maxMs: 5000 }));
    expect(result.current.booting).toBe(true);

    act(() => {
      vi.advanceTimersByTime(600);
    });
    expect(result.current.booting).toBe(false);
  });

  it("stops booting once ready AND minMs elapsed", () => {
    const { result } = renderHook(() => useBootGate({ ready: true, minMs: 200, maxMs: 5000 }));

    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(result.current.booting).toBe(true);

    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(result.current.booting).toBe(false);
  });

  it("gives up after maxMs even if the store never becomes ready", () => {
    // El caso que motivó el hook: daemon caído → health nunca resuelve
    // → loaded.health nunca pasa a true. La app tiene que aparecer igual.
    const { result } = renderHook(() => useBootGate({ ready: false, minMs: 100, maxMs: 1000 }));

    act(() => {
      vi.advanceTimersByTime(1500);
    });
    expect(result.current.booting).toBe(false);
  });

  it("never boots longer than maxMs", () => {
    const { result } = renderHook(() => useBootGate({ ready: true, minMs: 5000, maxMs: 800 }));

    act(() => {
      vi.advanceTimersByTime(900);
    });
    // maxMs pisa a minMs: el tope duro gana.
    expect(result.current.booting).toBe(false);
  });
});
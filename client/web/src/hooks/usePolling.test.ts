// Tests del hook usePolling. EP-0001-02.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { usePolling } from './usePolling';

describe('usePolling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('fetches immediately on mount and sets data', async () => {
    const fn = vi.fn().mockResolvedValue({ value: 42 });
    const { result } = renderHook(() => usePolling(fn, 1000));

    // La promesa del fetch se inicia; avanzamos el microtask queue.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(result.current.data).toEqual({ value: 42 });
    expect(result.current.error).toBeNull();
    expect(result.current.loading).toBe(false);
  });

  it('calls fn again after intervalMs', async () => {
    const fn = vi.fn().mockResolvedValue('ok');
    renderHook(() => usePolling(fn, 1000));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(fn).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(fn).toHaveBeenCalledTimes(2);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(1000);
    });
    expect(fn).toHaveBeenCalledTimes(3);
  });

  it('captures errors into the error state', async () => {
    const fn = vi.fn().mockRejectedValue(new Error('boom'));
    const { result } = renderHook(() => usePolling(fn, 1000));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });

    expect(result.current.data).toBeNull();
    expect(result.current.error).toBeInstanceOf(Error);
    expect(result.current.error?.message).toBe('boom');
  });

  it('cancels interval on unmount', async () => {
    const fn = vi.fn().mockResolvedValue('ok');
    const { unmount } = renderHook(() => usePolling(fn, 1000));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(fn).toHaveBeenCalledTimes(1);

    unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5000);
    });
    // No se invoca después de unmount.
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('refresh() re-triggers a fetch immediately', async () => {
    const fn = vi.fn().mockResolvedValue('ok');
    const { result } = renderHook(() => usePolling(fn, 10_000));

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(fn).toHaveBeenCalledTimes(1);

    act(() => result.current.refresh());

    await act(async () => {
      await vi.advanceTimersByTimeAsync(0);
    });
    expect(fn).toHaveBeenCalledTimes(2);
  });
});

// Tests del hook useAuth. EP-0001-02 (legacy token) + EP-0007 (user/session).

import { describe, expect, it, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import type { ReactNode } from 'react';
import { AuthProvider, useAuth } from './useAuth';
import { setToken } from '../api/client';

// Wrap helper: provides the AuthContext so the hook works in isolation.
// renderHook's `wrapper` expects a component ({ children }: { children: ReactNode }).
function withAuth({ children }: { children: ReactNode }) {
  return <AuthProvider>{children}</AuthProvider>;
}

describe('useAuth', () => {
  beforeEach(() => {
    setToken(null);
    sessionStorage.clear();
    localStorage.clear();
  });

  it('starts with no session when storage is empty', () => {
    const { result } = renderHook(() => useAuth(), { wrapper: withAuth });
    expect(result.current.token).toBeNull();
    expect(result.current.user).toBeNull();
    expect(result.current.isAuthenticated).toBe(false);
  });

  it('reflects the token already in sessionStorage', () => {
    setToken('pre-existing');
    const { result } = renderHook(() => useAuth(), { wrapper: withAuth });
    expect(result.current.token).toBe('pre-existing');
  });

  it('setSession updates token + user + isAuthenticated', () => {
    const { result } = renderHook(() => useAuth(), { wrapper: withAuth });
    const user = {
      id: 'u1',
      username: 'admin',
      role: 'Admin' as const,
      created_at: '2026-01-01T00:00:00Z',
      last_login_at: null,
    };
    act(() => result.current.setSession('jwt-token', user));
    expect(result.current.token).toBe('jwt-token');
    expect(result.current.user).toEqual(user);
    expect(result.current.isAuthenticated).toBe(true);
    expect(sessionStorage.getItem('neurox_token')).toBe('jwt-token');
    expect(JSON.parse(localStorage.getItem('neurox_user')!)).toEqual(user);
  });

  it('clear() removes token and user', () => {
    const { result } = renderHook(() => useAuth(), { wrapper: withAuth });
    act(() =>
      result.current.setSession('temp', {
        id: 'u1',
        username: 'admin',
        role: 'Admin',
        created_at: '',
        last_login_at: null,
      }),
    );
    expect(result.current.isAuthenticated).toBe(true);
    act(() => result.current.clear());
    expect(result.current.token).toBeNull();
    expect(result.current.user).toBeNull();
    expect(result.current.isAuthenticated).toBe(false);
    expect(sessionStorage.getItem('neurox_token')).toBeNull();
    expect(localStorage.getItem('neurox_user')).toBeNull();
  });
});
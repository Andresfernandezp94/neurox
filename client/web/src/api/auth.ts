// Auth API — wrappers around /v1/auth/* and /v1/users/*.
// EP-0007: replaces the legacy single-Bearer-token model with
// username + password + JWT.

import { apiGet, apiPost, apiPatch, apiDelete, buildApiUrl, ApiError } from './client';
import { getToken, setToken } from './client';

export type Role = 'Admin' | 'Operator' | 'Viewer';

export interface UserInfo {
  id: string;
  username: string;
  role: Role;
  created_at: string;
  last_login_at: string | null;
}

export interface LoginResponse {
  token: string;
  user: UserInfo;
}

export async function login(username: string, password: string): Promise<LoginResponse> {
  const res = await apiPost<LoginResponse>('/v1/auth/login', { username, password });
  // Persist immediately so subsequent requests in the same tick carry the token.
  setToken(res.token);
  return res;
}

export async function refresh(): Promise<string> {
  const res = await apiPost<{ token: string }>('/v1/auth/refresh', {});
  setToken(res.token);
  return res.token;
}

export async function logout(): Promise<void> {
  try {
    await apiPost<unknown>('/v1/auth/logout', {});
  } catch {
    // Even if logout fails server-side, the client should drop the token.
  }
  setToken(null);
}

export async function listUsers(): Promise<UserInfo[]> {
  return apiGet<UserInfo[]>('/v1/users');
}

export async function createUser(
  username: string,
  password: string,
  role: Role,
): Promise<UserInfo> {
  return apiPost<UserInfo>('/v1/users', { username, password, role });
}

export async function updateUser(
  id: string,
  patch: { role?: Role; password?: string },
): Promise<UserInfo> {
  return apiPatch<UserInfo>(`/v1/users/${encodeURIComponent(id)}`, patch);
}

export async function deleteUser(id: string): Promise<void> {
  await apiDelete<void>(`/v1/users/${encodeURIComponent(id)}`);
}

export async function changeMyPassword(
  oldPassword: string,
  newPassword: string,
): Promise<void> {
  await apiPatch<void>('/v1/users/me/password', {
    old_password: oldPassword,
    new_password: newPassword,
  });
}

/** True if the daemon requires JWT auth (drives the login screen gate). */
export async function isAuthRequired(): Promise<boolean> {
  try {
    const url = buildApiUrl('/health');
    const res = await fetch(url);
    if (!res.ok) return false;
    const data = (await res.json()) as { auth_required?: boolean };
    return data.auth_required === true;
  } catch {
    return false;
  }
}

/**
 * Hook helper: any 401 from a downstream request invalidates the session
 * and clears the token, so the LoginScreen re-renders.
 */
export function handleUnauthorized(): void {
  setToken(null);
}

// Re-export so callers don't need to import from client.
export { getToken, ApiError };
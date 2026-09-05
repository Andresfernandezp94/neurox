// Hook + Context que maneja la sesión del usuario actual: token JWT +
// datos del user. Persiste el token en sessionStorage y los datos del
// user en localStorage (para sobrevivir reloads). EP-0007.
//
// EP-0007 también requiere que el state sea COMPARTIDO entre todos los
// consumidores (LoginScreen, App.tsx, ConfigViewer). Si cada componente
// tuviera su propio useState local, setSession() desde LoginScreen no
// dispararía re-render en App.tsx → el gate auth_required && !token
// seguiría mostrando LoginScreen para siempre. El Context resuelve esto:
// un único state en el Provider, todos los useAuth() leen del mismo.

import {
  createContext,
  useCallback,
  useContext,
  useState,
  type ReactNode,
} from 'react';
import { getToken, setToken as persistToken } from '../api/client';
import { type UserInfo } from '../api/auth';

const USER_KEY = 'neurox_user';

function readStoredUser(): UserInfo | null {
  if (typeof window === 'undefined') return null;
  try {
    const raw = window.localStorage.getItem(USER_KEY);
    if (!raw) return null;
    return JSON.parse(raw) as UserInfo;
  } catch {
    return null;
  }
}

function writeStoredUser(user: UserInfo | null): void {
  if (typeof window === 'undefined') return;
  try {
    if (user) window.localStorage.setItem(USER_KEY, JSON.stringify(user));
    else window.localStorage.removeItem(USER_KEY);
  } catch {
    // ignore (quota, private mode)
  }
}

export interface AuthContextValue {
  token: string | null;
  user: UserInfo | null;
  setSession: (token: string, user: UserInfo) => void;
  clear: () => void;
  isAuthenticated: boolean;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [token, setTokenState] = useState<string | null>(() => getToken());
  const [user, setUser] = useState<UserInfo | null>(() => readStoredUser());

  const setSession = useCallback((newToken: string, newUser: UserInfo) => {
    persistToken(newToken);
    writeStoredUser(newUser);
    setTokenState(newToken);
    setUser(newUser);
    // EP-2026-09-02: notificar a las WebSockets (/v1/events, /v1/commands)
    // que ya hay un token disponible para que agreguen `?token=<jwt>`
    // al handshake. Sin esto, las WS quedan en loop 401 hasta el
    // próximo backoff.
    window.dispatchEvent(new CustomEvent('neurox:auth-changed'));
  }, []);

  const clear = useCallback(() => {
    persistToken(null);
    writeStoredUser(null);
    setTokenState(null);
    setUser(null);
    window.dispatchEvent(new CustomEvent('neurox:auth-changed'));
  }, []);

  return (
    <AuthContext.Provider
      value={{
        token,
        user,
        setSession,
        clear,
        isAuthenticated: token !== null && user !== null,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within an <AuthProvider>');
  return ctx;
}

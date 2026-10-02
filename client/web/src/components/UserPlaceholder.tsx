// UserPlaceholder — avatar + nombre + rol. El botón de power/logout
// vive en el sidebar footer (pasado como prop `footer`).

import type { ReactNode } from "react";
import { useAuth } from "../hooks/useAuth";

function initialsFrom(name: string | undefined | null): string {
  if (!name) return "U";
  const trimmed = name.trim();
  const parts = trimmed.split(/[\s._-]+/).filter(Boolean);
  if (parts.length === 0) return trimmed.slice(0, 2).toUpperCase();
  if (parts.length === 1) {
    const p0 = parts[0] || "U";
    return p0.slice(0, 2).toUpperCase();
  }
  const p0: string = parts[0] || "";
  const p1: string = parts[1] || "";
  if (p0.length === 0 || p1.length === 0) {
    const fallback = parts[0] || "U";
    return fallback.slice(0, 2).toUpperCase();
  }
  return (p0.charAt(0) + p1.charAt(0)).toUpperCase();
}

export interface UserPlaceholderProps {
  /** Clase extra aplicada al avatar button (ej: `conn-avatar conn-avatar--ok`). */
  avatarClassName?: string;
  /** Contenido extra que se renderiza dentro del user-card (ej: botón logout). */
  footer?: ReactNode;
  /** Click handler para el trigger del avatar (para abrir menú). */
  onTriggerClick?: () => void;
  /**
   * Estado del menu que abre el avatar. En mobile es lo unico que hay en el
   * pie, asi que el trigger tiene que anunciar si esta abierto: sin
   * `aria-expanded`, un lector de pantalla no distingue "avatar" de "avatar
   * con el menu abierto".
   */
  actionsOpen?: boolean;
}

export function UserPlaceholder({ avatarClassName, footer, onTriggerClick, actionsOpen }: UserPlaceholderProps = {}) {
  const { user } = useAuth();

  const initials = initialsFrom(user?.username);

  return (
    <div className="user-menu">
      <div className={`user-card ${avatarClassName ?? ""}`.trim()}>
        <div
          className="user-card__trigger"
          title={user ? `${user.username} (${user.role})` : "Cuenta"}
          data-testid="user-avatar"
          onClick={onTriggerClick}
          role="button"
          tabIndex={0}
          aria-expanded={actionsOpen === undefined ? undefined : actionsOpen}
          onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') onTriggerClick?.(); }}
        >
          <span className="user-avatar">
            <span className="user-avatar__initials" aria-hidden="true">
              {initials}
            </span>
          </span>

          {user && (
            <span className="user-info" data-testid="user-info">
              <span className="user-info__name">{user.username}</span>
              <span className="user-info__role">{user.role}</span>
            </span>
          )}
        </div>
        {footer && <div className="user-card__footer">{footer}</div>}
      </div>
    </div>
  );
}

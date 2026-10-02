import { useEffect, type ReactNode } from "react";
import { IconButton } from "../atoms/IconButton";

export interface ModalProps {
  open: boolean;
  title?: ReactNode;
  /** Se cierra con Escape, con click en el fondo, y con el boton de cerrar. */
  onClose: () => void;
  children: ReactNode;
  /** Barra de abajo. Si no se pasa, no hay footer. */
  footer?: ReactNode;
  className?: string;
  "data-testid"?: string;
}

/**
 * Modal generico: backdrop, dialog, Escape y click fuera.
 *
 * No existia uno. Habia `ConfirmDialog` y `AlertModal`, pero ninguno acepta
 * children, asi que no se podia meter un formulario adentro. Este cubre el
 * caso generico y los dos existentes siguen como estan para confirmaciones.
 *
 * El fondo cierra por `onMouseDown` con `target === currentTarget`, que es
 * la unica forma de distinguir click en el backdrop de click adentro: con
 * `onClick` el drag desde el contenido al backdrop lo cerraria.
 */
export function Modal({
  open,
  title,
  onClose,
  children,
  footer,
  className,
  "data-testid": testId,
}: ModalProps) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="modal-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className={["modal", className].filter(Boolean).join(" ")}
        role="dialog"
        aria-modal="true"
        data-testid={testId}
      >
        {title != null && (
          <header className="modal__header">
            <h2 className="modal__title">{title}</h2>
            <IconButton
              icon="IconClose"
              size="sm"
              variant="ghost"
              aria-label="Close"
              onClick={onClose}
              data-testid={testId ? `${testId}-close` : undefined}
            />
          </header>
        )}
        <div className="modal__body">{children}</div>
        {footer != null && <footer className="modal__footer">{footer}</footer>}
      </div>
    </div>
  );
}
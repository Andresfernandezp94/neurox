// ConfirmDialog — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Sin options complejas (variant: danger/primary)
// - Cierre con Escape

import type { ReactNode } from "react";
import { Modal } from "./molecules/Modal";
import { Button } from "./atoms/Button";

interface Props {
  open: boolean;
  title?: string;
  message: string | ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * Confirmacion sobre `Modal`.
 *
 * Antes montaba su propio overlay con `confirm-dialog-overlay`,
 * `confirm-dialog__card`, `confirm-dialog__title` y
 * `confirm-dialog__message`, y NINGUNA de las cuatro tenia CSS en ningun
 * archivo: el dialogo salia sin fondo, sin borde y sin padding. Sobre
 * `Modal` hereda la superficie, el Escape y el click fuera de una sola vez,
 * y deja de haber dos implementaciones de lo mismo.
 *
 * El `message` puede ser un `ReactNode`, asi que el modal acepta contenido
 * arbitrario; esta clase solo lo compone.
 */
export function ConfirmDialog({
  open,
  title,
  message,
  confirmLabel = "Confirmar",
  cancelLabel = "Cancelar",
  destructive = false,
  onConfirm,
  onCancel,
}: Props) {
  return (
    <Modal
      open={open}
      onClose={onCancel}
      title={title}
      className="confirm-dialog"
      data-testid="confirm-dialog"
      footer={
        <>
          <Button variant="secondary" onClick={onCancel}>
            {cancelLabel}
          </Button>
          <Button
            variant={destructive ? "danger" : "primary"}
            onClick={onConfirm}
            data-testid="confirm-dialog-confirm"
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <div className="confirm-dialog__message">{message}</div>
    </Modal>
  );
}
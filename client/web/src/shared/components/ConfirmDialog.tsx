// ConfirmDialog — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Sin options complejas (variant: danger/primary)
// - Cierre con Escape

import { useEffect, type ReactNode } from "react";
import { Card } from "./molecules/Card";
import { Row } from "./molecules/Row";
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
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div
      className="confirm-dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
    >
      <Card className="confirm-dialog__card">
        {title && <h3 className="strong confirm-dialog__title">{title}</h3>}
        <div className="muted confirm-dialog__message">{message}</div>
        <Row justify="between">
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
        </Row>
      </Card>
    </div>
  );
}
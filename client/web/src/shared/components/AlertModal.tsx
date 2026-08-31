// AlertModal — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Sin título hardcodeado (viene del prop)
// - Cierre con Escape

import { useEffect, type ReactNode } from "react";
import { Card } from "./molecules/Card";
import { Row } from "./molecules/Row";
import { Button } from "./atoms/Button";

interface Props {
  open: boolean;
  title?: string;
  message: string | ReactNode;
  onClose: () => void;
}

export function AlertModal({ open, title, message, onClose }: Props) {
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="alert-modal-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <Card className="alert-modal__card">
        {title && <h3 className="strong alert-modal__title">{title}</h3>}
        <div className="muted alert-modal__message">{message}</div>
        <Row justify="between">
          <span />
          <Button variant="primary" onClick={onClose}>
            OK
          </Button>
        </Row>
      </Card>
    </div>
  );
}
// SectionHeader — port one-way desde agent-studio (adaptado al admin).
// Snapshot: 2026-08-04 (EP-0002-01 Task 5)
// Migrated to atomic design in EP-0016 follow-up (FU-EP0016-01).
//
// Cambios vs. agent-studio:
// - Sin badge (no se usa en el admin)
// - Con actions slot (consumido por StatusPanel, etc.)

import type { ReactNode } from "react";
import { Row } from "./molecules/Row";

interface Props {
  title: string;
  description?: ReactNode;
  actions?: ReactNode;
}

export function SectionHeader({ title, description, actions }: Props) {
  return (
    <header className="section-header">
      <div>
        <h2 className="strong section-header__title">{title}</h2>
        {description && (
          <p className="muted text-sm section-header__description">{description}</p>
        )}
      </div>
      {actions && <Row gap="md">{actions}</Row>}
    </header>
  );
}
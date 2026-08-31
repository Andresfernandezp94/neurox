// PanelToggle — convenience wrapper around useHiddenPanels. Renders
// `children` only when the panel id is NOT in the `?hide=` URL
// parameter. Use this during refactors to isolate parts of the UI:
//
//   <PanelToggle id="contextBar">
//     <ContextBar />
//   </PanelToggle>
//
// Pair with `useHiddenPanels` for conditional logic that can't just
// be wrapped (e.g. "hide this sidebar AND collapse the main column
// when it's hidden"):

import type { ReactNode } from "react";
import { useHiddenPanels } from "../../hooks/useHiddenPanels";

export interface PanelToggleProps {
  /**
   * Stable id for the panel. Use snake_case or camelCase; the value
   * must match what you pass to `?hide=` (or to `isHidden()`).
   * Convention: kebab-case, scoped to the file
   * (`chat-context-bar`, `chat-history`, etc.).
   */
  id: string;
  /**
   * Rendered only when the panel is NOT hidden. `children` is the
   * raw subtree, so you can also pass a Fragment.
   */
  children: ReactNode;
}

export function PanelToggle({ id, children }: PanelToggleProps) {
  const { isHidden } = useHiddenPanels();
  if (isHidden(id)) return null;
  return <>{children}</>;
}
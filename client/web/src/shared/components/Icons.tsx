const s = {
  width: "1em",
  height: "1em",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.75,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": "true" as const,
  focusable: "false" as const,
  preserveAspectRatio: "xMidYMid meet",
};

/** Props for icon components. Optional `className` for sizing/coloring. */
export interface IconProps {
  className?: string;
}

export const IconMenu = () => (
  <svg viewBox="0 0 24 24" {...s}><line x1="3" y1="6" x2="21" y2="6" /><line x1="3" y1="12" x2="21" y2="12" /><line x1="3" y1="18" x2="21" y2="18" /></svg>
);

export const IconSidebar = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="3" y="4" width="18" height="16" rx="2" /><line x1="9" y1="4" x2="9" y2="20" /></svg>
);

export const IconChat = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" /></svg>
);

export const IconSettings = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" /><circle cx="12" cy="12" r="3" /></svg>
);

export const IconAdmin = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg>
);

export const IconSun = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="5" /><line x1="12" y1="1" x2="12" y2="3" /><line x1="12" y1="21" x2="12" y2="23" /><line x1="4.22" y1="4.22" x2="5.64" y2="5.64" /><line x1="18.36" y1="18.36" x2="19.78" y2="19.78" /><line x1="1" y1="12" x2="3" y2="12" /><line x1="21" y1="12" x2="23" y2="12" /><line x1="4.22" y1="19.78" x2="5.64" y2="18.36" /><line x1="18.36" y1="5.64" x2="19.78" y2="4.22" /></svg>
);

export const IconMoon = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" /></svg>
);

export const IconTrash = () => (
  <svg viewBox="0 0 24 24" {...s}><polyline points="3 6 5 6 21 6" /><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" /></svg>
);

export const IconCopy = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="9" y="9" width="13" height="13" rx="2" /><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" /></svg>
);

export const IconCheck = () => (
  <svg viewBox="0 0 24 24" {...s}><polyline points="20 6 9 17 4 12" /></svg>
);

export const IconPlus = () => (
  <svg viewBox="0 0 24 24" {...s}><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
);

export const IconClose = () => (
  <svg viewBox="0 0 24 24" {...s}><line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" /></svg>
);

export const IconSend = () => (
  <svg viewBox="0 0 24 24" {...s}><line x1="12" y1="21" x2="12" y2="4" /><polyline points="4 11 12 3 20 11" /></svg>
);

export const IconSidebarOpen = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="3" y="3" width="18" height="18" rx="2" /><line x1="9" y1="3" x2="9" y2="21" /><polyline points="14 8 17 12 14 16" /></svg>
);

export const IconSidebarClose = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="3" y="3" width="18" height="18" rx="2" /><line x1="9" y1="3" x2="9" y2="21" /><polyline points="16 8 13 12 16 16" /></svg>
);

// (IconFullscreen se define más abajo con un dibujo de brackets de
// las 4 esquinas — la versión vieja con flechas diagonales se removió
// porque ChatPanel solo usa la nueva.)

export const IconCpu = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="4" y="4" width="16" height="16" rx="2" /><rect x="9" y="9" width="6" height="6" /><line x1="9" y1="1" x2="9" y2="4" /><line x1="15" y1="1" x2="15" y2="4" /><line x1="9" y1="20" x2="9" y2="23" /><line x1="15" y1="20" x2="15" y2="23" /><line x1="20" y1="9" x2="23" y2="9" /><line x1="20" y1="14" x2="23" y2="14" /><line x1="1" y1="9" x2="4" y2="9" /><line x1="1" y1="14" x2="4" y2="14" /></svg>
);

export const IconRobot = ({ className }: IconProps = {}) => (
  <svg viewBox="0 0 24 24" className={className} {...s}><rect x="3" y="8" width="18" height="12" rx="2" /><path d="M12 2v6" /><circle cx="12" cy="2" r="1" fill="currentColor" /><circle cx="9" cy="14" r="1" fill="currentColor" /><circle cx="15" cy="14" r="1" fill="currentColor" /><path d="M9 18h6" /></svg>
);

export const IconBuilding = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="4" y="2" width="16" height="20" rx="2" /><line x1="9" y1="6" x2="9" y2="6.01" /><line x1="15" y1="6" x2="15" y2="6.01" /><line x1="9" y1="10" x2="9" y2="10.01" /><line x1="15" y1="10" x2="15" y2="10.01" /><line x1="9" y1="14" x2="9" y2="14.01" /><line x1="15" y1="14" x2="15" y2="14.01" /><path d="M9 22v-4h6v4" /></svg>
);

export const IconDatabase = () => (
  <svg viewBox="0 0 24 24" {...s}><ellipse cx="12" cy="5" rx="9" ry="3" /><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3" /><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" /></svg>
);

/** EP-0024: database dedicado (3 cilindros apilados) — usado para memory plugin. */
export const IconDatabaseStacked = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <ellipse cx="12" cy="5" rx="8" ry="2.5" />
    <path d="M4 5v6c0 1.38 3.58 2.5 8 2.5s8-1.12 8-2.5V5" />
    <path d="M4 11v6c0 1.38 3.58 2.5 8 2.5s8-1.12 8-2.5v-6" />
  </svg>
);

export const IconZap = () => (
  <svg viewBox="0 0 24 24" {...s}><polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" /></svg>
);

export const IconClipboard = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="8" y="2" width="8" height="4" rx="1" /><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" /></svg>
);

export const IconHelpCircle = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="10" /><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" /><line x1="12" y1="17" x2="12.01" y2="17" /></svg>
);

// IconInfo: 'i' in circle (info / metadata)
export const IconInfo = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="10" /><line x1="12" y1="16" x2="12" y2="12" /><line x1="12" y1="8" x2="12.01" y2="8" /></svg>
);

export const IconTerminal = () => (
  <svg viewBox="0 0 24 24" {...s}><polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" /></svg>
);

export const IconFileRead = () => (
  <svg viewBox="0 0 24 24" {...s}><line x1="5" y1="12" x2="19" y2="12" /><polyline points="12 5 19 12 12 19" /></svg>
);

export const IconFileWrite = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 20h9" /><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" /></svg>
);

export const IconFolder = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" /></svg>
);

export const IconStop = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="6" y="6" width="12" height="12" rx="2" /></svg>
);

export const IconFilter = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M3 5h18l-7 9v6l-4-2v-4z" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export const IconMode = ({ mode }: { mode: "plan" | "build" }) => (
  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
    {mode === "plan" ? (
      <>
        <path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z" />
        <path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z" />
      </>
    ) : (
      <>
        <path d="M15 12l-8.5 8.5c-.83.83-2.17.83-3 0a2.12 2.12 0 0 1 0-3L12 9" />
        <path d="M17.64 15L22 10.64" />
        <path d="M20.91 11.7l-1.25-1.25c-.6-.6-.93-1.4-.93-2.25V6.5a.5.5 0 0 0-.5-.5H16.5c-.85 0-1.65-.33-2.24-.93l-1.27-1.27a1 1 0 0 0-1.41 0L9 6.38" />
      </>
    )}
  </svg>
);

export const IconClock = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" /></svg>
);

export const IconHome = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /><polyline points="9 22 9 12 15 12 15 22" /></svg>
);

export const IconWorkspaces = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="3" y="3" width="7" height="7" rx="1" /><rect x="14" y="3" width="7" height="7" rx="1" /><rect x="3" y="14" width="7" height="7" rx="1" /><rect x="14" y="14" width="7" height="7" rx="1" /></svg>
);

export const IconFlows = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="5" cy="6" r="2" /><circle cx="19" cy="6" r="2" /><circle cx="5" cy="18" r="2" /><circle cx="19" cy="18" r="2" /><path d="M7 6h10" /><path d="M7 18h10" /><path d="M5 8v8" /><path d="M19 8v8" /></svg>
);

export const IconProviders = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="9" /><path d="M5 12h14" /><path d="M12 5v14" /></svg>
);

export const IconSave = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" /><polyline points="17 21 17 13 7 13 7 21" /><polyline points="7 3 7 8 15 8" /></svg>
);

export const IconHistory = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="10" /><polyline points="12 6 12 12 16 14" /><path d="M4.93 4.93l2.83 2.83" /></svg>
);

export const IconGlobe = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="10" /><line x1="2" y1="12" x2="22" y2="12" /><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" /></svg>
);

export const IconSparkles = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 2l2.4 7.2L22 12l-7.6 2.8L12 22l-2.4-7.2L2 12l7.6-2.8L12 2z" /></svg>
);

export const IconMonitor = () => (
  <svg viewBox="0 0 24 24" {...s}><rect x="2" y="3" width="20" height="14" rx="2" /><line x1="8" y1="21" x2="16" y2="21" /><line x1="12" y1="17" x2="12" y2="21" /></svg>
);

export const IconLink = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" /><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" /></svg>
);

export const IconSearch = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="11" cy="11" r="8" /><line x1="21" y1="21" x2="16.65" y2="16.65" /></svg>
);

export const IconClickUp = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M2 18.44l3.69-2.83c1.96 2.56 4.04 3.74 6.36 3.74 2.31 0 4.33-1.17 6.2-3.7L22 18.4C19.3 22.07 15.94 24 12.05 24 8.18 24 4.79 22.08 2 18.44z"/><path d="M12.04 6.15L5.47 11.81 2.44 8.29 12.05 0l9.54 8.3-3.05 3.51z"/></svg>
);

export const IconHammer = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M15 12l-8.5 8.5c-.83.83-2.17.83-3 0 0 0 0 0 0 0a2.12 2.12 0 0 1 0-3L12 9" /><path d="M17.64 15L22 10.64" /><path d="M20.91 11.7l-1.25-1.25c-.6-.6-.93-1.4-.93-2.25V6.5a.5.5 0 0 0-.5-.5H16.5a3.17 3.17 0 0 1-2.24-.93l-1.27-1.27a1 1 0 0 0-1.41 0L9 6.38" /></svg>
);

export const IconPlug = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 2v6" /><path d="M9 2v4" /><path d="M15 2v4" /><path d="M6 8h12v3a6 6 0 0 1-12 0z" /><path d="M12 17v5" /></svg>
);

export const IconShield = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" /></svg>
);

export const IconRefresh = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M21 12a9 9 0 1 1-3-6.7" /><polyline points="21 4 21 9 16 9" /></svg>
);

export const IconChevron = ({ className }: IconProps = {}) => (
  <svg viewBox="0 0 24 24" className={className} {...s}><polyline points="6 9 12 15 18 9" /></svg>
);

export const IconPin = () => (
  <svg viewBox="0 0 24 24" {...s}><line x1="12" y1="17" x2="12" y2="22" /><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z" /></svg>
);

export const IconPinFilled = () => (
  <svg viewBox="0 0 24 24" {...s} fill="currentColor"><line x1="12" y1="17" x2="12" y2="22" /><path d="M5 17h14v-1.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V6h1a2 2 0 0 0 0-4H8a2 2 0 0 0 0 4h1v4.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24Z" stroke="none" /></svg>
);

export const IconEdit = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
);

export const IconStar = () => (
  <svg viewBox="0 0 24 24" {...s}><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" /></svg>
);

export const IconStarFilled = () => (
  <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" /></svg>
);

export const IconMoreVertical = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="12" cy="5" r="1" fill="currentColor" stroke="none" /><circle cx="12" cy="19" r="1" fill="currentColor" stroke="none" /></svg>
);

export const IconUpdate = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" /><path d="M3 3v5h5" /><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" /><path d="M16 16h5v5" /></svg>
);

export const IconDefaultAgent = () => (
  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 2a5 5 0 0 1 4.7 3.2A4 4 0 0 1 20 9a4 4 0 0 1-1.3 6.8A5 5 0 0 1 12 20a5 5 0 0 1-6.7-4.2A4 4 0 0 1 4 9a4 4 0 0 1 3.3-3.8A5 5 0 0 1 12 2z"/>
    <path d="M12 2v18"/>
    <path d="M8 6c2 1 4 1 4 1"/>
    <path d="M8 10c2 .5 4 .5 4 .5"/>
    <path d="M8 14c2 .5 4 .5 4 .5"/>
    <path d="M16 6c-2 1-4 1-4 1"/>
    <path d="M16 10c-2 .5-4 .5-4 .5"/>
    <path d="M16 14c-2 .5-4 .5-4 .5"/>
  </svg>
);

export const IconCode = () => (
  <svg viewBox="0 0 24 24" {...s}><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>
);

export const IconUsers = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>
);

export const IconAttach = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48" /></svg>
);

export const IconVoice = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" /><path d="M19 10v2a7 7 0 0 1-14 0v-2" /><line x1="12" y1="19" x2="12" y2="23" /><line x1="8" y1="23" x2="16" y2="23" /></svg>
);

export const IconPlay = () => (
  <svg viewBox="0 0 24 24" {...s}><polygon points="5 3 19 12 5 21 5 3" /></svg>
);

export const IconPause = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <rect x="6" y="5" width="4" height="14" rx="1" />
    <rect x="14" y="5" width="4" height="14" rx="1" />
  </svg>
);

export const IconMic = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" /><path d="M19 10v2a7 7 0 0 1-14 0v-2" /><line x1="12" y1="19" x2="12" y2="23" /><line x1="8" y1="23" x2="16" y2="23" /></svg>
);

export const IconIntegrations = () => (
  <svg viewBox="0 0 24 24" fill="currentColor" width="1em" height="1em">
    <path fillRule="evenodd" clipRule="evenodd" d="M12 1.25C11.3953 1.25 10.8384 1.40029 10.2288 1.65242C9.64008 1.89588 8.95633 2.25471 8.1049 2.70153L6.03739 3.78651C4.99242 4.33487 4.15616 4.77371 3.51047 5.20491C2.84154 5.65164 2.32632 6.12201 1.95112 6.75918C1.57718 7.39421 1.40896 8.08184 1.32829 8.90072C1.24999 9.69558 1.24999 10.6731 1.25 11.9026V12.0974C1.24999 13.3268 1.24999 14.3044 1.32829 15.0993C1.40896 15.9182 1.57718 16.6058 1.95112 17.2408C2.32632 17.878 2.84154 18.3484 3.51047 18.7951C4.15616 19.2263 4.99241 19.6651 6.03737 20.2135L8.10481 21.2984C8.95628 21.7453 9.64006 22.1041 10.2288 22.3476C10.8384 22.5997 11.3953 22.75 12 22.75C12.6047 22.75 13.1616 22.5997 13.7712 22.3476C14.3599 22.1041 15.0437 21.7453 15.8951 21.2985L17.9626 20.2135C19.0076 19.6651 19.8438 19.2263 20.4895 18.7951C21.1585 18.3484 21.6737 17.878 22.0489 17.2408C22.4228 16.6058 22.591 15.9182 22.6717 15.0993C22.75 14.3044 22.75 13.3269 22.75 12.0975V11.9025C22.75 10.6731 22.75 9.69557 22.6717 8.90072C22.591 8.08184 22.4228 7.39421 22.0489 6.75918C21.6737 6.12201 21.1585 5.65164 20.4895 5.20491C19.8438 4.77371 19.0076 4.33487 17.9626 3.7865L15.8951 2.70154C15.0437 2.25472 14.3599 1.89589 13.7712 1.65242C13.1616 1.40029 12.6047 1.25 12 1.25ZM8.7708 4.04608C9.66052 3.57917 10.284 3.2528 10.802 3.03856C11.3062 2.83004 11.6605 2.75 12 2.75C12.3395 2.75 12.6938 2.83004 13.198 3.03856C13.716 3.2528 14.3395 3.57917 15.2292 4.04608L17.2292 5.09563C18.3189 5.66748 19.0845 6.07032 19.6565 6.45232C19.9387 6.64078 20.1604 6.81578 20.3395 6.99174L17.0088 8.65708L8.50895 4.18349L8.7708 4.04608ZM6.94466 5.00439L6.7708 5.09563C5.68111 5.66747 4.91553 6.07032 4.34352 6.45232C4.06131 6.64078 3.83956 6.81578 3.66054 6.99174L12 11.1615L15.3572 9.48289L7.15069 5.16369C7.07096 5.12173 7.00191 5.06743 6.94466 5.00439ZM2.93768 8.30737C2.88718 8.52125 2.84901 8.76413 2.82106 9.04778C2.75084 9.7606 2.75 10.6644 2.75 11.9415V12.0585C2.75 13.3356 2.75084 14.2394 2.82106 14.9522C2.88974 15.6494 3.02022 16.1002 3.24367 16.4797C3.46587 16.857 3.78727 17.1762 4.34352 17.5477C4.91553 17.9297 5.68111 18.3325 6.7708 18.9044L8.7708 19.9539C9.66052 20.4208 10.284 20.7472 10.802 20.9614C10.9656 21.0291 11.1134 21.0832 11.25 21.1255V12.4635L2.93768 8.30737ZM12.75 21.1255C12.8866 21.0832 13.0344 21.0291 13.198 20.9614C13.716 20.7472 14.3395 20.4208 15.2292 19.9539L17.2292 18.9044C18.3189 18.3325 19.0845 17.9297 19.6565 17.5477C20.2127 17.1762 20.5341 16.857 20.7563 16.4797C20.9798 16.1002 21.1103 15.6494 21.1789 14.9522C21.2492 14.2394 21.25 13.3356 21.25 12.0585V11.9415C21.25 10.6644 21.2492 9.7606 21.1789 9.04778C21.151 8.76412 21.1128 8.52125 21.0623 8.30736L17.75 9.96352V13C17.75 13.4142 17.4142 13.75 17 13.75C16.5858 13.75 16.25 13.4142 16.25 13V10.7135L12.75 12.4635V21.1255Z" />
  </svg>
);


// ============================================================
// Iconos custom para el admin del daemon (EP-0002-01)
// No existen en agent-studio — los creamos ad-hoc.
// ============================================================

// IconStatus: heart/activity (status del daemon)
export const IconStatus = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M22 12h-4l-3 9L9 3l-3 9H2" /></svg>
);

// IconLoop: circular loop with arrow (live events recurring stream)
export const IconLoop = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M21 12a9 9 0 1 1-3.5-7.1" /><polyline points="21 3 21 9 15 9" /></svg>
);

// IconTools: wrench (tools)
export const IconTools = () => (
  <svg viewBox="0 0 24 24" {...s}><path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" /></svg>
);

// IconConfig: gear (config)
export const IconConfig = () => (
  <svg viewBox="0 0 24 24" {...s}><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" /></svg>
);

// IconShieldCheck: shield with check (approvals). Reusamos IconShield
// de agent-studio y agregamos el check vía composición si hace falta.
// Por ahora usamos IconShield simple (ya existe en agent-studio).

// EP-0025: fullscreen toggle icons (lucide-style).
export const IconMaximize = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M3 9V5a2 2 0 0 1 2-2h4" />
    <path d="M21 9V5a2 2 0 0 0-2-2h-4" />
    <path d="M3 15v4a2 2 0 0 0 2 2h4" />
    <path d="M21 15v4a2 2 0 0 1-2 2h-4" />
  </svg>
);

export const IconMinimize = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M9 3v4a2 2 0 0 1-2 2H3" />
    <path d="M15 3v4a2 2 0 0 0 2 2h4" />
    <path d="M9 21v-4a2 2 0 0 0-2-2H3" />
    <path d="M15 21v-4a2 2 0 0 1 2-2h4" />
  </svg>
);

export const IconArrow = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M5 3 L5 18 L9 14 L11.5 20 L13.5 19 L11 13 L17 13 Z" />
  </svg>
);

export const IconHand = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M18 11V6a2 2 0 0 0-4 0v5" />
    <path d="M14 10V4a2 2 0 0 0-4 0v6" />
    <path d="M10 10.5V6a2 2 0 0 0-4 0v8" />
    <path d="M18 8a2 2 0 1 1 4 0v6a8 8 0 0 1-8 8h-2c-2.8 0-4.5-.9-5.8-2.6L4 15" />
  </svg>
);

export const IconMinus = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M5 12h14" />
  </svg>
);

export const IconGrid = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <rect x="3" y="3" width="7" height="7" />
    <rect x="14" y="3" width="7" height="7" />
    <rect x="3" y="14" width="7" height="7" />
    <rect x="14" y="14" width="7" height="7" />
  </svg>
);

// IconFullscreen: brackets en las 4 esquinas + flechas hacia afuera.
// Estilo estándar HTML5 fullscreen. Con --active o rotación puede
// mutar a "exit fullscreen" (flechas hacia adentro) si se prefiere,
// pero por simplicidad usamos el mismo dibujo con cambio de color
// via .chat__bar-actions__btn--active.
export const IconFullscreen = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <polyline points="3 8 3 3 8 3" />
    <polyline points="16 3 21 3 21 8" />
    <polyline points="21 16 21 21 16 21" />
    <polyline points="8 21 3 21 3 16" />
  </svg>
);

export const IconPower = () => (
  <svg viewBox="0 0 24 24" {...s}>
    <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
    <line x1="12" y1="2" x2="12" y2="12" />
  </svg>
);

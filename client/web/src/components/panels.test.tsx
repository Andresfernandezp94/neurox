// Tests rápidos de los paneles. EP-0001-02 / EP-0003-04.
// Los paneles ahora leen del store global; los tests inyectan data vía
// un componente Seed que despacha acciones al store.

import { describe, expect, it, afterEach, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { useEffect } from 'react';
import { AgentsPanel } from './AgentsPanel';
import { SessionsPanel } from './SessionsPanel';
import { ApprovalsPanel } from './ApprovalsPanel';
import { ConfigViewer } from './ConfigViewer';
import { StoreProvider, useStore, type StoreAction } from '../store/StoreContext';
import { AuthProvider } from '../hooks/useAuth';
import * as agentsApi from '../api/agents';
import * as sessionsApi from '../api/sessions';

function Seed({ actions }: { actions: StoreAction[] }) {
  const { dispatch } = useStore();
  useEffect(() => {
    for (const a of actions) dispatch(a);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return null;
}

function renderWithStore(ui: React.ReactElement, actions: StoreAction[] = []) {
  return render(
    <AuthProvider>
      <StoreProvider>
        <Seed actions={actions} />
        {ui}
      </StoreProvider>
    </AuthProvider>,
  );
}

describe('AgentsPanel', () => {
  afterEach(() => vi.restoreAllMocks());

  it('renders the empty state when no agents', () => {
    renderWithStore(<AgentsPanel />, [
      { type: 'SNAPSHOT_AGENTS', agents: [] },
    ]);
    expect(screen.getByText(/No agents registered/)).toBeInTheDocument();
  });

  it('renders the agents list from the store', () => {
    renderWithStore(<AgentsPanel />, [
      {
        type: 'SNAPSHOT_AGENTS',
        agents: [
          { id: 'default', kind: 'in_process', status: 'ready' } as never,
          { id: 'claude', kind: 'persistent', command: 'neurox-agent-claude' } as never,
        ],
      },
    ]);
    expect(screen.getAllByText('default').length).toBeGreaterThan(0);
    expect(screen.getByText('claude')).toBeInTheDocument();
  });

  it('shows a toggle for persistent agents', () => {
    renderWithStore(<AgentsPanel />, [
      {
        type: 'SNAPSHOT_AGENTS',
        agents: [{ id: 'foo', kind: 'persistent', command: 'bar' } as never],
      },
    ]);
    // EP-0024: the agent row has a toggle switch (not a Start button).
    expect(screen.getByTestId('agent-toggle-foo')).toBeInTheDocument();
  });

  it('toggling a persistent agent triggers the start API', async () => {
    const startSpy = vi.spyOn(agentsApi, 'startAgent').mockResolvedValue({} as never);
    // El snapshot hace 4 GETs en paralelo; silenciamos los errores con
    // mocks por defecto. Solo nos importa verificar que startAgent se llamó.
    vi.spyOn(agentsApi, 'listAgents').mockResolvedValue({
      persistent: [],
      ephemeral_templates: [],
      running: [],
      in_process: [],
    });
    renderWithStore(<AgentsPanel />, [
      {
        type: 'SNAPSHOT_AGENTS',
        agents: [{ id: 'foo', kind: 'persistent', command: 'bar', status: 'stopped' } as never],
      },
    ]);
    fireEvent.click(screen.getByTestId('agent-toggle-foo'));
    await waitFor(() => {
      expect(startSpy).toHaveBeenCalledWith('foo');
    });
  });
});

describe('SessionsPanel', () => {
  afterEach(() => vi.restoreAllMocks());

  it('renders the empty state when no sessions', async () => {
    vi.spyOn(sessionsApi, 'listSessions').mockResolvedValue({
      sessions: [],
    } as never);
    renderWithStore(<SessionsPanel />, [
      { type: 'SNAPSHOT_SESSIONS', sessions: [] },
    ]);
    // SessionsPanel now uses SessionList variant="expanded" with loading state;
    // wait for the initial listSessions() to resolve and the empty state to render.
    await waitFor(() => {
      expect(screen.getByText(/No sessions/)).toBeInTheDocument();
    });
  });

  it('renders a row per session from the store', async () => {
    vi.spyOn(sessionsApi, 'listSessions').mockResolvedValue({
      sessions: [
        {
          session_id: 'abc-123-def',
          agent_id: 'default',
          started_at: '2026-08-04T00:00:00Z',
          summary: 'hello',
          ended_at: null,
        },
      ],
    } as never);
    renderWithStore(<SessionsPanel />, [
      {
        type: 'SNAPSHOT_SESSIONS',
        sessions: [
          {
            session_id: 'abc-123-def',
            agent_id: 'default',
            started_at: '2026-08-04T00:00:00Z',
            summary: 'hello',
            ended_at: null,
          },
        ],
      },
    ]);
    await waitFor(() => {
      expect(screen.getByText('hello')).toBeInTheDocument();
    });
  });

  it('opens the session detail when clicking Open', async () => {
    const sampleSession = {
      session_id: 'abc-123-def',
      agent_id: 'default',
      started_at: '2026-08-04T00:00:00Z',
      summary: 'test',
      ended_at: null,
    };
    // SessionsPanel refreshes on mount via listSessions(). Return the same session
    // so the store isn't overwritten with an empty list.
    vi.spyOn(sessionsApi, 'listSessions').mockResolvedValue({
      sessions: [sampleSession],
    } as never);
    vi.spyOn(sessionsApi, 'getSessionMessages').mockResolvedValue({
      messages: [
        {
          id: 1,
          session_id: 'abc-123-def',
          role: 'user',
          content: 'hi',
          ts: '2026-08-04T00:00:00Z',
        },
      ],
    } as never);
    renderWithStore(<SessionsPanel />, [
      {
        type: 'SNAPSHOT_SESSIONS',
        sessions: [sampleSession],
      },
    ]);
    await waitFor(() => {
      expect(screen.getByText('test')).toBeInTheDocument();
    });
    fireEvent.click(screen.getByText('test'));
    await waitFor(() => {
      expect(screen.getByText('hi')).toBeInTheDocument();
    });
  });
});

describe('ApprovalsPanel', () => {
  it('shows the empty state when no approvals', () => {
    renderWithStore(<ApprovalsPanel />, [
      { type: 'SNAPSHOT_APPROVALS', approvals: [] },
    ]);
    expect(screen.getByText(/No approvals pending/)).toBeInTheDocument();
  });

  it('renders an approval from the store', () => {
    renderWithStore(<ApprovalsPanel />, [
      {
        type: 'SNAPSHOT_APPROVALS',
        approvals: [
          {
            id: 'apr-1',
            tool: 'shell',
            args: { command: 'rm -rf /' },
            requested_at: '2026-08-04T00:00:00Z',
          },
        ],
      },
    ]);
    expect(screen.getByText('shell')).toBeInTheDocument();
    expect(screen.getByText('Approve')).toBeInTheDocument();
    expect(screen.getByText('Reject')).toBeInTheDocument();
  });
});

// EP-0024: ToolsExplorer absorbido como tab "Tools" dentro de ConfigViewer.

describe('ConfigViewer', () => {
  // EP-0026-UX: el tab "General" se eliminó de ConfigViewer — su contenido
  // vive en el tab "Overview" de StatusPanel. "logs" y "capabilities"
  // también se eliminaron (viven en Status). Verificamos que ninguno
  // de ellos aparece en la barra de tabs.

  it('does not render the General tab anymore', () => {
    renderWithStore(<ConfigViewer />);
    expect(screen.queryByRole('tab', { name: 'General' })).not.toBeInTheDocument();
  });

  it('does not render the Logs tab anymore', () => {
    renderWithStore(<ConfigViewer />);
    expect(screen.queryByRole('tab', { name: 'Logs' })).not.toBeInTheDocument();
  });

  it('does not render the Capabilities tab anymore', () => {
    renderWithStore(<ConfigViewer />);
    expect(screen.queryByRole('tab', { name: 'Capabilities' })).not.toBeInTheDocument();
  });

  it('renders the Providers tab as default', () => {
    renderWithStore(<ConfigViewer />);
    const tab = screen.getByTestId('config-tab-providers');
    expect(tab).toHaveAttribute('aria-selected', 'true');
  });
});

// ─── EP-0020-02: 6 new panels — EP-0024: todos absorbidos como tabs en ConfigViewer ─────
//
// Cada panel fue renombrado a *Tab.tsx y se renderiza dentro de ConfigViewer:
//   LogsPanel    → LogsTab        (tab "logs")
//   PluginsPanel → MCP            (tab "plugins")
//   ToolsPanel   → ToolsPanelTab  (no usado; ToolsExplorerTab es el que se usa como tab "tools")
//   ToolsExplorer → ToolsExplorerTab (tab "tools")
//   ServicesPanel → ServicesTab   (tab "services")
//   SandboxPanel → SandboxTab     (tab "sandbox")
//   EnvPanel     → EnvTab         (tab "env")
//
// Sus tests unitarios fueron removidos; los *Tab.tsx siguen funcionales.

// EP-0024: DefaultAgentStatusPanel absorbido como tab dentro de StatusPanel.
// Su test vive ahora en StatusPanel.test.tsx ("renders default stats on the DefaultAgent tab").

// EP-0024: ModelsPanel absorbido como tab dentro de ProvidersPanel.
// Su componente ahora es ModelsTab.tsx. No se migra test (era casi vacío).

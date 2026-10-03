// Tests del StatusPanel. EP-0001-02 / EP-0003-04.
// EP-0024: cubre la tab DefaultAgent (antes DefaultAgentStatusPanel).
// EP-0026-UX: el contenido de Overview absorbe lo que antes vivía en
// el tab "General" de ConfigViewer. Overview ahora muestra runtime
// status + services.

import { describe, expect, it, afterEach, vi } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { useEffect } from 'react';
import { StatusPanel } from './StatusPanel';
import { StoreProvider, useStore, type StoreAction } from '../store/StoreContext';
import * as services from '../api/services';
import * as health from '../api/health';

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
    <StoreProvider>
      <Seed actions={actions} />
      {ui}
    </StoreProvider>,
  );
}

describe('StatusPanel', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  function mockAllApis() {
    vi.spyOn(services, 'listServices').mockResolvedValue({
      services: [],
      clients: [],
      server_time: '2026-01-01T00:00:00Z',
    });
    vi.spyOn(health, 'getHealth').mockResolvedValue({
      service: 'neurox',
      status: 'ok',
      version: '0.4.0',
      uptime_seconds: 0,
    });
  }

  it('shows the connecting state when no health yet', async () => {
    mockAllApis();
    renderWithStore(<StatusPanel />);

    await waitFor(() => {
      expect(screen.getByText(/Connecting to the daemon/)).toBeInTheDocument();
    });
    expect(screen.getByTestId('overview-content')).toBeInTheDocument();
  });

  it('overview: renders the Services section with counts', async () => {
    vi.spyOn(services, 'listServices').mockResolvedValue({
      services: [
        {
          id: 'neurox',
          name: 'Daemon',
          description: 'Local agent orchestrator',
          kind: 'daemon',
          status: 'ok',
          version: '0.4.0',
          endpoint: 'http://127.0.0.1:7878',
          latency_ms: 0,
          uptime_seconds: 42,
          last_checked_at: '2026-01-01T00:00:00Z',
        },
      ],
      clients: [],
      server_time: '2026-01-01T00:00:00Z',
    });
    vi.spyOn(health, 'getHealth').mockResolvedValue({
      service: 'neurox',
      status: 'ok',
      version: '0.4.0',
      uptime_seconds: 0,
    });

    renderWithStore(<StatusPanel />);

    await waitFor(() => {
      expect(screen.getByText(/Services \(1\)/)).toBeInTheDocument();
    });
    expect(screen.getByText('Daemon')).toBeInTheDocument();

    // DOT-fix: el status bar del Runtime status debe incluir un
    // .badge__dot porque showDot=true por default. El color del
    // dot sigue la variant del badge — aquí "ok" → verde.
    const statusBar = screen.getByTestId("status-bar");
    expect(statusBar).toHaveAttribute("data-status", "ok");
    const dot = statusBar.querySelector(".badge__dot");
    expect(dot).toBeInTheDocument();
    const badge = statusBar.querySelector(".badge");
    expect(badge).toHaveClass("badge--success");
  });

  it('overview: shows empty state when no services reported', async () => {
    mockAllApis();
    renderWithStore(<StatusPanel />);

    await waitFor(() => {
      expect(screen.getByText(/No services reported/)).toBeInTheDocument();
    });
  });

  it('overview: shows error banner when services endpoint fails', async () => {
    vi.spyOn(services, 'listServices').mockRejectedValue(new Error('boom'));
    vi.spyOn(health, 'getHealth').mockRejectedValue(new Error('boom'));

    renderWithStore(<StatusPanel />);

    await waitFor(() => {
      expect(screen.getByText(/boom/)).toBeInTheDocument();
    });
  });

  it('overview: refresh button reloads all data', async () => {
    const listServicesSpy = vi
      .spyOn(services, 'listServices')
      .mockResolvedValue({
        services: [],
        clients: [],
        server_time: '2026-01-01T00:00:00Z',
      });
    vi.spyOn(health, 'getHealth').mockResolvedValue({
      service: 'neurox',
      status: 'ok',
      version: '0.4.0',
      uptime_seconds: 0,
    });

    renderWithStore(<StatusPanel />);

    await waitFor(() => {
      expect(screen.getByTestId('overview-refresh')).toBeInTheDocument();
    });

    fireEvent.click(screen.getByTestId('overview-refresh'));

    await waitFor(() => {
      expect(listServicesSpy.mock.calls.length).toBeGreaterThan(1);
    });
  });
});

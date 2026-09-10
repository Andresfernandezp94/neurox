// EP-0018-04 — Tests for ProvidersPanel local service badge + Start/Stop.

import { describe, expect, it, afterEach, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';

// Mock the API client before importing the component.
vi.mock('../api/llm', async () => {
  const actual = await vi.importActual<typeof import('../api/llm')>('../api/llm');
  return {
    ...actual,
    getProviders: vi.fn(),
    startProvider: vi.fn(),
    stopProvider: vi.fn(),
    setActiveProvider: vi.fn(),
    addProvider: vi.fn(),
    updateProvider: vi.fn(),
    deleteProvider: vi.fn(),
    listModels: vi.fn(),
  };
});

import * as llmApi from '../api/llm';
import { ProvidersPanel } from './ProvidersPanel';
import type { LlmProviderStatus } from '../api/llm';

const remoteProvider: LlmProviderStatus = {
  id: 'anthropic',
  kind: 'anthropic',
  model: 'claude-3-5-haiku-latest',
  base_url: 'https://api.anthropic.com',
  api_key_env: 'ANTHROPIC_API_KEY',
  configured: true,
  active: true,
  service_state: null,
};

const localReady: LlmProviderStatus = {
  id: 'local-llama',
  kind: 'openai_compat',
  model: 'qwen2.5-1.5b-instruct',
  base_url: 'http://127.0.0.1:11435/v1',
  api_key_env: null,
  configured: true,
  active: false,
  service_state: 'ready',
};

const localStopped: LlmProviderStatus = {
  ...localReady,
  id: 'local-ollama',
  service_state: 'stopped',
};

const localFailed: LlmProviderStatus = {
  ...localReady,
  id: 'local-broken',
  service_state: 'failed',
};

beforeEach(() => {
  vi.clearAllMocks();
  // Default: no models to discover (the panel calls listModels on expand).
  vi.mocked(llmApi.listModels).mockResolvedValue([]);
  vi.mocked(llmApi.startProvider).mockResolvedValue({
    id: 'local-ollama',
    service_state: 'starting',
  });
  vi.mocked(llmApi.stopProvider).mockResolvedValue({
    id: 'local-llama',
    service_state: 'stopped',
  });
  vi.mocked(llmApi.setActiveProvider).mockResolvedValue(undefined);
  vi.mocked(llmApi.addProvider).mockResolvedValue(remoteProvider);
  vi.mocked(llmApi.updateProvider).mockResolvedValue(remoteProvider);
  vi.mocked(llmApi.deleteProvider).mockResolvedValue(undefined);
});

afterEach(() => {
  cleanup();
});

function mockGetProviders(responses: LlmProviderStatus[]) {
  // Always return the same payload; the panel refetches will see the same state.
  vi.mocked(llmApi.getProviders).mockResolvedValue({
    providers: responses,
    default_provider: responses[0]?.id ?? '',
    default_model: '',
  });
}

// Local service toggle (start/stop) UI is intentionally hidden in the
// provider card header per operator request. The handlers + endpoint
// remain wired (handleToggleService calls /v1/llm/providers/:id/start|stop)
// and the backend E2E tests cover the integration. These tests
// document the design decision: the UI does NOT expose Start/Stop
// buttons nor an svc-state badge in the provider card header. They
// will need to be revisited when the toggle buttons return to the
// header.
describe('ProvidersPanel — local service toggle is hidden in the UI (EP-0018-04)', () => {
  it('5.1: local provider ready → no Start/Stop buttons, no svc-state badge in header', async () => {
    mockGetProviders([localReady]);
    render(<ProvidersPanel />);
    // 'local-llama' is rendered as the raw provider id (no displayNameFor).
    await screen.findByText('local-llama');
    expect(screen.queryByRole('button', { name: /start local service/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /stop local service/i })).toBeNull();
    expect(screen.queryByTestId('svc-state-ready')).toBeNull();
    expect(screen.queryByTestId('svc-state-stopped')).toBeNull();
    expect(screen.queryByTestId('svc-state-failed')).toBeNull();
  });

  it('5.2: local provider stopped → no Start/Stop buttons, no svc-state badge in header', async () => {
    mockGetProviders([localStopped]);
    render(<ProvidersPanel />);
    await screen.findByText('local-ollama');
    expect(screen.queryByRole('button', { name: /start local service/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /stop local service/i })).toBeNull();
    expect(screen.queryByTestId('svc-state-ready')).toBeNull();
    expect(screen.queryByTestId('svc-state-stopped')).toBeNull();
    expect(screen.queryByTestId('svc-state-failed')).toBeNull();
  });

  it('5.3: local provider failed → no Start/Stop buttons, no svc-state badge in header', async () => {
    mockGetProviders([localFailed]);
    render(<ProvidersPanel />);
    await screen.findByText('local-broken');
    expect(screen.queryByRole('button', { name: /start local service/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /stop local service/i })).toBeNull();
    expect(screen.queryByTestId('svc-state-ready')).toBeNull();
    expect(screen.queryByTestId('svc-state-stopped')).toBeNull();
    expect(screen.queryByTestId('svc-state-failed')).toBeNull();
  });

  it('5.4: remote provider (service_state null) → no Start/Stop buttons, no svc-state badge', async () => {
    mockGetProviders([remoteProvider]);
    render(<ProvidersPanel />);
    await screen.findByText('anthropic');
    expect(screen.queryByTestId(/^svc-state-/)).toBeNull();
    expect(screen.queryByRole('button', { name: /start local service/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /stop local service/i })).toBeNull();
  });

  it('5.5a: clicking the provider card never invokes startProvider from the UI', async () => {
    mockGetProviders([localStopped]);
    render(<ProvidersPanel />);
    await screen.findByText('local-ollama');
    // Click the card header (the role="button" wrapper). The card has no
    // Start/Stop buttons; clicking anywhere on the card just toggles
    // the expanded state of the inline config form.
    const card = document.querySelector('.provider-card');
    expect(card).not.toBeNull();
    fireEvent.click(card!);
    await waitFor(() => {
      expect(llmApi.startProvider).not.toHaveBeenCalled();
      expect(llmApi.stopProvider).not.toHaveBeenCalled();
    });
  });

  it('5.5b: clicking the provider card never invokes stopProvider from the UI', async () => {
    mockGetProviders([localReady]);
    render(<ProvidersPanel />);
    // 'local-llama' is rendered as the raw provider id; match the id.
    await screen.findByText('local-llama');
    const card = document.querySelector('.provider-card');
    expect(card).not.toBeNull();
    fireEvent.click(card!);
    await waitFor(() => {
      expect(llmApi.startProvider).not.toHaveBeenCalled();
      expect(llmApi.stopProvider).not.toHaveBeenCalled();
    });
  });
});
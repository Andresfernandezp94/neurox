// Tests para los wrappers de API tipados. EP-0001-02.

import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
import { listAgents, getAgent, startAgent, stopAgent } from './agents';
import { listSessions, createSession, getSessionMessages, cancelSession, sendMessage } from './sessions';
import { listApprovals, respondApproval } from './approvals';
import { listTools } from './tools';
import { getHealth } from './health';

describe('api/agents', () => {
  beforeEach(() => {
    globalThis.fetch = vi.fn();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('listAgents hits /v1/agents', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await listAgents();
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/v1/agents',
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('getAgent encodes the id', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await getAgent('default/with/slash');
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/v1/agents/default%2Fwith%2Fslash',
      expect.anything(),
    );
  });

  it('startAgent / stopAgent POST to /v1/agents/:id/start|stop', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await startAgent('default');
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await stopAgent('default');
    expect(globalThis.fetch).toHaveBeenNthCalledWith(
      1,
      '/v1/agents/default/start',
      expect.objectContaining({ method: 'POST' }),
    );
    expect(globalThis.fetch).toHaveBeenNthCalledWith(
      2,
      '/v1/agents/default/stop',
      expect.objectContaining({ method: 'POST' }),
    );
  });
});

describe('api/sessions', () => {
  beforeEach(() => {
    globalThis.fetch = vi.fn();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('listSessions hits /v1/sessions with client_id filter', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await listSessions();
    const call = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0]!;
    expect(call[0]).toMatch(/^\/v1\/sessions\?client_id=web-[a-z0-9]+$/);
  });

  it('createSession sends agent_id + client_id in body', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await createSession('default');
    const call = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0]!;
    expect(call[0]).toBe('/v1/sessions');
    const body = JSON.parse(call[1].body);
    expect(body.agent_id).toBe('default');
    // D3 / partitioning: client_id is auto-injected so the daemon
    // can partition this session from sidebar sessions.
    expect(typeof body.client_id).toBe('string');
    expect(body.client_id).toMatch(/^web-[a-z0-9]+$/);
  });

  it('getSessionMessages hits /v1/sessions/:id/messages', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await getSessionMessages('abc-123');
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/v1/sessions/abc-123/messages',
      expect.anything(),
    );
  });

  it('cancelSession POSTs to /v1/sessions/:id/cancel', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await cancelSession('abc');
    expect(globalThis.fetch).toHaveBeenCalledWith(
      '/v1/sessions/abc/cancel',
      expect.objectContaining({ method: 'POST' }),
    );
  });

  it('sendMessage POSTs agent_id + text + provider_id + model + client_id', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await sendMessage('abc', 'default', 'hello', 'minimax', 'MiniMax-M3');
    const call = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0]!;
    const body = JSON.parse(call[1].body);
    expect(body.agent_id).toBe('default');
    expect(body.text).toBe('hello');
    expect(body.provider_id).toBe('minimax');
    expect(body.model).toBe('MiniMax-M3');
    // client_id is auto-injected for partitioning.
    expect(typeof body.client_id).toBe('string');
    expect(body.client_id).toMatch(/^web-[a-z0-9]+$/);
  });
});

describe('api/approvals', () => {
  beforeEach(() => {
    globalThis.fetch = vi.fn();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('listApprovals hits /v1/approvals', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await listApprovals();
    expect(globalThis.fetch).toHaveBeenCalledWith('/v1/approvals', expect.anything());
  });

  it('respondApproval POSTs the approve boolean', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await respondApproval('apr-1', "approve");
    const call = (globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls[0]!;
    expect(JSON.parse(call[1].body)).toEqual({ decision: "approve" });
  });
});

describe('api/tools and api/health', () => {
  beforeEach(() => {
    globalThis.fetch = vi.fn();
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('listTools hits /v1/tools', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await listTools();
    expect(globalThis.fetch).toHaveBeenCalledWith('/v1/tools', expect.anything());
  });

  it('getHealth hits /health', async () => {
    (globalThis.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce(
      new Response('{}', { status: 200 }),
    );
    await getHealth();
    expect(globalThis.fetch).toHaveBeenCalledWith('/health', expect.anything());
  });
});

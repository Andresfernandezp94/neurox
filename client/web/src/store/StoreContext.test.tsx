// Tests del reducer del StoreContext. EP-0003-03.

import { describe, expect, it } from 'vitest';
import { initialState, reducer } from './StoreContext';
import type { Agent, Approval, SessionSummary } from '../types';

describe('StoreContext/reducer', () => {
  it('returns initial state on unknown action', () => {
    // @ts-expect-error — testing unknown action
    const next = reducer(initialState, { type: 'UNKNOWN' });
    expect(next).toBe(initialState);
  });

  describe('SNAPSHOT actions', () => {
    it('SNAPSHOT_AGENTS populates the Map and marks loaded.agents', () => {
      const agents: Agent[] = [
        { id: 'default', kind: 'in_process' },
        { id: 'claude', kind: 'persistent' },
      ];
      const next = reducer(initialState, { type: 'SNAPSHOT_AGENTS', agents });
      expect(next.agents.size).toBe(2);
      expect(next.agents.get('default')).toEqual({ id: 'default', kind: 'in_process' });
      expect(next.loaded.agents).toBe(true);
    });

    it('SNAPSHOT_SESSIONS populates sessions and marks loaded.sessions', () => {
      const sessions: SessionSummary[] = [
        { session_id: 's1', agent_id: 'default', started_at: '2026-08-06T00:00:00Z' },
      ];
      const next = reducer(initialState, { type: 'SNAPSHOT_SESSIONS', sessions });
      expect(next.sessions.size).toBe(1);
      expect(next.sessions.get('s1')?.agent_id).toBe('default');
      expect(next.loaded.sessions).toBe(true);
    });

    it('SNAPSHOT_APPROVALS populates approvals', () => {
      const approvals: Approval[] = [{ id: 'a1', tool: 'shell' }];
      const next = reducer(initialState, { type: 'SNAPSHOT_APPROVALS', approvals });
      expect(next.approvals.size).toBe(1);
      expect(next.approvals.get('a1')?.tool).toBe('shell');
      expect(next.loaded.approvals).toBe(true);
    });

    it('SNAPSHOT_HEALTH sets health and marks loaded.health', () => {
      const next = reducer(initialState, {
        type: 'SNAPSHOT_HEALTH',
        health: { service: 'neurox', status: 'ok', version: '0.4.0' },
      });
      expect(next.connection.health).toEqual({ service: 'neurox', status: 'ok', version: '0.4.0' });
      expect(next.loaded.health).toBe(true);
    });
  });

  describe('Connection actions', () => {
    it('WS_STATUS_CHANGED updates connection.ws', () => {
      const next = reducer(initialState, { type: 'WS_STATUS_CHANGED', status: 'open' });
      expect(next.connection.ws).toBe('open');
    });

    it('HEARTBEAT_TICK updates latency/lastPongAt/isZombie', () => {
      const ts = 1_700_000_000_000;
      const next = reducer(initialState, {
        type: 'HEARTBEAT_TICK',
        latencyMs: 42,
        lastPongAt: ts,
        isZombie: false,
      });
      expect(next.connection.latencyMs).toBe(42);
      expect(next.connection.lastPongAt).toBe(ts);
      expect(next.connection.isZombie).toBe(false);
    });

    it('HTTP_RETRY_RECORDED updates retry counters', () => {
      const ts = 1_700_000_000_000;
      const next = reducer(initialState, {
        type: 'HTTP_RETRY_RECORDED',
        retriesTotal: 3,
        lastRetryAt: ts,
      });
      expect(next.connection.retriesTotal).toBe(3);
      expect(next.connection.lastRetryAt).toBe(ts);
    });
  });

  describe('Local updates', () => {
    it('AGENT_LOCAL_UPDATE merges patch into existing agent', () => {
      const s1 = reducer(initialState, {
        type: 'SNAPSHOT_AGENTS',
        agents: [{ id: 'default', kind: 'in_process', status: 'idle' }],
      });
      const s2 = reducer(s1, {
        type: 'AGENT_LOCAL_UPDATE',
        id: 'default',
        patch: { status: 'running' },
      });
      expect(s2.agents.get('default')?.status).toBe('running');
      expect(s2.agents.get('default')?.kind).toBe('in_process');
    });

    it('AGENT_LOCAL_UPDATE noop if agent not in map', () => {
      const next = reducer(initialState, {
        type: 'AGENT_LOCAL_UPDATE',
        id: 'default',
        patch: { status: 'running' },
      });
      expect(next).toBe(initialState);
    });

    it('AGENT_LOCAL_REMOVE removes from map', () => {
      const s1 = reducer(initialState, {
        type: 'SNAPSHOT_AGENTS',
        agents: [{ id: 'default', kind: 'in_process' }],
      });
      const s2 = reducer(s1, { type: 'AGENT_LOCAL_REMOVE', id: 'default' });
      expect(s2.agents.size).toBe(0);
    });

    it('APPROVAL_LOCAL_REMOVE removes from map', () => {
      const s1 = reducer(initialState, {
        type: 'SNAPSHOT_APPROVALS',
        approvals: [{ id: 'a1', tool: 'shell' }],
      });
      const s2 = reducer(s1, { type: 'APPROVAL_LOCAL_REMOVE', id: 'a1' });
      expect(s2.approvals.size).toBe(0);
    });
  });

  describe('EVENT_RECEIVED', () => {
    it('agent_spawned adds the ephemeral agent', () => {
      const next = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: { type: 'agent_spawned', ephemeral_id: 'tmp-1', agent_id: 'claude' },
      });
      expect(next.agents.get('tmp-1')?.status).toBe('running');
      expect(next.agents.get('tmp-1')?.kind).toBe('ephemeral');
    });

    it('agent_finished updates status of existing agent', () => {
      const s1 = reducer(initialState, {
        type: 'SNAPSHOT_AGENTS',
        agents: [{ id: 'tmp-1', kind: 'ephemeral', status: 'running' }],
      });
      const s2 = reducer(s1, {
        type: 'EVENT_RECEIVED',
        event: { type: 'agent_finished', ephemeral_id: 'tmp-1', status: 'ok' },
      });
      expect(s2.agents.get('tmp-1')?.status).toBe('ok');
    });

    it('session_started adds session to map', () => {
      const next = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: { type: 'session_started', session_id: 's-1', agent_id: 'default' },
      });
      expect(next.sessions.get('s-1')?.agent_id).toBe('default');
      expect(next.sessions.get('s-1')?.ended_at).toBeNull();
    });

    it('session_ended sets ended_at and summary', () => {
      const s1 = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: { type: 'session_started', session_id: 's-1', agent_id: 'default' },
      });
      const s2 = reducer(s1, {
        type: 'EVENT_RECEIVED',
        event: { type: 'session_ended', session_id: 's-1', summary: 'done' },
      });
      expect(s2.sessions.get('s-1')?.ended_at).not.toBeNull();
      expect(s2.sessions.get('s-1')?.summary).toBe('done');
    });

    // EP-2026-08-15: el daemon hace soft-delete vía SessionEnded con
    // summary="deleted". El cliente debe removerla del Map en lugar
    // de solo marcarla ended, si no la UI la sigue mostrando en el
    // filtro "closed" o el SessionList del panel Sessions.
    it('session_ended with summary="deleted" REMOVES the session from the map', () => {
      const s1 = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: { type: 'session_started', session_id: 's-1', agent_id: 'default' },
      });
      expect(s1.sessions.size).toBe(1);
      const s2 = reducer(s1, {
        type: 'EVENT_RECEIVED',
        event: { type: 'session_ended', session_id: 's-1', summary: 'deleted' },
      });
      expect(s2.sessions.size).toBe(0);
      expect(s2.sessions.has('s-1')).toBe(false);
    });

    it('SNAPSHOT_SESSIONS drops sessions with summary="deleted"', () => {
      const next = reducer(initialState, {
        type: 'SNAPSHOT_SESSIONS',
        sessions: [
          { session_id: 's-1', agent_id: 'a', started_at: 't', ended_at: null, summary: null },
          { session_id: 's-2', agent_id: 'a', started_at: 't', ended_at: 't2', summary: 'deleted' as unknown as null },
          { session_id: 's-3', agent_id: 'a', started_at: 't', ended_at: null, summary: null },
        ],
      });
      expect(next.sessions.size).toBe(2);
      expect(next.sessions.has('s-1')).toBe(true);
      expect(next.sessions.has('s-2')).toBe(false);
      expect(next.sessions.has('s-3')).toBe(true);
    });

    it('approval_request adds to map', () => {
      const next = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: {
          type: 'approval_request',
          request: { id: 'a-1', tool: 'shell', args: { cmd: 'ls' } },
        },
      });
      expect(next.approvals.get('a-1')?.tool).toBe('shell');
    });

    it('approval_resolved removes from map', () => {
      const s1 = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: { type: 'approval_request', request: { id: 'a-1', tool: 'shell' } },
      });
      const s2 = reducer(s1, {
        type: 'EVENT_RECEIVED',
        event: { type: 'approval_resolved', approval_id: 'a-1', decision: 'approve' },
      });
      expect(s2.approvals.size).toBe(0);
    });

    it('ignores events with no matching type', () => {
      const next = reducer(initialState, {
        type: 'EVENT_RECEIVED',
        event: { type: 'thinking', text: '...' },
      });
      expect(next).toBe(initialState);
    });
  });

  it('returns a NEW state object (inmutabilidad)', () => {
    const next = reducer(initialState, { type: 'WS_STATUS_CHANGED', status: 'open' });
    expect(next).not.toBe(initialState);
    // El top-level es nuevo; los Maps que no cambiaron mantienen la misma
    // referencia (es la convención de useReducer + Map).
    expect(next.connection).not.toBe(initialState.connection);
  });
});

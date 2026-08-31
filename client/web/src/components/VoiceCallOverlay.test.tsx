// Tests del VoiceCallOverlay. EP-0002.
//
// Cubrimos render básico, portal, cierre con Esc, cierre con click en
// backdrop, y exposición del state del hook en el atributo data-state.

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { VoiceCallOverlay } from './VoiceCallOverlay';

// ─── Mock del hook ──────────────────────────────────────────────────────────

const mockStop = vi.fn();
const mockToggle = vi.fn();

const defaultVoiceReturn = {
  state: 'idle',
  level: 0,
  partialTranscript: '',
  lastError: null as string | null,
  start: vi.fn(),
  stop: mockStop,
  toggle: mockToggle,
  sttSupported: true,
};

const mockUseVoiceCall = vi.fn(
  (_opts: unknown): typeof defaultVoiceReturn => ({ ...defaultVoiceReturn }),
);

vi.mock('../hooks/useVoiceCall', () => ({
  useVoiceCall: (opts: unknown) => mockUseVoiceCall(opts),
}));

beforeEach(() => {
  mockStop.mockClear();
  mockUseVoiceCall.mockClear();
  mockUseVoiceCall.mockReturnValue({ ...defaultVoiceReturn });
});

// ─── Tests ──────────────────────────────────────────────────────────────────

describe('VoiceCallOverlay', () => {
  it('renders nothing when closed', () => {
    render(<VoiceCallOverlay open={false} sessionId="sess-1" onClose={vi.fn()} />);
    expect(screen.queryByTestId('voice-overlay')).toBeNull();
  });

  it('renders the dialog with state badge when open', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'ready',
      level: 0,
      partialTranscript: '',
      lastError: null,
      start: vi.fn(),
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={vi.fn()} />);
    const overlay = screen.getByTestId('voice-overlay');
    expect(overlay).toBeInTheDocument();
    expect(overlay).toHaveAttribute('role', 'dialog');
    expect(overlay).toHaveAttribute('aria-modal', 'true');
    expect(overlay.querySelector('.voice-call-overlay__state')).toHaveAttribute(
      'data-state',
      'ready',
    );
  });

  it('shows interim transcript when STT is producing text', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'listening',
      level: 0.4,
      partialTranscript: 'hola agen',
      lastError: null,
      start: vi.fn(),
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={vi.fn()} />);
    const transcripts = screen.getByTestId('voice-overlay-transcripts');
    expect(transcripts.textContent).toContain('hola agen');
  });

  it('shows error message when state is error', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'error',
      level: 0,
      partialTranscript: '',
      lastError: 'Mic permission denied',
      start: vi.fn(),
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={vi.fn()} />);
    expect(screen.getByRole('alert')).toHaveTextContent('Mic permission denied');
  });

  it('closes on Esc keypress', () => {
    const onClose = vi.fn();
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={onClose} />);
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls voice.stop() and onClose when End button is clicked', () => {
    const onClose = vi.fn();
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={onClose} />);
    fireEvent.click(screen.getByTestId('voice-overlay-end'));
    expect(mockStop).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('closes when clicking the backdrop (not the panel)', () => {
    const onClose = vi.fn();
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={onClose} />);
    // Click en el overlay root simula click en el backdrop (fuera del panel).
    const overlay = screen.getByTestId('voice-overlay');
    fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does NOT close when clicking inside the panel', () => {
    const onClose = vi.fn();
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={onClose} />);
    // Click en el header (dentro del panel) — no debe disparar onClose.
    const header = document.querySelector('.voice-call-overlay__header')!;
    fireEvent.click(header);
    expect(onClose).not.toHaveBeenCalled();
  });

  it('reflects VU level via aria-valuenow', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'listening',
      level: 0.62,
      partialTranscript: '',
      lastError: null,
      start: vi.fn(),
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<VoiceCallOverlay open sessionId="sess-1" onClose={vi.fn()} />);
    const meter = screen.getByRole('meter');
    expect(meter).toHaveAttribute('aria-valuenow', '62');
  });
});

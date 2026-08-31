// Tests del MicButton. EP-0002.
//
// El botón es thin — delega casi todo al hook `useVoiceCall`. Acá
// verificamos:
//
//   - Render inicial: ícono de mic, no aria-pressed
//   - Disabled cuando sessionId es null
//   - Click → toggle del hook (mockeamos useVoiceCall)
//   - Estados visuales (clases CSS) según el state del hook
//   - Tooltip refleja el state

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MicButton } from './MicButton';

// ─── Mock del hook ──────────────────────────────────────────────────────────

const mockToggle = vi.fn();
const mockStart = vi.fn();
const mockStop = vi.fn();

const defaultVoiceReturn = {
  state: 'idle',
  level: 0,
  partialTranscript: '',
  lastError: null as string | null,
  start: mockStart,
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
  mockToggle.mockClear();
  mockStart.mockClear();
  mockStop.mockClear();
  mockUseVoiceCall.mockClear();
  mockUseVoiceCall.mockReturnValue({ ...defaultVoiceReturn });
});

// ─── Tests ──────────────────────────────────────────────────────────────────

describe('MicButton', () => {
  it('renders a mic button with default tooltip', () => {
    render(<MicButton sessionId="sess-1" />);
    const btn = screen.getByTestId('chat-mic');
    expect(btn).toBeInTheDocument();
    expect(btn).toHaveAttribute('aria-label', 'Dictate with voice');
    expect(btn).toHaveAttribute('aria-pressed', 'false');
    expect(btn).toHaveAttribute('data-state', 'idle');
  });

  it('is disabled when sessionId is null', () => {
    render(<MicButton sessionId={null} />);
    expect(screen.getByTestId('chat-mic')).toBeDisabled();
  });

  it('calls toggle on click', () => {
    render(<MicButton sessionId="sess-1" />);
    fireEvent.click(screen.getByTestId('chat-mic'));
    expect(mockToggle).toHaveBeenCalledTimes(1);
  });

  it('is disabled when STT is not supported', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'idle',
      level: 0,
      partialTranscript: '',
      lastError: null,
      start: mockStart,
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: false,
    });
    render(<MicButton sessionId="sess-1" />);
    const btn = screen.getByTestId('chat-mic');
    expect(btn).toBeDisabled();
    expect(btn).toHaveAttribute('aria-label', expect.stringContaining('not supported'));
  });

  it('shows active state when voice call is in progress', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'listening',
      level: 0.5,
      partialTranscript: 'hola',
      lastError: null,
      start: mockStart,
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<MicButton sessionId="sess-1" />);
    const btn = screen.getByTestId('chat-mic');
    expect(btn).toHaveClass('mic-button--active');
    expect(btn).toHaveClass('mic-button--listening');
    expect(btn).toHaveAttribute('aria-pressed', 'true');
    expect(btn).toHaveAttribute('data-state', 'listening');
  });

  it('shows error state with message in tooltip', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'error',
      level: 0,
      partialTranscript: '',
      lastError: 'Permission denied',
      start: mockStart,
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<MicButton sessionId="sess-1" />);
    const btn = screen.getByTestId('chat-mic');
    expect(btn).toHaveClass('mic-button--error');
    expect(btn.getAttribute('title')).toContain('Permission denied');
  });

  it('forwards onUserTranscript callback to the hook', () => {
    const cb = vi.fn();
    render(<MicButton sessionId="sess-1" onUserTranscript={cb} />);
    // Capturamos las opts pasadas al hook.
    const lastCall = mockUseVoiceCall.mock.calls[mockUseVoiceCall.mock.calls.length - 1]!;
    const opts = lastCall[0] as { onUserTranscript?: (t: string) => void };
    expect(opts.onUserTranscript).toBe(cb);
  });

  it('disables itself while connecting', () => {
    mockUseVoiceCall.mockReturnValue({
      state: 'connecting',
      level: 0,
      partialTranscript: '',
      lastError: null,
      start: mockStart,
      stop: mockStop,
      toggle: mockToggle,
      sttSupported: true,
    });
    render(<MicButton sessionId="sess-1" />);
    expect(screen.getByTestId('chat-mic')).toBeDisabled();
  });
});

// ChatFooter — sección inferior del ChatPanel.
// Contiene: tools bar (model, agent, history/sidebar/voice/fullscreen) +
// context bar (tokens, msgs, skills, facts) + workspace bar (cwd, branch,
// sandbox) + input area (textarea + mic + send/cancel).

import { PanelToggle } from "../shared/components/PanelToggle";
import { ModelSelector, type ModelSelection } from "./ModelSelector";
import { AgentSelector } from "./AgentSelector";
import { MicButton } from "./MicButton";
import {
  IconHistory,
  IconSidebar,
  IconVoice,
  IconFullscreen,
  IconSend,
  IconClose,
} from "../shared/components/Icons";
import type { DefaultAgentResponse } from "../api/default";

export interface ChatFooterProps {
  // tools bar
  sessionId: string | null;
  sessionModel: ModelSelection | null;
  onChangeModel: (m: ModelSelection) => void;
  currentAgent: string;
  onChangeAgent: (id: string) => void;
  showHistory: boolean;
  onToggleHistory: () => void;
  onToggleSidebar?: () => void;
  isSidebarHidden?: boolean;
  voiceOverlayOpen: boolean;
  onOpenVoiceCall: () => void;
  isFullscreen: boolean;
  onToggleFullscreen: () => void;

  // context bar
  defaultAgent: DefaultAgentResponse | null;

  // textarea
  input: string;
  onInputChange: (v: string) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  onMicTranscript: (text: string) => void;
  isStreaming: boolean;
  onSend: () => void;
  onCancel: () => void;
  inputRef: React.RefObject<HTMLTextAreaElement | null>;
}

export function ChatFooter({
  sessionId,
  sessionModel,
  onChangeModel,
  currentAgent,
  onChangeAgent,
  showHistory,
  onToggleHistory,
  onToggleSidebar,
  isSidebarHidden,
  voiceOverlayOpen,
  onOpenVoiceCall,
  isFullscreen,
  onToggleFullscreen,
  defaultAgent,
  input,
  onInputChange,
  onKeyDown,
  onMicTranscript,
  isStreaming,
  onSend,
  onCancel,
  inputRef,
}: ChatFooterProps) {
  return (
    <div className="chat__footer">
      <div className="chat__footer-body">
        {/* Tools bar */}
        <div className="chat__bar chat__bar--tools">
          {sessionId && (
            <ModelSelector
              sessionId={sessionId}
              currentModel={sessionModel}
              onChange={onChangeModel}
            />
          )}
          <div className="chat__bar-actions">
            <AgentSelector currentAgent={currentAgent} onChange={onChangeAgent} />
            <button
              type="button"
              className={`chat__bar-actions__btn${showHistory ? " chat__bar-actions__btn--active" : ""}`}
              onClick={onToggleHistory}
              title="Toggle history"
              aria-label="Toggle history"
              aria-pressed={showHistory}
              data-testid="chat-history-toggle"
            >
              <IconHistory />
            </button>
            {onToggleSidebar && (
              <button
                type="button"
                className={`chat__bar-actions__btn${!isSidebarHidden ? " chat__bar-actions__btn--active" : ""}`}
                onClick={onToggleSidebar}
                title="Toggle sidebar"
                aria-label="Toggle sidebar"
                aria-pressed={!isSidebarHidden}
                data-testid="chat-sidebar-toggle"
              >
                <IconSidebar />
              </button>
            )}
            <button
              type="button"
              className={`chat__bar-actions__btn${voiceOverlayOpen ? " chat__bar-actions__btn--active" : ""}`}
              onClick={onOpenVoiceCall}
              disabled={!sessionId}
              title="Open voice call"
              aria-label="Open voice call"
              aria-pressed={voiceOverlayOpen}
              data-testid="chat-voice-call-open"
            >
              <IconVoice />
            </button>
            <button
              type="button"
              className={`chat__bar-actions__btn${isFullscreen ? " chat__bar-actions__btn--active" : ""}`}
              onClick={onToggleFullscreen}
              title="Toggle fullscreen"
              aria-label={isFullscreen ? "Exit fullscreen" : "Enter fullscreen"}
              aria-pressed={isFullscreen}
              data-testid="chat-fullscreen-toggle"
            >
              <IconFullscreen />
            </button>
          </div>
        </div>

        {/* Context bar */}
        <PanelToggle id="chat-context-bar">
          <div className="chat__bar chat__bar--context">
            <span className="chat__context-label">CONTEXT:</span>
            <span className="chat__context-tokens">
              {defaultAgent?.context?.tokens_estimated ?? 0} tok
            </span>
            <span className="chat__metrics-sep">·</span>
            <span className="chat__context-msgs">
              {defaultAgent?.context?.messages ?? 0} msgs
            </span>
            {defaultAgent?.context?.needs_compaction && (
              <span className="chat__context-warn">⚠ needs compaction</span>
            )}
            {defaultAgent?.context?.has_summary && (
              <span className="chat__context-compacted">✓ compacted</span>
            )}
            <span className="chat__metrics-sep">·</span>
            <span className="chat__context-meta">
              {defaultAgent?.skills ?? 0} skills · {defaultAgent?.facts ?? 0} facts
            </span>
          </div>
        </PanelToggle>

        {/* Workspace bar */}
        <PanelToggle id="chat-workspace-bar">
          <div
            className="chat__bar chat__bar--workspace"
            data-testid="chat-workspace-bar"
          >
            <span className="chat__context-label">SCOPE:</span>
            <span
              className="chat__context-cwd"
              data-testid="chat-cwd"
              title={defaultAgent?.cwd ?? undefined}
            >
              {defaultAgent?.cwd ?? "cwd?"}
            </span>
            {defaultAgent?.git_branch && (
              <>
                <span className="chat__metrics-sep">·</span>
                <span
                  className="chat__context-branch"
                  data-testid="chat-git-branch"
                  title={`git: ${defaultAgent.git_branch}`}
                >
                  ⎇ {defaultAgent.git_branch}
                </span>
              </>
            )}
            <span className="chat__metrics-sep">·</span>
            <span
              className="chat__context-sandbox"
              data-testid="chat-sandbox-summary"
              data-sandbox-enabled={defaultAgent?.sandbox?.enabled ? "true" : "false"}
            >
              sandbox: {defaultAgent?.sandbox ? "enabled" : "n/a"}
            </span>
          </div>
        </PanelToggle>

        {/* Input */}
        <PanelToggle id="chat-input">
          <div className="chat__footer-input">
            <textarea
              ref={inputRef}
              className="chat__textarea"
              value={input}
              onChange={(e) => onInputChange(e.target.value)}
              onKeyDown={onKeyDown}
              placeholder={sessionId ? "Type a message…" : "Connecting…"}
              disabled={!sessionId || isStreaming}
              data-testid="chat-input"
              rows={1}
            />
            <div className="chat__footer-actions">
              <PanelToggle id="chat-mic">
                <MicButton
                  sessionId={sessionId}
                  onUserTranscript={onMicTranscript}
                />
              </PanelToggle>
              {isStreaming ? (
                <button
                  className="btn btn-primary chat__action"
                  onClick={onCancel}
                  disabled={!sessionId}
                  title="Cancel"
                  aria-label="Cancel"
                  data-testid="chat-cancel"
                >
                  <IconClose />
                </button>
              ) : (
                <button
                  className="btn btn-primary chat__action"
                  onClick={onSend}
                  // EP-2026-09-02: also disable when no model is picked.
                  // The user must explicitly choose a model in the
                  // selector before the chat can send anything —
                  // previously the daemon's default model was
                  // preselected silently, which surprised new users.
                  disabled={!sessionId || !sessionModel || input.trim() === ""}
                  title={
                    !sessionModel
                      ? "Pick a model first"
                      : "Send (Shift+Enter)"
                  }
                  aria-label="Send"
                  data-testid="chat-send"
                >
                  <IconSend />
                </button>
              )}
            </div>
          </div>
        </PanelToggle>
      </div>
    </div>
  );
}

// ChatFooter — sección inferior del ChatPanel.
// Contiene: tools bar (model, agent, history) + input area
// (textarea + send/cancel).
//
// EP-2026-10-03: se sacaron la context bar (tokens/msgs/skills/facts),
// la workspace bar (cwd/branch/sandbox) y el boton de maximizar. El
// footer queda con lo que se usa a diario; el resto de esa info sigue
// disponible en StatusPanel > DefaultAgent.

import { PanelToggle } from "../shared/components/PanelToggle";
import { ModelSelector, type ModelSelection } from "./ModelSelector";
import { AgentSelector } from "./AgentSelector";
import {
  IconHistory,
  IconSend,
  IconClose,
} from "../shared/components/Icons";

export interface ChatFooterProps {
  // tools bar
  sessionId: string | null;
  sessionModel: ModelSelection | null;
  onChangeModel: (m: ModelSelection) => void;
  currentAgent: string;
  onChangeAgent: (id: string) => void;
  showHistory: boolean;
  onToggleHistory: () => void;

  // textarea
  input: string;
  onInputChange: (v: string) => void;
  onKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
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
  input,
  onInputChange,
  onKeyDown,
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
          {/* El selector se monta tambien sin sesion: el modelo elegido
              vive en la tab (`sessionModel`) y se aplica a la sesion en el
              momento de crearla, no antes. */}
          <ModelSelector
            sessionId={sessionId}
            currentModel={sessionModel}
            onChange={onChangeModel}
          />
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
          </div>
        </div>

        {/* Input */}
        <PanelToggle id="chat-input">
          <div className="chat__footer-input">
            <textarea
              ref={inputRef}
              className="chat__textarea"
              value={input}
              onChange={(e) => onInputChange(e.target.value)}
              onKeyDown={onKeyDown}
              // La sesion se crea al mandar el primer mensaje, asi que el
              // textarea NO puede depender de `sessionId`: gatearlo dejaba
              // al usuario sin poder escribir, y sin escribir no habia
              // sesion que crear. Solo el streaming lo deshabilita.
              placeholder="Type a message…"
              disabled={isStreaming}
              data-testid="chat-input"
              rows={1}
            />
            <div className="chat__footer-actions">
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
                  disabled={!sessionModel || input.trim() === ""}
                  title={
                    !sessionModel
                      ? "Pick a model first"
                      : "Send (Enter)"
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

// ChatTabs — barra de tabs arriba del ChatPanel. Estilo navegador de
// Chrome: cada tab es una sesión independiente con el agente.
//
// EP-0024: el usuario puede abrir múltiples chats en paralelo,
// cerrarlos con la × y renombrarlos con doble-click en el título.

import { useState } from "react";
import type { ChatTab } from "../hooks/useChatTabs";

interface ChatTabsProps {
  tabs: ChatTab[];
  activeId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onCreate: () => void;
  onRename: (id: string, title: string) => void;
}

export function ChatTabs({
  tabs,
  activeId,
  onSelect,
  onClose,
  onCreate,
  onRename,
}: ChatTabsProps) {
  return (
    <div className="chat-tabs" role="tablist" aria-label="Chat sessions">
      {tabs.map((t) => (
        <ChatTabItem
          key={t.id}
          tab={t}
          active={t.id === activeId}
          onSelect={() => onSelect(t.id)}
          onClose={() => onClose(t.id)}
          onRename={(newTitle) => onRename(t.id, newTitle)}
        />
      ))}
      <button
        type="button"
        className="chat-tabs__new"
        title="New chat"
        aria-label="New chat"
        data-testid="chat-tab-new"
        onClick={onCreate}
      >
        +
      </button>
    </div>
  );
}

interface ChatTabItemProps {
  tab: ChatTab;
  active: boolean;
  onSelect: () => void;
  onClose: () => void;
  onRename: (newTitle: string) => void;
}

function ChatTabItem({ tab, active, onSelect, onClose, onRename }: ChatTabItemProps) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(tab.title);

  function commit() {
    if (value.trim() && value.trim() !== tab.title) {
      onRename(value);
    } else {
      setValue(tab.title);
    }
    setEditing(false);
  }

  return (
    <div
      className={`chat-tab${active ? " chat-tab--active" : ""}`}
      role="tab"
      aria-selected={active}
      data-testid={`chat-tab-${tab.id}`}
    >
      <button
        type="button"
        className="chat-tab__label"
        onClick={onSelect}
        onDoubleClick={() => {
          setValue(tab.title);
          setEditing(true);
        }}
        title={tab.title}
      >
        {editing ? (
          <input
            type="text"
            autoFocus
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                commit();
              } else if (e.key === "Escape") {
                setValue(tab.title);
                setEditing(false);
              }
            }}
            onClick={(e) => e.stopPropagation()}
            className="chat-tab__rename-input"
            data-testid={`chat-tab-${tab.id}-rename`}
          />
        ) : (
          <span className="chat-tab__title">{tab.title}</span>
        )}
      </button>
      <button
        type="button"
        className="chat-tab__close"
        title="Close chat"
        aria-label="Close chat"
        data-testid={`chat-tab-${tab.id}-close`}
        onClick={(e) => {
          e.stopPropagation();
          onClose();
        }}
      >
        ×
      </button>
    </div>
  );
}
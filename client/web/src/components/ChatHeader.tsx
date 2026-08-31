// ChatHeader — sección superior del ChatPanel.
// Contiene: tabs de sesiones + search bar.

import { PanelToggle } from "../shared/components/PanelToggle";
import { ChatTabs } from "./ChatTabs";
import { ChatSearchBar } from "./chat/ChatSearchBar";
import type { ChatTab } from "../hooks/useChatTabs";

export interface ChatHeaderProps {
  tabs: ChatTab[];
  activeId: string | null;
  activeTab: ChatTab | null | undefined;
  sessionId: string | null;
  onSelectTab: (id: string) => void;
  onCloseTab: (id: string) => void;
  onCreateTab: () => void;
  onRenameTab: (id: string, title: string) => void;
  searchQuery: string;
  searchActive: number;
  searchTotal: number;
  onSearchQueryChange: (q: string) => void;
  onSearchActiveChange: (n: number) => void;
  onSearchClear: () => void;
}

export function ChatHeader({
  tabs,
  activeId,
  activeTab,
  sessionId,
  onSelectTab,
  onCloseTab,
  onCreateTab,
  onRenameTab,
  searchQuery,
  searchActive,
  searchTotal,
  onSearchQueryChange,
  onSearchActiveChange,
  onSearchClear,
}: ChatHeaderProps) {
  return (
    <>
      <PanelToggle id="chat-tabs">
        <ChatTabs
          tabs={tabs}
          activeId={activeId}
          onSelect={onSelectTab}
          onClose={onCloseTab}
          onCreate={onCreateTab}
          onRename={onRenameTab}
        />
      </PanelToggle>

      {activeTab && sessionId && (
        <PanelToggle id="chat-search-bar">
          <div className="chat__bar--search-wrap">
            <ChatSearchBar
              tabId={activeTab.id}
              query={searchQuery}
              active={searchActive}
              total={searchTotal}
              onQueryChange={onSearchQueryChange}
              onActiveChange={onSearchActiveChange}
              onClear={onSearchClear}
            />
          </div>
        </PanelToggle>
      )}
    </>
  );
}

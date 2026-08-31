// ChatSearchBar — search-in-page for the active chat tab.
//
// Sits between the ChatTabs bar and the transcript. Layout (single
// row, agent-studio look):
//
//   [🔍 search... ]   [3/12]   [↑] [↓]   [✕]
//
// - `query` is debounced 150ms before committing to the tab state.
// - `total` and `active` are stored on the tab so closing+opening
//   the same tab keeps the markers consistent (the transcript
//   re-renders them on next render).
// - `Enter` → next match, `Shift+Enter` → prev, `Escape` → clear.
// - The active match is scrolled into view by the transcript
//   (via `data-match-index` on each <mark>); the bar only owns
//   the input + counter + nav buttons.

import { useEffect, useRef, useState } from "react";

interface ChatSearchBarProps {
  tabId: string;
  query: string;
  active: number;
  total: number;
  onQueryChange: (debounced: string) => void;
  onActiveChange: (next: number) => void;
  onClear: () => void;
}

export function ChatSearchBar({
  tabId,
  query,
  active,
  total,
  onQueryChange,
  onActiveChange,
  onClear,
}: ChatSearchBarProps) {
  // Local input state — committed to the tab via onQueryChange after
  // 150ms idle. Keeps the input snappy even on long transcripts.
  const [local, setLocal] = useState(query);
  const inputRef = useRef<HTMLInputElement | null>(null);

  // If the tab is reloaded (session restored from history) and the
  // stored query differs, sync the local input.
  useEffect(() => {
    setLocal(query);
  }, [query, tabId]);

  // Debounce commit.
  useEffect(() => {
    if (local === query) return;
    const t = window.setTimeout(() => {
      onQueryChange(local);
    }, 150);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [local]);

  function handleKey(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (total === 0) return;
      const step = e.shiftKey ? -1 : 1;
      const next = (active + step + total) % total;
      onActiveChange(next);
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (local) {
        setLocal("");
        onQueryChange("");
      } else {
        inputRef.current?.blur();
      }
    }
  }

  const hasQuery = local.trim().length > 0;
  const counterLabel =
    total === 0
      ? hasQuery
        ? "0/0"
        : ""
      : `${active + 1}/${total}`;

  return (
    <div className="chat__bar chat__bar--search" role="search" data-testid="chat-search-bar">
      <span className="chat__search-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="11" cy="11" r="7" />
          <path d="M21 21l-4.3-4.3" />
        </svg>
      </span>
      <input
        ref={inputRef}
        type="text"
        className="chat__search-input"
        placeholder="Search chat…"
        value={local}
        onChange={(e) => setLocal(e.target.value)}
        onKeyDown={handleKey}
        data-testid="chat-search-input"
        aria-label="Search chat"
      />
      {counterLabel && (
        <span
          className={`chat__search-count${total === 0 ? " chat__search-count--empty" : ""}`}
          data-testid="chat-search-count"
        >
          {counterLabel}
        </span>
      )}
      <button
        type="button"
        className="chat__search-nav"
        title="Previous match (Shift+Enter)"
        aria-label="Previous match"
        disabled={total === 0}
        onClick={() => onActiveChange((active - 1 + total) % total)}
        data-testid="chat-search-prev"
      >
        ↑
      </button>
      <button
        type="button"
        className="chat__search-nav"
        title="Next match (Enter)"
        aria-label="Next match"
        disabled={total === 0}
        onClick={() => onActiveChange((active + 1) % total)}
        data-testid="chat-search-next"
      >
        ↓
      </button>
      {hasQuery && (
        <button
          type="button"
          className="chat__search-clear"
          title="Clear (Escape)"
          aria-label="Clear search"
          onClick={onClear}
          data-testid="chat-search-clear"
        >
          ✕
        </button>
      )}
    </div>
  );
}

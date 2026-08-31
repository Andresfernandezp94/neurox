// highlight.tsx — case-insensitive substring highlight.
//
// Splits a string into an array of `string | <mark>` chunks so the
// caller can render the result inside any text container without
// breaking the surrounding React tree (works for plain text, Markdown
// children, etc.). Used by ChatSearchBar to highlight matches in the
// chat transcript.
//
// Pure utility — no React state, no DOM access. Safe to call in
// useMemo and inside render.

import { Fragment, type ReactNode } from "react";

export interface HighlightChunk {
  text: string;
  match: boolean;
}

/**
 * Split `text` on every (case-insensitive) occurrence of `query`.
 * Empty / whitespace-only query returns a single non-matching chunk.
 * Matches are returned as `match: true`, gaps as `match: false`.
 *
 * No regex special-character handling needed — `query` is treated as
 * a literal string. `String.prototype.split` with a string arg
 * already does the right thing for the user's perspective.
 */
export function splitOnMatch(text: string, query: string): HighlightChunk[] {
  if (!query || !query.trim()) {
    return text ? [{ text, match: false }] : [];
  }
  if (!text) return [];
  const needle = query.trim();
  const hayLower = text.toLowerCase();
  const needleLower = needle.toLowerCase();
  const out: HighlightChunk[] = [];
  let cursor = 0;
  while (cursor < text.length) {
    const idx = hayLower.indexOf(needleLower, cursor);
    if (idx === -1) {
      out.push({ text: text.slice(cursor), match: false });
      break;
    }
    if (idx > cursor) {
      out.push({ text: text.slice(cursor, idx), match: false });
    }
    out.push({ text: text.slice(idx, idx + needleLower.length), match: true });
    cursor = idx + needleLower.length;
  }
  return out;
}

/**
 * Count the (case-insensitive) occurrences of `query` in `text`.
 * Non-overlapping — matches are scanned left-to-right.
 */
export function countMatches(text: string, query: string): number {
  if (!text || !query || !query.trim()) return 0;
  const hayLower = text.toLowerCase();
  const needleLower = query.trim().toLowerCase();
  let n = 0;
  let cursor = 0;
  while (cursor < hayLower.length) {
    const idx = hayLower.indexOf(needleLower, cursor);
    if (idx === -1) break;
    n += 1;
    cursor = idx + needleLower.length;
  }
  return n;
}

/**
 * Render the result of `splitOnMatch` as React nodes. Each match
 * chunk becomes a `<mark>` with `data-match-index` so the search
 * bar can scroll the active match into view.
 *
 * The `globalIndex` is the running counter of matches across the
 * whole transcript — non-matching chunks pass `null`, the i-th
 * match chunk gets `globalIndex + i`.
 */
export function renderHighlight(
  chunks: HighlightChunk[],
  globalIndex: number,
  activeIndex: number,
  onMatchClick?: (matchIndex: number) => void,
): { nodes: ReactNode; nextIndex: number } {
  const nodes: ReactNode[] = [];
  let localIndex = globalIndex;
  chunks.forEach((chunk, i) => {
    if (!chunk.match) {
      nodes.push(<Fragment key={i}>{chunk.text}</Fragment>);
      return;
    }
    const mine = localIndex;
    const isActive = mine === activeIndex;
    nodes.push(
      <mark
        key={i}
        className={`chat__search-mark${isActive ? " chat__search-mark--active" : ""}`}
        data-match-index={mine}
        data-testid={isActive ? "chat-search-mark-active" : undefined}
        onClick={onMatchClick ? () => onMatchClick(mine) : undefined}
      >
        {chunk.text}
      </mark>,
    );
    localIndex += 1;
  });
  return { nodes, nextIndex: localIndex };
}

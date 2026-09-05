// Tests for `applyStreamChunk` — the unified per-chunk reducer that
// both the local SSE path (via `useChatStream`) and the remote WS
// path (via `useChatTabs`) call to keep the assistant message in
// sync. This test suite is symmetric: any chunk sequence that works
// for one path must work identically for the other.

import { describe, expect, it } from "vitest";
import type { Message } from "../../../types";
import {
  applyStreamChunk,
  initAccumulator,
  type StreamAccumulator,
} from "./applyChunk";

function emptyAssistant(id = 1): Message {
  return {
    id,
    session_id: "sess",
    role: "assistant",
    content: "",
    ts: "2026-09-05T00:00:00Z",
    timeline: [],
  };
}

/** Apply a sequence of chunks the way the SSE/WS handlers do:
 *  stateful accumulator, fresh message each iteration. */
function applySequence(
  msg: Message,
  chunks: Parameters<typeof applyStreamChunk>[1][],
): { final: Message; finalAcc: StreamAccumulator } {
  let m = msg;
  let acc = initAccumulator();
  for (const c of chunks) {
    const r = applyStreamChunk(m, c, acc);
    m = r.message;
    acc = r.acc;
  }
  return { final: m, finalAcc: acc };
}

describe("applyStreamChunk — basic collapse", () => {
  it("50 content chunks collapse to ONE timeline entry", () => {
    const chunks = Array.from({ length: 50 }, (_, i) => ({
      type: "content" as const,
      text: `chunk ${i}\n`,
    }));
    const { final } = applySequence(emptyAssistant(), chunks);
    expect(final.timeline).toHaveLength(1);
    expect(final.timeline![0]).toMatchObject({ type: "content" });
    expect((final.timeline![0] as { text: string }).text).toBe(
      chunks.map((c) => c.text).join(""),
    );
    expect(final.content).toBe(chunks.map((c) => c.text).join(""));
  });

  it("mixed thinking + content produces two timeline entries (in arrival order)", () => {
    const { final } = applySequence(emptyAssistant(), [
      { type: "thinking", text: "let me think\n" },
      { type: "content", text: "hi" },
      { type: "content", text: " there" },
      { type: "thinking", text: " — actually I'll explain\n" },
      { type: "content", text: "My answer." },
    ]);
    // 2 thinking entries (each appended separately, collapsed within
    // the same kind via `appendToTimeline`) and 2 content entries.
    expect(final.timeline).toHaveLength(4);
    expect(final.timeline![0]).toMatchObject({ type: "thinking" });
    expect(final.timeline![1]).toMatchObject({ type: "content" });
    expect(final.timeline![2]).toMatchObject({ type: "thinking" });
    expect(final.timeline![3]).toMatchObject({ type: "content" });
  });
});

describe("applyStreamChunk — tool_output redirect (the wire-format quirk)", () => {
  it("content chunk immediately after tool_call goes to the tool's result, not state.content", () => {
    const { final } = applySequence(emptyAssistant(), [
      { type: "tool_call", tool: "shell", args: { cmd: "ls" }, iteration: 0 },
      { type: "content", text: "file1.txt\nfile2.txt\n" },
    ]);
    expect(final.content).toBe(""); // the buggy output didn't leak into LLM reply
    expect(final.toolLog).toHaveLength(1);
    expect(final.toolLog![0]!.tool).toBe("shell");
    expect(final.toolLog![0]!.result).toBe("file1.txt\nfile2.txt\n");
    // The reducer redirects the first `content` chunk into the
    // matching tool entry's `result` IN PLACE — so the timeline still
    // has only ONE tool entry, but its `result` field is updated.
    expect(final.timeline).toHaveLength(1);
    expect(final.timeline![0]).toMatchObject({
      type: "tool",
      tool: "shell",
      iteration: 0,
    });
    expect((final.timeline![0] as { result?: string }).result).toBe(
      "file1.txt\nfile2.txt\n",
    );
  });

  it("LLM comment AFTER redirected tool output goes to state.content (not into the tool's result)", () => {
    const { final } = applySequence(emptyAssistant(), [
      { type: "tool_call", tool: "shell", args: {}, iteration: 0 },
      { type: "content", text: "shell stdout\n" },
      { type: "content", text: "I found two files." },
    ]);
    expect(final.content).toBe("I found two files.");
    expect(final.toolLog![0]!.result).toBe("shell stdout\n");
  });

  it("iteration-aware matching: content redirected after shell#1 goes to shell#1, not shell#0", () => {
    const { final } = applySequence(emptyAssistant(), [
      { type: "tool_call", tool: "shell", args: { cmd: "ls" }, iteration: 0 },
      { type: "tool_call", tool: "shell", args: { cmd: "pwd" }, iteration: 1 },
      { type: "content", text: "/home/user\n" },
    ]);
    expect(final.toolLog![0]!.result).toBeUndefined();
    expect(final.toolLog![1]!.result).toBe("/home/user\n");
  });
});

describe("applyStreamChunk — error surfacing", () => {
  it("error chunk returns error in the result; accumulators stay valid", () => {
    const msg = emptyAssistant();
    const acc = initAccumulator();
    const r1 = applyStreamChunk(
      msg,
      { type: "content", text: "partial answer " },
      acc,
    );
    const r2 = applyStreamChunk(
      r1.message,
      { type: "error", message: "shell failed: permission denied" },
      r1.acc,
    );
    expect(r2.error).toBe("shell failed: permission denied");
    // Partial assistant text stays intact — caller decides what to render.
    expect(r2.message.content).toBe("partial answer ");
  });

  it("latest error wins (overwrites previous)", () => {
    const msg = emptyAssistant();
    const r1 = applyStreamChunk(
      msg,
      { type: "error", message: "first" },
      initAccumulator(),
    );
    const r2 = applyStreamChunk(
      r1.message,
      { type: "error", message: "second" },
      r1.acc,
    );
    expect(r2.error).toBe("second");
  });
});

describe("applyStreamChunk — symmetry with SSE path", () => {
  // Property: the result of applying chunks via `applyStreamChunk`
  // matches the result of running them through `streamReducer`
  // directly (which is what the SSE path does internally). This
  // guarantees local SSE and remote WS render the same thing.
  it("matches the streamReducer output for a realistic chunk sequence", async () => {
    const { streamReducer, initStreamState } = await import("./reducer");
    const chunks: Parameters<typeof streamReducer>[1][] = [
      { type: "thinking", text: "I'll search for that.\n" },
      { type: "tool_call", tool: "shell", args: { cmd: "rg foo" }, iteration: 0 },
      { type: "content", text: "no matches\n" },
      { type: "content", text: "Let me try a wider search.\n" },
      { type: "tool_call", tool: "shell", args: { cmd: "rg -i foo" }, iteration: 1 },
      { type: "tool_result", tool: "shell", result: "src/foo.ts:1: foo bar\n", iteration: 1 },
      { type: "content", text: "Found it in src/foo.ts." },
    ];
    // Apply via applyStreamChunk
    const seq = applySequence(emptyAssistant(), chunks);
    // Apply via streamReducer (the SSE path's internal state)
    let s = initStreamState();
    for (const c of chunks) s = streamReducer(s, c);
    expect(seq.final.content).toBe(s.content);
    expect(seq.final.thinking).toBe(s.thinking);
    expect(seq.final.timeline).toEqual(s.timeline);
    expect(seq.final.toolLog).toEqual(s.toolLog);
  });
});

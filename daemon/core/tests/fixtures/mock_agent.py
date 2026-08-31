#!/usr/bin/env python3
"""Mock agent for session_agents E2E tests.

Speaks JSON-RPC 2.0 over stdin/stdout (one JSON object per line). Each
request is independent — the daemon spawns one of these per chat session
and the daemon's `JsonRpcStdio` sends single line requests and reads
single line responses.

Methods:
  "process"      → sleeps `--init-sleep-ms`, then returns
                   {text: "ok from <pid>", pid: <pid>, session_id: <sid>}
  "tool_result"  → same as "process"
  "ping"         → returns {pid: <pid>, ok: true} immediately

Args:
  --init-sleep-ms N   sleep this long before each "process"/"tool_result" reply
  --session NAME      label for stderr logging only

Notes:
  - Stderr is used for logging — it's inherited by the daemon (so test
    output is mixed in, but the daemon's stdout parser only reads
    stdout, so stderr is safe).
  - The mock sends EXACTLY one response per request (no streaming
    notifications). The daemon's `JsonRpcStdio.stream` accepts that —
    it just returns the first response with an `id` field.
"""

import argparse
import json
import os
import sys
import time


def emit_notification(method: str, text: str) -> None:
    """Write a JSON-RPC notification (no `id`). The daemon's
    JsonRpcStdio.stream() reads these and forwards them as
    Event::Content / Event::Thinking."""
    notif = {
        "jsonrpc": "2.0",
        "method": method,
        "params": {"text": text},
    }
    print(json.dumps(notif), flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--init-sleep-ms", type=int, default=0)
    parser.add_argument("--session", default="default")
    parser.add_argument("--stream-chunks", type=int, default=0,
                        help="emit N content_delta notifications before the final response")
    parser.add_argument("--thinking-chunks", type=int, default=0,
                        help="emit N thinking_delta notifications before the final response")
    parser.add_argument("--tool-call", type=str, default="",
                        help="if set, the first process response returns a tool_call with this name; "
                             "the daemon will execute it and send a tool_result back")
    args = parser.parse_args()

    pid = os.getpid()
    init_sleep = args.init_sleep_ms / 1000.0

    print(
        f"[mock_agent pid={pid} session={args.session}] started "
        f"(init_sleep_ms={args.init_sleep_ms}, "
        f"stream_chunks={args.stream_chunks}, "
        f"thinking_chunks={args.thinking_chunks}, "
        f"tool_call={args.tool_call!r})",
        file=sys.stderr,
        flush=True,
    )

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            err = {
                "jsonrpc": "2.0",
                "id": None,
                "error": {"code": -32700, "message": f"parse: {e}"},
            }
            print(json.dumps(err), flush=True)
            continue

        req_id = req.get("id")
        method = req.get("method", "")
        params = req.get("params", {})

        # Session id is wrapped inside params as {session_id, params: ...}
        # by the daemon's JsonRpcStdio stream method.
        session_id = "default"
        if isinstance(params, dict):
            session_id = params.get("session_id", "default")

        if method == "ping":
            result = {"pid": pid, "ok": True}
        elif method == "process":
            if init_sleep > 0:
                time.sleep(init_sleep)

            # Thinking chunks first (matches real agent ordering).
            for i in range(args.thinking_chunks):
                emit_notification("thinking_delta", f"think-{i}")

            # Then content chunks.
            for i in range(args.stream_chunks):
                emit_notification("content_delta", f"chunk-{i}")

            if args.tool_call:
                # Hand the daemon a tool_call so it executes the tool
                # locally via ToolRegistry and re-dispatches with the
                # result. The flat {id, name, args} shape matches
                # `crate::tools::ToolCall`.
                result = {
                    "tool_calls": [
                        {
                            "id": f"call-{pid}-1",
                            "name": args.tool_call,
                            "args": {"from": pid},
                        }
                    ],
                    "pid": pid,
                    "session_id": session_id,
                }
            else:
                result = {
                    "text": f"ok from {pid}",
                    "pid": pid,
                    "session_id": session_id,
                    "slept_ms": args.init_sleep_ms,
                }
        elif method == "tool_result":
            # Daemon sends us back the tool's output. Emit the final
            # text (optionally as a content chunk) and return.
            if init_sleep > 0:
                time.sleep(init_sleep)
            # When we initiated a tool_call, the round-trip path
            # always emits at least one chunk so the daemon can
            # forward it as Event::Content through the WS.
            if args.tool_call:
                emit_notification("content_delta", "final-chunk-0")
            for i in range(args.stream_chunks):
                emit_notification("content_delta", f"final-chunk-{i + 1}")
            result = {
                "text": f"final from {pid}",
                "pid": pid,
                "session_id": session_id,
            }
        else:
            err = {
                "jsonrpc": "2.0",
                "id": req_id,
                "error": {"code": -32601, "message": f"unknown method: {method}"},
            }
            print(json.dumps(err), flush=True)
            continue

        resp = {"jsonrpc": "2.0", "id": req_id, "result": result}
        print(json.dumps(resp), flush=True)

    print(f"[mock_agent pid={pid}] stdin EOF — exiting", file=sys.stderr, flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())

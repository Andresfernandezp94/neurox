# Agent Template

> **Use this template to bootstrap a new neurox agent.** The first agent
> (`agent`) is built from it. Subsequent agents should be `cp -r` and
> renamed — see the bootstrap recipe below.

## What's inside

```
_template/
├── Cargo.toml              ← crate manifest (rename me)
├── README.md               ← you are here
└── src/
    ├── main.rs             ← JSON-RPC scaffolding (ping + reset_session work out of the box)
    ├── identity.rs         ← STUB — copy from agent
    ├── llm.rs              ← STUB — copy from agent
    ├── memory.rs           ← STUB — copy from agent
    ├── mode.rs             ← STUB (defaults to Build)
    ├── skills.rs           ← STUB — copy from agent
    └── tools_filter.rs     ← STUB (pass-through, no filter)
```

## Bootstrap recipe (5 steps)

```bash
# 0. From the daemon repo root.

# 1. Copy the template to a new folder.
cp -r agents/_template agents/my_agent

# 2. Rename the crate inside the new folder.
sed -i 's/agent-template/my-agent/g; s/agent_template/my_agent/g' \
    agents/my_agent/Cargo.toml

# 3. Register the crate in the workspace.
#    Edit daemon/Cargo.toml:
#       members = ["core", "agents/agent", "agents/my_agent", "clients/tui"]

# 4. Register the agent in the daemon's default specs.
#    Edit daemon/core/src/config/mod.rs::default_agent_specs():
#       AgentSpec {
#           id: "my_agent".into(),
#           command: "my-agent".into(),
#           ...
#       }

# 5. Fill in the stubs:
$EDITOR agents/my_agent/src/{identity,llm,memory,mode,skills,tools_filter}.rs

# 6. Wire the modules into handle_process() and handle_tool_result()
#    in agents/my_agent/src/main.rs. Use `agent/src/main.rs` as a
#    complete reference (the full implementation is ~530 lines).

# 7. Build & test.
cargo check -p my-agent
cargo test  -p my-agent
```

## What's already wired (works out of the box)

- ✅ JSON-RPC 2.0 over stdin/stdout
- ✅ `ping` method — returns `{"pong": true, "session_id": ..., "agent": "agent-template"}`
- ✅ `reset_session` method — bumps the session counter
- ✅ Graceful parse errors with `-32700` JSON-RPC code
- ✅ Structured logging via `tracing` (stderr, configurable via `RUST_LOG`)

## What you need to wire

The `process` and `tool_result` methods are intentionally unimplemented.
They return `"not implemented yet — see TODO in src/main.rs"` so the
daemon gets a clean error instead of hanging.

To make this a real agent, implement `handle_process()` and
`handle_tool_result()` in `src/main.rs` following the recipe in the
`TODO` markers. The minimal flow is:

```
process(user_text):
    1. identity = load_identity(NEUROX_IDENTITY_DIR)
    2. mode     = detect_mode(user_text)
    3. skills   = match_skills(load_skills(...), user_text, top_n=3)
    4. tools    = filter_tools(all_tools, user_text, mode)
    5. memory.add_message("user", user_text)
    6. prompt   = build_system_prompt(identity, ..., mode.instruction())
    7. result   = llm.chat_stream(memory.get_messages_for_llm(), tools, stdout)
    8. if result.tool_calls:
         memory.add_raw_message(assistant_with_tool_calls)
         return {text, tool_call, tool_calls}
       else:
         memory.add_message("assistant", result.text)
         return {text, tokens_out, streamed: true}

tool_result(call_id, result_text):
    1. memory.add_raw_message({role: "tool", tool_call_id: call_id, content: result_text})
    2. reuse handle_process internals (filtered_tools = all_tools, mode = Build)
```

## How the daemon finds your agent

1. **Static config** — `daemon/examples/agents.yaml` registers the
   binary path:
   ```yaml
   agents:
     - id: my_agent
       command: my-agent
       # … per-agent config
   ```

2. **Runtime registration** — once the daemon starts, it spawns the
   binary and speaks JSON-RPC over stdio. The agent ID is matched
   against `session.agent_id` from incoming chat requests.

3. **In-process variant** — if you want your agent to live INSIDE the
   daemon (faster startup, lower IPC overhead), look at how `agent` is
   registered in `daemon/core/src/router/mod.rs`. The
   `in_process_default_agent_id` config key picks which agent gets the
   fast path.

## Test it standalone

The template compiles and runs as-is. Smoke-test it:

```bash
cargo build -p agent-template

# Pipe a ping request in:
echo '{"jsonrpc":"2.0","id":1,"method":"ping"}' | \
  cargo run -q -p agent-template
# Expected output:
# {"jsonrpc":"2.0","id":1,"result":{"pong":true,"session_id":0,"agent":"agent-template"}}
```

## Conventions

- **Don't break the JSON-RPC contract.** The daemon assumes frames are
  newline-delimited JSON. Always `flush()` after each write.
- **Use `tracing::info!` / `warn!` / `error!`** for logs (stderr), never
  stdout (stdout is the JSON-RPC channel).
- **Errors → `AgentError`-like string for now**, typed errors per
  EP-0003.
- **Tests live next to the code** (`#[cfg(test)] mod tests { ... }`).

## See also

- `agents/agent/` — the full reference implementation
- `agents/agent/src/main.rs` — read this top-to-bottom to see how
  everything ties together
- `daemon/core/src/router/mod.rs` — the daemon side that spawns your
  agent and speaks to it
- `daemon/docs/agents.md` — agent lifecycle from the daemon's POV

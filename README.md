# KovaClaw

Event-driven AI platform. WhatsApp bridge + CLI, built in Rust.

## Why?
OpenClaw's model identifies itself as Claude, hallucinates, and we have no control over the system. This is our own platform.

## Architecture

```
kovaclaw/
  config/
    kovaclaw.json           # LLM + general config
    identity/kova.md        # System prompt (identity definition)
  crates/
    kova-core/              # Event bus, LLM client, tools, sessions
    kova-cli/               # Interactive CLI (Claude Code-style REPL)
    kova-whatsapp/          # WhatsApp bridge (Baileys subprocess)
  bridge/
    baileys_bridge.js       # Node.js Baileys wrapper (~100 lines)
```

## Event System

All communication flows through the `Event` enum (tokio broadcast + mpsc):

| Event | Description |
|-------|-------------|
| `MessageIn` | Incoming message from WhatsApp/CLI |
| `LlmRequest` / `LlmResponse` | Model communication |
| `ToolRequest` / `ToolResult` | Tool calls |
| `MessageOut` | Outgoing message |

## Phases

### Phase 1: Core + CLI REPL ✅
- [x] Cargo workspace
- [x] Config system (kovaclaw.json + identity)
- [x] Event enum
- [x] LlmClient (reqwest, OpenAI-compat endpoint)
- [x] Agent (system prompt + history + LLM)
- [x] CLI REPL (stdin -> llama-server -> stdout)

### Phase 2: Tools + Session
- [ ] Tool trait + registry
- [ ] `read_file`, `write_file`, `shell_exec` tools
- [ ] Tool approval flow (y/n prompt)
- [ ] JSONL session persistence
- [ ] Streaming response (SSE)

### Phase 3: WhatsApp Bridge
- [ ] `bridge/baileys_bridge.js` - Baileys wrapper, stdin/stdout JSON lines
- [ ] `kova-whatsapp` - Node subprocess spawn, pipe events
- [ ] Reuse OpenClaw auth state

### Phase 4: TUI + Polish
- [ ] ratatui terminal UI
- [ ] Syntax highlighting
- [ ] Tool call display
- [ ] Session management UI

## Tech Stack

| Crate | Purpose |
|-------|---------|
| tokio | async runtime |
| reqwest | HTTP client (LLM API) |
| serde/serde_json | serialization |
| uuid, chrono | event ID + timestamp |
| thiserror/anyhow | error handling |
| tracing | logging |

Phase 4: `ratatui`, `crossterm`

## WhatsApp Strategy
Baileys as Node subprocess (stdin/stdout JSON). Pure Rust WA libs are immature, Baileys is battle-tested. Copy auth state from OpenClaw.

## Tool Calling
Qwen2.5 supports tool calling via Hermes format. llama-server with `--jinja` flag. Fallback: XML/regex parsing.

## Running

```bash
# start llama-server (port 8080, Qwen2.5)
cd kovaclaw
cargo run -p kova-cli
```

## Verification
1. `cargo build` - workspace compiles
2. `cargo run -p kova-cli` - ask "who are you", get a response
3. Phase 2: tool call test (read a file)
4. Phase 3: send a WhatsApp message, get a response

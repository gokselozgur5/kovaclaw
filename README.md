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

### Phase 2: Tools + Session ✅
- [x] Tool trait + registry
- [x] `read_file`, `write_file`, `shell_exec` tools
- [x] Tool approval flow (y/n prompt, auto-approve for read_file)
- [x] JSONL session persistence
- [x] Streaming response (SSE)

### Phase 3: WhatsApp Bridge ✅
- [x] `bridge/baileys_bridge.js` - Baileys wrapper, stdin/stdout JSON lines
- [x] `kova-whatsapp` - Node subprocess spawn, pipe events
- [x] Auth state configurable via BAILEYS_AUTH_DIR env

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
# CLI mode
cd kovaclaw
cargo run -p kova-cli

# WhatsApp mode
cd bridge && npm install && cd ..
BAILEYS_AUTH_DIR=/path/to/auth cargo run -p kova-whatsapp
```

## Verification
1. `cargo build` - workspace compiles
2. `cargo run -p kova-cli` - ask "who are you", get a response
3. Phase 2: tool call test (read a file)
4. Phase 3: send a WhatsApp message, get a response

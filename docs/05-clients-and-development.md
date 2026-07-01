# 5. Clients and development

[← Storage and query](./04-storage-and-query.md) · [Docs index](./README.md)

---

## IPC protocol

### Transport

- **Socket:** Unix domain socket (default `/tmp/chronicle.sock`)
- **Framing:** 4-byte big-endian length prefix + UTF-8 JSON body
- **Max response:** 16 MiB (client-side guard)

Implementation: `chronicle-ipc/src/client.rs`, `server.rs`.

### Request envelope

Serde internally tagged enum (`type` field, snake_case):

```json
{ "type": "get_status" }
```

```json
{
  "type": "search",
  "query": "cargo test",
  "mode": "keyword",
  "limit": 25
}
```

```json
{
  "type": "get_project_context",
  "project": "chronicle",
  "since": 1719000000000,
  "limit": 50
}
```

### Response envelope

Similarly tagged:

| `type` | Payload |
|--------|---------|
| `event` | Live subscribe stream |
| `timeline` | `{ spans: [...] }` |
| `timeline_events` | `{ events: [...] }` |
| `projects` | `{ projects: [...] }` |
| `project_context` | `{ project, spans, events }` |
| `span_detail` | `{ span, events }` |
| `config` | `{ watch_dirs: [...] }` |
| `status` | `{ uptime_secs, events_count, version }` |
| `ack` | `{ event_id }` |
| `error` | `{ code, message }` |

### Subscribe (live stream)

1. Client sends `{ "type": "subscribe", "event_types": [] }` (empty = all)
2. Server loops on `broadcast::Receiver` — each filtered event → `{ "type": "event", "event": … }`
3. Connection held until client disconnects

`event_types` filtering is reserved; currently all events broadcast.

### Connection model

- One request per connection for non-subscribe calls (handler reads once, responds, done)
- Subscribe holds connection open
- Each accept spawns a tokio task — no thread-per-connection blocking

---

## Tauri client

### Registered commands

`src-tauri/src/lib.rs` → `commands.rs`:

| Command | IPC request |
|---------|-------------|
| `get_status` | `GetStatus` |
| `get_timeline` | `GetTimeline` |
| `get_events` | `GetEvents` |
| `search_events` | `Search` |
| `list_projects` | `ListProjects` |
| `get_project_context` | `GetProjectContext` |
| `get_span_detail` | `GetSpan` |
| `get_config` / `set_config` | `GetConfig` / `SetConfig` |
| `install_shell_hook` | `InstallShellHook` |
| `start_event_stream` | `Subscribe` (background task) |
| `resolve_app_icon` | Local macOS (no IPC) |
| `resolve_path_icons` | Local macOS batch |

`DaemonState` holds `socket_path: "/tmp/chronicle.sock"`.

### Frontend routes

| Route | Data sources |
|-------|--------------|
| `/` | `get_events`, `get_timeline`, `start_event_stream` |
| `/projects` | `list_projects` |
| `/projects/[name]` | `get_project_context` |
| `/sessions/[id]` | `get_span_detail` |
| `/search` | `search_events` |
| `/settings` | `get_config`, `set_config`, `install_shell_hook`, `get_status` |

Shared UI utilities: `src/lib/format.js`, `src/lib/theme.svelte.js`, `AppIcon.svelte`, `ProjectIcon.svelte`.

### Dev workflow

```bash
# Terminal 1 — daemon
cargo run -p chronicle-daemon -- start --watch ~/Developer

# Terminal 2 — UI
bun install
bun run tauri dev
```

---

## MCP client (`chronicle-mcp`)

### Running

```bash
cargo build --release -p chronicle-mcp
./target/release/chronicle-mcp --socket /tmp/chronicle.sock
```

Speaks MCP over **stdio** (`rmcp` + `transport-io`). Logs go to stderr.

### Cursor configuration

`~/.cursor/mcp.json` (or project MCP settings):

```json
{
  "mcpServers": {
    "chronicle": {
      "command": "/absolute/path/to/chronicle-mcp",
      "args": []
    }
  }
}
```

### Tools

| Tool | Description | Default params |
|------|-------------|----------------|
| `chronicle_status` | Daemon version, uptime, event count | — |
| `search_events` | FTS search | `limit`: 25 |
| `list_projects` | Projects by `last_active` | `limit`: 25 |
| `get_timeline` | Recent spans | `since_ms`: 86400000 (24h), `limit`: 25 |
| `get_project_context` | Project + spans + events | `since_ms`: 24h, `limit`: 25 |

Returns JSON text in `CallToolResult` content blocks. Errors are JSON `{"error":"…"}` strings.

### Design rationale

- Separate binary — MCP SDK deps don't bloat daemon or GUI
- Same IPC as UI — one API surface to maintain
- Stdio transport — compatible with Cursor, Claude Desktop, other MCP hosts

---

## CLI reference

```bash
chronicle-daemon start [--socket PATH] [--store PATH] [--watch DIR]...
chronicle-daemon status [--socket PATH]
chronicle-daemon install [--watch DIR]...   # writes plist + config
chronicle-daemon uninstall
chronicle-daemon hook [--shell zsh|bash|fish]
chronicle-daemon hook-print zsh              # stdout script for manual install
```

If MCP shows "Not connected" while developing Chronicle: `./scripts/mcp-doctor.sh`, then restart Cursor or toggle MCP under Settings. Agent-stack conventions live in **Cursor User Rules** (not in this repo).

---

## Development setup

### Prerequisites

- Rust stable
- Bun
- macOS for full collector dev
- Xcode CLT (Swift icon helper in Tauri `build.rs`)

### Commands

```bash
make verify    # fmt + test + clippy + bun check (CI-equivalent)
cargo fmt --all
cargo clippy --workspace -- -D warnings
cargo test --workspace
bun run check
bun run build
```

### Project layout (quick reference)

```
crates/chronicle-core/src/lib.rs      # CanonicalEvent, Span, ProjectRecord
crates/chronicle-ipc/src/lib.rs       # DaemonRequest/Response enums
crates/chronicle-store/src/lib.rs     # Store queries
crates/chronicle-daemon/src/daemon.rs # Main loop
crates/chronicle-daemon/src/collectors/
crates/chronicle-mcp/src/main.rs
src-tauri/src/commands.rs
src/routes/
```

---

## Extending IPC

1. Add enum variants to `DaemonRequest` / `DaemonResponse` in `chronicle-ipc`.
2. Handle in `daemon.rs` `handle_connection` match arm.
3. Add test in `chronicle-ipc/src/lib.rs` `mod tests`.
4. Optional: Tauri command in `commands.rs`.
5. Optional: MCP `#[tool]` in `chronicle-mcp`.

**Backward compatibility:** New request types on old daemons return `error` JSON, not connection drop — important for UI built ahead of daemon upgrades.

---

## Extending the UI

1. Add route under `src/routes/`.
2. Call `invoke('…')` with typed payload.
3. Register new command in `lib.rs` `generate_handler!`.
4. Reuse `PageShell`, `format.js`, icon components.

SvelteKit uses static adapter — dynamic routes (`[name]`, `[id]`) work via client-side navigation and `fallback: index.html`.

---

## Debugging

### Daemon logs

```bash
tail -f ~/Library/Logs/chronicle.log ~/Library/Logs/chronicle.err
```

Or run foreground:

```bash
RUST_LOG=debug cargo run -p chronicle-daemon -- start --watch ~/Developer
```

### IPC probe (manual)

Use a small Rust test binary or `nc -U` won't work for length-prefixed JSON — use `chronicle-daemon status` or unit tests in `chronicle-ipc`.

### Common issues

| Issue | Check |
|-------|-------|
| UI timeout on projects | Daemon running? Socket path match? |
| MCP empty results | `chronicle_status` tool first |
| No git events | `config.toml` watch dirs cover repo parents |
| Hook not firing | `python3` available? UDP 9712 reachable? |
| Icons slow | Expected on first load; cached in frontend `appIcons.js` |

### Reset local state

```bash
launchctl unload ~/Library/LaunchAgents/com.chronicle.daemon.plist 2>/dev/null
rm -f ~/.chronicle/chronicle.db /tmp/chronicle.sock ~/.chronicle/daemon.lock
cargo run -p chronicle-daemon -- start --watch ~/Developer
```

---

## Testing strategy

| Layer | Tests |
|-------|-------|
| `chronicle-core` | Serde roundtrips, span close |
| `chronicle-store` | In-memory DB, insert/query/search |
| `chronicle-ipc` | Mock server accept/response |
| `chronicle-daemon` | `event_filter` unit tests |
| Frontend | `bun run check` (svelte-check) |

Integration tests against a live daemon are manual today.

---

## Contributing checklist

- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `bun run check` if touching `src/`
- [ ] Update docs if adding collectors, IPC types, or MCP tools

---

## Document index

1. [Introduction](./01-introduction.md)
2. [System design](./02-system-design.md)
3. [Capture pipeline](./03-capture-pipeline.md)
4. [Storage and query](./04-storage-and-query.md)
5. **Clients and development** (this document)

# Chronicle

Local-first developer observability.

Chronicle captures your coding activity — window focus, file changes, git operations, and terminal commands — and stores them in a local SQLite database. It runs as a background daemon on macOS with a Tauri + Svelte 5 desktop UI.

## Architecture

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│  Tauri App   │◄───►│  chronicle-daemon│◄───►│  Collectors  │
│  (Svelte 5)  │ UDS │  (Tokio/Rust)    │     │  (macOS)     │
└──────────────┘     └───────┬──────────┘     └──────────────┘
                             │
                    ┌────────▼─────────┐
                    │  chronicle-store │
                    │  (SQLite + WAL)  │
                    └──────────────────┘
```

- **`chronicle-core`**: Canonical event schema, spans, sessions, types
- **`chronicle-ipc`**: UDS protocol (length-prefixed JSON over Unix sockets)
- **`chronicle-store`**: SQLite persistence with FTS5 search
- **`chronicle-daemon`**: macOS daemon + collectors (window focus, filesystem, git, shell)
- **`chronicle-mcp`**: MCP server for AI tool access (planned)
- **`chronicle-plugin`**: Dynamic plugin loader (planned)

## Quick Start

```bash
# Install daemon
cargo build --release -p chronicle-daemon
./target/release/chronicle-daemon install
launchctl load ~/Library/LaunchAgents/com.chronicle.daemon.plist

# Check status
./target/release/chronicle-daemon status

# Launch UI
bun run tauri dev
```

## Collectors

| Collector | Source | Type | Emits |
|-----------|--------|------|-------|
| Window Focus | `osascript` polling (2s) | `os` → `process.focus` | App name, bundle ID, window title |
| Filesystem | `notify` (fsevents) | `filesystem` → `file.{modified,created,deleted}` | Path, extension, project |
| Git | `notify` on `.git/logs/HEAD` + `HEAD` | `git` → `commit.created`, `branch.checkout`, etc. | Branch, reflog message |
| Shell | UDP listener (`127.0.0.1:9712`) | `shell` → `command.{completed,failed}` | Command, exit code, cwd, duration |

## Privacy

- **No keystrokes, clipboard, screenshots, audio, or video.**
- All data stays on your machine (local-first).
- Cloud sync is optional and opt-in (future).
- Collectors are opt-in per-category.

## Storage

SQLite with WAL mode at `~/.chronicle/chronicle.db`. Schema includes FTS5 full-text search on events.

## License

MIT

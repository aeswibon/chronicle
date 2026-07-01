# Chronicle

**Local-first developer observability** for macOS.

Chronicle automatically records what you were doing while coding — window focus, git operations, terminal commands, and meaningful file changes — and stores it in a local SQLite database. Browse timelines in a Tauri + Svelte desktop app, search your history, or query it from AI tools via MCP.

> **Documentation:** [docs/README.md](docs/README.md) — five-part guide (introduction → system design → capture → storage → clients)

---

## Why Chronicle?

- **Remember context** — See which app, repo, and branch you were in hours ago.
- **Search your work** — Full-text search over activity, not just git log.
- **Project-aware** — Events grouped by git/cargo roots; per-project timelines.
- **AI-ready** — `chronicle-mcp` exposes search, projects, and sessions to Cursor/Claude.
- **Private by default** — No cloud, no keystrokes, no screenshots. Data stays in `~/.chronicle/`.

---

## Architecture (overview)

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────┐
│  Tauri App   │◄───►│  chronicle-daemon│◄───►│  Collectors  │
│  (Svelte 5)  │ UDS │  (Tokio/Rust)    │     │  (macOS)     │
└──────────────┘     └───────┬──────────┘     └──────────────┘
         ▲                   │
┌────────┴────────┐ ┌───────▼─────────┐
│  chronicle-mcp  │ │  chronicle-store │
│  (stdio / AI)   │ │  (SQLite + FTS)  │
└─────────────────┘ └─────────────────┘
```

| Component | Role |
|-----------|------|
| `chronicle-core` | Canonical event schema, spans, sessions |
| `chronicle-ipc` | JSON over Unix domain sockets |
| `chronicle-store` | SQLite persistence + FTS5 search |
| `chronicle-daemon` | Collectors, filtering, launchd service |
| `chronicle-mcp` | MCP tools for AI assistants |
| `src-tauri` + `src/` | Desktop UI |

Full design: **[docs/README.md](docs/README.md)** (start with [Introduction](docs/01-introduction.md))

---

## Quick start

```bash
# Build
cargo build --release -p chronicle-daemon -p chronicle-mcp
bun install

# Install background daemon (add your code directories)
./target/release/chronicle-daemon install --watch ~/Developer
launchctl load ~/Library/LaunchAgents/com.chronicle.daemon.plist

# Optional: shell hook for terminal commands
./target/release/chronicle-daemon hook --shell zsh

# Status + UI
./target/release/chronicle-daemon status
bun run tauri dev
```

Configure watch paths in the app under **Settings**, or edit `~/.chronicle/config.toml`.

---

## Collectors

| Collector | Emits | Notes |
|-----------|-------|-------|
| Window focus | `process.focus` | App name, bundle ID, window title |
| Filesystem | `file.created` / `file.deleted` | Source files under watch dirs; ignores `node_modules`, `target`, … |
| Git | `commit.created`, `branch.checkout`, … | Watches reflogs under discovered repos |
| Shell | `command.completed` / `command.failed` | UDP hook on `127.0.0.1:9712` (zsh, bash, fish) |

Each collector can be disabled in **Settings** or `~/.chronicle/config.toml`. See [capture pipeline](docs/03-capture-pipeline.md#collector-opt-in).

Extensions can emit events via [`emit_event`](docs/06-external-plugins.md) (UDS) — same pipeline as collectors.

---

## MCP (AI tools)

Build and register in your MCP client:

```json
"chronicle": {
  "command": "/path/to/chronicle-mcp",
  "args": []
}
```

Tools: `chronicle_status`, `search_events`, `list_projects`, `get_timeline`, `get_project_context`, `get_recent_errors`. Requires the daemon to be running.

---

## Privacy

- **No** keystrokes, clipboard, screenshots, audio, or video.
- **Yes** app names, window titles, shell commands, git messages, file paths (not contents).
- All data local unless you add sync later (not implemented).

Details: [Introduction — Privacy](docs/01-introduction.md) and [Capture pipeline](docs/03-capture-pipeline.md#what-is-not-captured)

---

## Development

```bash
cargo test --workspace
bun run check
```

See **[docs/05-clients-and-development.md](docs/05-clients-and-development.md)** for repo layout, adding collectors, and IPC extensions.

---

## License

MIT

# Changelog

All notable changes to Chronicle are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.2] - 2026-07-02

### Added

- **Ollama integration** — test connection and list models from Settings; default model `smallthinker`
- **Focus monitor bundling** — `chronicle-focus-monitor` shipped beside the daemon in app releases

### Changed

- **macOS focus capture** — daemon runs the focus monitor via `launchctl asuser` (no longer depends on the UI relay)
- **AI summaries** — stronger anti-reasoning prompt and post-processing for chain-of-thought models

### Fixed

- **Span detail 404** — `GetSpan` resolves active in-memory spans, not only closed rows in SQLite
- **Safari / window titles** — Accessibility window titles and tab sessions recorded again
- **Delete summary** — stable daily session IDs; delete no longer fails on stale IDs
- **Sessions UI** — safer delete handling and summarize error surfacing
- **MCP summarize** — handle `ai_error` in daily summary responses

## [0.1.1] - 2026-07-01

### Added

- **Persisted summary source** — daily rollups store `ai` vs `rules` on session rows; Sessions UI shows badges after reload
- **Auto daily rollups** — configurable local hour in Settings; daemon generates today's summary when missing
- **Enrichment backfill** — daemon fills `report_line` and activity labels on existing events at startup
- **Timeline cleanup** — Settings action to prune low-signal git and shell noise from existing databases
- **Delete summaries** — remove individual daily rollups from Sessions
- **Window/tab tracking** — `window.focus` events when the front window title changes within the same app (Safari tabs, Finder windows, etc.)
- **Native macOS focus capture** — NSWorkspace + Accessibility/CGWindow helper; GUI-session relay from Chronicle.app
- **Tab sessions** — per-window identity across IDEs, browsers, Finder, and terminals (dirty markers and spinner noise stripped)
- **Settings** — unified save for AI, collectors, privacy, and watch directories; Tauri v2 camelCase config IPC
- **Timeline purge** — reset capture events from Settings without deleting daily summaries

### Changed

- **Git first-run** — bootstrap cursors only (no historical backfill flood on fresh install)
- **UI** — 24-hour clock; hide inactive timeline sessions and empty daily rollups; stop tagging every app switch with a generic "focus" label
- **Summaries** — rule-based rollups rank top projects by focus time, drop generic git themes, cap displayed hours, and surface build/test failures only (not git push noise)
- **Homebrew** — cask checksums auto-updated on release

### Fixed

- Project path layout overflow; Timeline nav default highlight; session active indicators after restart
- CI rustfmt and clippy failures
- Activity feed empty while sessions showed focus events (`activity_label` without `tab_session_key`)
- `set_config` invalid args error in Settings (camelCase IPC)

## [0.1.0] - 2026-07-01

First public release — local developer observability for macOS.

### Added

- **Desktop app** — Tauri v2 + Svelte 5 UI with Timeline, Projects, Sessions, Errors, Search, and Settings
- **Background daemon** — `chronicle-daemon` with Unix-socket IPC (`~/.chronicle/chronicle.sock`) and SQLite store (`~/.chronicle/chronicle.db`)
- **Collectors** — window focus, filesystem (FSEvents), git reflogs, shell hook (UDP), HTTP ingress, IDE/browser extensions
- **Git capture** — poll all `.git/logs/**` every 20s, 72h backfill on first sight, persisted cursors, real reflog timestamps; supports commits, pushes, pulls, fetches, merges, rebases, checkouts
- **Projects** — auto-discovery from watch dirs and events, sorted by last activity
- **Sessions & summaries** — rule-based daily rollups; optional AI summaries via Ollama/API in Settings
- **Activity labels** — deterministic rule engine for shell, git, IDE, and browser events (`report_line`, `intent`, `tool`, `outcome` metadata)
- **Search** — keyword search over events; semantic expansion hook for AI-assisted queries
- **Privacy** — collector toggles, domain allowlist, retention prune, shell secret redaction, noise filtering
- **MCP server** — `chronicle-mcp` tools for timeline, search, sessions, summarize, project context
- **Plugins** — manifest discovery under `~/.chronicle/plugins/`
- **Standard-user setup** — per-user LaunchAgent, bundled daemon sidecar, auto-start on app launch (no admin password)
- **CI** — Rust fmt/clippy/test on Ubuntu and macOS; frontend typecheck and build
- **Release** — signed + notarized GitHub Release DMGs (Developer ID); unsigned fallback when `MACOS_SIGNING_ENABLED` is off ([docs/07-release-macos.md](./docs/07-release-macos.md))
- **Docs** — five-part architecture series, capture pipeline, clients, plugins, external integrations
- **Homebrew cask** — `Casks/chronicle.rb` (DMG-based tap install)

### Changed

- Socket and store defaults moved from `/tmp` to `~/.chronicle/`
- Window focus uses `lsappinfo` instead of AppleScript (no Accessibility prompt)
- Dev builds (`tauri dev`) spawn the daemon directly instead of fighting launchd
- Monorepo git repos (e.g. `autonomic-ai-dev/agent-brain`) discovered up to depth 8
- Daily summary sessions use generation time for ordering; same-day rollups replaced on regenerate

### Fixed

- Git pushes and pulls missed when daemon was offline or FSEvents did not fire inside `.git/`
- Project list stale ordering (`last_active` now uses event timestamps with `MAX`)
- Summarize today wrong timezone and noisy shell failures in highlights
- `tauri dev` launchctl bootstrap I/O errors from KeepAlive vs singleton lock conflicts
- Sessions summarize IPC when daemon lacked `summarize_day`
- Linux CI failures for window-focus parser tests and clippy dead-code warnings

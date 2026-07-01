# Changelog

All notable changes to Chronicle are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- **Release** — GitHub Actions Tauri bundle for Apple Silicon and Intel DMGs
- **Docs** — five-part architecture series, capture pipeline, clients, plugins, external integrations
- **Homebrew cask** — `Formula/chronicle.rb` (DMG-based)

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

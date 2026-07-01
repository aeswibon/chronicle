# 6. External plugins and extensions

[← Clients and development](./05-clients-and-development.md) · [Docs index](./README.md)

---

## Overview

Extensions (IDE plugins, browser extensions, custom scripts) send **engineering metadata** into Chronicle through the same canonical event pipeline as built-in collectors. They never write SQLite directly.

```
Extension  →  UDS emit_event  →  daemon mpsc  →  filter → rule engine → store
```

Shell hooks are the exception: they use **UDP** `127.0.0.1:9712` because hooks cannot hold a persistent socket.

Browser extensions use **HTTP** `POST http://127.0.0.1:9713/v1/events` with a `CanonicalEvent` JSON body (see `extensions/browser-chronicle/`).

---

## `emit_event` IPC

**Request** (length-prefixed JSON over `/tmp/chronicle.sock`):

```json
{
  "type": "emit_event",
  "event": {
    "version": "1.0",
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "timestamp": 1719750000000,
    "source": "vscode",
    "category": "ide",
    "type": "ide.test.run",
    "project": "chronicle",
    "workspace": null,
    "duration_ms": 4200,
    "metadata": {
      "test_framework": "cargo",
      "passed": 12,
      "failed": 0
    }
  }
}
```

**Response:**

```json
{ "type": "ack", "event_id": "550e8400-e29b-41d4-a716-446655440000" }
```

Events pass through `event_filter`, `rule_engine`, span processing, and FTS indexing — same as collector events.

### Privacy guidelines

| OK | Avoid |
|----|--------|
| File paths, test counts, debug session names | File contents |
| Command names without secrets | Tokens, env vars, passwords |
| Domain names (browser) | Full URLs with query params containing secrets |
| LSP action kinds | Keystrokes, clipboard |

---

## Sample script

[`scripts/emit_event.py`](../scripts/emit_event.py) — minimal Python client:

```bash
chmod +x scripts/emit_event.py

# Example: record a test run from a VS Code extension
./scripts/emit_event.py \
  --source vscode \
  --category ide \
  --type ide.test.run \
  --project chronicle \
  --duration-ms 3200 \
  --metadata '{"framework":"cargo","failed":0}'
```

---

## Recommended event types (extensions)

| Category | `type` | Metadata |
|----------|--------|----------|
| `ide` | `ide.file.focus` | `path`, `language` |
| `ide` | `ide.test.run` | `framework`, `passed`, `failed` |
| `ide` | `ide.debug.start` / `ide.debug.stop` | `adapter`, `project_path` |
| `browser` | `page.focus` | `domain`, `title` (allowlisted domains only) |
| `build` | `build.completed` | `tool`, `exit_code`, `duration_ms` |

Set `project` when the event belongs to a repo; include `project_path` in metadata when helpful.

---

## Future: native plugins

`chronicle-plugin` defines a Rust trait for `.dylib` plugins loaded from `~/.chronicle/plugins/`. Not wired in the daemon yet — use `emit_event` from any language today.

---

## Next steps

- **[Capture pipeline](./03-capture-pipeline.md)** — built-in collectors and rule engine
- **[Clients and development](./05-clients-and-development.md)** — IPC reference

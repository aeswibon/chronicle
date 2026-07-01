# Chronicle VS Code / Cursor extension

Emits `ide.*` events to chronicle-daemon over the Unix socket.

## Install (dev)

```bash
cd extensions/vscode-chronicle
npm install
npm run compile
```

In VS Code/Cursor: **Extensions → Install from VSIX** or symlink into `~/.vscode/extensions`.

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `chronicle.socket` | `/tmp/chronicle.sock` | Daemon socket path |

## Commands

- `Chronicle: Emit test run` — `ide.test.run`
- `Chronicle: Emit debug start` — `ide.debug.start`

File focus and save events are emitted automatically.

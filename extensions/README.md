# Chronicle extensions

Optional emitters that enrich the activity graph beyond built-in collectors.

| Extension | Transport | Events |
|-----------|-----------|--------|
| [vscode-chronicle](./vscode-chronicle/) | Unix socket (`/tmp/chronicle.sock`) | `ide.file.focus`, `ide.file.saved` |
| [browser-chronicle](./browser-chronicle/) | HTTP (`http://127.0.0.1:9713/v1/events`) | `page.focus` |

Requires `chronicle-daemon` running. See [docs/06-external-plugins.md](../docs/06-external-plugins.md).

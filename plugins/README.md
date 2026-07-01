# Chronicle plugins

Place plugin bundles under `~/.chronicle/plugins/<name>/` with a manifest:

**chronicle-plugin.toml**

```toml
name = "my-collector"
version = "0.1.0"
description = "Custom event source"
enabled = true
entry = "libmy_collector.dylib"
```

Or **plugin.json**:

```json
{
  "name": "my-collector",
  "version": "0.1.0",
  "enabled": true
}
```

The daemon lists installed plugins via `ListPlugins` IPC / `list_plugins` MCP tool.

Dynamic `.dylib` loading is reserved for a future release; manifests are discovered now so authors can scaffold bundles early.

Extensions that only emit events should use [HTTP ingress](../docs/06-external-plugins.md) or `emit_event` over the Unix socket instead.

# Chronicle documentation

A five-part guide for understanding, operating, and extending Chronicle.

| # | Document | Read this if you want to… |
|---|----------|---------------------------|
| 1 | [Introduction](./01-introduction.md) | Understand what Chronicle is, who it's for, and why you'd use it |
| 2 | [System design](./02-system-design.md) | See how processes, crates, and deployment fit together |
| 3 | [Capture pipeline](./03-capture-pipeline.md) | Learn how collectors, filtering, spans, and projects work |
| 4 | [Storage and query](./04-storage-and-query.md) | Understand the database schema, FTS, and query APIs |
| 5 | [Clients and development](./05-clients-and-development.md) | Integrate via IPC/MCP/UI or contribute code |
| 6 | [External plugins](./06-external-plugins.md) | Emit events from IDE/browser extensions via `emit_event` |
| 7 | [macOS release signing](./07-release-macos.md) | Sign, notarize, and publish Gatekeeper-safe DMGs |

**Suggested reading order:** 1 → 2 → 3 → 4 → 5. Skim 1–2 first; dive into 3–5 when building features or debugging capture. See **6** when building extensions.

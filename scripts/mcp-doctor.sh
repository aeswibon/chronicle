#!/usr/bin/env bash
# Reconnect agent-brain + agent-body MCP after upgrades or stale serve processes.
set -euo pipefail

echo "== agent-brain =="
agent-brain install --global --reload
agent-brain doctor --fix

echo ""
echo "== agent-body (autonomic organs) =="
agent-body start
agent-body doctor

echo ""
echo "Done. In Cursor: Settings → MCP → disable and re-enable agent-brain and agent-body,"
echo "or restart Cursor so this session can call route_task / agent-body tools."

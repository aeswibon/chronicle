#!/usr/bin/env python3
"""Emit a CanonicalEvent to chronicle-daemon via Unix domain socket.

Usage:
  ./scripts/emit_event.py --type ide.test.run --source vscode --project my-app
  ./scripts/emit_event.py --json event.json

Requires chronicle-daemon running with default socket /tmp/chronicle.sock.
"""

from __future__ import annotations

import argparse
import json
import socket
import struct
import sys
import time
import uuid

DEFAULT_SOCKET = "/tmp/chronicle.sock"


def emit(socket_path: str, event: dict) -> dict:
    payload = json.dumps({"type": "emit_event", "event": event}).encode("utf-8")
    frame = struct.pack(">I", len(payload)) + payload

    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        sock.connect(socket_path)
        sock.sendall(frame)

        header = sock.recv(4)
        if len(header) != 4:
            raise RuntimeError("short read on response length")
        (resp_len,) = struct.unpack(">I", header)
        body = sock.recv(resp_len)
        if len(body) != resp_len:
            raise RuntimeError("short read on response body")
        return json.loads(body.decode("utf-8"))
    finally:
        sock.close()


def build_event(args: argparse.Namespace) -> dict:
    if args.json:
        with open(args.json, encoding="utf-8") as f:
            return json.load(f)

    now_ms = int(time.time() * 1000)
    metadata = {}
    if args.metadata:
        metadata = json.loads(args.metadata)

    return {
        "version": "1.0",
        "id": str(uuid.uuid4()),
        "timestamp": now_ms,
        "source": args.source,
        "category": args.category,
        "type": args.type,
        "project": args.project,
        "workspace": args.workspace,
        "duration_ms": args.duration_ms,
        "metadata": metadata,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Emit event to chronicle-daemon")
    parser.add_argument("--socket", default=DEFAULT_SOCKET)
    parser.add_argument("--json", help="Path to full CanonicalEvent JSON file")
    parser.add_argument("--source", default="extension")
    parser.add_argument("--category", default="ide")
    parser.add_argument("--type", dest="type", default="ide.action")
    parser.add_argument("--project")
    parser.add_argument("--workspace")
    parser.add_argument("--duration-ms", type=int, dest="duration_ms")
    parser.add_argument("--metadata", help='JSON object, e.g. \'{"action":"save"}\'')
    args = parser.parse_args()

    event = build_event(args)
    resp = emit(args.socket, event)
    print(json.dumps(resp, indent=2))

    if resp.get("type") == "error":
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

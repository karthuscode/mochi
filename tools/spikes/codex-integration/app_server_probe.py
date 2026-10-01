#!/usr/bin/env python3
"""Read-only App Server structure probe that never emits conversation content."""

from __future__ import annotations

import argparse
import hashlib
import json
import selectors
import subprocess
import sys
import time
from typing import Any


def fingerprint(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8", "replace")).hexdigest()[:12]


class Client:
    def __init__(self, timeout: float) -> None:
        self.timeout = timeout
        self.next_id = 1
        self.process = subprocess.Popen(
            ["codex", "app-server", "--listen", "stdio://"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self.selector = selectors.DefaultSelector()
        assert self.process.stdout is not None
        self.selector.register(self.process.stdout, selectors.EVENT_READ)

    def send(self, message: dict[str, Any]) -> None:
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
        self.process.stdin.flush()

    def request(self, method: str, params: dict[str, Any]) -> tuple[dict[str, Any], list[str]]:
        request_id = self.next_id
        self.next_id += 1
        self.send({"method": method, "id": request_id, "params": params})
        notifications: list[str] = []
        deadline = time.monotonic() + self.timeout
        while time.monotonic() < deadline:
            ready = self.selector.select(max(0.0, deadline - time.monotonic()))
            if not ready:
                break
            line = self.process.stdout.readline()
            if not line:
                break
            message = json.loads(line)
            if message.get("id") == request_id:
                return message, notifications
            if isinstance(message.get("method"), str):
                notifications.append(message["method"])
        raise TimeoutError(f"no response for {method}")

    def close(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=2)


def summarize_item(item: Any) -> dict[str, Any]:
    if not isinstance(item, dict):
        return {"type": "malformed"}
    summary: dict[str, Any] = {"type": item.get("type", "unknown"), "fields": sorted(item.keys())}
    for field in ("status", "exitCode", "durationMs"):
        if field in item:
            summary[field] = item[field]
    if isinstance(item.get("changes"), list):
        summary["change_count"] = len(item["changes"])
        kinds = set()
        for change in item["changes"]:
            if not isinstance(change, dict):
                continue
            kind = change.get("kind", "unknown")
            if isinstance(kind, dict):
                kind = kind.get("type", "unknown")
            kinds.add(str(kind))
        summary["change_kinds"] = sorted(kinds)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cwd", required=True, help="Exact synthetic working directory to query")
    parser.add_argument("--include-turns", action="store_true")
    parser.add_argument("--timeout", type=float, default=10.0)
    args = parser.parse_args()

    client = Client(args.timeout)
    result: dict[str, Any] = {"schema_version": 1, "cwd_fingerprint": fingerprint(args.cwd)}
    try:
        initialize, initial_notes = client.request(
            "initialize",
            {
                "clientInfo": {"name": "mochi-brief-01a-probe", "title": "Mochi Brief 01A probe", "version": "1"},
                "capabilities": None,
            },
        )
        result["initialize_ok"] = "result" in initialize
        result["initialize_notifications"] = sorted(set(initial_notes))
        client.send({"method": "initialized", "params": {}})

        listing, list_notes = client.request(
            "thread/list",
            {
                "limit": 100,
                "cwd": args.cwd,
                "sourceKinds": ["cli", "exec", "appServer", "unknown"],
                "archived": False,
            },
        )
        threads = listing.get("result", {}).get("data", [])
        result["list_notifications"] = sorted(set(list_notes))
        result["thread_count"] = len(threads)
        result["threads"] = []
        for thread in threads:
            safe_thread = {
                "id_fingerprint": fingerprint(str(thread.get("id", ""))),
                "fields": sorted(thread.keys()),
                "source": thread.get("source"),
                "status": thread.get("status"),
                "cli_version": thread.get("cliVersion"),
                "has_git_info": thread.get("gitInfo") is not None,
                "turn_count_in_list": len(thread.get("turns", [])),
            }
            result["threads"].append(safe_thread)

        if args.include_turns and threads:
            read, read_notes = client.request(
                "thread/read", {"threadId": threads[0]["id"], "includeTurns": True}
            )
            hydrated = read.get("result", {}).get("thread", {})
            turns = hydrated.get("turns", [])
            result["read_notifications"] = sorted(set(read_notes))
            result["hydrated_turn_count"] = len(turns)
            result["turns"] = [
                {
                    "status": turn.get("status"),
                    "fields": sorted(turn.keys()),
                    "items": [summarize_item(item) for item in turn.get("items", [])],
                }
                for turn in turns
            ]
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except (BrokenPipeError, OSError, TimeoutError, json.JSONDecodeError) as error:
        print(json.dumps({"schema_version": 1, "probe_error": type(error).__name__}, sort_keys=True))
        return 1
    finally:
        client.close()


if __name__ == "__main__":
    sys.exit(main())

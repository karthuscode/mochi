#!/usr/bin/env python3
"""Bounded, fail-closed logger for the Brief 01A synthetic hook probe."""

from __future__ import annotations

import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import time
from typing import Any

MAX_INPUT_BYTES = 1_048_576
MAX_STRING_CHARS = 8_192
MAX_RECORD_BYTES = 65_536
SENSITIVE_KEY = re.compile(r"(?i)(?:api[_-]?key|access[_-]?token|authorization|password|secret)")

TOKEN_PATTERNS = (
    re.compile(r"\b(?:sk|pk|rk|ghp|github_pat|xox[baprs])[-_][A-Za-z0-9_-]{12,}\b"),
    re.compile(r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{8,}"),
    re.compile(r"(?i)(?<=://)[^\s/:@]+:[^\s/@]+@"),
    re.compile(r"(?i)\b(?:api[_-]?key|access[_-]?token|password|secret)\s*[:=]\s*[^\s,;]+"),
    re.compile(r"-----BEGIN [^-]+ PRIVATE KEY-----.*?-----END [^-]+ PRIVATE KEY-----", re.DOTALL),
)


def redact_text(value: str) -> str:
    value = value.replace(str(Path.home()), "[HOME]")
    probe_root = os.environ.get("MOCHI_PROBE_ROOT")
    if probe_root:
        value = value.replace(str(Path(probe_root).resolve()), "[PROBE_ROOT]")
    for pattern in TOKEN_PATTERNS:
        value = pattern.sub("[REDACTED_SECRET]", value)
    # Absolute paths outside the two known roots remain too identifying for evidence.
    value = re.sub(r"(?<![A-Za-z0-9_.-])/(?:[^\s\"'<>]+/?)+", "[ABSOLUTE_PATH]", value)
    if len(value) > MAX_STRING_CHARS:
        digest = hashlib.sha256(value.encode("utf-8", "replace")).hexdigest()[:16]
        return f"{value[:MAX_STRING_CHARS]}[TRUNCATED sha256={digest}]"
    return value


def sanitize(value: Any, depth: int = 0) -> Any:
    if depth > 12:
        return "[MAX_DEPTH]"
    if isinstance(value, str):
        return redact_text(value)
    if isinstance(value, list):
        return [sanitize(item, depth + 1) for item in value[:256]]
    if isinstance(value, dict):
        sanitized = {}
        for key, item in list(value.items())[:256]:
            safe_key = redact_text(str(key))[:256]
            sanitized[safe_key] = "[REDACTED_SECRET]" if SENSITIVE_KEY.search(safe_key) else sanitize(item, depth + 1)
        return sanitized
    if value is None or isinstance(value, (bool, int, float)):
        return value
    return redact_text(repr(value))


def allowed_output_path() -> Path | None:
    raw = os.environ.get("MOCHI_HOOK_PROBE_OUTPUT")
    if not raw:
        return None
    path = Path(raw).expanduser().resolve()
    allowed = Path("/private/tmp/mochi-codex-integration-probe").resolve()
    try:
        path.relative_to(allowed)
    except ValueError:
        return None
    return path


def main() -> None:
    started = time.perf_counter()
    output = allowed_output_path()
    if output is None:
        return

    raw = os.sys.stdin.buffer.read(MAX_INPUT_BYTES + 1)
    oversized = len(raw) > MAX_INPUT_BYTES
    payload: Any
    if oversized:
        payload = {"capture_gap": "input_exceeded_limit", "observed_bytes": len(raw)}
    else:
        try:
            payload = json.loads(raw.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            payload = {"capture_gap": "malformed_json", "observed_bytes": len(raw)}

    record = {
        "schema_version": 1,
        "received_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "logger_duration_ms": round((time.perf_counter() - started) * 1000, 3),
        "payload": sanitize(payload),
    }
    encoded = json.dumps(record, ensure_ascii=True, separators=(",", ":")).encode("utf-8")
    if len(encoded) > MAX_RECORD_BYTES:
        encoded = json.dumps(
            {
                "schema_version": 1,
                "received_at": record["received_at"],
                "capture_gap": "sanitized_record_exceeded_limit",
                "sha256": hashlib.sha256(encoded).hexdigest(),
            },
            separators=(",", ":"),
        ).encode("utf-8")

    output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    descriptor = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o600)
    try:
        with os.fdopen(descriptor, "ab", closefd=True) as handle:
            fcntl.flock(handle.fileno(), fcntl.LOCK_EX)
            handle.write(encoded + b"\n")
            handle.flush()
            os.fsync(handle.fileno())
    except OSError:
        # A probe failure must not affect or add output to the Codex turn.
        return


if __name__ == "__main__":
    main()

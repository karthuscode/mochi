# Brief 06 — Secret Detection & Redaction Engine

Implemented: 2026-09-20. Scope: one shared redaction engine for supported capture, Git, spool, and SQLite ingress paths. This brief adds no migration, automatic capture/import, analysis, learning, UI, or remote service.

## Architecture and integration

`crates/privacy/src/redaction.rs` owns all secret detectors, structured key classification, safe marker names, bounds, and `redaction-v1`. `CaptureSanitizer` replaces the earlier prototype regex implementation; its remaining responsibilities are adapter-local root/home/temp path masking and the 8 KiB post-redaction field limit. The old `PrototypeSanitizer` type name is only a compatibility alias for existing fixture callers; no second secret-rule implementation remains.

`CodexSessionSource` sanitizes prompt, final agent message, Bash command text, Bash command output, and retained file-tool response values before constructing normalized events. Nested tool output uses the shared structured walker, then adapter path masking. Non-Bash file-tool content still requires Brief 05 path approval. Unretained tool input is never spooled; retained Bash command input is redacted. Git applies the path policy first, then the shared engine to allowed file bytes; paths containing recognized secret values are excluded and unsafe branch text is omitted.

`Spool::append` defensively checks the complete record with the shared engine before writing. If a direct caller supplies recognized unsanitized content or processing fails, it writes a minimal metadata-only gap instead. `IngressRepository::import_spool_batch` checks the same record before source-identity hashing, dedupe, policy checks, or SQL content insertion. It rejects a forged unsafe record with metadata only and an ingress UUID tombstone. The database does **not** modify and accept a forged record, preserving clear provenance and the existing import transaction. The duplicate pass costs an extra bounded local scan but closes the Brief 03 audit's straightforward forged-spool bypass for supported patterns. Previously persisted rows are not retroactively rewritten.

## Detectors and canaries

Tests cover synthetic OpenAI-style keys, GitHub tokens, AWS IDs and contextual secrets, Bearer tokens, structurally plausible JWTs, Postgres/MySQL/MongoDB URL credentials, quoted/unquoted password and generic assignments, complete and unterminated private-key blocks, nested JSON, idempotence, and false-positive developer values. End-to-end capture fixtures place distinct canaries in a prompt, command, command output, structured tool response, final agent message, and allowed Git source. Tests scan normalized events and spool bytes for the original values. A command-output fixture also passes through SQLite import and scans database/sidecar bytes. Another isolated SQLite test forges a valid spool file after normal append and confirms rejection, acknowledgement, empty ingress content, and absence of the canary from database/sidecar bytes. Errors carry no input text.

## Performance and validation

The local debug benchmark with a compiled engine measures median/p95 over 25 runs: approximately 0.5/1.5 ms for 1 KiB ordinary text, 3.9/5.3 ms for 10 KiB mixed text, 24.0/24.6 ms for 64 KiB ordinary text, and 4.0/4.2 ms for repeated secrets. A 100-item nested structure took about 1.4 ms. These figures exclude process startup and disk I/O; they establish practical bounded local scanning rather than a release performance guarantee.

All 84 Rust tests and 18 frontend tests pass. The repository typecheck, frontend lint, frontend tests, frontend build, combined tests, combined lint, format check, Rust check, and native macOS app build pass. The packaged app and frontend distribution contain none of the tested synthetic canary strings. Tests use only inert synthetic values and isolated temporary storage.

## Limitations and next brief

The pattern set is intentionally finite and has no universal entropy detector. It does not interpret shell commands, validate tokens over a network, or find secrets deliberately split across unrelated fields. Historic spool/database records and external copies are not scrubbed. Existing Codex Desktop prompt and interrupt coverage remains partial, and no production capture is enabled. The next separately assigned brief should address the next roadmap dependency; this implementation does not start it.

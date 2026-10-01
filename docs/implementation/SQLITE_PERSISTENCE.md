# Brief 03 — Transactional SQLite persistence

Status: implemented and verified on 2026-09-17. Scope: local persistence only.

Brief 04 subsequently adds forward-only migration 0003 and assembly-specific repository operations without changing Brief 03's ingress/session separation. See [SESSION_ASSEMBLY_ENGINE.md](SESSION_ASSEMBLY_ENGINE.md).

## Result

Brief 03 adds `crates/persistence`, backed by `rusqlite 0.40.2` with bundled SQLite. The desktop opens `<app-data>/mochi.sqlite3` during Tauri setup, applies embedded checksummed migrations, and owns one mutex-guarded connection. Startup does not scan the capture spool and exposes no persistence IPC.

The database has two deliberately separate boundaries:

1. `ingress_events` stores sanitized normalized `SpoolIngressRecord` evidence before assembly. It preserves receive ordering and gaps and contains no canonical session, turn, completion, or inferred relationship.
2. The `sessions` aggregate tables store only an already validated domain `CodingSession`. Loading reconstructs every child row and reruns the authoritative Rust domain constructor.

There is no evidence-to-session mapper or ingress/session association in this brief. Brief 04 owns that transition.

## Storage and migrations

The application-support directory is restricted to `0700`; the database and its WAL/SHM sidecars are restricted to `0600` on Unix. SQLite is not application-encrypted. Opening an existing database runs `quick_check` before migration and after migration. Corruption, a newer schema, or a checksum mismatch fails startup without replacing or deleting the database.

Connection settings are foreign keys enabled, WAL journaling, full synchronous durability, a five-second busy timeout, in-memory temporary storage, and untrusted-schema mode after migration. Writes use immediate transactions and parameterized SQL.

- Migration 0001 creates projects, local capture policy, preassembly ingress, metadata-only rejections, replay tombstones, and spool eviction state.
- Migration 0002 creates sessions, turns, events, tool/command executions, file changes, and Git snapshots.

Migrations are additive, ordered, checksummed, and transactional. Down migrations are unsupported. Any future destructive migration must first create a private recovery backup retained no longer than seven days.

## Import durability and privacy

`Spool::scan_batch` returns no more than 100 opaque candidates. A candidate is either a strict record or a metadata-only rejection. Filenames must agree with the record's receive sequence and ingress UUID. Acknowledgement tokens are private, bound to their spool root, and cannot name arbitrary files.

An import transaction rechecks project existence, active tracking consent, and the exact policy revision before writing content. It then checks active tombstones and deduplicates by ingress UUID and a versioned SHA-256 identity derived only from safe source metadata. Identical text is never a dedupe key. Unknown, disabled, deleted, stale-policy, malformed, oversized, or unsupported records store reason metadata only.

The importer commits before acknowledging spool files. Database failure acknowledges nothing. Acknowledgement failure leaves a replayable file whose second import is a database no-op. Brief 05 provides file/path filtering before prototype capture/Git writes. Brief 06's shared engine redacts supported capture content and rejects recognized unsanitized spool records before source hashing or SQL content insertion; a rejection retains metadata only. Historic rows are not retroactively scrubbed, and pattern detection cannot recognize every secret.

Ingress deletion, retention purge, and project deletion create seven-day replay tombstones before deleting evidence. Project deletion disables tracking, increments policy revision, deletes ingress, and cascades validated sessions while retaining the soft-deleted policy row through the replay window. Session deletion cascades children. WAL checkpointing is explicit; deletion is not represented as forensic erasure.

## Verification

Automated tests cover:

- empty, repeat, populated upgrade, checksum mismatch, and newer-schema migration paths;
- private directory/database permissions and non-destructive corrupt-database rejection;
- ingress dedupe, sequence gaps, bounded pagination, malformed-record handling, policy rejection, tombstones, and project deletion;
- database commit followed by failed spool acknowledgement and idempotent replay;
- concurrent import through the single writer;
- exact session round trips, child cascades, identity conflicts, corrupt child rows, partial Desktop capabilities, interrupted unknown-exit commands, dirty/truncated Git, and unavailable Git.

The following gates passed on macOS 26.5.1 arm64:

- `pnpm typecheck`
- `pnpm lint:frontend` and `pnpm lint`
- `pnpm test:frontend` and `pnpm test:rust`
- `pnpm check:rust`
- `pnpm format:check`
- `pnpm build`
- `pnpm desktop:build`

The Rust workspace ran 50 unit tests, including 17 persistence tests and 17 capture tests; the frontend ran 18 tests. The native release build produced a local unsigned `Mochi.app` bundle. This implementation does not claim production capture, automatic retention scheduling, session assembly, analysis, learning behavior, signed distribution, or persistence UI.


## Later MVP amendment

The Brief 03 evidence above remains historical. The personal CLI MVP adds migrations 0004 (capture episodes/immutable ingress associations) and 0005 (analysis runs, validated learning documents, questions, exposures, reveals and attempts); schema is now 5. Associated-ingress session deletion now tombstones/deletes evidence and cascades learning children in the same storage transaction. Revision writes preserve the session parent identity. Empty spool scans also persist loss metadata. Native startup imports only the separately authorized production spool through narrow application controls; it never imports the legacy prototype spool. [Current schema/runtime evidence](INTERNAL_CLI_MVP.md).

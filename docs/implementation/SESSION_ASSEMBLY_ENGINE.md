# Brief 04 — Session Assembly Engine

Implemented: 2026-09-18. Scope: explicit, bounded interpretation of persisted normalized evidence into validated local `CodingSession` aggregates. This brief adds no automatic startup processing, IPC, UI, production capture, analysis, or learning behavior.

## Architecture

`crates/assembly` is a separate Rust service boundary between persistence and the authoritative domain:

```text
SQLite ingress_events
    → bounded AssemblyRepository query
    → pure deterministic reconstruction
    → CodingSession::new
    → one SQLite transaction:
         session aggregate rows
         assembly group
         ingress assignment rows
```

The engine consumes `PersistedIngress`; it never reads raw Codex input, spool files, undocumented client state, or UI state. Provider payload types remain private to capture. Persistence owns SQL and atomic writes, while the assembly crate owns event semantics. Existing `CodingSessionRepository::get_session` remains row rehydration and domain revalidation only.

`SessionAssemblyEngine::assemble_pending` is an explicit callable operation for tests and future runtime integration. Desktop startup does not invoke it.

## Grouping and identity

An assembly source group is the exact tuple:

```text
project ID
provider
adapter version
transport
client surface
external source session ID
```

No timestamp, prompt text, working directory, or repository proximity participates. Records without a nonempty external session identity are not guessed across sessions; they receive metadata-only `missing_session_identity` consumption state.

The group key is SHA-256 over versioned, length-prefixed tuple fields. The Mochi `SessionId` is a deterministic UUIDv8-shaped value derived from that group key. Turn, tool, command, and Git-derived file-change IDs use the same versioned deterministic-ID scheme with their explicit source references. Repeating assembly over the same evidence therefore cannot mint a different aggregate identity.

Assembly algorithm version `1` and an evidence fingerprint over ingress UUID, source identity key, and receive sequence are stored on the assembly group. They support diagnostics and a future explicit reassembly migration without exposing payload text.

This foundation intentionally treats one stable external session identity as one assembly group. More than one authoritative stop boundary is rejected as ambiguous instead of guessing continuation episodes. Brief 11 owns finalized-thread continuation, revisioned late events, and hardened resume/finalization policy.

## Ordering and canonical events

Evidence is loaded in `(receive_sequence, ingress_id)` order. Canonical `SessionEvent.sequence` retains the persisted receive sequence itself, so numeric gaps remain visible. The assembler never sorts solely by source or receipt timestamp and never manufactures provider sequence values.

Global receive-sequence gaps may also result from interleaved concurrent sessions. A numeric gap is therefore preserved but does not alone prove evidence loss. An explicit `capture.gap` event or truncation marker degrades completeness. Provider `sourceSequence` and `sourceTimestamp` remain provenance.

Ingress UUIDs become canonical event IDs. Every event is mapped from its strict normalized discriminator into the provider-independent domain payload. Inconsistent envelope/payload turn or tool references fail with a safe structured error.

## Lifecycle and turns

- A reliable `session.started` observation supplies the start boundary. Because current Codex hooks do not expose trustworthy event time, its persisted `receivedAt` is used as the observed boundary while `occurredAt` remains absent.
- If start evidence is absent, the earliest persisted observation is the required deterministic `startedAt` fallback and lifecycle completeness records the missing boundary.
- One `session.stopped` with `endBoundaryKnown=true` supplies the end boundary. `interrupted` maps to an interrupted session; other stop reasons, including the currently truthful `unknown` reason, establish a completed bounded episode while preserving the original reason in the event.
- No reliable stop produces `SessionStatus::Incomplete`. Tool success, an agent final message, inactivity, and a completed turn do not end the session.
- A failed tool never promotes the whole session to failed.

Turns are keyed only by an explicit external/payload turn reference. If a session has turn-scoped evidence but no turn references at all, the engine creates one deterministic unscoped turn. If some explicit turns exist, events lacking a reference remain event-level evidence and are not assigned by proximity.

A prompt is attached only from `user.prompt`. A final response is attached only from an `agent.message` explicitly marked `final`. Multiple competing prompts, final responses, turn completions, or session stops for the same target are rejected as ambiguous rather than silently selected.

## Tool and command correlation

Tool starts and completions correlate only by exact `toolCallRef`, checked against the ingress external tool reference when present.

- start + succeeded completion → completed tool;
- start + failed completion → failed tool;
- start + cancelled completion → interrupted tool;
- start + unknown completion → incomplete tool;
- start without completion → incomplete, or interrupted when the referenced turn has authoritative cancellation;
- completion without a start remains a canonical event and makes tool coverage partial; no tool aggregate is fabricated because the tool name is unknown.

Commands use exact `commandRef`, which the current Bash adapter shares with the tool reference. A start/result pair creates one command rather than duplicating the tool-derived execution. A known zero/nonzero exit code maps to completed/failed. An absent exit code remains absent and status remains unknown unless authoritative cancellation supports interrupted. A missing result remains incomplete. Separate references preserve failed and later successful attempts independently.

## Git and file semantics

Git context is optional and must be supplied with the exact `AssemblySourceKey`. The engine does not choose snapshots by timestamp or project proximity, so concurrent sessions cannot silently acquire a shared project's snapshots through heuristic matching.

Validated before/after snapshots are stored without flattening. Git-derived `FileChange` rows compare the before and after file observations, use `GitComparison`, and always use ambiguous attribution. An unchanged pre-existing dirty file is not emitted as a session change. Branch or detached-state changes suppress derived file changes and make Git completeness partial. Missing, truncated, or one-sided snapshots remain partial/unknown; a proven non-Git project is `not_applicable` and still produces a valid session.

Current normalized capture has no production `file.changed` event. Direct provider file attribution therefore remains unavailable until the later production normalization/Git briefs.

## Capture capabilities and completeness

Capability values come from captured `session.started` capability reports. If start evidence is absent, the established client-surface matrix is used: CLI prompt/interrupt are supported; Desktop and unknown surfaces keep both unknown. Observing another event form never upgrades an unknown capability.

Completeness is calculated deterministically across lifecycle, prompt, response, tools, commands, files, Git, and interrupts:

- present expected evidence is complete;
- supported expected prompt/response evidence that is absent is missing;
- unknown capability plus absent evidence stays unknown;
- activity dimensions with no observed activity are not applicable when the capability is known;
- unmatched execution pairs are partial;
- explicit gaps or truncation downgrade otherwise complete capture dimensions to partial.

The domain's existing rule derives overall `complete/partial/degraded`. No confidence percentage or model judgment is introduced.

## Persistence and transactional guarantees

Migration `0003_session_assembly.sql` adds:

- `assembly_groups`: the stable source group, deterministic session ID, `assembled/deleted` state, assembly version, evidence fingerprint, and timestamps;
- `ingress_assembly_state`: one explicit `assigned` or `ignored` state per consumed ingress record.

Absence of a state row means unassigned. Assigned evidence is retained; assembly never deletes ingress. `AssemblyRepository::persist_assembled_session` starts `BEGIN IMMEDIATE`, rechecks that every input row is still unassigned and that the complete current group was loaded, inserts the validated session through the existing relational writer, inserts the assembly group, and assigns every ingress row before commit. Any constraint or write failure rolls back all three effects.

Normal repeated assembly sees no unassigned candidate and creates nothing. If late evidence arrives after a group was assembled, it is retained and marked `arrived_after_assembly` rather than mutating the aggregate. Revisioned amendments are deferred to Brief 11.

Deleting an assembled session first changes its durable group state to `deleted`, then deletes the aggregate and cascading child rows. Its ingress remains assigned, so an ordinary run cannot recreate it. Late evidence for that source is retained as `deleted_session`. There is no user-facing or automatic reassembly path in this brief; version/fingerprint metadata allows a later explicit migration or administrative operation.

Project deletion marks its assembly groups deleted while deleting owned ingress and sessions. These guards are local crash/replay semantics, not cloud synchronization.

## Bounds and ambiguity

One call accepts at most 25 candidate source sessions and marks at most 100 unidentified records. Each candidate is limited to 20,000 ingress records and 20 MiB of serialized normalized event/sensitivity/source metadata. Counts and sizes are checked before loading, and the canonical aggregate is checked against the same byte bound before persistence. Oversized groups are deferred with evidence unchanged; nothing is silently discarded.

Errors are fixed structured variants for invalid relationships, unsupported schema, domain validation, ambiguity, bounds, and sanitized storage failures. They never contain prompts, commands, tool output, code, paths, or SQL parameters.

## Verification

Synthetic tests cover:

- complete CLI lifecycle, prompt, turn, tool, command, response, persistence, and traceability;
- failed command followed by a successful retry, retaining both;
- interrupted open tool/command with unknown exit code;
- missing session end producing an incomplete aggregate;
- Desktop prompt/interrupt capability remaining unknown;
- interleaved concurrent source sessions with no cross-contamination;
- explicit capture-gap preservation and completeness degradation;
- explicit Git before/after context, unchanged dirty baseline, and ambiguous Git attribution;
- valid no-Git assembly;
- repeated-run idempotency and deleted-session non-recreation;
- missing source identity, ambiguous duplicate boundaries, and bounded deferral;
- transaction rollback when an association constraint fails after the session insert step;
- clean migration and upgrade from populated Brief 03 schema.

On the current Apple Silicon development host, debug-build pure reconstruction measured approximately 1.7 ms for 10 events, 1.5–1.8 ms for 100 events, and 3.8–4.2 ms for 500 events. These are representative unit-fixture timings, not release benchmarks.

## Limitations

- The callable engine is not connected to desktop startup, a background scheduler, Tauri IPC, or UI.
- Stable external source session identity is required. Provider-thread continuation episodes and late-event revisioning remain Brief 11 work.
- Git snapshots require explicit safe source-session correlation from a future runtime; no timestamp-nearest matching exists.
- Current capture cannot directly attribute file events and often lacks provider timestamps, structured exit codes, durations, and semantic session-stop reasons.
- The engine trusts the persisted normalized-evidence boundary and does not re-sanitize content. Brief 05 guards file/path input upstream; Brief 06 redacts supported capture content and validates it again at SQLite ingress. Existing pre-Brief-06 rows are not retroactively scrubbed.
- No analysis, LLM, concept, lesson, quiz, challenge, knowledge, review, notification, account, sync, or additional-provider behavior is present.

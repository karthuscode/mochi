# Mochi session event schema

Baseline: 1.0. Authority: normalized events and capture lifecycle. This is a documentation contract, not application source. Domain events are independent of Codex/OpenAI formats. [DATA_MODEL.md](DATA_MODEL.md) maps persistence; [security](../security/PRIVACY_SECURITY.md) governs content.

## Identity and episode semantics

`Project` is an approved canonical local root with an opaque UUID and safe display alias. An absolute root is local-only metadata. `Session` is a bounded Mochi work episode, with UUID, project ID, source descriptor, provider thread reference (local only), and optional `continuationOfSessionId`.

Never equate a provider thread to one permanently growing Mochi session. Startup/resume while an episode is live continues it; compaction is an activity/capability detail, not a new episode. After finalization, new provider activity creates a continuation episode. Provider child/agent references correlate tool activity inside an episode unless the spike establishes a separate independent work boundary.

Create a provisional episode from the first approved identifiable activity if start is missing. Mark the start gap; a later start event does not duplicate it. Without usable source identity/project association, do not guess across sessions; reject content and expose metadata diagnostics. Correlation rules must be proven in Brief 01.

## Envelope

| Field | Type / rule |
|---|---|
| `schemaVersion` | Integer, exactly 1 |
| `id` | UUID minted once at helper ingress; retained on delivery retry |
| `sessionId` | Mochi episode UUID; mapping made by adapter/importer before storage |
| `projectId` | Approved Project UUID |
| `source` | `{provider: string, adapterVersion: string, transport: string, clientSurface: cli/desktop/unknown}`; V1 provider `codex` |
| `sourceEventId` | String or null; stable provider ID when available; local only |
| `sourceSequence` | Nonnegative integer or null; do not manufacture provider ordering |
| `occurredAt` | RFC3339 UTC timestamp or null if source does not provide a reliable time |
| `receivedAt` | RFC3339 UTC timestamp assigned at ingress |
| `sequence` | Positive integer, unique monotonic import order within episode |
| `eventType` | Discriminator from the table below |
| `origin` | `provider`, `git`, or `mochi` |
| `payload` | Strict object for discriminator; no arbitrary provider blob |
| `sensitivity` | `{classification: "sanitized" | "metadata_only", redactionCount: integer, rulesVersion: string, policyRevision: integer, truncated: boolean}`; new Brief 06 capture uses `redaction-v1`, while safely imported older evidence may retain its recorded version |

The original sketch's `timestamp` is resolved to source and receipt timestamps; `projectPath` is resolved to `projectId` plus local-only Project root. This avoids exporting/sending absolute paths. `classification=sanitized` describes processing, not a guarantee that all secrets were detectable. Suspected content that cannot be safely processed is discarded; only metadata remains.

All content-bearing strings pass the shared sanitizer before spool/storage. Canonical payloads reject unknown properties. Unknown future envelope version is dropped with safe metadata rather than interpreted as version 1. Unknown provider event types are handled inside the adapter, not stored verbatim. Null/absent data means unknown, never success/false/zero by implication.

## Payload contracts

Fields listed are required unless `?` marks optional; nullable values explicitly indicate unavailable data. References are opaque local IDs. Text limits follow the architecture bounds.

| eventType | Payload fields / semantics |
|---|---|
| `session.started` | `reason: startup/resume/observed`, `capabilities: capability-report`, `startBoundaryKnown: boolean` |
| `user.prompt` | `text: sanitized-string`, `turnRef: string|null` |
| `agent.message` | `text: sanitized-string`, `turnRef: string|null`, `messageKind: progress/final/unknown` |
| `tool.started` | `toolCallRef: string`, `toolKind: string`, `summary: sanitized-string|null`, `turnRef: string|null`; no raw arguments |
| `tool.completed` | `toolCallRef: string`, `status: succeeded/failed/cancelled/unknown`, `summary: sanitized-string|null`, `durationMs: integer|null` |
| `permission.requested` | `toolKind: string`, `toolCallRef: string|null`, `turnRef: string|null`, `summary: sanitized-string|null` |
| `file.changed` | `path: approved-relative-path`, `change: added/modified/deleted/renamed/unknown`, `oldPath?: relative-path`, `evidenceRef: string|null`, `attribution: provider_reported/observed/ambiguous` |
| `command.executed` | `commandRef: string`, `display: sanitized-string`, `workingDirectory: approved-relative-path|null`; never executable instruction |
| `command.result` | `commandRef: string`, `exitCode: integer|null`, `output: sanitized-string|null`, `durationMs: integer|null` |
| `test.result` | `runRef: string`, `status: passed/failed/cancelled/unknown`, `passedCount: integer|null`, `failedCount: integer|null`, `summary: sanitized-string|null`, `evidenceRef: string|null` |
| `error.encountered` | `category: tool/test/integration/runtime/unknown`, `summary: sanitized-string`, `relatedEventId: UUID|null`; no raw stack/path dump |
| `turn.completed` | `turnRef: string|null`, `status: succeeded/failed/cancelled/unknown`; activity boundary, not guaranteed session end |
| `session.stopped` | `reason: completed/user_finalized/interrupted/idle_confirmed/unknown`, `endBoundaryKnown: boolean` |
| `context.compacted` | `phase: before/after`, `turnRef: string|null`; records an exposed lifecycle boundary, not hidden pre-compaction content |
| `capture.gap` | `reason: missing_start/missing_end/paused/overflow/corrupt/unsupported/restart/ambiguous_attribution/invalid_input/sanitization_failed`, `from: timestamp|null`, `to: timestamp|null`, `droppedCount: integer|null` |

Capability-report keys are those in [ARCHITECTURE.md](ARCHITECTURE.md); values are `supported/unsupported/unknown`. The adapter emits only what it can establish. Parsed tests require reliable observed command/output evidence; do not infer “tests passed” from an agent message alone.

Brief 01B added the client surface, permission, compaction, unknown stop reason, and intake/sanitization gap states after real Codex payloads proved those distinctions necessary. In particular, `SessionEnd` did not expose a reliable semantic reason, so `unknown` is a required truthful state rather than an inferred completion.

## Example normalized event

```json
{
  "schemaVersion": 1,
  "id": "f06731de-14ed-4a0b-a167-9e700875dd55",
  "sessionId": "be29f427-0f53-44fd-873f-de02cb1059ee",
  "projectId": "c63cd799-55e3-4d50-9982-30ba0da9edb3",
  "source": {"provider": "codex", "adapterVersion": "alpha-fixture", "transport": "fixture", "clientSurface": "unknown"},
  "sourceEventId": "fixture-turn-1-stop",
  "sourceSequence": null,
  "occurredAt": "2026-09-13T10:30:00Z",
  "receivedAt": "2026-09-13T10:30:01Z",
  "sequence": 7,
  "eventType": "turn.completed",
  "origin": "provider",
  "payload": {"turnRef": "fixture-turn-1", "status": "succeeded"},
  "sensitivity": {"classification": "metadata_only", "redactionCount": 0, "rulesVersion": "1", "policyRevision": 1, "truncated": false}
}
```

`transport=fixture` is solely an example, not a supported production integration claim.

## Delivery, order, and deduplication

The helper's sanitized **spool ingress record** carries `schemaVersion`, ingress `id`, approved `projectId`, `source`, local-only provider thread/turn references, optional source event ID/sequence, timestamps, discriminator, sanitized payload and sensitivity. It intentionally omits core-owned `sessionId` and `sequence`. This small strict ingress contract is versioned separately from the completed envelope if it changes. Brief 03 persists it unchanged as preassembly evidence after rechecking local tracking policy; persistence does not resolve source-to-episode mapping. Brief 04 owns that mapping and canonical `SessionEvent` construction. This allows safe capture while the core is closed without a helper-side SQLite writer or guessed episode identity.

Brief 02 implements that completed provider-independent envelope in `crates/domain`. It adds an optional core-owned `turnId` to the envelope after correlation and groups provider details inside bounded `source` and `provenance` objects. Brief 04 uses the persisted ingress `receiveSequence` as canonical session-event `sequence`; selecting one stable source-session group preserves monotonic receive order and any numeric gaps without sorting by timestamp or manufacturing provider order. Concurrent source sessions may interleave, so non-contiguous receive numbers alone are retained as observable ordering rather than treated as proof of loss; explicit `capture.gap` evidence degrades completeness. The domain constructor preserves supplied event order and validates identity/sequence uniqueness without reconstructing or sorting the stream. Raw Codex payload fields never enter the completed envelope.

- At-least-once spool delivery; exactly-once database effects via unique ingress `id` and, when supplied, a versioned hash of provider/adapter/transport/surface/thread/sourceEventId metadata. Payload text is never a dedupe key.
- Import commits accepted, duplicate, tombstoned, or safely rejected outcomes before acknowledging exact root-bound spool tokens. Acknowledgement failure leaves a harmless replay.
- Mint IDs once before sanitized spool write; replay preserves them. If a provider has no event ID, helper retry reuses its ingress ID. Do not dedupe solely by text hash: identical prompts can be real separate events.
- `sequence` records import order. Display occurrence time where reliable; fall back to receipt/sequence. Late events never reverse a finalized lifecycle silently.
- Missing pairs produce unknown duration/status, not synthetic tool completion. Persist safe gap metadata. Future corrupt records are discarded without logging their raw content.
- Brief 04 remains the historical one-shot foundation. The implemented Brief 11 episode engine now revises live aggregates, treats turn Stop as idle, closes on explicit source end/user Finish, starts continuation episodes for new work, and records late non-start amendments with a higher revision. Associations and aggregates commit atomically; input changes make prior learning stale. Receive-sequence gaps remain preserved without synthetic events.

## Session lifecycle and analysis status

Capture states: `active`, `idle`, `finalizing`, `finalized`, `interrupted`. Coverage is orthogonal: `complete`, `partial`, or `metadata_only`; complete means expected declared coverage, not all possible provider events.

| Trigger | Result |
|---|---|
| Reliable start or first identifiable approved activity | `active`; provisional start records missing boundary |
| Provider turn completes | May become `idle`; does not finalize thread automatically |
| 15 minutes with no observed activity | `idle`, show “Finish session?”; no automatic completed claim |
| Proven session end or user Finish | `finalizing` → final Git snapshot → `finalized`; stopped reason explicit |
| New activity before finalization | `active` in current episode |
| New activity after finalization | New continuation episode and baseline |
| Core restart with uncertain old episode | `interrupted`, missing-end gap; resume same live episode only with reliable correlation, otherwise user finalize/new episode |
| Pause/disconnect | Gap; stop new content; user can finalize partial episode |

Analysis statuses: `not_requested`, `awaiting_consent`, `queued`, `running`, `ready`, `insufficient_context`, `failed`, `cancelled`, `stale`. Only finalized/interrupted user-finalized episodes may be analyzed. No automatic remote request while idle/live. A local ready session is not a ready lesson.

## Validation and limits

Reject invalid UUIDs, timestamps, negative counts/durations, duplicate object keys, absolute/out-of-root payload paths, invalid enum values, and oversized envelopes. Provider raw read cap 1 MiB; event serialized cap 64 KiB; text field 8 KiB; per-episode 20,000 events or 20 MiB. The shared `redaction-v1` engine sanitizes the full bounded input before truncation; spool writes and SQLite ingress validate the resulting record again. If redaction/validation cannot safely complete, emit a metadata-only gap where possible and drop content. Exercise replay, missing boundaries, late events, malformed input, overlapping sessions, and pause in [validation](../implementation/VALIDATION_PLAN.md).


## Authorized production delivery amendment

The app reads only its separate production spool and runs bounded import/episode assembly. The helper command reads current private policy under a lease before payload persistence; missing/stale/revoked consent drops content. Import validates strict bounded JSON/identity and commits before exact-token acknowledgement. Durable spool eviction counters warn of incomplete coverage without manufacturing a session event. Startup never installs hooks or imports the old prototype spool. Session deletion tombstones associated ingress before deleting it; new source work can still form a new continuation. [Current implementation](../implementation/INTERNAL_CLI_MVP.md).

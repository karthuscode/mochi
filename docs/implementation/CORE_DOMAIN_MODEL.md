# Brief 02 — Core domain model

Implemented: 2026-09-17. Scope: the provider-independent in-memory coding-session contract. This brief adds no persistence, importer, session assembler, learning logic, or product UI.

## Implementation and ownership

`crates/domain` is the authoritative Rust model for behavior that will later be persisted. `packages/domain` contains readonly TypeScript contracts, a lightweight boundary parser, the deterministic completeness helper, and a shared JSON fixture checked by both runtimes. Neither package imports React, Tauri, filesystem, Git command, SQLite, HTTP, OpenAI, or Codex types.

The implemented entities and value objects are:

- `Project`, `RepositoryIdentity`, and strongly named project/session/turn/event/tool/command/file-change IDs;
- `CodingSession` as the validated aggregate, with `SessionStatus` and minimal transition validation;
- `SessionTurn` with optional source identity, prompt, response, and timestamps;
- canonical `SessionEvent`, safe provenance, normalized payload variants, and sensitivity metadata;
- `ToolExecution` and `CommandExecution`, including interrupted/incomplete and unknown states;
- `FileChange`, with observation source separate from attribution;
- `GitSnapshot`, `GitFileState`, and available/unavailable `GitContext` retaining before and after evidence;
- `CaptureCapabilities` and dimensional `CaptureCompleteness`.

`CodingSession` and canonical `SessionEvent` each have an explicit schema version because they cross serialization boundaries and will be persistence/import contracts. Small nested values inherit their containing contract version rather than carrying redundant versions.

## Boundaries

Provider payloads end inside `SessionSource`. The current Rust capture crate continues to own private `CodexHookPayload` parsing and sanitized spool ingress. A future importer will map ingress identity and `receiveSequence` to a canonical session ID and per-session `sequence`; a future assembler will correlate events into turns and execution lifecycles. The domain model performs neither job and never stores raw provider payloads.

The model records what was observed. `userPrompt` is captured evidence and can be absent. There is no `userIntent` field. Later inferred intent belongs to the analysis layer and must retain its evidence references.

## Invariants

Validated construction and deserialization enforce these rules:

- IDs are typed UUIDs and aggregate references must resolve to the same session/project.
- Timestamps are canonical RFC3339 UTC values. An end cannot precede its start.
- `completed`, `interrupted`, and `failed` sessions require `endedAt`; `active` sessions forbid it. `incomplete` may omit an end boundary.
- Completed turns require an end time. A turn may omit source identity or prompt.
- Completed/failed tool and command executions require a completion time. Interrupted/incomplete executions may remain open, and command `exitCode` may be absent.
- Event IDs and positive per-session sequences are unique. Constructors preserve supplied order and do not reorder by timestamps.
- Turn/event/tool/command references must point inside the aggregate.
- Git and file paths are approved-root-relative where they represent captured files; rename records require a previous path.
- Available Git before/after snapshots share a repository root and chronological order. Either snapshot can explicitly report truncation.
- Only `active` can transition to a different terminal session state through the minimal transition helper. Session assembly policy remains deferred.

Public Rust deserialization routes through `CodingSession::new`, so invalid serialized aggregates are rejected. TypeScript uses `parseCodingSession` for untrusted IPC/fixture values; Rust remains authoritative for full invariant enforcement.

## Capability and completeness semantics

Capability describes what a source/client can expose: `supported`, `partial`, `unsupported`, or `unknown`. Completeness describes the evidence in one session: `complete`, `partial`, `missing`, `unknown`, or `not_applicable`. Neither implies that an activity occurred.

Overall completeness is derived from the dimensions and is never supplied by a model:

1. Any `missing` dimension produces `degraded`.
2. All dimensions `complete` or `not_applicable` produce `complete`.
3. Every other combination produces `partial`.

Git truncation is represented on each snapshot and the session Git dimension must consequently be `partial`; the type does not silently derive attribution or claim a complete diff. Before and after snapshots remain separate so pre-existing dirty state is visible. File entries can carry already-sanitized bounded content, its hash, staged/unstaged state, byte count, or an explicit omission reason. The domain never authorizes a read or bypasses the privacy layer.

## Sanitized examples

A complete CLI session can state that prompt and interrupt capture are supported while marking interrupts not applicable when none occurred:

```json
{
  "source": { "provider": "codex", "clientSurface": "cli" },
  "status": "completed",
  "turns": [{ "userPrompt": "Add a bounded parser." }],
  "captureCapabilities": {
    "userPrompt": "supported",
    "interrupts": "supported"
  },
  "captureCompleteness": {
    "prompt": "complete",
    "interrupts": "not_applicable"
  }
}
```

The complete shared fixture also preserves a dirty baseline: `src/preexisting.ts` exists in both snapshots while `src/parser.ts` first appears in the final observation. Its file attribution is `ambiguous`, so the model does not claim the whole diff was caused by Codex.

The currently proven Desktop route is represented without inventing a prompt or interruption result:

```json
{
  "source": { "provider": "codex", "clientSurface": "desktop" },
  "status": "incomplete",
  "turns": [{ "sourceTurnId": null, "userPrompt": null }],
  "captureCapabilities": {
    "sessionLifecycle": "supported",
    "userPrompt": "unknown",
    "agentResponse": "supported",
    "toolActivity": "supported",
    "commands": "supported",
    "interrupts": "unknown"
  },
  "captureCompleteness": {
    "sessionLifecycle": "partial",
    "prompt": "unknown",
    "interrupts": "unknown"
  }
}
```

Synthetic fixtures cover `CLI_COMPLETE_SESSION`, `CLI_INTERRUPTED_SESSION`, `DESKTOP_PARTIAL_PROMPT_SESSION`, `DIRTY_GIT_SESSION`, and `NO_GIT_SESSION`. They contain only invented paths, text, IDs, versions, and hashes.

## Deferred work

Brief 03 must map this model to transactional SQLite storage without treating database rows as the domain model. It must add migrations, repositories, exact deduplication/import transactions, restart behavior, and deletion semantics. Event-to-session assembly, production capture, privacy services, analysis, learning entities, and UI remain in their later briefs.

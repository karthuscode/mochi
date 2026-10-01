# Brief 08 — Codex integration installer

Implemented and validated native boundary: 2026-10-01. **The real CLI install/trust/task/disconnect gate passed on macOS Apple Silicon with Codex CLI 0.151.0.** This completes Brief 08 for the approved CLI-first internal MVP plan, not the MVP or production capture.

## Objective and dependencies

Allow an explicitly approved project hook change, preserving unrelated configuration and supporting safe upgrade, rollback, and disconnect. This addresses P02, P05, P14 and depends on Brief 07 detection and Briefs 05/06 privacy. The installed CLI reports `codex-cli 0.151.0`; this matches the previous feasibility baseline. The real task below proves invocation of the new installer's command groups for the exercised events.

Controlling specifications: [privacy](../security/PRIVACY_SECURITY.md), [architecture](../architecture/ARCHITECTURE.md), [event contract](../architecture/SESSION_EVENT_SCHEMA.md), and [roadmap](V1_ROADMAP.md). Official [Hooks](https://learn.chatgpt.com/docs/hooks) and [configuration](https://learn.chatgpt.com/docs/config-file/config-advanced) were fetched on 2026-10-01. Installation uses the documented event → matcher group → command handler hierarchy; trust is reviewed through Codex `/hooks`, never inferred from a local database or bypassed.

## Implemented interface and behavior

`crates/integration` now separates read-only detection from `CodexInstaller`. The native-only `InstallRequest` contains an approved canonical project, opaque project UUID, helper location, spool location and positive policy revision. The caller must obtain project/local capture consent before applying an installation. No frontend can currently call this API.

- `prepare_install` returns a memory-only `InstallPlan` and safe owned-entry `InstallPreview`; it does not edit Codex configuration.
- `prepare_disconnect` and `prepare_rollback` prepare equally reviewable inverse edits.
- `apply(plan, approved_plan_id)` requires the exact preview token, rechecks configuration/receipt/helper fingerprints, then applies only owned changes.
- `cleanup_backups(now)` performs bounded explicit retention maintenance; construction also runs it. No background scheduler is introduced.

Targets are project-local `<approved-root>/.codex/hooks.json` only. User-level configuration and Codex trust/account/session storage are not edited. Each of the 12 previously observed capture events receives a separate unfiltered command group with a three-second timeout. All arguments are literal shell-quoted native metadata. The helper command remains the existing prototype protocol; production authorization/pause remains Brief 09 work.

The development helper exposes `connect`, `disconnect` and `rollback-integration`. Each prints only the concrete Mochi preview and requires `approve <plan-id>` on stdin. EOF, a wrong token or an overlong response causes no configuration change. These are explicit developer commands, separate from silent `capture`; they are not an onboarding UI.

## Preservation and failure boundaries

JSON is read through a regular-file/no-follow boundary capped at 256 KiB. Parsing rejects duplicate keys, malformed groups, unsupported shapes and depth/container overflow rather than silently dropping entries. Existing non-hook bytes and formatting remain byte-for-byte intact. Only the `hooks` value is rendered; unrelated hook values are preserved semantically. New groups append without modifying existing matchers.

Inline hooks in adjacent `config.toml` are parsed with TOML and rejected for manual review, including quoted table names. The installer never rewrites TOML or combines representations in one layer. Existing unowned Mochi commands also require manual review. Edited or duplicated owned handlers are conflicts, not permission to remove user changes.

Per-target bounded locks serialize Mochi writers. Preview-time and immediate pre-replacement hashes detect intervening user edits. Atomic same-directory replacement and file/parent syncing prevent partial JSON. An unrelated editor does not participate in Mochi's lock; a last-instant external write remains an OS filesystem race, not a claimed compare-and-swap guarantee.

An app-owned private journal records before/after hashes and old/new owned entries before changing the target. A restart resolves either recognized configuration state without restoring an old whole-file backup. An ambiguous state reports `RecoveryRequired` and leaves configuration untouched. Rollback removes current owned entries and reapplies previous owned entries; subsequent unrelated edits survive. Disconnect removes a newly created file only if no user data remains. It never removes the project `.codex` directory or touches retained Codex trust.

Errors are fixed variants and contain no configuration contents, underlying errors, helper output, captured text or paths. Public preview paths/commands are local operational metadata expressly shown for approval; they are not diagnostics, learning input or export.

## Private state and dependencies

Integration state uses owner-only directories (`0700`), receipts/locks/backups (`0600`) and no SQLite migration. Receipts contain only schema version, target identity, Mochi-owned command, helper fingerprint, ownership flags and pending-operation metadata. Plans hold original bytes only in memory and are not serialized or debug-formatted.

A changed existing config gets an exact private recovery backup. It may contain preexisting user secrets, under the explicit configuration-recovery exception in the privacy specification; it is never sanitized into a lesson, indexed, printed or uploaded. Timestamp/UUID backup names expire after seven days; at most 1,024 entries are inspected per maintenance call. Expiry cleanup runs when the installer is next used, not while Mochi is stopped. This operational limitation is disclosed in the developer flow. There is no application encryption or forensic-erasure claim.

Dependencies reuse pinned workspace libraries: `fs2` for locks, SHA-256 for preconditions, UUID for opaque plans/artifacts, `libc` for no-follow/process-group controls, and TOML for safe inline-hook detection. JSON raw values permit surgical replacement without rewriting unrelated config. No ORM, daemon or network dependency is added.

## Detection corrections

Read-only detection now inspects actual handler structure instead of arbitrary strings. All required events, literal helper arguments and unrestricted matchers are needed for a configured result. Descriptions, shell substitutions, incomplete event coverage and duplicate-key JSON cannot establish readiness.

Version-command deadlines include stdout completion. Each Unix command has an isolated process group; descendants holding the output pipe are terminated on deadline/error. No unbounded reader join remains. Desktop remains partial, and unknown trust remains unknown.

## Validation evidence

Automated fixtures cover preview/approval refusal, idempotent installation, exact non-hook byte preservation, unrelated hook preservation, upgrade, rollback after user edits, edited-handler conflict, original/new-file disconnect, config/helper concurrent edits, two prepared writers, malformed/duplicate/oversized JSON, inline TOML, symlink rejection, owner permissions, backup expiration, and restart before/after config replacement. A real helper executable test proves explicit connection approval, silent synthetic capture and disconnect in an isolated temporary project. It does **not** invoke a real Codex coding task.

The restart baseline passed all nine requested repository gates and produced a local unsigned macOS `.app`; the baseline is committed as `04698f6`. All nine gates also passed on the updated implementation (112 Rust + 18 frontend tests and local unsigned macOS `.app`). Outcomes are recorded in [MVP implementation](MVP_IMPLEMENTATION.md).

## Real CLI validation

After the user approved the concrete temporary-project preview, the release helper installed exactly the 12 displayed project-local handlers. Codex recognized all 12 in its normal `/hooks` UI. Each Mochi handler was reviewed and trusted individually; eight unrelated new/changed hooks were left untrusted. No trust bypass or update was used.

The first task attempt was rejected by the provider because the configured `gpt-6.1-sol` model was unsupported with this CLI's ChatGPT login. The CLI's model menu offered `gpt-5.6-sol`; a fresh process used that model via a process-local `-m` argument, without editing model configuration. It changed a synthetic `add` implementation from subtraction to addition and successfully ran `python3 test_calculator.py`.

The successful task produced 19 normalized records under one external session identity: one session start, one prompt, four tool starts, three command executions, four tool completions, three command results, one final agent message, one turn completion and one session stop. Source hooks observed were SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop and SessionEnd. PermissionRequest, Interrupt, compaction and subagent events were not exercised in this installer trial; their earlier capability evidence is not expanded by it. No Git/file-change coverage or production lifecycle is claimed.

Across the trust-review session, rejected-model attempt and successful task, the isolated spool contained 23 records with contiguous receive sequences 1–23, the approved project ID and policy revision, and external session IDs. Record files were `0600`, the spool directory `0700`; classification was sanitized or metadata-only, with no truncation or capture-gap records. These are observations of this trial, not a new general privacy audit. The helper emitted no diagnostic content into the coding flow.

After all CLI processes exited, the exact preview-approved disconnect removed the newly created hook file. The project `.codex` directory and Codex's ordinary retained trust state were preserved. The validation automation did not directly inspect or edit user-project files, global hook configuration, account settings, private session storage or API keys; Codex used its existing login and normal trust mechanism. Temporary captured records and configuration receipts remain outside the repository; only aggregate validation facts are recorded here.

This follow-up changed documentation only. `pnpm format:check` and `git diff --check` passed; the nine full gates above apply to the unchanged installer source at `34fd1cf` and were not repeated for this documentation update.

## Acceptance and next brief

Brief 08's native implementation, automated failure-boundary checks and real CLI install/trust/task/disconnect gate passed. Brief 09 — Production Event Bridge is the next bounded implementation task. No trust bypass, transcript parser, API-key read, private account copy or user-project edit was used.

No production capture policy, automatic importer/assembler, persistent jobs, IPC, product UI, remote analysis or learning was implemented here. These remain the next bounded units of the approved MVP plan. The next production bridge must fail closed on missing/revoked policy before any payload write; installation success alone must never be reported as production readiness.

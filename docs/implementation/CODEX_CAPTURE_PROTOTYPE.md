# Codex passive capture prototype

Status: Brief 01B complete on 2026-09-17. Desktop gate: **PARTIAL**.

This report records the bounded prototype implemented after the Brief 01A conditional-go result. It is evidence for later production work, not a claim that Mochi capture, persistence, onboarding, or learning features are ready for users.

## Desktop gate result

The installed Codex Desktop 26.908.40834 does invoke trusted project lifecycle hooks for a task whose saved project is the Mochi checkout. The mandatory pre-implementation gate used a temporary minimal observer in a controlled synthetic repository. After the gate allowed implementation, the representative test was repeated with the Rust helper.

The preliminary gate observed `SessionStart`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`, and `SessionEnd`. It captured shell and patch activity, a failing test followed by a fix and passing test, the final response, and a preserved pre-existing dirty file. The initial task and a follow-up delegated through the Desktop task API produced no `UserPromptSubmit` hook. App Server diagnostic reads represented the delegated input as a `functionCallOutput`, not a direct `userMessage`, so this harness cannot establish whether a prompt typed directly into the Desktop UI emits the hook. Desktop interruption was not practical in this harness. No screen scraping, Accessibility capture, UI automation, undocumented local transcript parsing, or network interception was used.

The Rust-helper Desktop repeat produced one session and one turn with 27 normalized records:

- one session start and one session end;
- five tool starts, including `Bash` and `apply_patch`;
- seven tool completions;
- four command starts and six command results;
- one permission request;
- one sanitized final agent message and one turn completion;
- zero user-prompt events.

The task reproduced the calculator failure, fixed the implementation, created the requested file, passed the follow-up test, and preserved the dirty file checksum. Two `PostToolUse` references in a parallel command batch had no observed `PreToolUse` reference. The adapter retained those references as observed and did not invent pairs.

This is Outcome B from the brief. Desktop is **PARTIAL**. Session identity, activity, commands, file-changing tools, final response, permission, completion, and end are usable. Prompt and interrupt capabilities are `unknown`, and Git remains the independent source of final file state. The missing prompt can be handled later as partial coverage, with bounded reconstruction from observable activity/final response and optional officially supported reconciliation; it does not justify UI scraping or an undocumented storage dependency.

## Supported Codex surfaces

| Capability | CLI 0.151.0 | Desktop 26.908.40834 |
|---|---|---|
| Session identity and project association | Supported | Supported in tested saved-project task |
| Session start/end | Supported | Supported |
| User prompt | Supported | Unknown; delegated Desktop input emitted no hook |
| Final agent message and turn boundary | Supported | Supported |
| Tool lifecycle | Supported with observable gaps | Supported with observable gaps |
| Command activity | Supported; result fields are partly unstructured | Supported; result fields are partly unstructured |
| Permission request | Supported by hook contract and prior probe | Observed |
| File events | Unknown; use Git and file-changing tool evidence | Unknown; use Git and `apply_patch` evidence |
| Interrupt | Observed and supported | Unknown; not exercised |
| Compaction | Hook names available; live behavior unexercised | Hook names available; live behavior unexercised |
| Subagent association | Unvalidated | Unvalidated |

Capabilities live in `SessionSourceCapabilities`; the rest of the domain does not branch on CLI versus Desktop field names.

## Prototype architecture

```text
existing Codex CLI or Desktop task
        │ trusted lifecycle hook, JSON on stdin
        ▼
standalone mochi-hook process
        │ validate approved root and envelope
        │ sanitize in memory
        │ normalize through CodexSessionSource
        ▼
private bounded atomic JSON event spool
        │ later importer (not implemented in this brief)
        ▼
canonical Mochi SessionEvent

read-only Git baseline + final snapshot
        │ parallel evidence; not assembled in this brief
        ▼
future GitSessionContext / session assembler
```

`apps/mochi-hook` is independent of React, Tauri, and the Mochi window. `crates/capture` owns the provider boundary, normalized spool-ingress model, sanitizer, spool, capability model, and Git reader. Raw `CodexHookPayload` is private to `CodexSessionSource`; no provider object crosses into the normalized model.

The spool record intentionally has no canonical Mochi `sessionId` or per-session `sequence`. It carries a unique ingress UUID, approved project UUID, bounded source descriptor, external session/turn/tool references, optional stable source-event key, local receive timestamp, and monotonic capture-stream sequence. A future importer will partition episodes, deduplicate, and assign the canonical envelope without the helper guessing application state.

## Rust helper

The release helper exposes three prototype commands:

```text
mochi-hook capture --project-id UUID --approved-root PATH \
  --client-surface cli|desktop|unknown --policy-revision N [--spool-root PATH]

mochi-hook inspect [--spool-root PATH]

mochi-hook git-snapshot --project-id UUID --approved-root PATH \
  --role baseline|final --output PATH
```

`capture` reads at most 1 MiB from stdin, never prompts, emits no normal stdout/stderr, catches panic, ignores persistence failure at the hook boundary, and exits successfully so Mochi cannot block the coding turn. `inspect` is developer-only and prints strict normalized/sanitized records. `git-snapshot` writes a private atomic JSON snapshot for an already approved repository.

The default production-shaped spool path is deterministic:

```text
~/Library/Application Support/dev.mochi.desktop/capture/v1/spool
```

Directories use mode `0700` and files use `0600` on Unix. Test-only runs use `--spool-root` under a temporary directory.

## Normalized event mapping

| Codex hook | Normalized event |
|---|---|
| `SessionStart` | `session.started` with observed start reason and capability report |
| `UserPromptSubmit` | sanitized `user.prompt` |
| `PreToolUse` | `tool.started`; `Bash` also yields `command.executed` |
| `PostToolUse` | `tool.completed`; `Bash` also yields `command.result` |
| `PermissionRequest` | `permission.requested` |
| `Stop` | sanitized final `agent.message` plus `turn.completed` |
| `Interrupt` | `turn.completed` with `cancelled` status |
| `SessionEnd` | `session.stopped` with `unknown` reason |
| `PreCompact` / `PostCompact` | `context.compacted` before/after marker |
| subagent or unknown hook | metadata-only `capture.gap` with `unsupported` reason |

The adapter preserves `tool_use_id` as the external tool reference and command reference. Pairs with the same provider reference correlate; unmatched starts or completions remain unmatched. Live CLI interruption produced `tool.started` and `command.executed` for `/bin/sleep 30`, followed by a cancelled turn and no fabricated tool completion.

Codex did not supply trustworthy event time, structured exit code, duration, or a reliable semantic session-end reason in the tested hook payloads. The prototype therefore leaves `sourceTimestamp`, exit code, and duration absent; completion status and end reason are `unknown` where required. `receivedAt` is Mochi's ingress time and is never presented as provider occurrence time.

The CLI regression produced 40 normalized records for one external session and three distinct turns: three prompts, two final messages, seven tool starts, nine tool completions, six command starts, eight command results, three turn completions, and the start/end boundaries. Four completion references had no observed start and one non-interrupted start had no observed completion; the interrupted sleep contributed the other incomplete start. This demonstrates why later assembly must use correlation IDs and explicit coverage gaps rather than positional pairing.

## Local spool and ordering

The spool writes one strict JSON record per atomic rename. A short cross-process lock reserves a persistent monotonic receive sequence; content writes occur after reservation so concurrent helpers do not hold the lock through disk persistence. Obvious provider deliveries get a stable `sourceEventId`, while every invocation keeps a distinct ingress UUID and sequence so later deduplication can be audited instead of silently dropping identical text.

Prototype limits are:

- 1 MiB raw provider input;
- 8 KiB per sanitized text value;
- 64 KiB serialized event record;
- 100 MiB total spool;
- seven-day maximum event age;
- 100 Git files, 64 KiB per Git file, and 2 MiB sanitized Git content per snapshot.

An oversized normalized record becomes a metadata-only `capture.gap` with `overflow` and `truncated=true`. Oldest/expired event files are removed, and only eviction count/time are stored in `spool-state.json`. A process can reserve a sequence and fail before its atomic event write, so a later reader must tolerate a receive-sequence gap. Pruning can be skipped while another writer holds the lock and is applied by a later invocation; bounds may therefore be transiently exceeded during an active burst.

## Privacy boundary

The original Brief 01B durable path was parse → validate → sanitize → normalize → atomic write. Raw hook JSON was never logged or spooled. Brief 06 replaced the prototype's secret rules with the shared [redaction engine](SECRET_REDACTION_ENGINE.md), retaining adapter-local masking of the approved project root, home path, and temporary root. The shared engine sanitizes the full bounded value before truncation; spool and SQLite ingress now add defensive validation.

Automated and binary-level tests used only synthetic credentials. A live CLI prompt contained a fake `OPENAI_API_KEY=sk-test_…` value; the durable prompt contained `[REDACTED:SECRET_ASSIGNMENT]`, and the original token was absent from every spool file. Nested sensitive fields and oversized payloads were also exercised. An injected sanitizer failure produced only a metadata `capture.gap`. Git snapshots apply hard path exclusions before reads and sanitize allowed text before hashing or writing it.

This sanitizer is intentionally incomplete. Brief 05 now supplies a shared file/path policy, but the sanitizer cannot recognize every secret or semantic private value and must be replaced/hardened in Brief 06 before production capture.

## Git baseline approach

`GitContextReader` has a read-only `GitCliContextReader` implementation. It requires the discovered repository root to equal the approved canonical root. Git runs with a cleared environment, no system/global configuration, hooks disabled, no external diff helper, argument arrays, a ten-second timeout, and a 2 MiB command-output cap.

Each baseline/final snapshot records a root placeholder, branch/detached state, HEAD, tracked and standard-ignore-respecting untracked paths, status, staged/unstaged flags, rename origin, size, binary/omission state, sanitized content, and a hash of that sanitized content. It never runs a mutating Git command.

The automated dirty-repository test captured an already modified file, then modified another file, deleted a file, renamed the dirty file, and added an oversized file. The pre-existing content hash matched after the rename; delete, rename, and size omission remained explicit. The real CLI and Desktop scenarios likewise preserved the pre-existing dirty file byte-for-byte. Hard pre-read exclusions cover `.git`, `.codex`, environment and key files, credential-named files, dependency/build/cache directories, binary files, symlinks, and oversize content.

## Failure behavior

- Malformed, empty, oversized, missing-identity, missing-tool-ID, out-of-root, and deeply nested inputs never panic. Hook mode returns without blocking Codex; where safe, it writes a metadata-only gap.
- A sanitizer failure writes no content.
- A spool permission, disk, serialization, or contention failure is silent to Codex. The event may be lost; the helper does not fabricate delivery.
- Atomic rename prevents partial event files. A crash after sequence reservation can leave an ordering gap.
- Duplicate delivery remains visible with a stable dedupe key and distinct ingress IDs.
- Two session identities remain separate even when captured into the same stream.
- A session with no end event remains in the spool; capture does not delete it.
- An interrupted tool remains started without a synthetic result or exit code.
- The main Tauri/React application was closed during the real CLI capture and was not involved in either hook path.

## Performance

Measurements used the optimized arm64 Mach-O helper (about 2.9 MiB) on the current Apple Silicon host. The Brief 01A Python probe median was approximately 41.75 ms.

| Case | Result |
|---|---|
| First invocation immediately after a fresh release link | 376.709 ms |
| Later fresh-spool invocations, 20 samples | 14.954 ms median, 18.061 ms p95, 19.815 ms max |
| Repeated small payload, 30 samples | 15.870 ms median, 18.024 ms p95, 20.864 ms max |
| Repeated 7 KiB payload, 30 samples | 14.771 ms median, 15.907 ms p95, 17.053 ms max |
| Simultaneous 20-process burst | all 20 retained; 177.764 ms wall time; 88.528 ms individual median, 165.303 ms p95, 173.812 ms max |

Steady-state invocation materially improves on the Python probe. The one-time post-link startup outlier is real and repeatable after rebuilding, but subsequent brand-new spool directories do not reproduce it. A production installer should investigate or absorb that first-execution cost in an explicit connection self-test. The burst test initially exposed event loss because the spool lock covered writes and pruning; narrowing the lock to sequence reservation and adding a concurrency regression test retained all 20 events.

## Development hook installation, verification, upgrade, and removal

This brief deliberately does not add an installer. Development installation is manual because overwriting a user's hook configuration is unacceptable.

1. Build `mochi-hook` in release mode and record its absolute path.
2. Choose the approved canonical project root, project UUID, client surface, policy revision, and spool root.
3. Inspect `<project>/.codex/hooks.json` if it exists. For each required event array, append a Mochi command hook; preserve every unrelated object and hook in place. Required prototype keys are `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Stop`, `Interrupt`, `PreCompact`, `PostCompact`, `SubagentStart`, and `SubagentStop`.
4. Start Codex in that project, open `/hooks`, review the exact command for each event, and trust only the Mochi entry through the normal Codex trust UI. Never use a trust bypass.
5. Run a synthetic task, then use `mochi-hook inspect --spool-root PATH`. Verify source/session/turn/tool references, monotonic sequences, sanitized content, and empty hook stdout/stderr.
6. After rebuilding or changing arguments, repeat `/hooks` review because Codex correctly marks the command modified.
7. To remove the prototype, delete only entries whose command exactly matches the owned Mochi helper configuration. Remove an empty group or file only when Mochi created it and no unrelated data remains. The prototype does not edit or revoke Codex's retained trust database.

The future installer must parse and merge transactionally, preserve formatting where practical, detect concurrent edits, keep private backups, test/rollback, support idempotent upgrade/removal, and surface required re-trust. Those requirements remain in Brief 08.

## End-to-end reconstruction evidence

The CLI controlled flow contained a session start, exact prompts, repository inspection, failing calculator assertion, implementation edit, requested note file, passing test, final response, interruption, and session end. The sanitized spool plus Git before/after evidence is sufficient for a future assembler to recover intent, important activity, failure and recovery, final changes, preserved pre-existing dirt, and completion coverage. The prototype intentionally does not assemble or analyze that evidence.

Desktop provides the same activity/change/completion evidence in the tested delegated path, but intent is partial because prompt visibility is unknown. That session must be labeled partial rather than silently promoted to complete coverage.

## Acceptance status and known gaps

| Area | Status | Evidence / limitation |
|---|---|---|
| Desktop hard gate | **PARTIAL** | Real trusted Desktop tasks emitted useful lifecycle/activity/final/end hooks; direct prompt and interrupt remain unknown |
| Rust helper and UI independence | **PASS** | Standalone release binary captured while the Mochi UI was not involved |
| CLI regression | **PASS** | Prompt, failure/recovery, edits, commands, interrupt, final response, and end observed |
| Provider boundary and normalized ingress | **PASS** | Private external payload type, provider-independent trait/model, provenance and capability flags |
| Ordering, duplicates, bounds, concurrency | **PASS** | Automated sequence/dedupe/rotation/overflow/missing-end tests and 20-process retained burst |
| Privacy boundary | **PASS for prototype scope** | Synthetic tokens absent from durable spool; sanitizer failure metadata-only; full production sanitizer deferred |
| Git context | **PASS for prototype scope** | Read-only bounded baseline/final snapshots, dirty-state preservation, exclusions and omission markers |
| Overall Brief 01B | **PARTIAL** | Prototype acceptance passes; Desktop surface remains explicitly partial |

Remaining gaps are limited to real evidence gaps and deferred brief boundaries:

- a prompt typed directly in Desktop and a Desktop interrupt still need an approved manual validation path;
- real streams showed unmatched tool-hook references, so later assembly needs explicit gap accounting and must never pair by order;
- compaction and subagent hooks are mapped conservatively but were not exercised live;
- Brief 05 strengthens the file/path boundary, but the sanitizer remains a prototype defense pending Brief 06;
- manual hook merge/removal is not a safe product installer;
- the spool is not SQLite and has no importer, session assembler, UI, consent workflow, or learning analysis;
- the one-time post-build process startup outlier and extreme burst latency need later performance hardening;
- Windows, Linux, client version 0.154.0+, clean-machine installation, signing, and notarization are untested.

## Recommendation

The hooks-plus-Git architecture does not require another redesign before the app/domain foundation continues. Proceed to Brief 02's review of the existing scaffold, then follow the roadmap into domain and persistence. Carry Desktop prompt/interrupt as capability `unknown`, retain partial-coverage semantics, and require later detection/installer/session-assembly briefs to validate direct Desktop prompts, surface unmatched hook evidence, and avoid any UI-scraping or undocumented-storage fallback.

Do not treat this recommendation as authorization to skip Briefs 05–12. Production capture still needs consent, detection, safe config merging, importer/persistence, full file filtering/redaction, and deterministic session assembly.

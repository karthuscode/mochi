# Mochi V1 architecture

Baseline: 1.0. Authority: components, boundaries, runtime ownership. Contracts: [events](SESSION_EVENT_SCHEMA.md), [storage](DATA_MODEL.md), [learning](../learning/LEARNING_MODEL.md), [security](../security/PRIVACY_SECURITY.md).

## System shape

```text
Existing supported Codex client
    → proven passive integration transport
    → mochi-hook helper: authorization + normalization + sanitization
    → durable sanitized local spool
    → Rust core: policy-checked import → preassembly evidence
    → session assembly → validated CodingSession
    → SQLite evidence/session repositories
    → consent gate + sanitized analysis bundle
    → AnalysisProvider → OpenAIAnalysisProvider (HTTPS, optional)
    → output validation + sanitization
    → debrief / assessments / evidence / due reviews
    ↔ typed Tauri IPC ↔ React desktop UI
```

The exact Codex transport is G01-gated. The helper is a small adapter-side executable/process, not an independent daemon or agent. Hooks are the preferred hypothesis; only proven supported behavior can be implemented. App Server is a documented integration surface, but operating a new harness does not prove passive observation of existing client work. See [decision register](../product/DECISIONS.md).

## Stack and repository layout

Tauri 2, React, TypeScript, Rust; pnpm and Vite. Brief 03 adds bundled SQLite through `rusqlite` and a dedicated Rust persistence crate. The supplied Brief 00 — Repository Foundation explicitly authorizes the minimal scaffold before G01, superseding the original docs-only Brief 00 sequencing for this task. Pinned versions, actual commands and OS validation limits are recorded in [FOUNDATION.md](../implementation/FOUNDATION.md).

```text
apps/desktop/          Minimal React shell, Tauri configuration, Rust entry points
crates/domain/          Authoritative provider-independent coding-session domain
crates/assembly/        Deterministic persisted-evidence interpretation
crates/integration/     Read-only coding-agent detection and capabilities
packages/domain/       Matching TS domain/IPC contracts, pure display helpers
packages/ui/           Reusable accessible visual components
docs/                  This specification pack
AGENTS.md
README.md
```

These folders now exist. Public build-info IPC and a minimal visual wrapper are implemented. Brief 01B implements the bounded `mochi-hook`, Codex adapter, normalized spool ingress, capability report, and read-only Git snapshots. Brief 02 adds the validated provider-independent coding-session aggregate in `crates/domain`; `packages/domain` matches its serialized contract and shares a fixture checked by both runtimes. Brief 03 adds checksummed migrations and project, ingress, and validated-session repositories in `crates/persistence`. Brief 04 adds `crates/assembly`, which groups persisted ingress by stable project/source/session identity, reconstructs only evidence-backed relationships, validates through `CodingSession::new`, and delegates one atomic aggregate-and-association write to persistence. Brief 05 adds the shared `crates/privacy` file/path policy used by prototype file-tool response handling and Git pre-read filtering. Brief 06 adds the shared, versioned content-redaction engine used before supported spool writes and for SQLite ingress validation. Brief 07 adds `crates/integration`: a provider-independent read-only detection contract plus a macOS-first Codex detector for distinct CLI/Desktop installations, exact-version capability provenance, bounded hook metadata, helper compatibility, and deterministic readiness. Brief 08 adds a separate native project-scoped installer with preview-token approval, bounded strict configuration parsing, owned-entry upgrade/rollback/disconnect, private recovery journals and backups. Its real Codex trust/task gate is pending; the developer connection flow does not create a production capture policy. Desktop startup owns the database connection but does not run detection, import the spool, run assembly, or expose persistence/detection IPC. Production capture and the learning system remain future work. Rust is authoritative for domain behavior and persisted types. Generate TS contracts where practical; otherwise verify matching schemas with contract fixtures. Do not duplicate domain state transitions or knowledge/scheduling logic in React.

## Component responsibilities

| Component | Owns | Must not do |
|---|---|---|
| SessionSource / CodexAdapter | Declared session capabilities, approved configuration plan, source identity, event translation | Leak raw provider payloads into domain/UI; start agent turns |
| Integration installer | Explicitly approved project hooks, ownership receipts, private recovery, safe disconnect | Infer trust, silently start capture, overwrite unrelated configuration |
| Integration detector | Read-only installations, versions, capability provenance, hook/helper metadata, readiness | Modify configuration, trust hooks, inspect sessions, start capture |
| mochi-hook | Minimal authorized ingress; shared sanitizer; bounded atomic spool writes | Network calls, SQLite writes, UI dependency, agent-context output |
| Spool importer | Validation, idempotent import, acknowledgement after commit, gaps/corruption metadata | Persist rejected raw content; block Codex |
| Session assembler | Episode boundaries, lifecycle, recovery, source correlation | Treat a turn stop as entire thread completion without evidence |
| Git collector | Read-only filtered baseline/final file state and deltas | Commit/stash/reset/checkout/fetch; invoke repository diff drivers |
| Privacy service | Authorization, exclusions, redaction, limits, policy revision | Treat API key presence as outbound permission |
| Persistence | Migrations, repositories, transactions, jobs, deletion | Network access or semantic knowledge promotion |
| Learning engine | Bundle, stage orchestration, evidence validation, ranking, assessment rubric | Execute snippets; accept invented references |
| AnalysisProvider | Typed generation/grading, request cancellation, model capabilities | Decide consent or directly change knowledge |
| Knowledge / review engine | Evidence eligibility, deterministic state and due dates | Infer mastery from exposure, LLM opinion, or UI percentages |
| UI / native shell | Screens, menu bar, notifications, user choices | Direct arbitrary file/SQL/shell/Keychain/network access |

## Ports

`IntegrationDetectionService` reports provider installations and operational readiness without making application code depend on a provider command or filesystem layout. Its Codex implementation keeps CLI and Desktop separate, retains every PATH/bundle match, and treats only CLI 0.151.0 and Desktop 26.908.40834 as exact evidence baselines. A different or unreadable version requires validation. Native paths remain inside the integration boundary. Trust is `unknown` unless a legitimate source explicitly establishes it.

`SessionSource` exposes capability report, proposed install/uninstall changes, integration test, and normalized event ingestion. Capability values are `supported`, `unsupported`, or `unknown` for `identity`, `projectAssociation`, `activity`, `prompts`, `agentMessages`, `toolLifecycle`, `permissions`, `interrupts`, `commands`, `fileEvents`, and `sessionEnd`. Capabilities are stored per session and client surface, not assumed from provider name. Brief 01B required the surface distinction because CLI prompt/interrupt coverage was proven while the tested delegated Desktop route left both unknown.

`AnalysisProvider` exposes capabilities plus generation/grading of versioned `LearningAnalysisInput` and typed stage outputs. Inputs include cancellation, selected model, timeout, and output budget. Errors are typed: `offline`, `unauthorized`, `rate_limited`, `timeout`, `refused`, `invalid_output`, `unavailable`, `cancelled`. No provider transport objects escape this port.

Additional ports: `Clock`, `GitContextReader`, `ProjectAuthorizer`, `SecretStore`, `LocalRepository`, `NotificationSink`. Linux/Windows implementations are deferred; macOS-specific Keychain/menu/signing code remains behind these ports.

## Runtime and recovery

- Single desktop core per local profile, single database writer. Window close keeps the core alive; full quit stops it.
- Helper can spool while the core is closed, using a minimal atomically written capture-policy file. Missing/corrupt/disabled policy fails closed. Helper never depends on a network or core response.
- The explicit core importer reads at most 100 spool candidates, rechecks the stored project policy revision, and produces exactly-once database effects through ingress/source identity constraints and immediate transactions. Delete spool entries only after committed import or a committed metadata-only rejection. Desktop startup does not run this importer yet.
- The explicit Session Assembly Engine processes at most 25 source-session candidates per call and enforces 20,000-event/20-MiB per-session bounds. It never reads the spool or raw provider input. Session rows and ingress assignment commit together; normal replay skips assigned evidence, and session deletion retains a local assembly-group tombstone. Desktop startup does not run assembly.
- Durable jobs carry input revision, policy revision, stage, retry count, and consent epoch. Crash recovery marks running jobs pending; it revalidates consent and data existence before resuming.
- Session capture lifecycle and analysis status are separate. Live sessions do not have a final lesson. New episodes can continue the same provider thread after finalization.
- Queue per-session analysis once per revision. One outbound generation/grading request at a time in V1; no unlimited backlog polling. Cached outputs reused only if revision/model/policy/stage contract match.
- Maximum 3 total attempts per stage for transient errors, with bounded exponential delays (2s, 8s, capped Retry-After at 60s). No automatic retry for key/credit errors, refusal, cancellation, or revoked consent. One schema repair attempt counts inside that limit. User retry creates a new job revision.
- Request timeout 60s; total analysis-job active time budget 5 minutes. Exceeding it produces an actionable failure, not a ready lesson. On ambiguous billed timeout, do not assert provider idempotency; retries can cost extra.

## Git evidence

Canonical approved root must contain all files accessed, including after symlink resolution. Discover Git root only within the approved boundary; ask for broader root approval if needed. Disable external diff/text conversion, avoid filters and repository hooks, use argument arrays and read-only commands with bounded time/output. Git objects/working tree may change during reading: record capture time and consistency warning; never silently rewrite repository state.

Baseline and final include HEAD reference (local only), dirty status, filtered tracked/untracked text snapshots, hashes, and limits. Never hash excluded content as a substitute for exclusion. Compare **baseline file content to final file content**, not just HEAD-to-working-tree after the session. This correctly separates preexisting dirty changes where both snapshots exist.

Include approved untracked files, additions, deletions, and staged/unstaged content; staged status is metadata, final working-tree content is the teaching reference. Resolve renames heuristically and label uncertain matches. If baseline unavailable or bounds exceeded, preserve that gap and avoid full-delta attribution. No-Git projects get only proven event/file evidence.

Changes are observed during the episode, not proof of exclusive AI authorship. Overlapping sessions, manual edits, branch switches, and other tools set attribution `ambiguous`; detected branch changes may make comparison `partial`. UI/LLM may say “observed changes”; they may say “Codex changed” only with explicit provider evidence.

## Resource bounds

| Resource | V1 default / overload behavior |
|---|---|
| Incoming provider message | 1 MiB read cap; oversize rejected before persistence |
| Normalized event | 64 KiB serialized cap; text reduced before final validation |
| Single text content field | 8 KiB; redaction precedes truncation |
| Spool | 100 MiB / 7 days; oldest entries evicted with metadata gap summary |
| Episode events | 20,000 or 20 MiB, whichever first; preserve lifecycle/gap metadata, stop excess content |
| Git collection | 100 text files, 64 KiB each, 2 MiB aggregate per snapshot; max 10s each |
| Analysis input | 128 KiB serialized UTF-8 and ≤24,000 estimated tokens; use lower provider context budget |
| Analysis output | 64 KiB per stage; ≤12 KiB per explanation section |
| UI lists | Paginate 50 rows; no unbounded transcript DOM rendering |

These are planning defaults, shared with security/event specs. Limits never authorize reading excluded files. Truncation preserves explicit omissions and source references. Capture never waits on generation. Performance targets and benchmark fixtures: [VALIDATION_PLAN.md](../implementation/VALIDATION_PLAN.md).

## IPC and application security

Use narrowly named commands for list/read session, finalize, consent/config changes, queue/cancel analysis, submit attempt, due reviews, export, and deletion preview/commit. Validate IDs, limits, authorization and revision on the Rust side. No general “execute”, “read path”, “SQL”, or “HTTP” command. Frontend cannot retrieve the API key. Tauri capabilities restrict filesystem/dialog/notification access to required functions. Render model prose/code as inert text or restricted Markdown with escaped HTML; deny remote assets and unsafe links.

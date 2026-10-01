# Mochi V1 privacy and security

Baseline: 1.0. Authority: authorization, data minimization, sensitive-content handling and retention. Privacy is a release requirement, not a later feature. Local-first describes data ownership; it does not mean all analysis is local or all source code is harmless after redaction.

## Consent and trust boundaries

| Boundary | Default | Requirement |
|---|---|---|
| Codex detection | Read-only minimal detection | No config changes, key reads, or transcript collection |
| Integration installation | Disabled | Explicit preview/approval; conservative backup/rollback |
| Session tracking | Off | Global tracking enablement plus approved canonical project root |
| Remote generation/grading | Off | Separate disclosure and consent; API-key entry alone insufficient |
| Per-session send | Manual approval | Preview sanitized bundle; grant bound to bundle hash/policy revision |
| Project automatic sends | Off | Explicit optional approval for this project and disclosed data categories; never inherited by new roots |
| Notifications/login startup | Off until opted in | OS permission and app preference; no source code in notifications |
| Export/deletion | User initiated | Preview contents/impact and chosen local destination |

Remote disclosure lists provider, sanitized source excerpts/diffs, selected prompts/actions/errors, concept state, and learner answers for grading; explains API costs and possible proprietary code transfer. Provide capture-only mode without key/account. Additional collection categories, changed provider/endpoint, or widened project access invalidate the relevant consent.

Recheck authorization before file read, before durable write, before network send, and before accepting an asynchronous result. Revocation atomically changes helper policy/consent epoch, cancels queued/in-flight requests, and discards results from old epochs. Already transmitted content cannot be recalled; explain that limit in the revocation UI. An explicit session approval is tied to the sanitized bundle hash; changed content requires renewed approval unless the project automatic policy applies.

An unknown project may surface only temporary safe source/project metadata for approval. Do not spool/store payloads before root consent. Approved scope is resolved canonically; symlinks outside it, `..` escapes, broad home/root selection, and boundary expansion are rejected or explicitly narrowed. Project root may be inside a wider Git root; approve that wider root before reading it.

## Data lifecycle

```text
Bounded provider ingress / authorized file read
    → path exclusion + content sanitizer + schema/size validation
    → sanitized durable spool / SQLite
    → minimized bundle + sanitizer again + current consent
    → optional OpenAI HTTPS request
    → response schema/evidence validation + sanitizer
    → published lessons / attempts / evidence
    → expiry, explicit deletion, or sanitized local export
```

Recognizable secrets must never be persisted first and cleaned later. This includes helper spool, event payloads, code snapshots, attempt answers, model outputs, diagnostics, migration backups and exports. A failed sanitizer or uncertain high-risk block is discarded, with metadata-only reason. Never dump the rejected input to debug logs.

## File and path filtering

Brief 05 implements the shared, versioned [file and path policy](FILE_PATH_POLICY.md) in `crates/privacy`. It returns `Allow`, `MetadataOnly`, or `Deny` with fixed safe reason codes and project-relative storage paths only. Capture file-tool responses and explicit Git snapshots use that policy before durable content storage; future content readers and analysis bundles must reuse it. A malformed `.mochiignore` or uncertain filesystem result fails closed. Brief 06 adds the separate [shared content-redaction boundary](SECRET_REDACTION.md) after allowed-path classification.

Case-insensitive conservative exclusions apply to **baseline, final, untracked, deleted-file history, diff content, commands referencing paths, and source excerpts**. Filter filenames before reading Git objects/files or invoking diff collection. A full repository diff captured before filtering is prohibited.

Defaults include `.env`, `.env.*`, `*.pem`, `*.key`, `credentials*`, private certificate/SSH/keychain files, dependency/cache/build directories (`node_modules/`, `.git/`, `target/`, `dist/`, `build/`, caches), binary files, database dumps, and known token/auth configuration files. Do not create a blanket Git-ignore bypass: ignored files remain excluded unless a narrow safe allow rule is explicitly approved; hard secret/key exclusions cannot be disabled in V1.

`.git/` exclusion forbids collecting its contents as learning files or recursively scanning it. Narrow read-only Git metadata/object access to reconstruct an **already approved, filtered source file** is permitted within the approved Git root; excluded file objects are never retrieved. Do not include arbitrary Git internals in analysis.

User custom exclusions can only narrow capture automatically. Any widening of a soft exclusion requires explicit root/policy preview and applies to future reads, not retroactive secret collection. Files may be reclassified by content; decoding errors/binary detection drop content. Sensitive-looking excluded paths are represented as count/categories, not copied into transcripts/bundles.

Persist localRoot only in local Project metadata. Outbound/export paths are approved relative paths with sensitive segments masked. Replace home/user/temp absolute paths in prompts/errors/output with stable placeholders. Repository aliases may also reveal confidential names; preview and editable safe aliases are required.

## Secret detection and redaction

Brief 06 implements one provider-independent `RedactionEngine` in `crates/privacy`, used by capture, Git, spool validation, and SQLite ingress validation. Its supported classes, bounded processing, safe markers, false-positive strategy, and limitations are specified in [SECRET_REDACTION.md](SECRET_REDACTION.md). A forged spool record containing a recognized secret is rejected before SQL content storage; the original text is not rewritten and accepted.

The shared sanitizer recognizes versioned provider token/key patterns, PEM blocks, authentication headers, contextual secret assignments, and credential-bearing connection strings. It uses deterministic placeholders such as `[REDACTED:OPENAI_API_KEY]` and never retains original secret fragments or hashes. There is no universal entropy detector: ordinary code identifiers, hashes, and package integrity strings are preserved unless a supported secret context applies. Redaction failures and malformed high-risk private-key blocks drop content.

Sanitize complete bounded input **before truncation**, so split/newline/multiline secrets cannot escape by clipping. If input exceeds 1 MiB, reject before persistence; do not log its first chunk. Outputs and learner submissions pass the same engine. User-facing preview can report counts/categories/rules without showing detected values. Pattern detection is best effort and cannot guarantee all secrets or confidential business data were recognized; users remain able to decline any send.

## Credentials and remote analysis

Store OpenAI API key only in macOS Keychain with app-owned reference; never SQLite/frontend state/logs/export. Frontend can set/replace/delete key through narrow IPC and receives only configured/error status. Avoid inherited environment-variable credential discovery in V1. Capture never accesses the analysis key.

OpenAI adapter uses fixed approved HTTPS endpoints, TLS verification, no tool calls, no file upload, no remote browsing, no provider conversation thread as local source of truth. Prefer explicit `store=false` for Responses requests; disable provider background mode and avoid unnecessary retained application state. Do not claim that this guarantees zero retention: endpoint/account policies may still retain/process submitted data, including abuse monitoring and caching. Explain the provider policy and verify it at implementation/release time. Official [OpenAI data controls](https://developers.openai.com/api/docs/guides/your-data) describe these distinctions; source checked 2026-09-13.

Retries/cancellation/timeout limits follow [architecture](../architecture/ARCHITECTURE.md). No endless key validation calls or automatic sends while live/awaiting consent. Key test uses a minimal disclosed non-project request only after user action. Cost estimates are estimates; no current price hardcoded into this pack.

## Configuration installation / uninstall

Prepare a concrete edit plan listing file, owned entries and rollback behavior. Require explicit user action before Codex configuration mutation. Parse and preserve semantic unrelated settings; do not overwrite/reformat the entire user config unnecessarily. Validate destination paths and permissions; create private backup; compare file hash immediately before mutation to detect concurrent edit. Write atomically; validate resulting config and integration test. Roll back only owned changes when safe.

The backup may contain preexisting user secrets: it is a private short-lived **configuration recovery artifact**, never a session payload/diagnostic/export. Restrict to owner read/write (0600), retain ≤7 days, exclude from indexing/model input, disclose storage and cleanup, and never print contents. This exception does not authorize raw session persistence. Uninstall removes only Mochi-owned entries; never restore an old whole-file backup over user changes. If conflict is unsafe, show the exact owned-entry remediation and leave unrelated config intact.

Helper must produce no agent-context stdout, run no network requests, and fail open for the coding workflow while failing closed for data collection. All launch options use safe arguments, minimal environment and bounded runtime. Errors cannot veto user commands or change Codex trust/approval rules.

Brief 08 implements native preview-token approval and project-only JSON hook merging, exact owned-entry rollback/disconnect, bounded strict reads, symlink/owner checks, private journals and configuration backups. The developer flow discloses capture scope, manual Codex trust and backup retention; there is no IPC/onboarding or automatic installation. Backup expiry cleanup runs on installer use or explicit maintenance, not while the app is stopped. Its real CLI install/trust/task/disconnect gate passed; production policy/pause remains pending. Detection remains read-only. See [implementation evidence](../implementation/CODEX_INTEGRATION_INSTALLER.md).

## Local storage, permissions and retention

Use app-owned macOS application support storage, private directories (0700) and files (0600). No Mochi cloud storage. SQLite is not application-encrypted in V1; rely on OS account permissions and recommend device encryption in the product explanation without claiming protection from malware or another process running as the same user.

| Data | Default retention / bounds |
|---|---|
| Sanitized event/code/Git context and bundles | 30 days; user may shorten, extension requires storage disclosure |
| Sanitized spool | 7 days or 100 MiB; metadata-only gap on eviction |
| Metadata diagnostics | 7 days or 10 MiB; no transcript, answers, credentials, absolute paths |
| Published lessons, attempts, knowledge evidence, reviews | Until explicit deletion; source copies already sanitized |
| Config recovery / migration backups | ≤7 days; private local-only; included in storage/deletion inventory |
| Deletion tombstones | ≥7 days while spool may replay, minimal opaque identity only |

Other resource limits match [architecture](../architecture/ARCHITECTURE.md). Never retain a payload simply because diagnostics are enabled. No telemetry, crash-payload upload, external analytics or remote font/image calls by default.

Deletion cascades and recomputes progress/reviews according to [DATA_MODEL.md](../architecture/DATA_MODEL.md). Cancel owners, clear related spool, tombstone source identities, checkpoint WAL and compact safely. Warn that deletion cannot erase provider-side submissions, exports copied elsewhere, filesystem snapshots/backups, or the user's original Codex/repository files. Do not promise forensic erasure.

## Threats and controls

| Threat | Control / verification |
|---|---|
| Secret-bearing prompts/output/diffs | Pre-persistence sanitizer on every channel; synthetic canary scan across artifacts |
| Injection in code/transcripts/model output | Context treated as data; no tools; inert rendering; strict reference/enum validation |
| Symlink/path traversal / malicious repo Git settings | Canonical authorized roots; safe arguments; disable external diff/textconv; no hooks/network |
| Forged/corrupt spool | Private files, schema and source-session correlation checks, bounded parser; drop raw content |
| Duplicate/crash replay | Durable IDs, DB uniqueness and transactions |
| Deleted/revoked data recreated by jobs | Tombstones, epoch/revision guards, coordinated single writer |
| Same-user malware / compromised OS | Outside V1 guarantee; minimized content/retention reduces impact |
| Ambiguous independent learning | Self-report assistance, uncertain grades, conservative deterministic thresholds |

## Privacy release gate

Brief 05 file/path policy, Brief 06 shared redaction, and Brief 07 read-only integration detection are implemented. Detection reads bounded executable/app metadata and hook configuration summaries only; it does not inspect session storage or persist configuration content. Production capture still requires the remaining consent, installation, lifecycle, and release gates. Brief 01 debug logs must already be sanitized; prototype permission never permits raw-secret logs. Brief 35 audits every durable/outbound surface with canaries, rejected payloads, revoked consent, output leaks, exports/backups, and recovery. Any known recognizable-secret leak or silent non-consensual send blocks release. [VALIDATION_PLAN.md](../implementation/VALIDATION_PLAN.md) defines the complete scenarios.

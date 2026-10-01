# Mochi V1 data model

Baseline: 1.2. Authority: local persistence, provenance, transactional effects, retention/deletion relationships. SQLite is the source of truth. Briefs 03 and 04 implement the project, preassembly-ingress, validated-session, and assembly-association subset below; later learning tables remain planned.

## Integration recovery metadata

Brief 08 adds versioned private files outside SQLite for installer ownership and pending configuration transactions. They contain only native target identity, Mochi-owned command, helper fingerprint, ownership flags and before/after hashes. Original config bytes are kept only in memory or private short-lived recovery backups, never in session/learning tables, diagnostics or outbound data. No migration is added. See [installer](../implementation/CODEX_INTEGRATION_INSTALLER.md).

## Conventions

UUID primary keys unless noted. UTC RFC3339 timestamps; integer booleans/counts; validated canonical enum values. JSON content uses strict versioned schemas and size limits. Enable foreign keys on every connection; WAL with a single core writer and atomic migrations. Use parameterized statements and indexed bounded queries. Keep schema-migration version, event-contract version, learning-contract version, and knowledge-rule version separate.

Absolute paths, provider references, Keychain item references, and config-backup locations are **local-only metadata**, never part of outbound learning context or default export. No API-key values, raw provider transcripts, secret fingerprints, executable challenge artifacts, or hidden payload log table.

## Implemented domain and persistence

Brief 02 implements the provider-independent in-memory contract in `crates/domain`, with checked readonly TypeScript contracts and shared synthetic fixtures in `packages/domain`. Brief 03 maps it in `crates/persistence` and adds a separate preassembly evidence store. Brief 04 adds deterministic interpretation in `crates/assembly` and migration 0003's explicit evidence-assignment state. Brief 06 adds versioned redaction before supported evidence writes, with a second privacy validation before ingress import; it changes no database schema. Rust validated construction and deserialization remain authoritative.

The domain aggregate uses typed UUIDs and RFC3339 UTC timestamps. `SessionEvent.sequence` is positive unique import order within an episode; provider time remains optional provenance and never replaces ordering. Missing prompts, source turn IDs, command exit codes, Git context, tool completions, and session end boundaries are valid explicit uncertainty. `incomplete` differs from `failed`. Before/after Git snapshots remain separate, and each carries `truncated`, working-tree state, already-sanitized bounded file content/metadata or omission reasons, and warnings so dirty baselines and bounded observations survive persistence.

Capabilities use `supported/partial/unsupported/unknown`. Per-session completeness uses `complete/partial/missing/unknown/not_applicable` across lifecycle, prompt, response, tools, commands, files, Git, and interrupts. Overall completeness is derived: any missing dimension is `degraded`; all complete/not-applicable is `complete`; other combinations are `partial`. No percentage or LLM judgment participates. Full invariants and examples are in [CORE_DOMAIN_MODEL.md](../implementation/CORE_DOMAIN_MODEL.md).

### Implemented tables

| Table | Purpose / constraints |
|---|---|
| `schema_migrations` | Ordered checksummed migration history; newer versions and checksum drift fail closed |
| `projects` | Domain project plus local `trackingEnabled`, monotonically increasing `policyRevision`, and soft deletion; active canonical root unique |
| `ingress_events` | Sanitized normalized spool evidence keyed by ingress UUID and safe source identity; no canonical session or inferred relationship |
| `ingress_rejections` | Metadata-only spool token hash, bounded reason, optional receive sequence, and time; no raw bytes or paths |
| `ingress_tombstones` | Minimal ingress/source identity, reason, and expiry preventing replay for at least the spool lifetime |
| `import_state` | Last observed spool eviction count/time |
| `sessions` | Validated aggregate header and canonical content hash; project cascade |
| `session_turns`, `session_events`, `tool_executions`, `command_executions`, `file_changes`, `git_snapshots` | Ordered versioned JSON rows with extracted relational IDs, sequences, roles, foreign keys, and cascading deletion |
| `assembly_groups` | Stable project/source/external-session mapping to one deterministic Mochi session ID, assembly version/fingerprint, and `assembled/deleted` state |
| `ingress_assembly_state` | Explicit per-ingress `assigned` or metadata-only `ignored` state; assigned rows retain session traceability and ignored rows retain a bounded reason |

`crates/persistence` loads all session rows and calls `CodingSession::new`; it never returns an invalid partial aggregate. `crates/assembly` is the only evidence interpreter. Absence of an `ingress_assembly_state` row means unassigned. Session deletion changes its assembly group to `deleted` before removing the aggregate; assigned ingress remains consumed, so an ordinary assembly run cannot recreate the session. This state is a local recovery/deletion guard, not cloud synchronization.

## Planned V1 entities / table contract

| Table | Key fields | Relationships / constraints |
|---|---|---|
| `projects` extension | remotePolicy (`off/per_session/automatic`) and later consent relationships | Current local tracking policy remains separate from remote analysis consent |
| `integrations` | id, provider, clientDescriptor, version, transport, capabilitiesJson, state, ownedConfigPatchJson, backupPath?, lastTestAt | Only Codex V1; patch metadata excludes sensitive config values |
| `sessions` extension | integrationId?, providerThreadRef?, continuationOfSessionId?, captureState, lastActivityAt, finalizedAt?, inputRevision, analysisStatus, attribution, summary? | Added only when the owning brief defines those states |
| `code_evidence` | id, sessionId, snapshotId?, relativePath, language?, origin, excerpt, contentHash, startLine?, endLine?, beforeExcerpt?, deltaJson?, attribution, truncated | Session cascade; snapshot set null on expiry; sanitized text, hash of sanitized included content only |
| `analysis_jobs` | id, sessionId?, attemptId?, inputRevision, policyRevision, consentEpoch, provider, model, contractVersion, stage, status, attempts, createdAt, lastErrorCode?, usageJson? | One owner: session generation or attempt grading; owner cascade; active owner/revision uniqueness |
| `analysis_bundles` | id, jobId, revision, inputJson, evidenceManifestJson, sanitizerVersion, inputHash, expiresAt | Job cascade; sanitized bounded payload, not raw archive |
| `debriefs` | id, sessionId, revision, status, contractVersion, provider, model, reconstructionJson, prioritiesJson, sectionsJson, createdAt | Session cascade; unique(sessionId, revision); publication atomic |
| `concepts` | id, canonicalKey, title, description, aliasesJson, category?, createdAt | Unique canonicalKey; stable registry, never use model label as identity |
| `session_concepts` | sessionId, conceptId, debriefId, rank, scoreComponentsJson, evidenceRefsJson | Composite PK(sessionId, conceptId); only published priority exposures, rank 1–3 |
| `assessments` | id, debriefId, conceptId, kind, questionJson, answerSpecJson, rubricJson, evidenceRefsJson, version | Debrief cascade; immutable after attempt; `mcq/predict_output/explain_why/challenge` |
| `assessment_exposures` | id, assessmentId, kind (`hint/solution/feedback`), occurredAt | Assessment cascade; recorded before UI reveals content; prior answer-revealing exposures disqualify later independent evidence for this question |
| `attempts` | id, assessmentId, conceptId, reviewRunId?, submissionId, answerText, submittedAt, assistance (`independent/assisted/unknown`), solutionSeen, outcome, gradingMethod, graderConfidence?, rubricResultJson?, status | Assessment cascade; unique submissionId for double-submit/retry; outcome `correct/incorrect/uncertain/ungraded` |
| `knowledge_evidence` | id, conceptId, sessionId?, attemptId?, kind (`exposure/practice/recall/challenge`), outcome?, eligibilityJson, occurredAt, independenceBasis?, ruleVersion | Exactly one provenance owner per evidence row; cascading owner deletion; unique exposure(sessionId, conceptId), unique attemptId |
| `knowledge_records` | conceptId (PK), state, exposureCount, practiceCount, independentCorrectDays, eligibleDelayedRecallCount, independentChallengeCount, lastPracticeAt?, lastEligibleRecallAt?, needsRefresh, ruleVersion, updatedAt | Materialized cache derived from retained evidence; not a free-form LLM profile |
| `review_items` | conceptId (PK), intervalIndex (0–4), dueAt, lastProcessedAttemptId?, paused, snoozeUntil?, policyVersion | At most one queue item per concept; computed transactionally from evidence; pause/snooze are user preferences |
| `progress_resets` | conceptId (PK), resetAt, generation | Latest reset cutoff retained independently of evidence; pre-reset session exposure/attempt credit cannot be restored by regeneration |
| `review_runs` | id, startedAt, endedAt?, mode | Groups attempts; deletion/recompute semantics below |
| `settings` | key (PK), validatedValueJson, revision | Consent defaults, retention, UI/notification preferences; no key secret |
| `consent_records` | id, projectId?, sessionId?, kind, granted, epoch, policyRevision, provider?, bundleHash?, recordedAt | Minimal audit metadata; session/project cascade; global revoke increments epoch |
| `deletion_tombstones` extension | session/learning owner identities | Brief 03 tombstones ingress/source identities only |

`sourceIdentityKey` is derived from provider + client/adapter source instance + thread + stable source event ID. For provider events without an ID, ingress UUID is the dedupe key. Map provider thread/episode boundaries using session metadata; do not create source identity from secret text. Core-assigned UUID mappings must survive restart.

`eligibilityJson` contains deterministic flags `independentCertain`, `delayedRecall`, `application`, and `scheduledReview`. The first can be true for an immediate independent certain answer, which can support one correct day; it cannot by itself satisfy delayed recall or advance review. The other flags apply the timing, primary-concept challenge and due-task rules in the knowledge spec. Compute them from attempt/provenance/exposure history and clock, never from an LLM eligibility assertion. Cached flags are recalculated on rule/reset/evidence changes.

## Evidence references

Reference shape: `{kind: event/code, id: UUID, excerptHash: string|null}`. Reconstruction claims and every selected concept must resolve to evidence belonging to the same session/revision. Source excerpts in a published debrief are sanitized **copies**, so lessons survive raw-context retention. Preserve original reference IDs and hash plus `availability=retained/expired` at display resolution. An expired original is not claimed to be live code or a working-tree line.

Line numbers are snapshot locations. Hashes are for sanitized evidence integrity and cache invalidation, not secret tracking. No statement that a learner wrote code independently without the assistance record.

## Transactions / exactly-once effects

1. **Evidence import (implemented):** reject records containing recognized unsanitized content before source identity hashing → validate → recheck local tracking policy revision → tombstone/dedupe → insert preassembly evidence or metadata-only rejection → commit → spool acknowledgement. Duplicate delivery has no second database effect.
2. **Session assembly (implemented):** select a bounded stable source-session group → derive explicit boundaries/uncertainty → validate `CodingSession` → insert aggregate, assembly group, and ingress assignment atomically. Persistence owns rows and transactions but does not interpret event semantics.
3. **Finalize:** state transition and final-snapshot job scheduled once. On completion save evidence and mark finalized/coverage. Failure preserves partial evidence and explicit warning.
4. **Publish:** validate/sanitize all generation output and references; insert debrief/questions/challenge; replace active session priority links; reconcile exposure evidence; recompute knowledge/review; set ready in one transaction. A session finalized before a concept's latest progress reset cannot re-create its exposure credit; attempts submitted before the cutoff cannot requalify. Partial generation is never exposed as a complete lesson.
5. **Submit:** sanitize bounded answer; unique submissionId; save attempt pending or locally graded. Pending AI grading produces no eligible promotion. Evaluation completion sets outcome and inserts one evidence row; update knowledge and review atomically. Repeat callbacks are ignored by owner/revision/status checks.
6. **Regenerate:** preserve old attempted assessments/debrief revision as history. New revision becomes current only after publish. Exposure is once per session/concept; remove unsupported old exposure links, but preserve historical real attempts for still-retained old assessments. Regeneration never erases wrong answers or fabricates independent evidence.
7. **Revoke/delete:** invalidate job epoch/revision, cancel requests, apply tombstones, delete dependent rows/recompute in one controlled operation. Stale completion cannot recreate deleted data.

All cached knowledge and review state must be reproducible from evidence/attempt history plus explicit pause/preference data and a versioned clock policy. `lastProcessedAttemptId` is not the only idempotency mechanism; unique evidence ownership and transaction status guard replays.

## Review persistence

Store `dueAt` as the absolute submission timestamp plus the interval; timezone changes affect display only. A review run selects due unpaused concepts and snapshots question IDs. Closing mid-review preserves answered attempts; unanswered items remain due. One correct eligible result per concept per UTC day can advance the interval; same-day repeats do not. Deletion replays the scheduler chronologically from retained eligible attempts; rule details: [KNOWLEDGE_MODEL.md](../learning/KNOWLEDGE_MODEL.md).

## Retention and deletion

Retention of sanitized session events, Git snapshots, code context and analysis bundles defaults to 30 days; published debrief copies, attempts, knowledge evidence, and derived reviews remain until deleted. Expiring source context marks references expired; it does not fabricate or discard historical learning evidence. Sessions retain summary/coverage metadata after context expiry. Spool is 7 days/100 MiB; diagnostics 7 days/10 MiB.

| Action | Effects |
|---|---|
| Delete session | Cancel analysis, tombstone associated ingress UUID/source identities for at least seven days, delete owned ingress and validated aggregate with all learning children, retain a deleted episode/group metadata marker; no knowledge/review cache exists yet |
| Delete project | Disable root policy first, cancel jobs, delete all project sessions and consent; preserve global concept registry only |
| Reset concept progress | Delete its attempts/evidence/review history and session-concept links, rebuild record as NEW; retained lessons remain inert until a new encounter/practice; no retroactive exposure replay |
| Delete review run | Delete its attempts and evidence, recompute affected knowledge/reviews; other session practice stays |
| Reset all learning data | Disable capture and remote policy, cancel jobs; clear spool, data, retained context and tombstones after drain; preserve app preferences only if chosen |
| Remove API key | Delete Keychain item, cancel outbound work; local learning data remains |
| Disconnect integration | Disable helper first; remove only owned integration config changes safely; local history remains |

Deletion previews list counts and consequences. Tombstones last at least spool TTL (7 days); reset drains spool before releasing them. Prevent in-flight import from racing deletion through the single-writer coordination barrier. Do not erase the user's repository, Codex transcript/config, or unrelated Keychain items. Config backup restoration is conservative and only if user config has not diverged.

SQLite logical deletion is not a forensic-erasure guarantee. Checkpoint/truncate WAL and vacuum when safe after deletion; do not claim erasure from filesystem snapshots/backups. See [security](../security/PRIVACY_SECURITY.md).

## Export and migrations

Export versioned JSON with projects aliases, session summaries, published sanitized lessons, assessments/attempts, evidence state, knowledge, and reviews. Re-sanitize on export and omit absolute roots, provider correlation IDs, key references, config backups, and payload diagnostics. Warn that source excerpts and personal learning history are included. Export is a user-selected local save, not a network upload. Import is deferred.

Each migration runs atomically against synthetic populated fixtures and empty DB. Back up the sanitized DB locally before potentially destructive migrations; keep the backup ≤7 days, include it in deletion/reset inventory, and never upload it. Failure leaves previous DB usable or read-only with recovery instructions; never silently drop data. Index session(projectId,lastActivityAt), events(sessionId,sequence), jobs(status,createdAt), attempts(conceptId,submittedAt), evidence(conceptId,occurredAt), reviews(dueAt), and consent(projectId,epoch).


## Internal MVP persisted amendment — schema 5

Migrations 0001–0003 are unchanged. 0004 adds `capture_episodes` (source key, state, revision, continuation and truthful pause/restart/late flags) and immutable `episode_ingress` associations. 0005 adds `analysis_runs`, `learning_documents`, `selfcheck_questions`, `learning_exposures`, `selfcheck_reveals` and `selfcheck_attempts`, with session-owned cascading foreign keys and transactional publication/attempt idempotence. The future generic knowledge/review tables above are not implemented. Questions remain immutable; repeated publication does not promote knowledge.

Current version-1 learning references are opaque `event:<uuid>` or `code:<role>:<sanitized-hash>` IDs resolved against the exact input manifest. Code evidence records a sanitized immutable excerpt/hash, relative path, snapshot time, first line and truncation. These IDs are not provider thread IDs or live working-tree references. Persisted domain Git context additionally supports `FinalOnly { after, baselineReason }`; it never fabricates a before snapshot.

Analysis runs retain safe authorization metadata, not provider payload logs; the sanitized input/evidence copies exist only in a published validated document. Startup cancels unfinished runs rather than automatically resending. The internal publication includes explanation, questions, delayed variants and exposure, without mini challenge/knowledge/review or full ready status. Automatic 30-day partial-context expiry and export/reset UI are not implemented at this checkpoint; explicit ingress cutoff/checkpoint primitives and deletion are available. [Current limits and evidence](../implementation/INTERNAL_CLI_MVP.md).

# Mochi V1 implementation roadmap

Baseline: 1.0 with the 2026-09-13 foundation amendment. Authority: dependency order and work packages. Each entry becomes a concrete task using [BRIEF_TEMPLATE.md](BRIEF_TEMPLATE.md). The supplied Brief 00 — Repository Foundation explicitly authorizes the stack scaffold now; this is the only exception to the original G01-first application sequencing. See [FOUNDATION.md](FOUNDATION.md).

## Sequencing and gates

The user-assigned Brief 05/06 privacy sequence superseded the old file-filter/redaction entries numbered 11/12. Brief 07 is now the implemented detection service, and the remaining capture-foundation steps use the actual sequence below.

**00 → 01 (G01) → 02 → 03 → 04 → 05 → 06 → 07 → 08 → 09 → 10 → 11 → 12 → 13 → 14 → 15 → 16 → 17**, followed by the dependency table below.

Privacy 05/06 precedes production capture. Brief 01 uses a minimal prototype sanitizer from the outset, but never raw payload logs. Brief 31 hardens an already functional core/helper background separation; Brief 29/30 completes product settings/onboarding after earlier narrow consent/config controls. Consent is never deferred to the final settings screen.

G01 is the highest-risk stop/go gate: pass using a real intended Codex client before integration or substantial product investment. The minimal stack scaffold alone did not pass G01; Brief 01 supplied the evidence used by the explicitly assigned Brief 02 domain work. Signing/OS/provider choices are gates G02–G04 in [DECISIONS.md](../product/DECISIONS.md).

## Briefs and acceptance

| ID / phase | Objective / deliverable | Dependencies | Acceptance / expected change area |
|---|---|---|---|
| 00 Repository foundation | Master specs plus explicitly requested minimal Tauri/React/TS/Rust workspace, packages, tooling and CI | None (explicit task amendment) | Docs aligned; supplied scaffold acceptance and native evidence recorded in FOUNDATION.md; no product features |
| 01 Feasibility | Prove passive Codex observation and safe Git evidence | 00 | Real sanitized debug events and support matrix; normal coding unaffected; safe cleanup; report under docs/implementation |
| 02 Core domain model | Provider-independent Project, CodingSession, Turn, Event, execution, file, Git and capture-uncertainty contracts | 01 accepted | Rust/TS fixture matches, invalid aggregates rejected, Desktop unknowns represented; no persistence/assembly/learning |
| 03 Persistence | Private SQLite migrations plus separate project, preassembly-ingress, and validated-session repositories | 02 | Checksummed migration, policy/dedupe/commit-before-ack replay, bounded reads, cascade/tombstone, corruption and domain round-trip tests pass; no automatic import/IPC/assembly |
| 04 Session assembly foundation | **Implemented:** persisted normalized evidence → validated `CodingSession` plus durable ingress association | 03 | Deterministic boundary/gap/uncertainty fixtures pass; no invented prompts, results, completion, attribution, or relationships; aggregate and association commit atomically |
| 05 File/path policy | **Implemented:** shared root-bound file/path decisions, user exclusions, capture and Git pre-read guards | 03,04 | [Security contract](../security/FILE_PATH_POLICY.md) and [implementation report](FILE_PATH_POLICY.md); no automatic capture or complete secret engine |
| 06 Secret redaction | **Implemented:** shared bounded content sanitizer and defensive spool/SQLite ingress checks | 05 | [Security contract](../security/SECRET_REDACTION.md) and [implementation report](SECRET_REDACTION_ENGINE.md); synthetic token/output/Git canaries absent from tested durable fixtures, failure drops content; no automatic capture or analysis |
| 07 Codex detection and capabilities | **Implemented:** read-only CLI/Desktop discovery, exact-version evidence, hook/helper metadata, deterministic readiness | 03,06 | [Implementation report](CODEX_DETECTION_CAPABILITIES.md); missing, mixed, malformed, timeout and side-effect fixtures pass; no mutation, transcript reads, IPC, or capture startup |
| 08 Codex integration installer | Consented owned config edits, test/rollback/disconnect | 07 | Idempotent install, unrelated config preserved, concurrent edit safely rejected, private backup, uninstall; adapter/native |
| 09 Event bridge | Independent helper and sanitized atomic spool | 08,06 | UI/core down still safe bounded spool; no stdout agent context/network/blocking; helper/core |
| 10 Production normalization | Harden adapter ingress → canonical SessionEvent after production bridge/privacy services | 09,04 | Strict discriminated schemas, mapping/dedupe/gaps, unknown types safe; adapter/domain |
| 11 Assembly lifecycle hardening | Episodes, finalization, resume, crash/gaps over production-normalized evidence | 10,04 | Lifecycle fixtures, restart and late revision handling deterministic; core |
| 12 Git context | Filtered baseline/final and delta evidence | 11,05,06 | Dirty/untracked/no-Git/overlapping/branch-switch cases truthful; no excluded content or Git mutation; core |
| 13 Bundle | Minimized sanitized LearningAnalysisInput | 12,06 | Valid references, limits, omissions, exact preview hash and consent revisions; core + preview control |
| 14 Provider | AnalysisProvider/OpenAI adapter, Keychain, retries/cancel | 13 | Structured generation, refusal/error/timeout handling, no send without current consent; G03 model recorded; core/native |
| 15 Reconstruction | Objective goal/change/decision/failure/fix claims | 14 | Fixture claims grounded, uncertain attribution and tests explicit; learning core |
| 16 Concepts | Registry, extraction and deterministic ranking | 15 | ≤5 candidates/≤3 priorities, valid refs, scores/ties/zero-case tested; learning core |
| 17 Debrief | Full project-grounded lesson generation | 16 | Required sections, snapshot code, uncertainty, length and semantic rubric pass; staged result, not yet complete ready lesson without 20/22 |
| 18 Session detail | First full product screen | 10,15 | Activity/Git/coverage/analysis status and retry/consent accessible; desktop UI |
| 19 Debrief UI | Concept reading and project-code display | 17,18 | Correct sections, evidence links, snapshot/stale labels, keyboard/contrast checks; desktop UI |
| 20 Assessment generator | Self-checks plus delayed variants, structured rubrics | 17 | Every priority has self-check + unseen delayed variant, all formats validated, no ambiguous keys; learning core |
| 21 Assessment experience | Submit, grade, feedback and persist attempts | 20,19,03,14 | Double-submit/retry/uncertain/assisted/offline handling verified; no promotion before valid evaluation; UI/core |
| 22 Mini challenge | One primary-concept 5–10 minute text challenge | 20,21 | Rubric/independence disclosure, safe answer storage, no execution; full ready publication now complete; UI/core |
| 23 Knowledge engine | Five-state deterministic evidence derivation | 21,22,03 | No mastery from exposure; distinct-day thresholds/reset/deletion/replay pass; core |
| 24 Knowledge UI | Concept state/history/profile | 23 | Counts/states match evidence, assistance and refresh explained, no percentages; UI |
| 25 Scheduler | Deterministic spaced review queue | 23 | 1/3/7/14/30 days, incorrect/same-day/timezone/pause/snooze/recompute pass; core |
| 26 Review experience | Focused due review, offline cached recall | 25,24,21 | ≤3-concept batch, unknown grades don't advance, interrupted answers retained; UI/core |
| 27 Home | Real activity, unfinished lesson and due review dashboard | 26,19 | Empty/active/capture-only/ready states use actual data; UI |
| 28 Library | Search/filter/paginated session history | 18,27 | Accurate bounded queries, expired context and empty/error states; UI/storage |
| 29 Settings | Complete integration/privacy/AI/storage controls | 28,14,25 | Existing consent surfaces consolidated, export/delete/reset/Keychain removal correct; UI/core |
| 30 Onboarding | Guided first-run connection and test | 29,08,22 | Fresh user consent/skip/unsupported/config rollback journey passes; UI/native |
| 31 Background | Harden lifecycle independent of window | 30,09,11 | Window close, full quit, restart, sleep/wake policy safe; no duplicate imports; native/core |
| 32 Menu bar | Current state and core actions | 31,27 | Open/pause/resume/finish/quit state truthful, no sensitive text; native/UI |
| 33 Notifications | Opt-in ready lesson / review reminders | 32,26,22 | Once per publication, coding suppression, permission-denied path, no payload leaks; native/core |
| 34 Recovery | Cross-component failure hardening | 33 | Crashes, malformed spool, huge/no-Git sessions, consent/delete races, offline/provider errors preserve truth/data; core/UI |
| 35 Privacy audit | Audit durable/outbound surfaces and installer | 34 | Zero known recognizable-secret leaks or silent sends in canary suite; threat checklist resolved; report + fixes |
| 36 Performance audit | Bound long sessions/large repo/UI work | 35 | Documented budget targets pass on arm64 reference machine; overload/gaps visible; report + fixes |
| 37 Complete E2E | Validate fresh-user full learning loop | 36 | Real Codex and later review end-to-end, recovery/accessibility checks pass; QA evidence |
| 38 macOS alpha | arm64 package, signing/notarization, clean install/uninstall | 37, G02/G04 | External alpha installs on clean supported Mac; owned config cleanup/data choice documented; packaging/release docs |

## Milestone gates

| Milestone | Must be true |
|---|---|
| M0 — Specification | 00 accepted; contract ownership and provenance clear |
| M1 — Feasibility | 01 real-client evidence accepted; exact supported route/version and any partial surface capabilities documented; otherwise revise plan |
| M2 — Safe capture | 02–12 relevant dependencies pass; privacy before every durable capture; Git/evidence gaps truthful |
| M3 — Analysis | 13–17 pass stage contracts; consent/model/limits validated; no incomplete lesson called ready |
| M4 — Learning loop | 18–22 pass; full atomic publication includes checks, delayed variants, challenge and exposure |
| M5 — Memory | 23–26 pass; knowledge/review deterministic and honest |
| M6 — Native product | 27–33 pass; fresh-user shell and background experience complete |
| M7 — Release | 34–38 pass audits, E2E and clean-machine package; P01–P16 satisfied |

At M4 before 23, evidence can be persisted using domain contracts, but no unsupported knowledge status is shown; 23 completes the authoritative reducer. Implement transactional exposure/attempt ownership early (03/21/22), not retroactively from UI events.

## Task handoff and release discipline

For each brief attach objective acceptance evidence, relevant commands/manual steps, changed specs, and limitations. Brief 00 scaffold evidence is in [FOUNDATION.md](FOUNDATION.md). Brief 01A established a **CONDITIONAL GO** for hooks plus Git, and Brief 01B implemented the bounded prototype with Codex CLI supported and Codex Desktop marked **PARTIAL**; evidence is in [CODEX_INTEGRATION_FEASIBILITY.md](CODEX_INTEGRATION_FEASIBILITY.md) and [CODEX_CAPTURE_PROTOTYPE.md](CODEX_CAPTURE_PROTOTYPE.md). Brief 02 implements the core coding-session contract in [CORE_DOMAIN_MODEL.md](CORE_DOMAIN_MODEL.md). Brief 03 implements the persistence boundary in [SQLITE_PERSISTENCE.md](SQLITE_PERSISTENCE.md). Brief 04 implements explicit bounded assembly in [SESSION_ASSEMBLY_ENGINE.md](SESSION_ASSEMBLY_ENGINE.md). Brief 05 implements the shared path policy in [FILE_PATH_POLICY.md](FILE_PATH_POLICY.md). Brief 06 implements bounded pattern redaction in [SECRET_REDACTION_ENGINE.md](SECRET_REDACTION_ENGINE.md). Brief 07 implements read-only Codex detection in [CODEX_DETECTION_CAPABILITIES.md](CODEX_DETECTION_CAPABILITIES.md). Do not infer hook installation, production capture, automatic import/assembly, detection of every possible secret, or learning functionality from these foundations.

Development fixtures may replace unavailable analysis during ordinary UI tests, but release requires real integration and consented model-quality evaluation. Tests/audits are continuous within every brief; hardening numbers are final cross-system verification, not permission to postpone failure handling or privacy.

The website remains post-V1. No brief includes website implementation, backend deployment, accounts, additional session providers, or source-code execution challenges.

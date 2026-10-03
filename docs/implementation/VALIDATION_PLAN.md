# Mochi V1 validation plan

Baseline: 1.0 with the 2026-09-13 foundation amendment. Authority: acceptance evidence and release checks. Foundation checks now run against the minimal application; product/release scenarios below remain future requirements.

## Documentation and foundation gates

Verify all requested paths plus index/decision/template/validation docs exist; relative Markdown links resolve; JSON examples parse; shared statuses/limits/state names and roadmap dependencies agree; established versus specified defaults are identified. The supplied Brief 00 authorizes application/dependency foundation files only. Human review focuses on semantic consistency rather than counting headings.

Foundation: install with `pnpm install --frozen-lockfile`; run `pnpm typecheck`, `pnpm test`, `pnpm lint`, `pnpm format:check`, `pnpm check:rust`, `pnpm build`, and `pnpm desktop:build`. Start `pnpm dev` on an arm64 Mac, verify the development screen, then activate **Check desktop connection** and verify the real Rust version response. Record exact environment and outcomes in [FOUNDATION.md](FOUNDATION.md). Frontend IPC mocks and shared fixtures do not substitute for this native launch/IPC check, G01, or clean-machine release validation.

## Product acceptance matrix

| Requirement | Scenario / required evidence | Brief owner |
|---|---|---|
| P01,P16 | Clean supported arm64 Mac: install/launch/onboarding without account/browser, complete loop, uninstall with data choice | 30,37,38 |
| P02,P03,P15 | Exact supported existing Codex client: real episode events, safe project mapping, known end semantics; no manual transcript; adapter types absent from core contracts | 01,08–10 |
| P04 | Core crash between event commit/spool ack and restart: same sessions/attempts/evidence/due dates, no duplicate effects | 03,04,07,09,34 |
| P05 | Synthetic canary secrets in prompts/tools/commands/code/baseline/deleted files/answers/model output: absent across spool, DB/WAL, logs, bundles, export, migration backups; config backups obey their separate private-recovery rule and never collect session payload | 11,12,35 |
| P06 | Default capture/remote off; enable local only; revoke remote during queue/send; changed bundle needs consent; no upload on due-list open | 06,13,14,29,35 |
| P07 | Dirty Git baseline/final, untracked/deleted/renamed files, no Git, late baseline, simultaneous writers, branch switch: accurate gaps/attribution/test claims | 10,15,34 |
| P08,P09 | Representative real/synthetic episodes: ≤5 candidates/≤3 priorities, required sections/code references; empty/excluded episode has no fake lesson | 16,17,22 |
| P10 | MCQ/output/reasoning/challenge fixtures, ambiguous key rejected, assistance/reveal tracked, double-submit idempotent, uncertain/offline rubric attempt not promoted | 20–22 |
| P11 | Correct immediate quiz, later recall, repeats/hints, challenge, wrong review, deleted/reset evidence: exact state/counts, no mastery percentage | 23,24 |
| P12 | Injected clock: intervals/correct/incorrect/daily policy/snooze/pause/timezone; offline variants; deleted history replay yields correct queue | 25,26 |
| P13 | Window close/core running, full quit/helper spool, sleep/wake, menu state, once-only opt-in notification, keyboard/screen reader/reduced motion | 31–33,37 |
| P14 | Unsupported client, config conflict, key/credit failure, malformed spool, huge diff, expired context, provider refusal, delete/job race: recoverable safe state and controls | 29,34 |

## Feasibility proof — release-critical

Brief 01 uses the intended real Codex client/version, not mocks. Capture useful activity and safe identity/project association, enough change evidence to teach at least one concept, defensible finalization, and client behavior unaffected by helper/core failure. Record actual event mappings, missing optional fields, integration trust/config changes, and support matrix. Apply synthetic canary scanning to debug artifacts before declaring pass.

If useful passive capture fails, do not continue to UI/product investment. State exactly what passed and failed and revise the plan. A new Mochi-owned App Server coding session is a different product experiment.

## Learning/model quality gate

Evaluate a fixed versioned corpus of ≥12 sanitized representative episodes: frontend/backend/general concepts, error/fix, successful/failed tests, no Git, dirty baseline, tiny/huge change, multiple writers, empty/excluded evidence, and injection-bearing content. Fixture reports must reference actual outputs/model/config.

Release rubric:

- 100% valid counts, schemas, references, attribution labels and no invented project snippets.
- Zero known factual contradictions in final inspected explanations/answer keys; any discovered issue fixed or question excluded.
- At least 90% of selected priorities rated relevant/useful by two human reviews or a user and an independent reviewer; disagreements recorded.
- ≥30 pre-labeled assessment responses spanning correct/incorrect/ambiguous/assisted; no uncertain/assisted response awarded qualifying independent evidence.
- No known confidently incorrect promotion-causing grade; AI grading disagreement must be visible as uncertainty and reviewed before release.
- All question formats represented; every priority has immediate self-check and unseen delayed-review variant; one 5–10 minute challenge per nonempty ready lesson.

These are alpha engineering gates, not a scientific claim of learning efficacy. Assess delayed recall with voluntary alpha feedback before making stronger marketing claims. Do not upload real private transcripts as eval fixtures.

## Performance gate

Document arm64 machine model/RAM/macOS, Codex version, release build, corpus and timings. Use a 2-hour synthetic episode at 20,000-event/20-MiB boundary; a large approved repository with more than 100 candidate files; a full 100-MiB spool; paginated history of 10,000 sessions; offline/provider timeout scenarios.

Targets: helper completes bounded ingest/write within 100ms p95 on reference machine (excluding provider's invocation startup); it never waits on core/network. Core commits eligible queued events within 2s p95 while running. Local navigation/input responds within 200ms p95; Rust core plus UI idle resident memory ≤250 MiB, peak ≤500 MiB during bounded capture/context processing. No unbounded CPU/queue growth. Git snapshot ≤10s and analysis job ≤5min active budget; reaching caps yields partial/gap/failed state. Disk caps/retention are enforced. Document measured deviations and adjust a default only through the decision register; do not hide event drops to meet targets.

## Security and deletion checks

Check canonical-root authorization, symlink traversal, ignored/hard-excluded files, malicious Git external diff/textconv settings, injected command-looking text, HTML/link content, malformed/oversize JSON, sanitizer failure, config backup privacy, key frontend isolation, unsafe install rollback, revocation epochs, tombstones and import/delete races. Review permissions/capabilities and network destinations.

Deletion must remove session-owned context/lessons/attempts/evidence and recompute knowledge/reviews; reset progress must prevent pre-reset regeneration re-credit; core restart/stale provider callback must not resurrect rows. Source context expiry leaves sanitized published lesson copies with expired-source labels. Exports omit absolute paths/key/config metadata and pass sanitizer scan. No claims of forensic erasure.

## Full real-user walkthrough

Install → detect supported Codex → approve root/local tracking → approve integration → configure optional remote analysis → real test → normal coding → finalization → approve sanitized context → ready notification → open lesson → self-check → independent text challenge → knowledge evidence → next-day due review → export/delete/disconnect → clean uninstall. Repeat with offline analysis, denied permissions and a crash. Use an injected clock for automated timing; include an actual later-day manual recall check for alpha validation.

## Evidence/report format

Each brief records artifact/revision, scenario, expected/actual result, automated command or manual steps, pass/fail, safe logs/screenshots where necessary, and unresolved risk. Diagnostics must remain sanitized. CI can cover contracts/core/frontend; native/manual checks and real model/client proof are explicitly labeled. Release blocks on a known recognizable-secret leak, non-consensual send, misleading knowledge promotion, failed real capture, data resurrection or unavailable clean-machine arm64 package.

## Bounded Bento Home checkpoint - 2026-10-03

[Bento Home evidence](BENTO_HOME.md) records native one-folder selection/cancel,
focused unchecked approval, persisted return/project selection, keyboard/theme/
minimum-window inspection and automated error/race/stale-state coverage. Its
synthetic app profile and frontend fixtures certify only these bounded UI/local
preference behaviors. Existing real capture, live model quality, Keychain,
clean-machine distribution, full accessibility and release audits retain their
separate gates.

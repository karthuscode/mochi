# Personal CLI learning MVP — implementation and trial

Status: implementation checkpoint, 2026-10-01. This is an internal macOS Apple Silicon test build. It is not the full V1 learning milestone, a distribution release, or an accepted live model-quality gate.

## Scope and ownership

The user authorized sequential implementation through the first testable personal learning loop. Contracts were instantiated in Briefs 09, 10, 11, 12, local controls, 13, 14, 15–19 and 20–21 before their implementation. Earlier reports describe their historical boundaries; this report owns the current runtime checkpoint.

`crates/bridge` coordinates current local capture authorization. `crates/assembly` owns evidence interpretation and revisioned episodes; `crates/persistence` owns transactions. `crates/learning` owns versioned, provider-independent analysis, ranking, explanation and assessment validation. The desktop core owns native Keychain, the fixed OpenAI HTTPS adapter and a cancellable single-job service. React receives narrow, validated views and has no direct filesystem, shell, credential-read or provider-network authority. No knowledge reducer is duplicated in the UI.

The new HTTP/TLS dependencies (`reqwest`, `tokio`) serve the fixed HTTPS analysis boundary; Core Foundation supports native Security.framework Keychain calls. Existing pinned privacy/domain/storage dependencies are reused. No provider SDK payload enters the coding-session domain.

## Capture and sessions

Startup opens private SQLite, reconciles existing project authorization conservatively, cancels unfinished analysis, marks interrupted capture and starts a bounded worker. It never installs hooks, grants new consent, imports the old developer prototype spool or sends analysis. The worker imports at most 100 records and processes at most 25 source groups per tick. One profile lock prevents two application writers.

Project approval creates a disabled policy. Connection shows the exact owned command and target; confirmation installs only those project handlers. Normal Codex `/hooks` trust remains a separate manual review. The production command is `capture-authorized --project-id <uuid> --policy-root <private-directory>`: no static root, spool or revision supplied by the caller. The silent helper reads current private policy and holds a shared authorization lease through the sanitized spool write. Revocation obtains the exclusive lease, publishes disabled policy first, changes SQLite revision, then publishes the matching state. Failed transitions stay disabled. Helper stdin is bounded to 1 MiB and a 750 ms receive deadline; malformed or missing authorization never stores payloads.

New production evidence uses `capture-v2-spool` under Tauri app data. Records remain sanitized before any durable text write. Strict JSON rejects duplicate keys, excessive nesting and unsupported identity. Import commits evidence/rejection metadata before acknowledging exact spool tokens. Replays have one database effect. Spool eviction counts are durable even when no event files remain, and visible as coverage warnings rather than invented events.

A turn `Stop` makes an episode idle; it does not prove the coding session ended. Source `SessionEnd` or the user's Finish action closes an episode with truthful reason/coverage. New work after closure becomes a continuation; late non-start evidence amends the old revision and makes prior analysis stale. Restart and pause remain visible. No successful exit, missing prompt or independent authorship is invented.

Git is read-only and filtered before content collection. The first asynchronously collected baseline is labelled as possibly late; it does not prove exact task-start state. A closed episode first observed afterward gets final-only context. Dirty/no-Git/truncated states are retained. Overlapping changes have ambiguous attribution; no Git evidence proves the agent authored every change. Git subprocesses and output are bounded, but capture/pause can report temporary contention during collection.

## Analysis and self-check

Analysis requires a finalized session, sufficient retained evidence, a Keychain API key, remote permission for the current app run, and approval of an exact short-lived request preview. Local tracking and key presence grant no network consent. The preview contains the model, endpoint, complete request JSON, sanitized evidence, omissions and privacy/input revisions. No automatic project sends exist.

Analysis input includes at most 50 selected event excerpts and 20 filtered snapshot excerpts. It removes absolute paths and provider correlation IDs, rechecks current file exclusions and sanitizes before clipping. Raw tool responses and command output without reliable file provenance are omitted. A policy fingerprint invalidates previews when exclusions change. The budget is 128 KiB and a conservative 24,000-token estimate; reduction records omissions. This can leave a useful capture too sparse for analysis, which is reported explicitly.

The initial evaluation model is `gpt-4o-mini-2024-07-18`, at the fixed OpenAI Responses endpoint, with `store:false`, strict JSON schemas, no tools, no redirects and no background jobs. Keychain is the only durable credential store; the frontend can set/delete a key and see configured state, never retrieve it. API billing is separate from ChatGPT subscriptions. `store:false` does not promise zero provider retention; normal provider abuse-monitoring policy can still apply.

Requests have a 10-second connection timeout, 60-second request timeout and 512-KiB response cap. At most three transient attempts occur, with 2/8-second delays. Quota/authentication, unknown errors, refusal and invalid model output are not retried. Consent, policy and current input are rechecked before retries and publication. Local cancellation discards late results; transmitted data cannot be recalled. Running jobs are cancelled on restart and require a new preview; they are never automatically resent. Raw model bodies, keys and request content are not logged.

A validated explanation has six reconstruction sections, observed/inferred/unknown claim labels, resolving evidence references and at most three ranked concepts. Known aliases use a small local registry; ranking is deterministic and exposes component scores. Snapshot quotations must match retained sanitized evidence exactly. Reference and schema validation do not mechanically prove every natural-language claim true: the live model-quality gate still needs human review. No mastery score or knowledge promotion is shown.

Each selected concept has an immutable self-check and a hidden delayed variant. MCQ and restricted deterministic JavaScript literal arithmetic/equality prediction are graded locally. Generated code is never executed; unsupported prediction forms are rejected. Explain-why answers are saved pending and can be graded only through a separately approved rubric/answer request. Low confidence or uncertain criteria preserve an uncertain result. Assistance is self-reported; revealing feedback/solutions marks subsequent practice assisted. Attempt UUIDs make retries idempotent, while ID reuse with changed content is a conflict.

Explanation, initial/delayed questions, exposure records and publication status commit atomically. Answers and feedback survive restart. Old attempted questions remain immutable through regeneration. Exposure is not mastery. A partial explanation is labelled an internal test result, never a complete ready lesson before the mini challenge exists.

## Schema and deletion

Migrations 0001–0003 remain unchanged. Additive 0004 creates `capture_episodes` and `episode_ingress`; additive 0005 creates analysis runs, learning documents, immutable questions, exposures, reveals and attempts. Schema version is 5. Aggregate revisions replace relational children without deleting the session parent, so learning history is preserved. All learning children cascade on session/project deletion.

Deleting a session cancels its analysis, tombstones associated ingress UUID/source identities for at least seven days, deletes owned ingress and the aggregate with its learning children, and retains metadata preventing ordinary replay. New source work may form a new episode. Project deletion first disables tracking and advances policy, then tombstones/deletes ingress and cascades sessions. It never deletes the user's project or Codex transcript. Disconnecting removes only owned configuration and keeps Mochi history.

SQLite/spool/config recovery are protected by filesystem permissions, not application-level encryption. Logical deletion is not forensic erasure. Explicit ingress retention and checkpoint primitives exist; automatic 30-day partial-context expiry, export/reset UI and background retention are still future V1 work. Sanitization recognizes supported patterns, not every possible secret or proprietary datum.

## Reproduce the personal trial

1. Build with `pnpm desktop:build`, then open `target/release/bundle/macos/mochi.app`.
2. Start with a disposable synthetic project. Enter its canonical root and a non-sensitive alias; approve the root. Review Connect's exact project hook change and enable local tracking explicitly.
3. In normal Codex CLI 0.151.0, review each Mochi handler using `/hooks`. Do not trust unrelated handlers merely because they appear beside Mochi.
4. Perform a small normal coding task with tests. Return to Mochi, inspect captured events/coverage and Git warnings, then Finish the episode if needed. Quit/reopen and verify no duplicate sessions.
5. Supply your own OpenAI API key through Mochi's password field; it goes to Keychain. Enable remote analysis for this run. Preview the session request, inspect every data category, and approve only if acceptable. This is a billed API request.
6. Compare explanation claims and snapshot quotations to the retained evidence. Complete a short check; for explanation grading, review and approve its separate request. Restart and verify the answer/feedback remain, while remote permission is off again.
7. Exercise pause, cancellation, exclusion changes, offline error and session/project deletion. No stale result should recreate deleted data. Remove the test key and disconnect project hooks when finished.

Do not run this trial on private material merely to validate the build. Desktop capture is still partial and outside this personal CLI support commitment.

## Validation evidence and remaining gates

Automated verification covers authorization/revocation, strict ingress and secret canaries, commit-before-ack replay, episodes/restart/continuation, migration upgrades, session deletion/replay, Keychain ports, exact preview consent, exclusions invalidating grants, cancellation/late responses, malformed output, bounded evidence, local/advisory grading, idempotent attempts and restart persistence. Native Keychain set/read/delete was also exercised using one disposable synthetic entry, then removed.

The Brief 08 real CLI install/trust/task/disconnect gate is recorded in [installer evidence](CODEX_INTEGRATION_INSTALLER.md). Process tests exercise the new authorized helper; native learning-flow tests use injected provider responses. These are useful implementation evidence, not a substitute for the new complete live CLI/BYOK trial, model semantic-quality evaluation, clean-machine installation, signing/notarization, sleep/wake or long-session performance audits. Final repository-gate and native GUI evidence is recorded in [the execution record](MVP_IMPLEMENTATION.md).

Official adapter references: [Responses migration](https://developers.openai.com/api/docs/guides/migrate-to-responses), [Structured Outputs](https://developers.openai.com/api/docs/guides/structured-outputs), [model](https://developers.openai.com/api/docs/models/gpt-4o-mini), [provider data controls](https://developers.openai.com/api/docs/guides/your-data), [reqwest](https://docs.rs/reqwest/0.13.5/reqwest/).

The 2026-10-02 [interface finalization](UI_FINALIZATION.md) adds the selected Signal artwork, lowercase application branding and separate Sessions/Settings navigation. This presentation checkpoint does not change the live-trial or release gates above.

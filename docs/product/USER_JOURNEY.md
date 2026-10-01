# Mochi V1 user journey

Baseline: 1.0. Authority: end-to-end behavior. Screen rules: [V1_UX.md](../design/V1_UX.md). Consent details: [PRIVACY_SECURITY.md](../security/PRIVACY_SECURITY.md).

## First launch

| Step | User action | Mochi behavior / success | Recovery |
|---|---|---|---|
| 1. Welcome | Opens installed app | Explains learning from actual work; no account | Quit safely without changing Codex |
| 2. Detect Codex | Runs detection | Shows supported client/version and available coverage | Missing/unsupported: actionable instructions; do not install Codex automatically |
| 3. Privacy | Reads summary, approves local tracking | Lists captured data and exclusions; tracking still limited to selected roots | Decline: use empty app/settings; no capture |
| 4. Projects | Chooses canonical project root | Shows scope, exclusions, local storage, Git availability | Non-Git allowed with qualified evidence; denied path not read |
| 5. Connect | Explicitly approves integration configuration | Preview changes, backup, idempotent install, test | Parse/permission/concurrent-edit failure: unchanged config or safe rollback |
| 6. Analysis | Optionally enables remote analysis and enters API key | Key goes to Keychain; remote terms explain code transfer and cost | Skip: capture-only mode; key failure does not break capture |
| 7. Integration test | Starts a small real Codex action in approved project | Sanitized test session appears with identity/activity/completion and coverage | Diagnostics contain metadata only; retry/disconnect available |
| 8. Ready | Returns to normal coding | Menu bar reports tracking status | Notification permission optional; no forced OS permission prompts |

Configure remote consent per session by default when enabled, or explicitly select automatic analysis for an approved project. Global capture approval and project automatic-analysis approval are separate controls. Unrecognized project activity creates only a temporary metadata consent prompt, not a stored transcript.

## A normal coding episode

1. User works in their existing supported Codex client. Mochi observes approved activity through the proven adapter. No prompt intervention.
2. The helper sanitizes before writing spool. The core imports events idempotently and captures a filtered Git baseline as early as possible.
3. Menu bar shows active episode, project alias, elapsed observed time, and coverage. Code changes are not called learned concepts during capture.
4. On proven completion, or the user's confirmed end after inactivity, Mochi freezes the episode and captures final Git context. Missing boundaries remain visible.
5. Local session becomes available. If remote analysis is not authorized, the CTA is **Review analysis context**, followed by explicit approval. If project automatic analysis is authorized, queue generation using current policy.
6. Sanitized context is previewable. Analysis reconstructs what changed, chooses up to 3 priorities, and produces a validated debrief/questions/challenge.
7. A ready lesson appears in Learn/Home. Notify once if permission and preference allow. No notification for insufficient context or an active coding turn.

## Learning and recall

| Stage | User experience | Stored consequence |
|---|---|---|
| Session detail | Goal, timeline, changes, evidence, coverage, status | No knowledge promotion from opening |
| Debrief | What built/changed, decisions, failures/fixes, project examples, why, misconception | Valid selected concepts produce exposure once per episode |
| Self-check | Answer MCQ, predict output, or explain why; then feedback | Immutable sanitized attempt, outcome, assistance, evidence |
| Mini challenge | 5–10 minute text solution in Mochi; optional hint | Advisory rubric evaluation; no execution; self-reported independence |
| Knowledge | Concept states and supporting history | Derived state recomputed deterministically |
| Review later | Due queue, one cached question per concept, feedback | Review interval/state update only from eligible recall evidence |

Reading is useful but does not count as demonstrated understanding. Viewing the solution or using hints marks assisted practice. Uncertain AI grading does not promote state; allow a revised answer or additional question.

## Returning after restart

Resume durable jobs only if consent still holds. Recover spool once without duplicated sessions/knowledge. Home presents unfinished learning and due review. A crash does not imply the Codex agent finished successfully. Continued provider threads are bounded into episodes according to [SESSION_EVENT_SCHEMA.md](../architecture/SESSION_EVENT_SCHEMA.md).

## Failure and control paths

| Condition | User-facing result | Allowed next action |
|---|---|---|
| No Git / late baseline | Evidence limitations shown | Learn only from available evidence |
| Missing events / overlapping writers | Partial or ambiguous attribution badge | Inspect timeline; finalize/retry when appropriate |
| Offline / provider timeout | Local session safe; generation queued or failed | Retry after connection with valid consent |
| Invalid key / insufficient provider credit | Explain provider access failure | Change key/settings; no automatic repeated billing attempts |
| Model refusal / invalid result | No ready lesson published | Retry once through policy or inspect sanitized context |
| Huge / excluded context | Truncation or insufficient-context explanation | Adjust approved filters/limits for a future explicit analysis |
| Tracking paused | No new payload writes; cached data available | Resume explicitly; gap preserved |
| Remote consent revoked | Cancel pending sends; discard revoked in-flight result | Local history/review continue |
| Session/project deletion | Show exact affected history, knowledge, review counts | Confirm deletion; cancel work and prevent spool replay |
| Quit Mochi fully | Core stops; installed helper can spool approved activity within limits | Reopen to import; disable tracking to stop helper capture |

Closing the window keeps the core running. Full quit and disconnect are different actions, explained in the menu and settings. Disconnect removes Mochi-owned integration changes and disables capture; local learning history remains unless explicitly deleted.

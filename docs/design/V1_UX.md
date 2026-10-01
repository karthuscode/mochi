# Mochi V1 UX specification

Baseline: 1.0. Authority: navigation, screens, visible states, accessibility and native experience. Underlying states/consent are owned by [events](../architecture/SESSION_EVENT_SCHEMA.md), [knowledge](../learning/KNOWLEDGE_MODEL.md), and [security](../security/PRIVACY_SECURITY.md).

## Design direction

Mochi is a calm, friendly small robot companion. It helps the user understand their work without guilt, forced streaks, skill-score theater or interrupting the coding tool. Use restrained robot artwork, clear typography, generous reading space and code legibility. Brand assets can be designed later; no generated mascot asset is needed to implement V1 logic.

English is the initial UI language. Follow system light/dark preference with optional override. Colors communicate status alongside text/icon; no color-only information. Keep technical protocol details in expandable diagnostics, while coverage/privacy explanations remain visible in plain language.

## Navigation and routes

| Navigation | Screen / primary action |
|---|---|
| Home | Active/latest session, continue learning, due review, recent concepts |
| Sessions | Local session library → session detail |
| Learn | Ready/stale learning debriefs → concept cards → self-check/challenge |
| Knowledge | Concept list → evidence history and due status |
| Review | Due queue → focused review run → feedback |
| Settings | Codex, project access, privacy, AI analysis, notifications, storage |

Onboarding precedes normal shell on first launch but may be skipped into capture-disabled settings. Deep links/menu notifications open a session/debrief by opaque ID; missing/deleted IDs show safe empty state, not a recreated session. Sessions owns activity evidence; Learn owns teaching. Avoid two separately editable copies of a lesson.

## Screen contracts

### Home

Show tracking state, current episode(s), latest completed session, pending analysis consent, unfinished lesson, and up to 3 due concepts. Do not show “concepts learned” or percentages from session activity. Empty state: “Connect Codex to learn from your next session” with integration/settings link. Without remote consent: “Sessions stay on this Mac. Enable analysis when you're ready.”

### Sessions library and detail

Library: project alias/title/date, capture state, coverage, lesson status. Search local safe summaries; filter project/date/status. Paginate 50 rows. Details: observed goal, elapsed time when known, normalized timeline, filtered changed-file list, before/after snippets, test outcomes and attribution warning. Large code/timeline panels load incrementally.

Coverage labels: “Partial capture”, “Git unavailable”, “Start not observed”, “Changes may include other activity”. `complete` coverage describes declared adapter coverage, not omniscience. Never conflate stopped/completed with tests succeeding.

Actions: Finish active/idle episode, Review analysis context, Approve/send, Cancel, Retry, Open lesson, Export/delete. Finalize CTA states that it ends Mochi's episode, not Codex's agent. No live lesson generation. Analysis unavailable states explain missing consent/key/network/context separately.

### Analysis context preview

Show provider/model, safe project alias, included source/data categories, redaction count, truncation/gaps, sanitized excerpts and estimated request size. Explain remote processing and API costs. Primary explicit action: “Send this context to OpenAI”. Project automatic-analysis option has its own unchecked control with a clear scope statement. Changed bundle hash invalidates old per-session approval.

Never reveal detected secrets in preview. Read-only sanitized preview is the exact bundle used for the approved request; instructions/contract metadata can be collapsed. Revocation messaging explains that already sent data cannot be recalled.

### Learn / debrief

Lesson header: session/project/date, reading estimate, coverage/stale note. Sections follow [LEARNING_MODEL.md](../learning/LEARNING_MODEL.md). Concept cards have project excerpt (snapshot date/path), explanation, why it matters, misconception and self-check. “Try it yourself” contains one small challenge with primary concept, estimated 5–10 minutes and constraints.

Self-check: answer → submit → feedback → next. Preserve answer after navigation. Reveal/hint option clearly marks assisted practice before use. Feedback states correct/incorrect/uncertain/ungraded, why, and what evidence counted. A pending remote grade must not show success or knowledge promotion. No confetti from exposure.

Challenge submission is a code/plain text area with language highlighting, copy/paste and keyboard accessibility. Label “Your solution is reviewed against a rubric; Mochi doesn't run it.” Ask a simple independence self-report, allow unknown/assisted, and never force a false claim. No terminal/run button.

### Knowledge

List title, five-state label, due/refresh status and counts. Detail shows encounters, practices/outcomes, eligible recall days, independent challenge count and source history. State explanations must be available with keyboard and screen reader. Wrong recent reviews show refresh without rewriting history. Reset progress previews evidence removed and explains retained lessons remain available.

### Review

Default batch ≤3 concepts or 10 minutes; show other due items. Use an unseen cached question with known assumptions. Submit/feedback/next, optional skip and explicit snooze/pause. Interrupted runs retain answered attempts; unanswered items stay due. If no independent-eligible variant exists, offer practice with clear no-promotion label or an explicitly authorized generation action.

Offline: cached MCQ/output questions grade locally; rubric text remains ungraded. Due list doesn't trigger remote calls. End-of-run reports evidence-based outcomes and next dates, never a fabricated overall mastery score.

### Settings

| Section | Controls |
|---|---|
| Codex | Detected client/version, capabilities, connect/test, pause/disconnect, remediation |
| Projects | Approved canonical roots/aliases, exclusions, tracking, remote per-session/automatic choice |
| Privacy | Collection disclosure, preview policy, remote revoke, data retention |
| AI | OpenAI key configured status, set/replace/delete, model selection and user-triggered minimal test |
| Notifications | Lesson-ready/review controls, OS permission status, login startup opt-in |
| Storage | Local usage by category, expiry, export JSON, delete project/session/learning data |

Key fields are write-only masked entry; existing keys cannot be revealed by frontend. Config mutation/deletion previews are reviewable and do not overwrite unrelated values. Friendly errors are actionable and avoid dumping raw paths/config/payloads.

## Global status presentation

| Canonical status | Display |
|---|---|
| capture active / idle | “Tracking session” / “Session inactive — finish when ready” |
| interrupted / partial | “Capture interrupted; some activity may be missing” |
| awaiting_consent | “Review context before analysis” |
| queued / running | “Waiting to analyze” / “Preparing your lesson” with cancel |
| ready | “Lesson ready — N concepts” where N is 1–3 |
| insufficient_context | “Not enough safe context to create a lesson” |
| failed | Cause + retry/settings action; local session retained |
| cancelled | “Analysis cancelled”; explain remote revocation if relevant |
| stale | “New session evidence is available; this lesson uses an earlier snapshot” |

Use separate capture and lesson status badges; “session ready” alone is ambiguous. No success notification for insufficient context. Loading skeletons must not suggest nonexistent data. All empty/error states keep navigation/settings accessible.

## Native behavior

Menu bar shows disabled/paused/active/idle state, safe project alias, elapsed observed duration and pending lesson/review count. Actions: Open Mochi, current session, pause/resume, Finish episode, Settings, Quit. Closing window leaves core running. Quit text explains installed helper may still spool approved sessions; disabling tracking stops collection.

Lesson notification: “Your Mochi lesson is ready. N concepts to review.” No code, prompt, secret, absolute path or confidential project title. Notify once per debrief publication; regeneration does not spam. Optional review reminder at most once per local day. Suppress nonurgent notifications while an observed coding turn is active, then deliver at idle if enabled.

## Accessibility and sizing

Minimum intended window 900×640 points; readable debrief content capped around 760 points wide. Side navigation can collapse; system text sizing and zoom remain usable. Verify keyboard-only traversal, visible focus, logical heading order, screen-reader names, adequate contrast (target WCAG AA), reduced motion and code copy feedback. Do not use tooltips as the only state explanation or custom editor interactions that trap focus.

## Acceptance

Walk through first-run, normal lesson, offline review, partial capture, revoked consent and deletion on a real arm64 Mac. No essential action requires a browser, account, developer console, mouse-only input, or hidden setting. See [VALIDATION_PLAN.md](../implementation/VALIDATION_PLAN.md).

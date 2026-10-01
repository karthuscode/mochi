# Mochi V1 product specification

Baseline: 1.0. Authority: product promise and release outcomes. Scope details: [V1_SCOPE.md](V1_SCOPE.md); provenance/defaults: [DECISIONS.md](DECISIONS.md).

## Problem and promise

AI-assisted developers can ship changes they cannot later explain. Mochi preserves the useful learning opportunities from their own coding work without interrupting it. **Use AI to build. Use Mochi to understand.**

Mochi is not a generic course generator or a session summary alone. Its complete loop is capture → reconstruct → select useful concepts → explain from project evidence → test understanding → retain evidence-based knowledge → review later.

## Users

| Persona | Job to be done | Successful outcome |
|---|---|---|
| CS student | Learn from AI-assisted assignments and personal projects | Can explain and reproduce an important idea from their own code |
| Vibe coder | Understand the feature they approved | Connects agent changes to concepts and tests their understanding |
| AI-heavy developer | Refresh skills weakened by delegation | Revisits relevant ideas and earns evidence from later recall |

Mochi does not infer incompetence from AI use or treat exposure as mastery. Lessons are optional and respectful. No surveillance of unrelated work.

## Product requirements

| ID | Requirement | Observable acceptance |
|---|---|---|
| P01 | Entire V1 lives in one desktop application | Install → learn → later review without a web platform/account |
| P02 | Passive, approved Codex capture | Real supported Codex session captured without transcript export/copy-paste; activity continues if Mochi fails |
| P03 | Truthful session evidence | Project, activity, available events, completion reason, coverage gaps, and Git baseline/final context preserved |
| P04 | Local data ownership | Sessions, knowledge, attempts, reviews survive restart in local SQLite; usable offline |
| P05 | Privacy at every boundary | Recognizable secrets absent from spool, DB, logs, exports, and outbound analysis; exclusions apply before file reads |
| P06 | Explicit remote analysis choice | Default off; opt-in identifies provider, code/text transfer, API cost, and revocation behavior |
| P07 | Evidence-grounded reconstruction | Goal, changes, decisions, failures, fixes reference captured evidence or state uncertainty |
| P08 | Focused learning | At most 5 candidates, 1–3 priorities when justified; no forced filler concepts |
| P09 | Useful debrief | Clear session overview, project code, why it works, misconception, checks, and one mini challenge |
| P10 | Active assessment | Every priority concept has ≥1 self-check; generated lesson set includes a reasoning/output format when suitable |
| P11 | Honest knowledge | Five canonical states with visible evidence history; no promotion from reading/AI usage alone |
| P12 | Later review | Due concepts scheduled deterministically; offline local questions supported; no duplicate promotion on replay |
| P13 | Native companion | Background capture, menu bar status, optional lesson-ready notification, accessible keyboard operation |
| P14 | Manage and recover | Pause/disconnect, retry, change API settings, inspect storage, export and delete; actionable failure states |
| P15 | Provider-independent domain | Core/UI consume normalized `SessionEvent` and `AnalysisProvider` results, not Codex/OpenAI SDK types |
| P16 | Deliverable arm64 release | Packaged macOS Apple Silicon app verified on a clean supported machine with integration/uninstall behavior |

## First useful experience

After onboarding and project approval, a normal Codex work episode yields a locally visible session. If remote analysis is authorized and configured, Mochi prepares a sanitized bundle and creates a debrief. The user understands the actual change, practices 1–3 concepts, submits a small challenge, sees truthful knowledge evidence, and later completes a scheduled review.

Capture succeeds independently of API configuration. Without remote consent, the user sees local session history and can practice already cached material, but new AI-generated lessons are unavailable. Local-first does not imply a shipped on-device LLM.

## Release definition of done

P01–P16 pass the scenarios in [VALIDATION_PLAN.md](../implementation/VALIDATION_PLAN.md). In particular, a fresh user must be able to install, connect a proven Codex client, code normally, finish a session, receive an authorized notification, inspect changes, learn 1–3 concepts, answer questions, complete a challenge, see knowledge update, and return for review. No manual transcript collection, developer-only setup, fake metrics, or hidden uploads.

Partial/no-Git sessions remain browsable. They receive a qualified debrief only when enough evidence exists. An empty, fully excluded, or unsupported session yields a clear insufficient-context result instead of invented lessons.

The approved 2026-10-01 personal CLI MVP is an intermediate engineering milestone: a real approved coding episode, grounded explanation and persisted self-check. It is not the complete V1 release; mini challenge, knowledge and delayed review remain required for their corresponding publication/release milestones. See [execution record](../implementation/MVP_IMPLEMENTATION.md).

## Quality and success measures

Evaluate with a small consented alpha cohort and synthetic fixtures; no production telemetry backend. Locally expose time-to-ready, event drops/gaps, analysis failures, and completed learning activities. Voluntary alpha feedback should assess whether users can explain a real project decision and later recall it. Session duration and lesson views are engagement signals, not learning proof.

Release targets: sanitized capture remains durable when the window is closed; UI actions remain responsive during capture; bounded resource use follows [ARCHITECTURE.md](../architecture/ARCHITECTURE.md); every learning claim has valid provenance; privacy checks have zero known leaks in the test corpus. Performance and grader quality thresholds are defined in the validation plan and verified before release.

## Non-goals

See [V1_SCOPE.md](V1_SCOPE.md). Mochi never writes the user's project as part of learning, executes challenge code, replaces Codex, or interrupts a coding session with a required quiz.

# V1 decision register

Baseline: 1.0, 2026-09-13. Authority: decision provenance and change control. The original conversation established direction and examples; it did not establish a working integration or all operational constants.

## Established product decisions

2026-10-01 — Companion 01 assignment: after choosing Signal, the user explicitly requested a live desktop preview. Implement the bounded static transparent-window trial, before animation/lifecycle work. `MOCHI_COMPANION_PREVIEW=1` selects a synthetic main page before any WebView loads and skips capture/database/analysis startup. Normal startup keeps the companion hidden; normal main close still exits. Window operations use distinct capabilities and fixed-label Rust commands. No preferences migration, website, release publication or capture authorization is implied. See [trial evidence](../implementation/COMPANION_STATIC_TRIAL.md).

| ID | Decision | Consequence |
|---|---|---|
| D01 | Name: Mochi; friendly small robot companion | Calm, helpful identity; no complex gamification |
| D02 | Learn from the user's actual AI-assisted project work | Evidence-grounded debrief, active recall, mini challenge, later review |
| D03 | Desktop contains the whole learning loop | Website deferred; no browser needed after install |
| D04 | Local-first storage in SQLite | No account/backend/cloud-sync requirement |
| D05 | macOS Apple Silicon first | Cross-platform boundaries, one release platform |
| D06 | Codex-only capture in V1 | `SessionSource` with one `CodexAdapter`; no alternate provider work |
| D07 | Provider-agnostic internal event and analysis contracts | Source adapter and analysis adapter evolve separately |
| D08 | Git before/after supplements transcript | Actual changes are evidence; transcript is not sole truth |
| D09 | Privacy before persistence and before LLM calls | Exclusions, secret detection, path filtering, size limits |
| D10 | `AnalysisProvider`, first adapter OpenAI | BYOK in OS Keychain; no hosted Mochi analysis service |
| D11 | At most 5 candidates, at most 3 priorities | Small focused lessons; zero is valid when evidence is insufficient |
| D12 | Multiple question formats, one 5–10 minute challenge | MCQ alone is insufficient; challenge is not a new project |
| D13 | NEW, EXPOSED, PRACTICING, DEMONSTRATED, STRONG | No unsupported language mastery percentages |
| D14 | Simple review intervals 1, 3, 7, 14, 30 days | Correct increases interval; incorrect shortens it |
| D15 | Home, Sessions, Learn, Knowledge, Review, Settings | One coherent native product |
| D16 | Recommended stack: Tauri 2 / React / TS / Rust / SQLite | Stack scaffold versions pinned by supplied Brief 00; bundled SQLite implemented by Brief 03 |
| D17 | Brief 01 proves passive integration before UI investment | Real Codex capture, no manual transcript export/copy-paste |
| D18 | Small helper and durable local event spool | Capture independent of foreground UI; no dependency on LLM/network |

## Implementation defaults specified by this pack

These complete underspecified details; they are changeable through an explicit documented decision, not historical user quotations.

| ID | Default | Owner |
|---|---|---|
| S01 | One Mochi session is a bounded work episode; resumed provider threads can create new episodes | [Event schema](../architecture/SESSION_EVENT_SCHEMA.md) |
| S02 | Idle confirmation after 15 minutes, never a fabricated provider completion | Event schema |
| S03 | Rust owns domain behavior; TypeScript consumes matching contracts | [Architecture](../architecture/ARCHITECTURE.md) |
| S04 | Capture requires global enablement and approved project root; remote defaults off, per-session approval or optional project automatic approval | [Security](../security/PRIVACY_SECURITY.md) |
| S05 | No challenge execution; rubric grading is advisory; independence is self-reported | [Learning](../learning/LEARNING_MODEL.md) |
| S06 | Deterministic knowledge thresholds and distinct-day evidence | [Knowledge](../learning/KNOWLEDGE_MODEL.md) |
| S07 | Sanitized events/context retained 30 days; lessons and derived evidence until deleted; spool 7 days / 100 MiB | Security |
| S08 | Bounded ingestion/analysis resources; overload is visible and never blocks Codex | Architecture / security |
| S09 | English documents and initial UI; localization deferred | [UX](../design/V1_UX.md) |
| S10 | JSON export only, no import/cloud backup; export sanitized and excludes secrets/config backups | [Data model](../architecture/DATA_MODEL.md) |
| S11 | Logical deletion with spool tombstones and recomputation; no forensic-erasure promise | Data model / security |
| S12 | Signed, notarized arm64 distribution for external alpha; local unsigned dev builds allowed earlier | [Roadmap](../implementation/V1_ROADMAP.md) |

## Feasibility gates and unresolved implementation choices

| Gate | Must establish | Timing / impact |
|---|---|---|
| G01 | Intended Codex client, actual version, supported passive transport, trust/config behavior, available events, termination semantics | Brief 01; blocks Briefs 02–38. Supplied Brief 00 explicitly permits only the minimal pre-gate scaffold |
| G02 | Compatible Tauri/Rust/frontend versions; minimum supported macOS version demonstrated on a clean test machine | Brief 02 provisional, Brief 38 final; no unsupported OS compatibility claim |
| G03 | OpenAI model available to alpha testers that supports required structured contracts, acceptable quality/cost/latency | Brief 14 + 17 eval; configurable model, no hardcoded current-model claim |
| G04 | Signing/notarization credentials and release channel | Brief 38; external distribution blocked until available |

G01 cannot be resolved by launching a separate App Server and controlling a new coding agent. Official [Hooks](https://learn.chatgpt.com/docs/hooks) documentation describes lifecycle hooks; [App Server](https://learn.chatgpt.com/docs/app-server) documents a client integration surface. Neither alone proves Mochi can passively observe the user's intended existing Codex client. This distinction is an implementation inference to be tested, not an asserted limitation of every Codex version. Sources checked 2026-09-13; verify again during the spike.

## Change protocol

2026-10-01 — Final identity selection: the user selected Round 02 / A **Signal**, an orange/graphite geometric robot, superseding the first clay cream/mint direction. Palette: coral `#FF7048`, graphite `#171B24`, paper `#FAFAFA`. Its notched helmet and two direct eyes define the flat vector mark, Mochi wordmark lockup and macOS icon; the mascot uses graphic 2.5D rendering. Static final assets and the native app icon are delivered in the separate brand brief. This does not authorize or claim companion runtime, capture/learning readiness, website implementation or external publication. See [identity](../design/MOCHI_IDENTITY.md) and [brand evidence](../implementation/MOCHI_BRAND_IDENTITY.md).

2026-10-01 — Revised identity exploration: the user requested robot, mochi-creature and abstract alternatives, aiming for a distinctive cool developer brand with slight playfulness. Generate visibly different forms/styles/palettes; do not retain robot/clay/cream-mint as mandatory design constraints. Round 02 is available for feedback in [visual identity](../design/MOCHI_IDENTITY.md); no production identity has been selected.

2026-10-01 — Identity visual-review correction: the user rejected the first three clay robot concepts and explicitly rejected the cream/mint palette. Reopen character, palette and rendering-style exploration before final identity selection. Preserve the first round only as rejected design history; do not integrate it as production artwork. Previously selected stationary companion interactions and native/distribution direction remain unchanged unless the user revises them. See [visual identity](../design/MOCHI_IDENTITY.md).

2026-10-01 — User-assigned identity/companion extension to D01/D03: soft matte clay chibi robot, cream/graphite/mint palette, robot-face mark plus Mochi wordmark, and an optional stationary floating desktop character with blink/float and three pointer-entry reactions (wave, squash-and-spring, smiling head tilt). Ordinary windows remain underneath; fullscreen workspaces hide the companion. Execute as separate bounded identity, static native feasibility and animation/lifecycle briefs. Three concepts are saved; final identity selection is pending. Distribution is intended through a future download website rather than the Mac App Store; macOS transparent Tauri private-API support is acceptable for that direction, subject to native feasibility. This does not add website implementation to V1, complete G04, change capture/learning contracts or authorize publication. Evidence and acceptance: [Brand execution](../implementation/MOCHI_BRAND_IDENTITY.md), [visual identity](../design/MOCHI_IDENTITY.md).

2026-09-13 — D16/D17 and G01/G02 sequencing amendment: the user's supplied **Brief 00 — Repository Foundation** requests a runnable minimal workspace now, while the original roadmap put that work in Brief 02 after G01. Follow the explicit task by moving scaffold/toolchain/CI work into Brief 00; keep G01 unresolved and retain every capture, privacy and product gate. Brief 02 becomes a post-spike foundation compatibility review. Evidence and acceptance impact: [FOUNDATION.md](../implementation/FOUNDATION.md). Affected documents: README, AGENTS, architecture, roadmap, brief template and validation plan. No persisted product data or migration is introduced; minimum-macOS and release validation remain open.

For each changed decision, record ID, date, reason, evidence, affected specs, and migration/acceptance impact. Routine details within these contracts need no new product approval. A failed integration gate, new remote transfer, new provider, or scope expansion requires a concrete revised proposal before dependent work. Do not silently pivot to an IDE, screen recorder, private transcript parser, or manual import workflow.

2026-10-01 — Internal MVP milestone: the user selected personal Apple Silicon macOS use with Codex CLI, aiming at a real approved work episode → evidence-grounded explanation → persisted self-check. The full V1 scope and ready-lesson publication requirements remain unchanged; partial explanation is not a full ready lesson. Remote analysis still defaults off with per-session preview approval and Keychain BYOK. Implementation proceeds through bounded units with real-client gates; see [MVP execution](../implementation/MVP_IMPLEMENTATION.md). Native installer ownership/recovery is now implemented; production capture/analysis/learning are not.

## Personal CLI learning MVP implementation — 2026-10-01

The user authorized sequential implementation through a first testable personal macOS/CLI learning loop. Briefs 09–21 now implement that bounded subset: current capture authorization, revisioned episodes, filtered Git, exact per-request consent, Keychain BYOK and durable explanation/self-check. The fixed initial G03 evaluation model is `gpt-4o-mini-2024-07-18`, with strict OpenAI Responses schemas and `store:false`; it is not accepted as meeting semantic quality/cost targets until a live consented evaluation. Remote permission resets off every run and unfinished analysis is cancelled rather than automatically resent. Restricted literal JavaScript prediction is validated without code execution. Delayed variants are stored but not scheduled. Full ready publication still requires Brief 22; knowledge/review/distribution remain later. See [current implementation and limits](../implementation/INTERNAL_CLI_MVP.md).

## 2026-10-02 — personal MVP interface finalization

User-authorized presentation polish uses the selected Signal assets, restrained glassmorphism and lowercase `mochi` in UI/window/bundle naming. The current milestone exposes Sessions/Settings with the existing consent/learning controls; full V1 navigation remains future work. Appearance override is window-local, not a durable preference. Browser rendering is a disabled appearance preview with no fake session data or native calls. This does not accept the live CLI/BYOK model-quality gate, complete lessons or the pending static companion native scenarios.

2026-10-03 - User-authorized Bento Home amendment to D16/D17: native single-folder
selection, explicit coding-source choice and targeted routing to existing
Sessions approval replace only the post-tour guide. A Rust-owned private atomic
UI preference remembers Home reached and last approved project UUID; no path,
key, permission or connection proof is persisted there. Shared typed local reads
and selection epochs prevent old-project publication. Welcome/slides, exact
connection/send approvals, draft preservation and unavailable Desktop/Claude/
Cowork boundaries remain. No migration or new integration/learning capability.
Affected UX, architecture, data model and privacy documents are updated;
[implementation evidence](../implementation/BENTO_HOME.md) records acceptance.

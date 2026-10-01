# Mochi V1 scope

Baseline: 1.0. Authority: feature inclusion and exclusion. [Product spec](V1_PRODUCT_SPEC.md) owns outcomes; [Roadmap](../implementation/V1_ROADMAP.md) owns sequencing.

## Required

| Area | V1 behavior | Limit |
|---|---|---|
| Platform | macOS Apple Silicon desktop | Windows/Linux/Intel releases are not acceptance criteria |
| Integration | Detect, consented configure, test, pause, disconnect Codex | Exact client/version/transport proven in Brief 01 |
| Capture | Helper, sanitized durable spool, normalization, session assembly, recovery | Only available provider events; gaps explicit |
| Project context | Approved local roots; Git baseline/final; filtered snippets | No arbitrary filesystem access, no Git mutation |
| Storage | SQLite migrations, history, attempts, knowledge, due reviews | Single OS user/local profile; no Mochi account |
| Analysis | Sanitized bundle, staged reconstruction, concepts, lessons | OpenAI BYOK first; remote off by default |
| Learning | ≤5 candidates; ≤3 priorities; self-checks; one 5–10 minute mini challenge | Text submissions only; no runtime/editor platform |
| Knowledge | Canonical concepts, evidence, deterministic states | Honest concept-level states, no language percentages |
| Review | 1/3/7/14/30-day scheduler and offline cached questions | No advanced adaptive/Anki-equivalent algorithm |
| UI | Home, Sessions, Learn, Knowledge, Review, Settings | One desktop UI; friendly robot identity kept small |
| Native | Background runtime, menu bar, opt-in notifications | Login startup optional and user-controlled |
| Data control | Preview outbound context, pause, export JSON, delete/reset, retention | No import or cloud backup |
| Release | Recovery, privacy/performance audits, end-to-end checks, packaged arm64 alpha | External alpha requires signing/notarization |

## Conditional/degraded behavior

- Missing Git: capture activity; mark Git unavailable and avoid actual-change claims unsupported by evidence.
- Late/missing baseline: retain final state, label baseline unavailable; never present whole working-tree diff as session delta.
- Missing prompt/tool events: use proven available activity and Git; surface missing coverage. Minimum learning viability must still pass Brief 01.
- Offline/missing API key/no remote consent: capture/history/cached lessons/local assessments work; new generation waits for user action or authorized connectivity.
- Insufficient evidence: no learning priorities, no assessment/challenge fabrication, no knowledge exposure from guessed concepts.
- Multiple simultaneous sessions in one repository: mark attribution ambiguous; do not assert which actor changed each line.

These paths are required robustness, not substitutes for a passing normal end-to-end journey.

## Explicitly excluded

Website/marketing-page implementation, web dashboard, web learning platform, mobile app, cloud sync, multi-device account, hosted Mochi backend, billing/subscriptions, teams/workspaces, friends/social/leaderboards, complex gamification, marketplace, GitHub profile analysis, additional coding providers (Cursor/Claude Code/Windsurf/Copilot), embedded coding agent/IDE, screen recording, keystroke capture, general computer surveillance, executing challenge code, automatic source modifications, and production telemetry uploads.

The future marketing/download site may be a separate post-V1 task. It cannot block the app. Cross-platform ports and abstraction seams are included; speculative implementations are excluded.

## Scope discipline

Every brief identifies included behavior, exclusions, dependency gate, data/privacy impact, and objective acceptance. Do not expand a brief to solve future-provider, cloud, or website needs. If a gate fails, document evidence and a revised proposal. For bounded implementation details, use the defaults in [DECISIONS.md](DECISIONS.md).

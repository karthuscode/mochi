# Internal CLI learning MVP — execution record

Decision: 2026-10-01. The user selected a personal macOS Apple Silicon MVP using Codex CLI. The complete V1 product scope is retained; this adds an intermediate milestone, not an external alpha or a replacement release definition.

## Observable target

Approve a project/local tracking → normal CLI work → local session with truthful coverage and changes → inspect sanitized analysis bundle and approve the exact send → evidence-grounded explanation with at most three priority concepts → submit short checks → retain answers and feedback after restart. Remote defaults off; BYOK stays in Keychain. Partial explanations must not use the full ready-lesson status before mini-challenge publication exists.

## Bounded execution units

| Unit | Acceptance | Current state |
|---|---|---|
| 0 Restart checkpoint | Reviewed source inventory, complete existing gates, local versioned baseline; targeted detection regressions | Baseline `04698f6` on `codex/cli-mvp`; all existing gates passed; detection corrections implemented |
| 1 Brief 08 | Exact consented hook merge, ownership, replay/recovery, rollback/disconnect; real CLI proof | Native implementation and synthetic real-helper flow passed; real Codex trust/task gate pending |
| 2 Brief 09–12 | Authorized helper lifecycle, continuous bounded import, episode continuation/finalization, truthful Git context, minimal local controls/session display | Not implemented; instantiate each bounded brief after unit 1 gate |
| 3 Brief 13–19 | Exact sanitized bundle preview/consent, Keychain BYOK/provider cancellation, grounded reconstruction/concepts/explanation and display | Not implemented |
| 4 Brief 20–21 | Valid questions, uncertainty-aware feedback, immutable questions and transactional idempotent attempt persistence | Not implemented |

Later: mini challenge, knowledge reducer, delayed review, full native surfaces and distribution gates remain the existing roadmap. No cloud service, additional coding source or Desktop support commitment is added.

## Verification

Restart baseline: `pnpm typecheck`, `pnpm lint:frontend`, `pnpm test:frontend`, `pnpm build`, `pnpm test`, `pnpm lint`, `pnpm format:check`, `pnpm check:rust`, `pnpm desktop:build` passed on 2026-10-01. 98 Rust + 18 frontend tests; local unsigned `.app` produced. This does not claim fresh native GUI/accessibility or clean-machine validation.

Installer validation: all nine repository gates listed above passed again after implementation, including the native `.app` build. 112 Rust tests (27 integration tests and 1 real-helper connection test included) + 18 frontend tests passed; the helper release build also passed. Fixtures and runtime directories are isolated and contain no real transcripts or credentials. Neither user Codex configuration nor trust state has been changed by this implementation.

The installed `/opt/homebrew/bin/codex --version` reports CLI `0.151.0`. A fresh real-task proof of the new installer's command groups is still required; historical capture evidence and direct-helper tests are not substituted for it.

## Next concrete gate

Prepare one disposable synthetic CLI project, show its exact hook target/command/spool and cleanup plan, obtain temporary integration approval, then use the normal Codex trust UI and run a small synthetic coding task. Verify silent helper behavior and normalized coverage; disconnect only recorded owned entries. If this fails, report the actual failure and fix Brief 08 before production bridge work. Never claim the full MVP complete from the current native boundary.

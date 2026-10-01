# Internal CLI learning MVP — execution record

Decision: 2026-10-01. The user selected a personal macOS Apple Silicon MVP using Codex CLI. The complete V1 product scope is retained; this adds an intermediate milestone, not an external alpha or a replacement release definition.

## Observable target

Approve a project/local tracking → normal CLI work → local session with truthful coverage and changes → inspect sanitized analysis bundle and approve the exact send → evidence-grounded explanation with at most three priority concepts → submit short checks → retain answers and feedback after restart. Remote defaults off; BYOK stays in Keychain. Partial explanations must not use the full ready-lesson status before mini-challenge publication exists.

## Bounded execution units

| Unit | Acceptance | Current state |
|---|---|---|
| 0 Restart checkpoint | Reviewed source inventory, complete existing gates, local versioned baseline; targeted detection regressions | Baseline `04698f6` on `codex/cli-mvp`; all existing gates passed; detection corrections implemented |
| 1 Brief 08 | Exact consented hook merge, ownership, replay/recovery, rollback/disconnect; real CLI proof | Passed: native checks and real CLI 0.151.0 install/trust/task/disconnect on 2026-10-01 |
| 2 Brief 09–12 | Authorized helper lifecycle, continuous bounded import, episode continuation/finalization, truthful Git context, minimal local controls/session display | Not implemented; instantiate each bounded brief after unit 1 gate |
| 3 Brief 13–19 | Exact sanitized bundle preview/consent, Keychain BYOK/provider cancellation, grounded reconstruction/concepts/explanation and display | Not implemented |
| 4 Brief 20–21 | Valid questions, uncertainty-aware feedback, immutable questions and transactional idempotent attempt persistence | Not implemented |

Later: mini challenge, knowledge reducer, delayed review, full native surfaces and distribution gates remain the existing roadmap. No cloud service, additional coding source or Desktop support commitment is added.

## Verification

Restart baseline: `pnpm typecheck`, `pnpm lint:frontend`, `pnpm test:frontend`, `pnpm build`, `pnpm test`, `pnpm lint`, `pnpm format:check`, `pnpm check:rust`, `pnpm desktop:build` passed on 2026-10-01. 98 Rust + 18 frontend tests; local unsigned `.app` produced. This does not claim fresh native GUI/accessibility or clean-machine validation.

Installer validation: all nine repository gates listed above passed again after implementation, including the native `.app` build. 112 Rust tests (27 integration tests and 1 real-helper connection test included) + 18 frontend tests passed; the helper release build also passed. Automated fixtures and runtime directories are isolated and contain no private transcripts or credentials. Source baseline `34fd1cf` contains the installer implementation; the subsequent real trial changed only its disposable project hooks and used normal Codex project/hook trust review.

The real CLI gate passed on the installed `0.151.0`: all 12 owned handlers were recognized and individually trusted, a synthetic calculator fix and test succeeded, and the completed task yielded 19 correlated normalized records. The exact disconnect then removed the created hook file. The configured default model was unsupported; the successful retry used a CLI-offered model via a process-local argument. See [installer evidence](CODEX_INTEGRATION_INSTALLER.md) for coverage and limitations. Captured content remains outside Git.

## Next bounded brief

Brief 09 — Production Event Bridge must authorize the helper from current project policy before any payload write, fail closed on missing/revoked consent, and supply a bounded durable handoff to the application. Its contract must be instantiated before implementation. Automatic assembly, session lifecycle/Git context and minimal local UI remain subsequent bounded units. The real installer trial does not make the current static prototype capture arguments a production consent mechanism.

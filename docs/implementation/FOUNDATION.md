# Brief 00 — Repository Foundation handoff

Created: 2026-09-13; validation handoff finalized 2026-09-14. Scope: the user's supplied Brief 00, which explicitly requests the minimal runnable scaffold. Original roadmap Brief 00 was documentation-only and placed this work in Brief 02 after G01. The explicit task supersedes that sequencing only for the foundation; G01 is not passed, Brief 01 is not started, and all product work remains gated. See the amendment in [DECISIONS.md](../product/DECISIONS.md).

## Implementation and boundaries

- pnpm workspace: `apps/desktop`, `packages/domain`, `packages/ui`; Rust workspace at the root.
- Desktop: minimal React screen, typed native adapter, Tauri configuration and single main window. Rust entry point, command registration and public build-info logic are separate modules.
- Domain: matching `AppInfo` contract only, including runtime response validation and a shared JSON serialization fixture. No session or knowledge model implementation.
- UI: one reusable semantic `AppFrame` wrapper. No native imports or product screens.
- Only IPC command: `get_app_info`, returning `{schemaVersion: 1, name: "Mochi", version: "0.1.0"}`. `AppManifest::commands` enables command ACL generation; only `allow-get-app-info` is granted to the local main window. No plugins or remote capabilities.
- Production CSP denies remote assets/connections except Tauri IPC. Development CSP adds loopback Vite/HMR and inline React refresh/style support. Production retains Tauri's automatic hash/nonce injection. This follows [Tauri capabilities](https://v2.tauri.app/security/capabilities/) and [CSP guidance](https://v2.tauri.app/security/csp/), checked 2026-09-13.
- A simple vector M icon and derived PNG/ICNS satisfy native context/bundle requirements; no design system or mascot was created.
- Strict TypeScript, package import restrictions, Vitest/React Testing Library, ESLint/Prettier, Rust tests/rustfmt/Clippy, and two-job GitHub Actions CI. Root commands are in [README.md](../../README.md).

No filesystem/network/storage capability, consent state, user data, Codex configuration access, session collection, database, provider call or future feature is implemented. Default scaffold behavior quits with its window; background/window-close behavior remains Brief 31 work.

## Versions and dependency purpose

Direct dependencies use exact versions; `pnpm-lock.yaml` and root `Cargo.lock` pin resolution. Workspace packages remain private. pnpm enforces strict peers, engine checks and a one-day minimum release age; no lifecycle-build approvals are needed by the installed scaffold dependencies.

| Tool / dependency | Version | Current purpose |
|---|---|---|
| Node / pnpm | 24.12.0 / 11.19.0 | Workspace and development commands |
| Rust | 1.98.1 | Native compiler, rustfmt and Clippy |
| Tauri Rust / build crate | 2.11.5 / 2.6.3 | macOS runtime and generated context/ACL |
| Tauri JS API / CLI | 2.11.1 / 2.11.4 | Narrow IPC and development/bundling |
| React / React DOM | 19.3.0 | Minimal frontend runtime |
| TypeScript | 5.9.3 | Strict frontend/contracts; within typescript-eslint's supported range |
| Vite / React plugin | 8.3.0 / 6.1.1 | Frontend build and development refresh |
| Vitest / jsdom | 5.0.0 / 28.1.0 | Node contract tests and DOM smoke/IPC tests; jsdom compatible with pinned Node |
| Testing Library React / DOM / jest-dom | 16.3.3 / 10.4.1 / 7.0.1 | Observable accessible UI assertions |
| ESLint / typescript-eslint | 10.10.0 / 8.70.0 | Supported TS/React linting |
| React hooks / refresh lint plugins | 7.1.1 / 0.5.6 | Hook correctness and refresh boundaries |
| Prettier | 3.9.6 | Frontend/configuration formatting |
| Serde / serde_json (test-only direct use) | 1.0.229 / 1.0.151 | Rust IPC serialization and shared contract verification |

`@eslint/js` supplies core rules and `@types/*` supplies compile-time declarations. SQLite/OpenAI/Git/secret-detection dependencies remain deferred. Vite and test-runner engine/peer requirements were verified through official npm metadata; [Vite's guide](https://vite.dev/guide/) documents its Node requirements. ESLint 9 was replaced after npm marked it unsupported. Tauri supplies transitive native dependencies; their presence is not implementation of menu bar or notification behavior.

## Validation evidence

Environment: local Apple Silicon (`arm64`), macOS 26.5.1 (25F80), Xcode 26.3 (17C529), Node 24.12.0, pnpm 11.19.0. Rust 1.98.1, rustfmt and Clippy were installed in temporary `/private/tmp/mochi-cargo` and `/private/tmp/mochi-rustup`, with no shell-profile changes. Validation shell commands set those paths explicitly; a developer's normal rustup installation uses the documented root commands unchanged.

| Check actually run | Result |
|---|---|
| `pnpm install` | Passed; official registry dependencies installed and pnpm lockfile generated |
| `pnpm install --frozen-lockfile --offline` | Passed against the final lockfile; no dependency/manifest drift |
| `cargo generate-lockfile` | Passed; root Cargo.lock generated |
| `pnpm typecheck` | Passed for domain, UI, desktop and test/build configuration |
| `pnpm test` | Passed: 11 frontend tests across 2 files and 1 Rust unit test; doc-test target passed with no tests |
| `pnpm lint` | Passed: ESLint and Clippy (`--workspace --all-targets --locked -- -D warnings`) |
| `pnpm format` | Passed: Prettier and rustfmt |
| `pnpm format:check` | Passed: Prettier and `cargo fmt --all -- --check` |
| `pnpm check:rust` | Passed: `cargo check --workspace --all-targets --locked` |
| `pnpm build` | Passed: strict typecheck and Vite production assets |
| `pnpm desktop:build` | Passed: optimized native binary and `target/release/bundle/macos/Mochi.app` |
| `pnpm dev` | Vite started on loopback port 1420, Rust compiled, and Cargo launched `target/debug/mochi-desktop`; manually stopped the development process afterward. Window rendering/live IPC are not visually verified |
| Computer Use native app inspection | Blocked: tool returned **Computer Use permissions are not granted**. No native button activation or screenshot was obtained |
| `file` and `plutil -p` on the bundle | Passed: Mach-O arm64 binary; correct name, version, identifier, icon and provisional 12.0 deployment target |
| Local documentation link/JSON scan (Node script) | Passed: 17 Markdown files, 84 relative links, 1 JSON fenced example |
| `git check-ignore` | Passed for environment variants, credentials.json, dependencies, frontend/Rust output, OS artifacts and generated command permissions |
| `git diff --check` and source/config/dependency review | Passed; no unintended future feature or speculative package found |

The initial missing-icon native compile failure was fixed by supplying the required placeholder icon, then native checks and the release build passed. Initial IPC-mock tests were fixed by explicitly setting and restoring the mocked Tauri environment flag; the production detection path was not weakened.

Frontend tests prove rendering, the checked IPC adapter, safe rejection of native failures/malformed responses, and imports from both shared packages. The Rust test serializes the actual build-info response against the same JSON fixture. These are mock/contract checks; they do **not** establish live React → WebView → Rust command dispatch.

## Brief 00 acceptance status

Scaffold implementation and automated foundation criteria pass: workspace/packages, strict TypeScript, native compilation, testing, quality tools, documented root commands, initial CI, ignore rules, aligned documentation and scope discipline. **Full acceptance is not yet confirmed**: the criteria that React runs visibly inside Tauri and that included IPC works through the live WebView remain unverified because native Computer Use permissions are unavailable.

Remaining manual check on this arm64 Mac: with normal Rust prerequisites on PATH, run `pnpm dev`, confirm **Mochi / V1 development build**, activate **Check desktop connection**, and confirm **Desktop connected · Mochi 0.1.0**. Repeat against the built `.app` to verify the production CSP and command capability in the actual WebView. Record the result before checking off those acceptance items. This is a foundation check, not permission to start Brief 01 or a substitute for its real-client gate.

## Limits and next brief

The configured deployment target is macOS 12.0 and Vite targets Safari 15. This is a provisional build setting, not a tested minimum OS. G02 clean-machine/minimum-OS verification and G04 signing/notarization remain open. No Windows/Linux desktop coverage is claimed. CI is authored locally; a hosted run requires a requested push. The local machine still needs a normal Rust installation on PATH for root commands outside the temporary validation environment; the temporary toolchain did not change user settings. Native GUI/live IPC and accessibility verification remain blocked by Computer Use permissions; do not treat automated rendering assertions as screen-reader or native accessibility coverage.

Brief 01 must prove the exact existing Codex client/version and passive transport, safely collect synthetic sanitized evidence and leave normal coding unaffected. Use the existing scaffold only if useful to that bounded spike; do not assume it proves capture feasibility. Brief 02 reviews compatibility with the proven route. There is no domain-state or persistence implementation to migrate.

# Mochi

**Use AI to build. Use Mochi to understand.**

Mochi is a local-first desktop learning companion for developers who build with AI and want to understand what they built. V1 is planned to turn approved Codex sessions and Git evidence into focused lessons, practice and later review.

**Current status: Brief 07 Codex detection and capabilities.** The repository now has a read-only native service that detects distinct Codex CLI and Desktop installations, records exact-version capability provenance, inspects bounded hook metadata, checks the Mochi helper, and derives explicit readiness. It does not install hooks or start capture. [Detection evidence](docs/implementation/CODEX_DETECTION_CAPABILITIES.md) describes the contract and limits. Brief 04's bounded session assembly remains explicit and has no startup trigger, IPC, or UI. Codex CLI 0.151.0 is the verified baseline; Codex Desktop 26.908.40834 remains partial with prompt and interrupt coverage unknown. There is still no production capture or learning feature.

## Development

Use an Apple Silicon Mac, Node **24.12.0** (`.node-version`), pnpm **11.19.0** (`packageManager`), and Rust **1.98.1** with rustfmt/Clippy (`rust-toolchain.toml`). Install pnpm using its official installation instructions and Rust using rustup; put both on PATH. Install Xcode and select its developer directory (`xcode-select -p` should point to the installed Xcode). See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The configured macOS deployment floor is 12.0; minimum-OS and clean-machine compatibility remain unverified.

From the repository root:

```bash
pnpm install --frozen-lockfile
pnpm dev
```

The development app displays **Mochi / V1 development build**. Activate **Check desktop connection** to verify the Rust IPC response. Quit the app or press Ctrl+C in the terminal to end development. Port 1420 must be available. `pnpm dev:web` starts the browser preview without native IPC; this is a development aid for the desktop frontend.

| Command                             | Purpose                                                                |
| ----------------------------------- | ---------------------------------------------------------------------- |
| `pnpm dev` / `pnpm desktop:dev`     | Start the native Tauri development app and Vite                        |
| `pnpm dev:web`                      | Start the frontend preview on loopback port 1420                       |
| `pnpm typecheck`                    | Strict TypeScript checks for all packages and build/test configuration |
| `pnpm test`                         | Frontend tests and Rust tests                                          |
| `pnpm lint`                         | ESLint and Clippy, warnings treated as errors                          |
| `pnpm format` / `pnpm format:check` | Prettier and rustfmt; master docs retain their original layout         |
| `pnpm check:rust`                   | Compile-check every native target                                      |
| `pnpm build`                        | Typecheck and build frontend assets                                    |
| `pnpm desktop:build`                | Build the native release binary and local macOS `.app`                 |

Capture-focused Rust commands:

```bash
cargo build -p mochi-hook --release --locked
cargo test -p mochi-capture --locked
target/release/mochi-hook inspect
```

Domain-focused checks:

```bash
cargo test -p mochi-domain --locked
pnpm test:frontend
```

Persistence-focused checks:

```bash
cargo test -p mochi-persistence --locked
```

Assembly-focused checks:

```bash
cargo test -p mochi-assembly --locked
```

Integration-detection checks:

```bash
cargo test -p mochi-integration --locked
```

The helper's production-default spool is `~/Library/Application Support/dev.mochi.desktop/capture/v1/spool`. The `--spool-root` override is for isolated development tests. Hook installation remains a manual development procedure documented in the capture report; do not replace an existing Codex hook file wholesale.

Focused commands: `pnpm test:frontend`, `pnpm test:rust`, `pnpm lint:frontend`, `pnpm lint:rust`. Rust checks require the native prerequisites; run `pnpm build` before direct release-mode Cargo commands so embedded frontend assets exist. Normal `desktop:build` handles this automatically.

Local output: `apps/desktop/dist/`, `target/debug/mochi-desktop`, and `target/release/bundle/macos/Mochi.app`. These are ignored. The local app bundle is not a signed/notarized distribution release. CI checks frontend tooling on Ubuntu and native compilation, tests and `.app` building on macOS; it does not prove integration or clean-machine installation.

## Repository layout and ownership

```text
apps/desktop/
  src/                    Minimal React shell; native/ holds the typed IPC adapter
  src-tauri/              Rust entry points, commands, Tauri configuration/capabilities
apps/mochi-hook/          Standalone trusted-hook bridge and developer inspector
crates/capture/           Provider adapter, normalized ingress, sanitizer, spool, Git context
crates/privacy/           Shared file and path policy
crates/domain/            Authoritative provider-independent coding-session model
crates/persistence/       SQLite migrations, ingress/project/session repositories
crates/assembly/          Deterministic persisted-evidence to CodingSession assembly
crates/integration/       Provider-independent detection contract and native Codex detector
packages/domain/          Readonly matching TS contracts, parser, helpers, shared fixtures
packages/ui/              One reusable presentation wrapper; no native access
docs/                     Authoritative product and architecture specifications
.github/workflows/ci.yml  Frontend and native foundation checks
Cargo.toml                Rust workspace; root Cargo.lock pins dependency resolution
```

Rust owns native integration, privacy, persistence and deterministic learning behavior. `crates/integration` owns the generic read-only detection contract and Codex-specific native detector. `crates/privacy` owns the shared file/path decision contract. `crates/domain` owns validated domain invariants; React consumes narrow IPC, while domain TypeScript holds checked matching contracts and pure helpers. `crates/persistence` owns bundled SQLite, migrations, policy-checked evidence import, replay tombstones, and validated session repositories. `crates/assembly` owns deterministic evidence interpretation and delegates its atomic write to persistence. Remote analysis, learning logic, automatic assembly, and product capture screens remain unimplemented.

## Product boundaries

- One desktop app contains capture, analysis, learning, knowledge, review, and settings.
- macOS Apple Silicon is the first release target.
- Codex is the only V1 coding-session source; internal contracts are provider-agnostic.
- SQLite is the local source of truth. Remote analysis is optional and requires separate consent.
- OpenAI is the first analysis adapter, using a user-supplied API key in macOS Keychain.
- No account backend, cloud sync, web learning platform, billing, or website dependency in V1.
- Passive capture currently has a conditional prototype basis: CLI is supported and Desktop is partial with prompt/interrupt capabilities marked unknown.

## Read in this order

| Document                                                          | Purpose / authority                                          |
| ----------------------------------------------------------------- | ------------------------------------------------------------ |
| [AGENTS.md](AGENTS.md)                                            | Rules for implementation tasks                               |
| [Product specification](docs/product/V1_PRODUCT_SPEC.md)          | Product promise and release requirements                     |
| [V1 scope](docs/product/V1_SCOPE.md)                              | Included, excluded, and conditional capabilities             |
| [User journey](docs/product/USER_JOURNEY.md)                      | End-to-end behavior and recovery paths                       |
| [Decisions](docs/product/DECISIONS.md)                            | Established decisions, specified defaults, feasibility gates |
| [Architecture](docs/architecture/ARCHITECTURE.md)                 | Component boundaries and runtime ownership                   |
| [Session event schema](docs/architecture/SESSION_EVENT_SCHEMA.md) | Canonical ingestion contract                                 |
| [Data model](docs/architecture/DATA_MODEL.md)                     | Persistence, transactions, provenance, deletion              |
| [Persistence report](docs/implementation/SQLITE_PERSISTENCE.md)   | Implemented SQLite boundary and verification                 |
| [Assembly report](docs/implementation/SESSION_ASSEMBLY_ENGINE.md) | Implemented evidence interpretation and traceability         |
| [Privacy and security](docs/security/PRIVACY_SECURITY.md)         | Consent and data lifecycle                                   |
| [Learning model](docs/learning/LEARNING_MODEL.md)                 | Evidence-grounded analysis and assessment                    |
| [Knowledge model](docs/learning/KNOWLEDGE_MODEL.md)               | Deterministic state and review rules                         |
| [V1 UX](docs/design/V1_UX.md)                                     | Screens, states, accessibility, native behavior              |
| [Roadmap](docs/implementation/V1_ROADMAP.md)                      | Dependency-ordered briefs 00–38                              |
| [Brief template](docs/implementation/BRIEF_TEMPLATE.md)           | Contract for individual implementation tasks                 |
| [Validation plan](docs/implementation/VALIDATION_PLAN.md)         | Release acceptance and meaningful verification               |

## Next task

Brief 07 implements read-only Codex detection and capability reporting. The next bounded task is **Brief 08 — Codex Integration Installer**; it must require user consent and preserve unrelated configuration. Each implementation request must remain bounded to its assigned brief.

## Documentation maintenance

Technical identifiers and documents use English; initial product UI is English. Localization is deferred. Each specialized spec names its authority. Keep shared terms, limits, statuses, consent, and brief dependencies aligned. Record changes in [DECISIONS.md](docs/product/DECISIONS.md), update affected documents in the same change, and run the documentation checks in [VALIDATION_PLAN.md](docs/implementation/VALIDATION_PLAN.md).

The source product discussion was **Projekt tervezése**, conversation `6aa66c66-9dbc-83eb-a890-2616cc3eb048`. Later desktop/Codex-only decisions supersede earlier website, multi-provider, and percentage-based knowledge sketches. Concrete defaults added to make this pack implementable are explicitly distinguished in the decision register.

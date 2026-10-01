# Brief 07 — Codex Detection & Capability Service

Implemented: 2026-09-24. Scope: read-only native detection of Codex CLI and Codex Desktop installations, version-aware capability evidence, bounded hook/helper inspection, and deterministic readiness. This brief adds no configuration writes, trust action, automatic capture, background watcher, persistence, IPC, or UI.

## Detection architecture

`crates/integration` owns a provider-independent `IntegrationDetectionService` and serialized operational contracts. `CodexDetector` is the sole provider implementation. CLI and Desktop remain separate `ClientSurface` reports, each containing zero or more installations, a capability matrix with provenance, summarized hook/helper state, trust state, readiness, and privacy-safe error codes.

The detector is created with explicit native inputs: PATH, Codex home, approved project roots to inspect, Desktop search roots, expected helper path/version/architecture, command timeout, and an optional legitimately known trust state. `CodexDetectorOptions::system` supplies macOS-first defaults and never scans project directories unless the caller explicitly provides approved roots. Detection is a fresh scan; no cache or watcher exists.

Absolute paths are native-only operational metadata. No Tauri command or TypeScript contract was added, so paths do not cross into UI, learning, persistence, or outbound data.

## CLI detection

Every executable named `codex` in PATH order is retained, up to a 32-installation output bound. Canonical executable identity removes duplicate symlink aliases without choosing among genuinely distinct installations. Each candidate is invoked directly with the argument array `["--version"]`; no shell or interpolated command is used, and the child environment is cleared. Standard input is closed, standard error is discarded, output is capped at 16 KiB, and the process is killed after the configured timeout (two seconds by default).

Version parsing accepts one bounded numeric dotted version token and distinguishes installed-but-unreadable from absent. Command failure, timeout, permission denial, malformed text, and excessive output have fixed error variants that contain no command output or path.

## Desktop detection

On macOS, the detector checks top-level `.app` bundles beneath the configured application roots, normally `/Applications` and `~/Applications`. It reads `Contents/Info.plist` with Apple's `/usr/bin/plutil` without launching the application. Only bundles with identifier `com.openai.codex` become Codex installations. The report retains the app location, bundle identifier, short version, build, and Mach-O architecture when readable.

Matching bundles remain in deterministic path order, up to the same 32-installation output bound. Multiple matches produce `ambiguous_installations`; they are not silently collapsed. On non-macOS targets Desktop detection reports `unsupported_platform` and does not claim coverage.

## Version and capability provenance

The local evidence registry has two exact baselines:

- Codex CLI `0.151.0`, using research evidence `H-OBS`, `H-PERM`, and `G-OBS`.
- Codex Desktop `26.908.40834`, using controlled Desktop evidence `D-OBS` and Git evidence `G-OBS`.

An exact CLI match is `verified`. A Desktop short-version or build match is `partially_verified` because direct prompt and interrupt coverage remain unknown. Any older, newer, or unreadable version is `requires_validation`. The matrix may still show the historical baseline, but its evidence level becomes `historical_baseline`; readiness never treats that as current verification.

The CLI baseline reports lifecycle, prompt, final response, tools, commands, permission, interrupt, and Git supplementation supported; direct file events remain unknown. The Desktop baseline reports lifecycle, final response, tools, commands, permission, and Git supplementation supported; prompt, direct file events, and interrupt remain unknown. Capabilities use explicit `supported/partial/unsupported/unknown` values rather than booleans.

## Hook configuration, helper, and trust

The detector reads only `hooks.json` at the configured user Codex home and explicitly approved project roots. Each file must be a regular non-symlink JSON object no larger than 256 KiB. Strict parsing is bounded to depth 16 and 256 items per container; duplicate keys and overflow fail closed. The result contains only state, counts, and whether unrelated hooks exist; it never returns hook commands or arbitrary configuration content.

Configuration states are `absent`, `present_without_mochi`, `mochi_configured`, `mochi_command_mismatch`, `malformed`, and `unreadable`. Unrelated hooks are valid and remain untouched.

The Mochi helper check distinguishes missing, present, non-executable, version mismatch, architecture mismatch, and unreadable. `mochi-hook version` now emits only its product name and package version. Architecture inspection reads eight executable-header bytes; it does not load a whole binary.

Codex trust is not inferred from private state. The system options return `unknown`. A caller may supply a known state only if a future legitimate source establishes it. This means an otherwise complete real scan remains `requires_trust` until the user verifies the command through Codex's supported trust UI.

## Readiness model

Readiness is derived in fixed precedence:

1. no installation → `not_installed`;
2. more than one installation → `ambiguous_installations`;
3. unknown or unreadable version → `requires_compatibility_validation`;
4. missing/incompatible helper → `requires_mochi_hook`;
5. absent/mismatched Mochi hook configuration → `requires_codex_configuration`;
6. malformed/unreadable configuration → `error`;
7. unknown or untrusted hook → `requires_trust`;
8. exact verified CLI with trusted configuration → `ready`;
9. exact partially verified Desktop with trusted configuration → `degraded`.

The model also reserves `detected` for later orchestration that may intentionally stop before evaluating setup. No fuzzy score or LLM participates.

## Privacy and side effects

Detection reads executable metadata, bounded version output, app bundle metadata, and bounded hook JSON. It does not inspect Codex transcripts, session files, databases, credentials, account data, or unrelated configuration files. Errors do not contain raw output, config text, or paths. Tests compare configuration bytes before and after detection. There are no writes, installers, process launches of the Desktop app, trust changes, capture starts, database changes, or background activity.

## Performance

A serial local debug fixture measured a first CLI lookup/version invocation at approximately 210 ms, Desktop plist metadata at 6.8 ms, and a warm full two-surface scan at 19.6 ms. The first subprocess measurement includes process/toolchain startup effects and varies by machine. External commands remain capped by a two-second timeout; detection is intended for explicit refresh, not continuous polling.

## Validation

Dedicated tests cover absent, CLI-only, Desktop-only, and combined installations; deterministic multiple PATH entries; exact, older, newer, malformed, failing, and timed-out CLI versions; exact historical capability matrices; Desktop prompt/interrupt uncertainty; absent, unrelated, matching, mismatched, and malformed hook configuration; missing, valid, wrong-version, non-executable, and wrong-architecture helpers; readiness precedence; unknown trust; and byte-for-byte no-side-effect behavior.

The full repository gates and native macOS build are recorded in the completion report. Tests use isolated temporary directories and synthetic fixtures only.

## Known limitations and deferred work

- Compatibility is exact-version evidence, not a semantic version range. A new Codex release requires a new validation record.
- Hook trust cannot currently be established through a supported noninteractive contract, so real system detection reports it as unknown.
- Hook discovery covers official `hooks.json` scopes supplied to the detector. It does not parse arbitrary TOML values or discover project roots without explicit approval.
- Architecture detection recognizes thin arm64/x86_64 and universal Mach-O headers; scripts and unfamiliar formats remain unknown, and universal slice membership is not enumerated in this brief.
- No Windows/Linux Desktop detector, configuration installer, self-test, persistence, IPC, onboarding, automatic capture, importer, or assembly trigger is included.
- Brief 08 owns consented installation, transactional merging, backup/rollback, trust guidance, testing, upgrade, and removal.

## 2026-10-01 restart corrections

Read-only configuration inspection now requires documented event/matcher/handler structure, complete required event coverage and literal valid helper arguments. Arbitrary descriptions and incomplete groups cannot establish readiness. Reads are capped on the open handle, duplicate keys and bounded-structure overflow are rejected. Version deadlines cover output completion as well as the direct child, with Unix process-group cleanup for inherited pipes. Installer functionality is separate and documented in [Brief 08](CODEX_INTEGRATION_INSTALLER.md); detection still never edits configuration or trust.

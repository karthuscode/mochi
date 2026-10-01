# Brief 05 — Sensitive File & Path Policy

Implemented: 2026-09-20. Scope: a shared provider-independent policy and bounded integration with the existing prototype capture and Git readers. No automatic capture, import, analysis, learning, UI, or new persistence tables were added.

## Architecture and rule precedence

`crates/privacy` owns `FilePolicy`, `Decision`, `Reason`, and version `file-path-v1`. Loading canonicalizes the approved root and compiles at most 256 local `.mochiignore` patterns. Evaluation returns only safe fixed reasons and an optional project-relative path. The ordering is built-in deny, filesystem/root deny, user deny, built-in metadata-only, size metadata-only, then allow. No user rule can re-enable a protected file. See [the security contract](../security/FILE_PATH_POLICY.md) for complete rule and size behavior.

The crate depends only on Rust filesystem APIs and `regex` for bounded ignore patterns. It has no Tauri, React, provider SDK, analysis, or learning dependency. No schema migration or policy column was needed: capture and Git apply the policy before durable evidence is written.

## Integration

`CodexSessionSource` loads the policy at construction. For non-shell file-tool completions it extracts structured paths and patch headers from the provider input. Retaining a response requires at least one identified path, all paths `Allow`, existing regular files within the 64 KiB content bound, and a successfully loaded policy. Otherwise the response is metadata-only. Tool identifiers and completion status remain available for assembly. Brief 06's shared redaction engine now protects retained text and ordinary prompt/command handling. Unsupported free-form file-tool payloads are restricted, rather than assumed safe.

`GitCliContextReader` loads the same policy for each explicit snapshot. It checks both current and rename-origin paths before file reads. On Unix, it opens with no-follow/nonblocking flags, verifies the opened inode against a path still inside the approved root, and limits the read to 64 KiB plus one byte to detect growth. The already-present `libc` version is a direct capture dependency for these open flags. Denied paths are counted and omitted entirely. Metadata-only paths retain relative change metadata with `Policy`, `Binary`, or `SizeLimit` omission; no bytes or hashes are retained. Dirty-baseline, deletion, and rename behavior remains intact.

## Verification

Unit tests cover sensitive files/directories, case variants, manifests, lockfiles, build/generated/binary files, size below/at/above bounds, absolute and relative root handling, `..` traversal, symlink escape, missing files, user ignore patterns and negation, malformed policy, denied file-tool spool output, and Git exclusions. Existing capture tests, including dirty-baseline behavior, remain green.

On macOS Apple Silicon, `cargo test --workspace --locked` passed all 71 Rust tests (including 6 privacy and 22 capture tests). `pnpm typecheck`, `pnpm lint:frontend`, `pnpm test:frontend` (18 tests), `pnpm build`, `pnpm test`, `pnpm lint`, `pnpm format:check`, `pnpm check:rust`, and `pnpm desktop:build` all passed. The app bundle was produced at `target/release/bundle/macos/Mochi.app`. These are automated/synthetic checks; they are not a clean-machine production capture gate.

On the local debug test run with a two-rule `.mochiignore`, evaluating 1, 100, and 1,000 synthetic paths took approximately 0.24 ms, 0.46 ms, and 4.1 ms respectively. This measures cached policy evaluation with filesystem checks on missing paths; it is not a production end-to-end capture benchmark.

## Limitations and subsequent redaction

The current provider adapter exposes only a bounded set of structured file-tool paths. Arbitrary shell commands and command output can refer to or print protected files without a reliable structured path; Brief 05 deliberately does not parse shell semantics or claim to stop those outputs. Future file readers must invoke this policy before access, recheck after obtaining size, perform bounded reads, and handle filesystem races. Future analysis bundles must reuse this policy and separately enforce consent and redaction. The policy does not provide forensic path erasure from preexisting external files.

Brief 06 implemented the shared secret-redaction boundary for supported capture, structured output, Git, spool, and SQLite ingress, with synthetic canary verification. See [SECRET_REDACTION_ENGINE.md](SECRET_REDACTION_ENGINE.md). Future outbound analysis must use that engine again after consent checks; no outbound analysis exists yet.

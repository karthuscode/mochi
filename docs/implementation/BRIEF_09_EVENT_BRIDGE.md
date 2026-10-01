# Brief 09 — Authorized event bridge

## Objective and authority

The independently invoked helper may write sanitized evidence only while current project and global tracking consent permit it, including while the desktop core is down. This implements P02/P04/P05/P14 under the privacy, architecture and event contracts. The user authorized work through the internal CLI learning MVP on 2026-10-01; this is its next bounded unit.

## Current state and dependencies

Brief 08 native installer and real CLI 0.151.0 install/trust/task/disconnect gate passed. Briefs 03/05/06 provide transactional import and shared privacy. Prototype `capture` trusts static developer arguments and remains an explicit development mode; it must not be used by product installation. Desktop owns SQLite but imports nothing automatically.

## Scope

Add a private versioned project capture policy, bounded cross-process leases, a silent `capture-authorized` helper mode and an installer entry point for its exact command. Add coordinated project-policy transitions and a bounded import operation. Missing, malformed, disabled, stale, foreign-owner or symlink policy fails closed before input normalization or spool creation. Keep prototype developer interfaces compatible.

No session assembly/lifecycle, Git snapshots, network analysis, learning, automatic configuration mutation or frontend in this unit. Those follow separately.

## Native contract and data impact

`CaptureAuthorization` contains schema 1, opaque project UUID, approved canonical root, private spool root, positive monotonically increasing policy revision, global/project tracking flags. Local operational paths never become learning content. Store one bounded JSON file per project (`0600`) under an app-owned `0700` directory, with a per-project lock file. The helper holds a shared lease from policy read through spool write; policy transitions require the exclusive lease. Acquisition is bounded at 500ms and drops capture on contention.

The desktop transition first acquires the exclusive lease and writes a disabled policy, then changes SQLite consent/revision, then publishes the matching policy. A failure leaves collection disabled; a crash between operations cannot grant capture. Startup reconciles only already approved DB projects, never grants new consent. Revocation waits for any already admitted bounded write; after it returns no old lease can write. Import independently checks the DB revision. Same-user malware remains outside the security guarantee.

A native bridge coordinates these boundaries; the helper does not open SQLite or use network/UI. Imports process at most 100 candidates per operation. Spool limits remain 100MiB/seven days and commit-before-ack remains unchanged. No new SQL schema required.

## Validation and acceptance

Isolated fixtures must prove enabled capture, missing/corrupt/disabled/global-off rejection without spool files, exact project/revision matching, owner permissions, symlink refusal, bounded lock contention and revocation serialization. A real helper process must remain silent and exit zero on capture failure. Existing installer ownership tests must still pass for the new command. Preserve sanitized spool while core is absent and durable replay when importing.

Run relevant Rust tests and the repository gates at the next complete native/UI checkpoint. Record actual results and unresolved limitations in the implementation report; no Desktop completeness, clean-machine or full MVP claim.

## Files and handoff

`crates/capture` owns the minimal authorization lease; a dedicated `crates/bridge` coordinates SQLite policy and bounded import; `apps/mochi-hook` adds the authorized protocol; `crates/integration` previews/installs it. Update architecture/privacy/event contract, README and execution record after acceptance. No raw payload/configuration logs, guessed provider events, arbitrary paths in acknowledgement or trust bypass.

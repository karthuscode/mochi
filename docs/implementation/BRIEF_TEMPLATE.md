# Mochi implementation brief template

Baseline: 1.0. Authority: individual task handoff format. This template is not authorization to implement. Roadmap entries summarize work packages; before executing each, fill this template with the real repository state, actual commands, concrete files, and acceptance evidence. Do not falsely describe all 39 summaries as fully instantiated standalone briefs.

## Required preflight

Read root `AGENTS.md`, [product specification](../product/V1_PRODUCT_SPEC.md), [architecture](../architecture/ARCHITECTURE.md), [privacy](../security/PRIVACY_SECURITY.md), current dependencies and feature specs. Verify prerequisites from [V1_ROADMAP.md](V1_ROADMAP.md). Separate an established decision from an unresolved gate. Never use a failed spike as permission to create an IDE or raw transcript archive.

## Brief NN — TITLE

### Objective

One observable outcome. Explain the user/system behavior that changes.

### Why this exists

Which product requirement or risk this resolves; reference Pxx and the controlling spec.

### Product context

Relevant V1 behavior, platform/provider boundaries, and user journey stage.

### Current project state

What exists now, relevant contracts/migrations, actual known limitations. No assumptions that future work is already done.

### Dependencies

Brief IDs, required artifacts and pass evidence. Include feasibility/privacy gates and order constraints.

### In scope / out of scope

Bound the work. List adjacent/future-provider features explicitly excluded from this task.

### Functional requirements

Numbered behaviors covering normal path and user controls.

### Technical requirements / architecture constraints

Ownership, ports, schemas, IPC, versions, resource bounds, deterministic logic and permitted dependencies.

### Data model impact

Tables, fields, migrations, provenance, uniqueness, transactional effects, retention/deletion/replay consequences. State “none” with reason when applicable.

### Privacy requirements

What can be read, persisted, exported or sent; consent checkpoints; sanitizer/error handling; Keychain/config controls. Never leave this implicit.

### UX requirements

Screens, visible statuses, actions, accessibility and recovery copy. For a headless spike, specify safe report/log output instead.

### Error / edge cases

Missing permissions/source fields, malformed input, duplicate/reordered events, offline/crash/cancellation, dirty/overlapping Git, oversized inputs, revoked consent, deletion races, uncertain grades as applicable.

### Test requirements

Meaningful observable behavior, synthetic fixtures, real-client/manual evidence where essential, and actual commands from the current repo. Avoid tests that only restate implementation.

### Acceptance criteria

Concrete pass/fail checks a reviewer can perform; no “works correctly” placeholder. Include failure behavior and privacy verification.

### Expected file changes

Actual repository paths and purpose, narrowed to this brief. The supplied Brief 00 — Repository Foundation explicitly includes the minimal scaffold; later briefs must not repeat or expand it without their own authorization.

### Documentation updates

Owning specs, decision register, README commands/support matrix and handoff report.

### Do not

Forbidden shortcuts relevant to this task: raw logs, invented hooks, frontend native access, global consent, fake mastery, executing user snippets, unnecessary scope expansion.

### Definition of done / handoff

Acceptance passed; relevant checks passed or explicitly blocked; docs consistent; report includes changed behavior, tests/evidence, remaining constraints, next eligible brief. Do not mark a gate passed using mocks.

## Brief 01 acceptance checklist to instantiate

- Name the exact intended existing Codex client and version; verify current official integration docs and trust/config semantics.
- Obtain an explicit temporary integration approval after showing the edit plan; use a disposable/safe test repository and synthetic credentials.
- Observe real session identity, approved working directory association, prompt **or reliable activity**, available tool events, turn boundary, and defensible end/finalization semantics without manual transcript export.
- Save a **sanitized** local normalized debug event log, baseline/final Git evidence and coverage/support matrix. The debug log is not an exception to privacy.
- Demonstrate duplicate/restart behavior, one missing optional event type, helper/core unavailable behavior, and cleanup of only owned configuration.
- Demonstrate source code/event evidence sufficient to reconstruct a real change and teach at least one concept. Git-only unexplained changes do not prove the Codex learning promise.
- Record pass/fail, transport choice, event mapping, limitations and changes to this pack. If failure, propose a concrete compatible revision and stop dependent implementation.

A successful test for a different Codex client must be described as that narrower support; do not advertise desktop-wide coverage. An App Server-controlled agent session, screenshots, parser of undocumented private transcripts, or manual import does not satisfy the current passive-companion requirement.

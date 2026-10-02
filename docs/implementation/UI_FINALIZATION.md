# Personal CLI MVP — interface finalization

Checkpoint: 2026-10-02. Scope: [bounded UI brief](BRIEF_MVP_UI_FINALIZATION.md). This polishes the implemented Briefs 09–21 personal macOS CLI build; it does not complete V1 learning, release or live-model acceptance.

## Implemented interface

Selected local Signal face mark, static orange/graphite character and macOS icon now form one restrained glassmorphism interface. Application-owned text/window titles and bundle naming use lowercase `mochi`; earlier capitalized identity exports are preserved, and technical build-info DTOs remain compatible. No dependency, migration, remote asset or native permission was added by this polish. The existing companion trial's transparent-window permission remains separate.

Sessions owns project approval, exact connection review, tracking, bounded evidence/code context, exact analysis preview and explanation/self-check. Settings owns Keychain, run-local remote permission, static companion show/hide and window-local System/Light/Dark appearance. Appearance resets with the window; it is not a persisted native preference. Sessions remains mounted during Settings navigation, preserving unsaved answers and previews. Native expiry/policy validation still rejects stale send grants.

The interface handles loading, first-run, empty sessions, unavailable storage, partial/interrupted evidence and deletion. Switching projects immediately clears the previous list and connection consent/preview. Finalized sessions have a disabled Finish control; the UI explains that Finish ends mochi's episode rather than stopping Codex. Deletion confirmation receives keyboard focus. Readable light/dark colors, opaque reduced-transparency fallback, reduced-motion handling, forced-color fallback, native inputs and visible keyboard focus are implemented. Browser rendering is a labelled appearance preview: native controls are disabled, no IPC polling runs, and no synthetic sessions masquerade as captured work.

## Verification evidence

- Frontend checks cover the current shell/appearance, browser isolation, default-off permissions, project draft preservation, independent exact connection consent, project-switch clearing, deletion confirmation, safe storage failures, and preservation of an actual self-check draft plus exact send preview through Settings navigation. The full frontend suite passes 43 tests.
- The complete Rust suite passes 144 tests; one disposable native Keychain test remains intentionally ignored in the routine suite. Its separate prior native evidence is in INTERNAL_CLI_MVP.md; this UI task did not read/store a live key.
- All nine repository gates are recorded after the final pass below. Earlier concurrent/whole-suite runs intermittently returned `HelperUnavailable` in installer fixtures. Temporary test-only diagnostics were removed; four isolated integration-suite runs passed without changing implementation, test deadlines or coverage. This documents timing sensitivity rather than claiming a newly fixed installer bug.
- Built the arm64 `target/release/bundle/macos/mochi.app` with its executable bundled helper. Native QA used a temporary bundle copy with a distinct process identity to avoid disturbing the existing companion preview. Normal startup rendered the real local interface; no project was approved, hook installed, key stored, remote permission enabled or analysis sent.
- Native light/dark rendering, Sessions/Settings navigation, desktop connection IPC, write-only key field, disabled remove-key control, unchecked remote permission and unchecked root approval were inspected. Show/Hide returned visible/hidden companion status. Keyboard Tab moved from alias to folder with a visible focus ring. Browser appearance rendering loaded both local images without console errors and had equal 900-pixel viewport/content widths at a 900×640 CSS viewport. Browser sizing is not a native window-resize test.
- The screenshots below contain only the empty native interface and placeholders, no private project or captured context. A native inspection call took unexpectedly long; later screenshot/navigation calls completed normally. The temporary QA app was quit after inspection.

![Empty native light interface](assets/ui-finalization-light.png)

![Empty native dark interface](assets/ui-finalization-dark.png)

## Remaining acceptance gates

The complete new live authorized CLI → BYOK OpenAI → explanation/answer → restart trial and human semantic-quality/cost review remain pending. Mocked provider responses cannot establish them. Follow [the personal trial](INTERNAL_CLI_MVP.md) with a disposable project and separately approved exact send.

The static companion is not yet fully accepted: physical dragging, alpha compositing over other apps, Spaces/fullscreen, background focus and multiple displays retain the gates in COMPANION_STATIC_TRIAL.md. This polish does not add animation, saved placement, background/menu-bar lifecycle, complete ready lessons, challenges, knowledge/review, automatic expiry or distribution. Full screen-reader/contrast, populated long-session UI, sleep/wake and clean-machine acceptance still need broader validation. Local storage is permission-protected, not application-encrypted; deletion cannot recall provider submissions or claim forensic erasure.

## Final repository gates

All nine required gates passed on the final sources: `pnpm typecheck`, `pnpm lint:frontend`, `pnpm test:frontend`, `pnpm build`, `pnpm test`, `pnpm lint`, `pnpm format:check`, `pnpm check:rust`, `pnpm desktop:build`. Final whole-suite count: 43 frontend and 144 Rust tests passed, with one intentional Keychain ignore. Final asset-manifest byte counts and SHA-256 checksums match; the packaged helper reports `mochi-hook 0.1.0`. No test timeout, production deadline or global test configuration was changed to obtain this result.

The final build removes temporary diagnostic instrumentation and adds explicit decorative-image hiding for screen readers. Native screenshots precede that accessibility-only attribute change; visible artwork/layout are unchanged. The temporary QA copy was removed only after its process was confirmed stopped. No signing, notarization, push, publication or external account modification was performed.

# Companion 01 — static floating-window trial

Assigned: 2026-10-01. Status: static prototype delivered; native acceptance **partial**, dependent animation brief remains gated. Separate bounded brief after [Brand 01](MOCHI_BRAND_IDENTITY.md), explicitly requested by the user to see the selected Signal character on the desktop.

## Objective and dependencies

Show the selected static Signal character in a transparent local Tauri window above ordinary macOS windows; drag it and click to open the existing main window. Read repository/product/scope/architecture/privacy/UX instructions and inspect the shared working tree before edits. Preserve parallel capture/analysis work. Brand assets are selected, rendered and packaged; this trial is independent of capture/learning acceptance.

## Scope and implementation

- Fixed 160×160-point `companion` window; local bundled PNG, no border, native shadow, resize controls or unsolicited focus. Initially hidden in normal mode.
- Show/hide controls in the existing main UI, a distinct companion capability, fixed-label Rust commands and native drag permission only. No access to sessions, keys, integration, filesystem or network from the companion.
- Main-window monitor work area supplies first placement, 24 points from its lower-right usable edge. Subsequent shows preserve placement during this run. Persistent preferences/position, display-removal recovery and animations are the next brief.
- macOS transparent WebView uses user-approved `macos-private-api`. Public AppKit collection behavior excludes fullscreen spaces and joins ordinary Spaces; prove actual behavior instead of inferring it from flags.
- A developer-only `MOCHI_COMPANION_PREVIEW=1` mode selects a dedicated local main page before any WebView loads, skips LocalCapture/Analysis startup entirely and immediately shows the character. Preview main-window close hides that window so click-open can be tested; Dock reopen restores it as a fallback. Normal close behavior remains quit; destroy the extra window when the normal main closes.
- New versioned trial DTO `{schemaVersion: 1, visible: boolean}`. No domain/SQLite migration or durable preference write.

## Failure/privacy boundaries

Wrong caller labels and missing windows reject with fixed safe errors. Broken image displays an accessible error without a blank invisible click surface. Failed drag does not open the main window. Only a deliberate click/keyboard activation opens and focuses main; pointer hover does not. No screen/other-app inspection, keys, transcripts, remote assets, capture enablement, installed hook mutation or API call is introduced. Native trial mode does not open the product database or acquire the capture profile lock.

## Acceptance and tests

Test drag/click separation, keyboard activation and safe IPC failures; validate native monitor placement including negative origins and small displays. Check actual alpha/Retina display, background focus retention, native dragging, click-open after hiding main, Spaces and fullscreen behavior on this arm64 Mac. Multi-monitor proof requires an available second display; label unavailable scenarios honestly. Run relevant repository gates and native build. Record screenshots only of the synthetic preview, with no user coding context. Stop at this trial handoff; no hover/idle animation or production background lifecycle in this brief.

## Change areas and handoff

Desktop Rust companion module/commands/ACL/config; separate frontend companion and synthetic-preview entries; a small main-window show/hide control; frontend/native tests; UX/README/decision documentation. Target-only objc2 bindings expose the public macOS fullscreen collection policy and are already present transitively at pinned versions; no new service or provider dependency.

## Executed validation — 2026-10-01

- `pnpm typecheck`, `pnpm lint:frontend`, `pnpm build`, `pnpm lint`, `pnpm check:rust`, `pnpm desktop:build`, and the final `pnpm format:check` passed. The local bundle executable is arm64. No signing, notarization, installation or publication occurred.
- All 36 frontend tests passed with `pnpm exec vitest run --maxWorkers=1` after compilation finished; this includes nine new companion interaction/IPC tests. `cargo test --workspace --locked` passed, including the two new physical-placement tests. Native Keychain roundtrip remains an explicitly ignored unrelated test.
- Earlier parallel test runs timed out, including worker-start failures; `pnpm test` did not reach its Rust stage in that run. Frontend/Rust suites were subsequently verified independently. An earlier format check found two concurrently changing unrelated files; the final whole-repository format check passed. No test timeout or global test configuration was changed to mask failures.
- Vite emits SVG/PNG as local assets (`assetsInlineLimit: 0`); the logo appeared through the existing self-only image CSP. There is no new data-URL/network permission.
- Launched the bundled app with the isolated preview environment. Native inspection confirmed `tauri://localhost/companion-preview.html`, the selected logo and static character, and a 2000×1440 screenshot of the 1000×720-point main window. The 160-point companion produced a 320×320 Retina screenshot with no title bar or native border.
- Repeated native main-close → companion click → same main-window reopen passed. Show/hide returned visible/hidden status; main content kept its accessibility focus when showing the companion. Command-Q terminated the preview process successfully; relaunched it for the user to inspect.
- Native UI automation initially failed to start its pipe, then recovered. Its drag call on the nonfocusable companion returned `noWindowsAvailable`; it did **not** prove a physical move. Frontend tests prove threshold/click suppression/error behavior, but do not substitute for a real drag.

## Open native gate and next handoff

The preview is usable for visual review. Do not call Companion 01 fully accepted or start frame animation integration yet. Still prove physical dragging and retained placement, alpha compositing over light/dark ordinary windows, background-app keyboard focus/hover, Spaces transitions, fullscreen exclusion and multiple monitors on this Mac. Isolated screenshots establish rendering, not alpha composition over another app or Spaces visibility. No unsupported/native scenario is marked passed. Normal capture-mode startup/close was deliberately not launched for this visual task.

After these native scenarios pass, assign the separate animation/lifecycle brief: local frames, float/blink/three reactions, reduced motion/sleep suspension, durable atomic preferences, display-removal recovery, and menu-bar/background lifecycle. Website, release packaging and updates remain later tasks.

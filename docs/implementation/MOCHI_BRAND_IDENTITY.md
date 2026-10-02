# Brand 01 — Mochi identity

Assigned: 2026-10-01. Status: **selected static identity delivered and app icon bundled; unrelated shared-tree lint failures recorded below**. This is a separate bounded visual brief, not capture roadmap Brief 01 or production bridge Brief 09. Companion runtime remains unimplemented by this brief.

## Objective and current state

Create one coherent Mochi logo, macOS app icon and character identity. The initially selected clay cream/mint robot direction was rejected after the user saw the three concepts; the revised **Signal** orange/graphite robot was selected from the second round. The repository started this brief with the foundation M icon and minimal main window. The source inventory and clean Git working tree were reviewed before this brief. Parallel capture work subsequently changed the shared tree; those runtime changes are not part of this brand brief.

The user approved three separate work packages: identity; static floating-window feasibility; animated desktop companion. Identity choice precedes final assets, and real native feasibility precedes animation integration. Stop at each bounded handoff. This checkpoint supplies the three requested choices rather than selecting a brand on the user's behalf.

## Inputs and decisions

Required reading: repository instructions, README, product/scope, architecture, privacy, UX and brief template. The selected Signal character/palette, logo relationship, reactions, fullscreen behavior and future download distribution are recorded in [Mochi identity](../design/MOCHI_IDENTITY.md).

Use the built-in image-generation tool for the clay character. Final vector marks/lettering should be authored as vectors, not raster placeholders. No new runtime dependency, provider transfer of repository text, or application API is needed for this visual brief.

## Deliverables and acceptance

1. Three saved concept sheets, each showing a distinct robot silhouette and matching icon study, plus exact prompts. **Delivered.**
2. Revised visual direction, new concept choices, human selection and a fixed character reference. **Delivered: Round 02 / A Signal selected by the user.**
3. Production transparent character, SVG logos/variants, PNG/ICNS app icon and monochrome menu-bar icon. **Delivered:** 28 source/export/proof assets plus versioned manifest and character prompt under `docs/design/assets/mochi/final/`.
4. Inspect edges against light/dark backgrounds, logo readability, icon recognition at 16–32 pixels and identity consistency. **Passed static visual review:** [visual proof](../design/assets/mochi/final/visual-proof.png). Human approval establishes the concept; generated pose and vector simplification are routine production derivatives, not a separate brand selection.
5. Integrate the selected final app icon, build the macOS bundle and inspect its icon. **Passed:** existing app SVG/PNG/ICNS replaced; release `.app` built, arm64 executable verified, bundled ICNS bytes match the selected source.

No floating window, background lifecycle, menu-bar runtime, automatic tracking, analysis, learning, website, external publication or signing is implemented in this brief. No persisted product-data or domain-contract change. Parallel capture changes were preserved; do not attribute their behavior or tests to brand implementation.

## Evidence at this checkpoint

- Round 02: Signal (orange/graphite geometric robot), Mallow (periwinkle/peach organic creature) and Loop (cobalt/orange abstract mascot) saved with exact prompts under `docs/design/assets/mochi/concepts/round-02/`. Each is 1774×887 RGBA with fully transparent pixels. Signal has an icon-edge halo and is rendered with more depth than its flat-illustration prompt; these are concept limitations to resolve after selection. Loop's C-like silhouette also needs human review for brand fit.

- Built-in `image_gen`, three independent prompts, transparency enabled. Repository copies and prompts are under `docs/design/assets/mochi/concepts/`.
- A/Pebble and B/Capsule: 1536×1024 RGBA; C/Dumpling: 1774×887 RGBA. All contain fully transparent pixels and partially transparent edges.
- Visual inspection: three distinct silhouettes with matching icon studies. A/B decorative glow and C edge artifacts make these previews unsuitable for direct production integration.
- The first/second-round sheets remain concept history. Only selected Signal derivatives under `final/` are final static identity assets. No animation is claimed complete.
- The idle character is 1254×1254 RGBA with genuine alpha and no attached glow/shadow. The hand-authored flat logo has custom vector lettering and no embedded raster, font, script or external reference. Menu-bar bitmap contains black ink plus alpha, including transparent eye holes.
- SVG sources and PNG decoding/alpha verified; manifest dimensions/checksums recorded; integrated PNG/ICNS hashes match the final exports. Relative documentation links and brand JSON validate; `git diff --check` passes.

## Repository and native verification

Performed on 2026-10-01. Checks reflect the shared tree at their execution time; parallel capture edits continued during this brief. No unrelated runtime files were edited to force gates green.

| Check | Result |
| --- | --- |
| `pnpm typecheck` | Passed |
| `pnpm lint:frontend` | Passed before concurrent LocalCapture UI arrived |
| `pnpm test:frontend` | Passed: 18 tests at the first check |
| `pnpm build` | Passed |
| `pnpm test` | Passed later shared-tree snapshot: 19 frontend + 123 Rust tests |
| `pnpm check:rust` | Passed |
| `pnpm format:check` | Passed at its execution snapshot |
| Brand generator ESLint and changed-file Prettier | Passed after final assets |
| `pnpm lint` | Failed in concurrently added `apps/desktop/src/LocalCapture.tsx`: synchronous state update in effect; the file subsequently changed outside this brief |
| `pnpm lint:rust` | Failed in concurrent `crates/bridge/src/git_context.rs:55` and `crates/bridge/src/lib.rs:99`: Clippy `cmp_owned` |
| `pnpm desktop:build` | Passed, local unsigned arm64 `.app` created |
| Bundle resource inspection | Passed: `Contents/Resources/icon.icns` matches the final source, SHA-256 `9b9cb5626c9af6289e7cd88b3599a736461688919d2e1bd3cdc132ecd59f491a` |

Pinned pnpm initially could not verify its package-manager registry signature inside the network-restricted sandbox. The same verification succeeded with approved execution; no identity/security check was bypassed. A local HTML proof was saved, but browser policy disallowed opening `file:` URLs. Static local rendering/viewing supplied visual evidence instead; no browser or native GUI interaction/accessibility coverage is claimed. Native bundle inspection is not clean-machine, signing/notarization, minimum-OS or companion runtime acceptance.

## Next bounded work packages

**Brand 01 handoff:** selected Signal static assets are delivered and bundled. Shared-tree lint failures above belong to the parallel capture work. Any later visual revision should edit the versioned vector source or reference-preserving character prompt, update exports/manifest and rerun applicable checks.

**Companion 01 — static feasibility:** instantiate its own brief. Use a separate local transparent `companion` Tauri window, enable `macos-private-api` only as required on macOS, preserve the `main` window, and grant minimal window-specific capabilities. On a real arm64 Mac prove transparency, no unsolicited focus, click-open, drag/click separation, Spaces/fullscreen policy, Retina and multiple displays. Failure stops dependent animation work.

**Companion 02 — animation and lifecycle:** only after static feasibility passes, integrate local PNG animation frames, CSS idle float, the three one-shot hover reactions, opt-in preferences and persisted position. Rust owns native lifecycle and atomic versioned preference writes. Add minimal app/menu-bar controls, hide-on-close/reopen/full-quit behavior, reduced motion and animation suspension. Validate actual native behavior and resource use. Do not infer capture or learning readiness from companion behavior.

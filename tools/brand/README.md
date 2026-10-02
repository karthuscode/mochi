# Mochi brand assets

The selected Signal identity is specified in [Mochi identity](../../docs/design/MOCHI_IDENTITY.md). `build-assets.mjs` contains the editable helmet geometry and original Mochi glyphs; it needs only Node to rebuild SVG sources and the local HTML proof:

```bash
node tools/brand/build-assets.mjs
```

To reproduce PNG exports, provide an existing installed `sharp` module path. Sharp is an optional developer rasterizer, not an application dependency; no package install is needed by the app. The initial export used the bundled workspace runtime's Sharp.

```bash
node tools/brand/build-assets.mjs --sharp-module /absolute/path/to/installed/sharp
```

The script writes only `docs/design/assets/mochi/final/`. It does not generate or alter the AI character cutout, ICNS, application configuration, or project data. Keep the character edit prompt with its source reference; regeneration uses the built-in image-generation tool with transparency enabled.

Generate native ICNS with the repository's pinned Tauri CLI, using a disposable output directory because that command also emits non-macOS files:

```bash
pnpm --filter @mochi/desktop tauri icon ../../docs/design/assets/mochi/final/app-icon.svg --output /private/tmp/mochi-signal-iconset
```

Copy only the resulting `icon.icns` into the final asset folder as `app-icon.icns` and into `apps/desktop/src-tauri/icons/icon.icns`. Copy the final SVG master and 1024-pixel PNG into the app's existing `icon.svg` and `icon.png`. Do not add unused platform outputs to the repository. Inspect light/dark proof, icon at 16/32 pixels and the native bundle after changes.

The menu-bar PNGs contain only black ink and alpha, with transparent eye holes. Set macOS template rendering when they are eventually integrated. Animation and desktop-window behavior are outside this tool.

# Project notes

freecode reads this file before every change. Add anything it should always remember about this project.

## What this is

Taste Curator: a non-designer marks up references and exports a machine-actionable PDF for a
zero-context agent. One workspace, three parts.

- `native/` — **the app**. Rust + Rune, the native desktop curator. Package
  `taste-curator-native`, binary `TasteCurator.exe` in a release. Ships to Windows through
  `.github/workflows/windows.yml` (zip with the exe + `seed/`).
- `src/` — the engine the app calls. Telemetry, artifact import, plate layout, PDF export, gates.
  Binaries: `engine` (`manifest` / `export`) and `demo` (regenerates sample images, PDF, plates).
- `app/` — the first web prototype (React 19, Vite+, Tailwind 4, shadcn/ui). Superseded by
  `native/`; kept for reference and still builds. Sample data lives in `app/public/seed/`.

The native app is the deliverable. Rune cannot render in the preview panel, so it is verified by
`--snap`, `--tree` and `--script`, never by the panel.

## Build and test

```sh
source ~/.cargo/env && export CARGO_TARGET_DIR=/tmp/tc-target && cargo test --lib
cd app && pnpm run build          # tsc -b + vp build
cd app && pnpm run dev -- --port $PORT --host 0.0.0.0
```

- **Always** set `CARGO_TARGET_DIR=/tmp/tc-target`; the disk is small (16 GB) and `target/` in the
  workspace fills it. `cargo` is not on PATH until `~/.cargo/env` is sourced.
- A full debug build of Rune + the app needs ~5 GB of target dir. If a link fails with
  `ld terminated with signal 7 [Bus error]` or `No space left on device`, delete
  `$CARGO_TARGET_DIR/debug/build/*/out` and retry; that is a full disk, not a code error.
  `rm -rf /tmp/tc-target` frees it all (rebuild takes ~15 min).
- Native verification:
  `$CARGO_TARGET_DIR/debug/taste-curator-native --snap out.png --scene project`,
  `--tree out.json`, `--script native/scripts/draw.json --at mark:end`. Scenes: `empty`,
  `project`, `marking`, `drawn`.
- Native tests: `cargo test -p taste-curator-native` (8 tests: draft commit, tiny-click
  rejection, freehand points, undo, metadata, pixel-aspect geometry, real PDF export + reparse,
  camera round-trip).
- The shadcn CLI only works through `pnpm dlx shadcn@latest` (npm refuses the `devEngines` block).

## Decisions worth remembering

- `rune` is a git dependency pinned to rev `631e8a67cb3c40225efa3f6c4f57b80e7b18f190`.
  `parley_engine` is vendored under `vendor/parley_engine` via `[patch.crates-io]`; plain
  crates.io Rune fails to build with the `line-dictionaries` feature. Do not re-add
  `tiny-skia-path`.
- No direct `taffy` dependency (feature conflict with `rune-taffy`); parley handles PDF text flow.
- TypeScript 7 removed `baseUrl` — use `paths` only. `src/lib/utils.ts` re-exports `cn` from the
  `cn` package (not clsx + tailwind-merge).
- `krilla` page coordinates are top-left origin: never flip y with `PAGE_H - y`.
- Rune layout gotchas hit while building `native/`: `el()` is a **row**, so any vertical stack
  must be `col()`. There is no `El::row()` / `.p()` builder method — use `row()` and `.padding()`.
  Only width-constrained text truncates, so a label in a row needs `.max_w()` or `.w_full()`.
- The native app persists to `AppDirs::new("dev.tastecurator.app").data()/project.json` through
  `native/src/store.rs`, writing only when the JSON changed (`last_saved`).
- `native/scripts/*.json` are Rune script fixtures: `draw.json` (three markings), `note.json`
  (draw + type a note + move a dial), `export.json` (draw twice, then export through the scripted
  file dialog to `/tmp/from-app.pdf`).
- PDF page numbering: synthesis is page 1, then overviews, then details. `render_pdf_page` takes
  1-based numbers.
- Fonts are registered as `Taste Inter` / `Taste Mono` with `FontInfoOverride { family_name }`, so
  only the bundled Inter + IBM Plex Mono get embedded. Both are SIL OFL; licences sit beside them.
- `app/vite.config.ts` uses `base: "./"` and `server.allowedHosts` from
  `FREECODE_PREVIEW_HOST` for static publishing.

## Data contract

`app/src/curator/model.ts` mirrors `src/model.rs` field for field; a project JSON round-trips
between them. `app/src/curator/export.ts` rebuilds the manifest and the conflict rule in
TypeScript for instant feedback — if you change the conflict rule, change both.

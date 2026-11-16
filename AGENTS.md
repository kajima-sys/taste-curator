# Project notes

freecode reads this file before every change. Add anything it should always remember about this project.

## What this is

Taste Curator: a non-designer marks up references and exports a machine-actionable PDF for a
zero-context agent. One workspace, two Rust crates.

- `native/` — **the app**. Rust + Rune, the native desktop curator. Package
  `taste-curator-native`, binary `TasteCurator.exe` in a release. Ships to Windows through
  `.github/workflows/windows.yml` (zip with the exe + `seed/`).
- `src/` — the engine the app calls. Telemetry, artifact import, plate layout, PDF export, gates.
  Binaries: `engine` (`manifest` / `export`) and `demo` (regenerates sample images, PDF, plates).
- `seed/` — the sample project shipped inside the Windows zip, and what the app loads when there is
  no saved work. Paths inside it are relative to its own root (`seed/artifacts/...`).

There is **no JavaScript or TypeScript in this repo**, on purpose: the deliverable is a native
Rust + Rune app. A superseded React prototype (`app/`) and its static plate preview (`web/`) were
deleted; `cargo run --bin demo` regenerates the sample under `seed/`. Do not reintroduce a web UI
or a JS toolchain.

The native app is the deliverable. Rune cannot render in the preview panel, so it is verified by
`--snap`, `--tree` and `--script`, never by the panel.

## Build and test

```sh
source ~/.cargo/env && export CARGO_TARGET_DIR=/tmp/tc-target && cargo test --lib
cargo test -p taste-curator-native
cargo run --bin demo              # regenerate seed/
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

## Decisions worth remembering

- `rune` is a git dependency pinned to rev `631e8a67cb3c40225efa3f6c4f57b80e7b18f190`.
  `parley_engine` is vendored under `vendor/parley_engine` via `[patch.crates-io]`; plain
  crates.io Rune fails to build with the `line-dictionaries` feature. Do not re-add
  `tiny-skia-path`.
- Rune patches `winit` and `wgpu-hal` in its own repo, and a dependency's `[patch]` does not reach
  a workspace that depends on it, so apps must vendor them too: `vendor/winit` (Windows taskbar and
  first-frame placement, `warm_keyboard_layout`) and `vendor/wgpu-hal` (`set_present_rects`, DX12
  adapter pick). Both are patched in the root `[patch.crates-io]`.
- The whole wgpu family must stay on **30.0.1** (`wgpu`, `wgpu-core`, `wgpu-hal`, `wgpu-types`,
  `wgpu-naga-bridge`, `wgpu-core-deps-*`). A `cargo update` that bumps `wgpu-core` to 30.0.2 pulls
  `wgpu-hal` 30.0.2 from crates.io, silently bypasses the vendored patch, and the Windows build
  fails with `E0599: no method named set_present_rects`. Check `Cargo.lock` has no `source`/
  `checksum` on `wgpu-hal` after any dependency change.
- The app binary sets `windows_subsystem = "windows"` (no console window). Anything scripted on
  Windows must use `Start-Process -Wait` to wait for it.
- No direct `taffy` dependency (feature conflict with `rune-taffy`); parley handles PDF text flow.
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
## Data contract

`src/model.rs` is the only definition of a project. It is serialised as-is to `project.json` (in
`seed/`, and in the app data dir between launches), and the same structs build the manifest and
the PDF. There is no second implementation to keep in sync — change the model and every caller
follows.

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
  no saved work. Paths inside it are relative to its own root (`seed/artifacts/...`). It includes
  `landing.html` + `landing.css`, an HTML artifact, so the page feature is visible on first launch.

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
- HTML is rendered to pixels by `src/html.rs` (Blitz + Servo's style engine + `vello_cpu`), entirely
  offscreen: no `blitz-shell`, no `blitz-net` (that pulls OpenSSL — do not add it). Pages fetch only
  `file:` URLs through a small `NetProvider`, so linked CSS/images beside the page resolve and
  nothing touches the network. JavaScript never runs, by design — say so rather than "fixing" it.
- An HTML artifact is an ordinary bitmap page (`ArtifactSource::Html { path, width, height }`), which
  is why the canvas, telemetry and PDF export needed no HTML-specific code. Do not special-case it.
- `taffy`'s `flexbox_balance` feature (which Blitz needs) adds `FlexWrap` variants that
  `rune-taffy` matches exhaustively, so `vendor/rune-taffy` is patched via
  `[patch."https://github.com/anomalyco/rune"]` to fold `Balance`/`BalanceReverse` into plain wraps.
  The packed wrap field is 2 bits; do not try to give them new codes.
- The dev profile drops most debug info (`debug = "line-tables-only"`, `debug = false` for deps):
  with a CSS engine in the build, a full debug build no longer fits this VM's disk.
- No direct `taffy` dependency of our own; parley handles PDF text flow. Blitz brings taffy in
  transitively (see the `rune-taffy` patch above).
- `krilla` page coordinates are top-left origin: never flip y with `PAGE_H - y`.
- Rune layout gotchas hit while building `native/`: `el()` is a **row**, so any vertical stack
  must be `col()`. There is no `El::row()` / `.p()` builder method — use `row()` and `.padding()`.
  Only width-constrained text truncates, so a label in a row needs `.max_w()` or `.w_full()`.
- The native app persists to `AppDirs::new("dev.tastecurator.app").data()/project.json` through
  `native/src/store.rs`, writing only when the JSON changed (`last_saved`).
- `native/scripts/*.json` are Rune script fixtures: `draw.json` (three markings), `note.json`
  (draw + type a note + add a label), `export.json` (draw twice, then export through the scripted
  file dialog to `/tmp/from-app.pdf`).
- PDF page numbering: synthesis is page 1, then overviews, then details. `render_pdf_page` takes
  1-based numbers.
- Fonts are registered as `Taste Inter` / `Taste Mono` with `FontInfoOverride { family_name }`, so
  only the bundled Inter + IBM Plex Mono get embedded. Both are SIL OFL; licences sit beside them.
## The label model (no taxonomies)

The curator's words are the only structure. Do not reintroduce `Scope`, `Medium`, adjective
`Spectrum` dials, or any fixed chip list: the user explicitly rejected all four as forced,
inconsistent vocabulary that only worked for web UI.

- `Annotation` has `labels: Vec<Label>` (a `Label` is just `String`) and the free-form `Note`.
  Labels are the curator's own short words, deduped case-insensitively; `add_label` /
  `remove_label` / `toggle_label` / `has_label` live on `Annotation`.
- `Annotation::vocabulary(&[Artifact])` is the **only** source of suggested words in the
  inspector: labels actually typed somewhere in the project, most-used first. Never ship a
  built-in word list.
- A `Conflict` is a shared label on a `Like` and a `Dislike` (`manifest::find_conflicts`).
  Different labels never conflict; nothing is inferred.
- Measurements are facts, not grades: the inspector shows plain words ("Size: about 20% of the
  frame") and keeps the numbers behind the `exact values` toggle (`App::exact`, `show_values`).
- The model must stay medium-agnostic (design, digital art, photography, print, code).

## Active Counterfactual Calibration & UI design system

- `src/calibrate.rs` implements six pure-Rust single-axis perturbations (`Warmth`, `Contrast`,
  `Chroma`, `Lightness`, `Crispness`, `Breathing`), adaptive probing (`DontCare` skips the opposite
  direction), and `build_envelope`, which compiles `hard_invariants` (targeted anchor telemetry
  only on flinched axes), `verified_tolerances`, `boundary_counterexamples`, and
  `contrastive_pairs` into `Manifest` v2 (`src/manifest.rs`) and page 1 of the PDF (`src/pdf/mod.rs`).
- `native/src/theme.rs` defines the Linear/Raycast-inspired 4-tier Obsidian/Alabaster Rune token
  system (`bg_canvas`, `bg_rail`, `bg_surface`, `bg_elevated`, 1px `border_hair`, Linear indigo
  `#5E6AD2` primary CTA, semantic `keep`/`flinch`/`contrast`, and `kbd` keycap badges) used across
  `main.rs`, `shelf.rs`, `calibrate_view.rs`, `inspector.rs`, and `files.rs`.

## Data contract

`src/model.rs` is the only definition of a project. It is serialised as-is to `project.json` (in
`seed/`, and in the app data dir between launches), and the same structs build the manifest and
the PDF. There is no second implementation to keep in sync — change the model and every caller
follows.

# Project notes

freecode reads this file before every change. Add anything it should always remember about this project.

## What this is

Taste Curator: a non-designer marks up references and exports a machine-actionable PDF for a
zero-context agent. Two halves, one repo.

- `app/` — the curator. React 19, Vite+ (`vp`), Tailwind 4, shadcn/ui (radix, `vega` preset),
  `@/` alias, icons from `lucide-react`. State is `localStorage` under
  `taste-curator/project/v1`; the sample is `app/public/seed/`.
- `src/` — the engine. Telemetry, artifact import, plate layout, PDF export, gates. Also two
  binaries: `engine` (`manifest` / `export`) and `demo` (regenerates sample images, PDF, plates).

There is **no native window**. Rune cannot render into the preview panel, so the UI moved to the
web app and the Rust crate stayed as the export engine. Keep it that way unless asked.

## Build and test

```sh
source ~/.cargo/env && export CARGO_TARGET_DIR=/tmp/tc-target && cargo test --lib
cd app && pnpm run build          # tsc -b + vp build
cd app && pnpm run dev -- --port $PORT --host 0.0.0.0
```

- **Always** set `CARGO_TARGET_DIR=/tmp/tc-target`; the disk is small (16 GB) and `target/` in the
  workspace fills it. `cargo` is not on PATH until `~/.cargo/env` is sourced.
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

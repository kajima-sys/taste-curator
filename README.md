# Taste Curator

A local-first tool for someone who is not a designer. You collect images, code and documents,
draw a box around the details that catch your eye, and say what each one *is* — colour, spacing,
type, shape, surface, feel — with three levels of "keep it", "never do that", "interesting
contrast" and four perceptual dials.

The output is one PDF written for a machine, not a person: a synthesis, a vector overview plate
per source, one detail plate per marking with an unoccluded full-resolution crop beside its exact
telemetry, and the whole thing embedded as JSON. A zero-context agent reads it and distils a
design system that holds across UI, web, video and illustration.

Everything stays on your machine. No accounts, no uploads, no inference.

## Two halves, one project

| | What it is | Where |
|---|---|---|
| **Curator** | React 19 + Vite+ + shadcn/ui. Shelf, canvas, inspector, export sheet. Persists to `localStorage`. | `app/` |
| **Engine** | Rust. Telemetry, artifact import, plate layout, PDF writing, round-trip checks. | `src/` |

The browser builds the manifest in TypeScript (`app/src/curator/export.ts`) so the export sheet
answers instantly, using the same shapes as the Rust model. The PDF itself is produced by the
Rust engine: the page never rasterises your annotations, and the crops it places are the original
pixels.

## Running the curator

```sh
cd app
pnpm install
pnpm run dev
```

Then open the printed URL. The app opens on the sample project (three generated sources, eight
markings) so there is something to look at before you import anything.

- `R` box, `O` oval, `F` freehand, `V` select, `H` pan, hold space for a temporary pan
- scroll to pan, ctrl/⌘ + scroll to zoom, `⌘Z` / `⇧⌘Z` to undo and redo
- `⌘E` opens the export sheet; `Delete` removes the selected marking
- **Add** in the shelf imports a PNG, JPEG or WebP from disk

Your edits are saved to `localStorage` as you go. Nothing leaves the browser.

## Running the engine

```sh
# Regenerate the sample: source images, the PDF, and every plate as PNG for the web viewer.
cargo run --bin demo

# Print the manifest the PDF will carry, for a project JSON.
cargo run --bin engine -- manifest app/public/seed/project.json

# Write the PDF.
cargo run --bin engine -- export app/public/seed/project.json out.pdf
```

`demo` writes into `web/` and `app/public/seed/`, so re-running it refreshes the sample that both
the static viewer and the web app show.

```sh
cargo test --lib
```

39 tests cover the model, telemetry (OKLCH clustering, contrast, geometry), the camera, the tool
state machine, the manifest and its conflict detection, and the PDF gates: every callout is
re-parsed with `hayro` and must match its source region at IoU ≥ 0.99, and every detail crop is
re-rendered and compared pixel for pixel against the source.

## How a project is stored

`project.json` is the whole project. Sources keep their original path and pixel size — the
artifact is never modified or re-encoded. A marking is a normalised rectangle
(`x0, y0, x1, y1` in `0..1`) plus, for freehand, its polyline; drawing is an overlay, never a
rasterised copy. Text and PDF selections keep their verbatim string.

```jsonc
{
  "name": "Studio handoff — spring",
  "artifacts": [
    {
      "id": 1,
      "name": "dashboard.png",
      "source": { "kind": "image", "path": "seed/artifacts/dashboard.png", "width": 1200, "height": 900 },
      "annotations": [
        {
          "id": 2,
          "polarity": "like",
          "scope": "spacing_density",
          "medium": "ui",
          "shape": "rect",
          "region": { "x0": 0.06, "y0": 0.2, "x1": 0.42, "y1": 0.55 },
          "chips": [["tight_airy", 0.86]],
          "notes": [{ "text": "Room between the cards and around their content." }]
        }
      ]
    }
  ],
  "next_artifact": 4,
  "next_annotation": 9
}
```

## What the PDF contains

1. **Synthesis** — counts by polarity, the scopes they touch, unresolved conflicts stated as
   instructions ("keep the airy side, but avoid the tight side"), and the brief.
2. **Overview plates** — the whole source, with numbered vector badges and callouts placed
   outside the art they point at, so nothing is occluded.
3. **Detail plates** — one per marking: an untouched full-resolution crop, the numbers measured
   from the unannotated pixels (dominant OKLCH/OKLab clusters, contrast ratios, aspect), and the
   curator's own words.
4. **Embedded manifest** — `taste-manifest.json` inside the PDF, so an agent can read the exact
   data instead of guessing from ink.

## Layout

```
src/
  model.rs       artifacts, annotations, polarities, scopes, spectra
  telemetry.rs   OKLab/OKLCH, WCAG contrast, deterministic k-means, geometry
  artifact.rs    image/PDF import via hayro
  camera.rs      pan and zoom in artifact space
  tools.rs       the tool state machine, and what a drag means
  manifest.rs    the readings and conflicts the PDF and the agent both read
  pdf/           plate layout and parley→krilla text
  bin/demo.rs    generates the sample and renders every plate
  bin/engine.rs  manifest / export for the web app
app/
  src/curator/   model, colour, canvas, shelf, inspector, export, persistence
```

## Notes

- `rune` is pinned to a revision and `parley_engine` is vendored under `vendor/` (see
  `Cargo.toml`). The plain crates.io `rune` fails to build with the `line-dictionaries` feature.
- Fonts embedded in the PDF: Inter and IBM Plex Mono, both SIL OFL, under `assets/fonts/`.
- The web app is the curator; there is no native window. The Rust side is the engine.
- `web/` is the first static preview of the exported plates, kept as an archive.

MIT.

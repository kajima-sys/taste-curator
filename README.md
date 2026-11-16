# Taste Curator

A local-first tool for someone who is not a designer. You collect images, web pages, code and
documents, draw a box around the details that catch your eye, and say what each one *is* — colour, spacing,
type, shape, surface, feel — with three levels of "keep it", "never do that", "interesting
contrast" and four perceptual dials.

The output is one PDF written for a machine, not a person: a synthesis, a vector overview plate
per source, one detail plate per marking with an unoccluded full-resolution crop beside its exact
telemetry, and the whole thing embedded as JSON. A zero-context agent reads it and distils a
design system that holds across UI, web, video and illustration.

Everything stays on your machine. No accounts, no uploads, no inference.

## The app, on Windows

The curator is a native desktop app: Rust + [Rune](https://github.com/anomalyco/rune), no browser,
no server. It lives in `native/`.

**[Download the Windows build →](https://github.com/kajima-sys/taste-curator/releases/latest)**

Unzip it anywhere and double-click `TasteCurator.exe`. The zip carries a sample project in
`seed/`, so the first launch has three images, a web page and eight markings to look at.

- `V` select, `H` pan, `R` box, `O` oval, `F` freehand
- scroll to pan, Ctrl + scroll to zoom, `Ctrl+0` to fit
- `Ctrl+Z` undo, `Ctrl+O` import, `Ctrl+E` export the PDF, `Delete` removes a marking
- drag on a marking with Select to bring it up in the inspector

Your work is saved as you go, in
`%APPDATA%\dev.tastecurator.app\project.json`, and reopened next launch. References are read from
where they are on your disk; nothing is copied or uploaded.

### Web pages

An `.html` file imports as a page, not as text: the app renders it — parsing the markup, applying
its stylesheets, laying it out and painting it — and shows the result on the canvas, where it can
be marked up like a screenshot. The stylesheets and images next to the page are read from your disk;
nothing is fetched from the internet, and scripts do not run, so a page that builds itself with
JavaScript shows only its static content. Rendering happens in pure Rust (Blitz, with Servo's style
engine), with the same Inter font the PDFs use, so a page looks the same on every machine.

### Building it yourself

```sh
cargo test -p taste-curator-native          # state machine, telemetry, export
cargo run --release -p taste-curator-native
```

`.github/workflows/windows.yml` builds the exe on `windows-latest`, runs the tests, renders a
smoke-test frame, zips the exe with the sample project, uploads it as an artifact, and attaches
the zip to the release when you push a `v*` tag.

## The app and its engine

Two Rust crates, no other language in the build:

| | What it is | Where |
|---|---|---|
| **App** | Rust + Rune. The curator you run. | `native/` |
| **Engine** | Rust. Telemetry, HTML rendering, artifact import, plate layout, PDF writing, gates. | `src/` |

The app calls the engine's own model and exporter, so a marking drawn in the app is the same
struct the PDF plates and the embedded manifest are built from. Nothing ever rasterises your
annotations: geometry is a separate layer at normalized `[0..1]` coordinates, and the crops the
exporter places are the original pixels.

## Running the engine

```sh
# Regenerate the sample images, project and PDF in seed/.
cargo run --bin demo

# Print the manifest the PDF will carry, for a project JSON.
cargo run --bin engine -- manifest seed/project.json

# Write the PDF.
cargo run --bin engine -- export seed/project.json out.pdf
```

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
  html.rs        HTML + CSS to pixels in pure Rust, local files only, no JavaScript
  artifact.rs    image/PDF import via hayro
  camera.rs      pan and zoom in artifact space
  tools.rs       the tool state machine, and what a drag means
  manifest.rs    the readings and conflicts the PDF and the agent both read
  pdf/           plate layout and parley→krilla text
  bin/demo.rs    generates the sample and renders every plate
  bin/engine.rs  manifest / export from the command line
native/
  src/           the Rune window: shelf, canvas, inspector, files, saved work
  scripts/       scripted input used to drive the app headlessly
seed/            the sample project, shipped inside the Windows zip
```

## Notes

- `rune` is pinned to a revision and `parley_engine` is vendored under `vendor/` (see
  `Cargo.toml`). The plain crates.io `rune` fails to build with the `line-dictionaries` feature.
- Fonts embedded in the PDF: Inter and IBM Plex Mono, both SIL OFL, under `assets/fonts/`.
- The app is `native/`; the engine in `src/` is what it calls. There is no JavaScript,
  TypeScript or browser code anywhere in the build.

MIT.

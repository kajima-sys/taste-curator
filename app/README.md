# Taste Curator — the curator

The web app. React 19, Vite+ (`vp`), Tailwind 4, shadcn/ui (radix, `vega` preset).

```sh
pnpm install
pnpm run dev        # development, hot reload
pnpm run build      # typecheck + production build into dist/
pnpm run lint
```

Opens on the sample project from `public/seed/`. Sources, markings and notes are saved to
`localStorage` under `taste-curator/project/v1`; nothing is uploaded.

## Where things live

- `src/curator/model.ts` — the data model. Mirrors `../src/model.rs` field for field, so a project
  JSON round-trips between the browser and the Rust exporter.
- `src/curator/color.ts` — OKLab/OKLCH, WCAG contrast, deterministic k-means. Mirrors
  `../src/telemetry.rs` so the inspector can show numbers the instant a box is drawn.
- `src/curator/Canvas.tsx` — camera, the five tools, hit testing, live drafts. Draws the artifact
  and its markings on two separate canvases so pan and zoom never rebuild the DOM.
- `src/curator/Shelf.tsx` — sources and markings, with a polarity filter.
- `src/curator/Inspector.tsx` — polarity, scope, medium, the four dials, notes, live telemetry.
- `src/curator/export.ts` — builds the manifest and the conflict rule in TypeScript for instant
  feedback. The actual PDF comes from the Rust engine.
- `src/curator/ExportDialog.tsx` — the read, the plates, the embedded JSON.
- `src/curator/Workspace.tsx` — shell, history, shortcuts, file import.
- `src/components/ui/` — shadcn/ui. Regenerate with `pnpm dlx shadcn@latest add <name>`.

## Exporting a real PDF

The page cannot write PDFs. Build the engine in the repository root and run:

```sh
cargo run --bin engine -- export public/seed/project.json out.pdf
```

The samples in `public/seed/plates/` and `public/seed/taste-curator.pdf` are the sample project's;
`cargo run --bin demo` regenerates them.

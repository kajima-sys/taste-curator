/** The curated project. Shapes and names match the Rust model one-to-one, so a project JSON
 * round-trips between the browser and the native exporter without translation. */

export type Polarity = "like" | "dislike" | "contrast";
export type Scope =
  | "color"
  | "spacing_density"
  | "typography"
  | "shape_edge"
  | "surface_texture"
  | "motion_mood"
  | "other";
export type Medium = "ui" | "web" | "video" | "illustration";
export type Spectrum = "tight_airy" | "crisp_soft" | "flat_layered" | "warm_clinical";
export type Shape = "rect" | "ellipse" | "freehand" | "text_span" | "pdf_object";

export interface NormRect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export interface Span {
  text: string;
  start: number;
  end: number;
  font: string;
}

export interface Annotation {
  id: number;
  polarity: Polarity;
  scope: Scope;
  medium: Medium;
  shape: Shape;
  region: NormRect;
  points: [number, number][];
  chips: [Spectrum, number][];
  notes: { text: string }[];
  partner: number | null;
  spans: Span[];
  page: number | null;
}

export type ArtifactSource =
  | { kind: "image"; path: string; width: number; height: number }
  | { kind: "text"; path: string; language: string }
  | { kind: "pdf"; path: string; pages: number };

export interface Artifact {
  id: number;
  name: string;
  source: ArtifactSource;
  annotations: Annotation[];
}

export interface Project {
  name: string;
  artifacts: Artifact[];
  next_artifact: number;
  next_annotation: number;
}

const clamp01 = (v: number) => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);

export function normRect(x0: number, y0: number, x1: number, y1: number): NormRect {
  return {
    x0: clamp01(Math.min(x0, x1)),
    y0: clamp01(Math.min(y0, y1)),
    x1: clamp01(Math.max(x0, x1)),
    y1: clamp01(Math.max(y0, y1)),
  };
}

export const rectWidth = (r: NormRect) => Math.max(0, r.x1 - r.x0);
export const rectHeight = (r: NormRect) => Math.max(0, r.y1 - r.y0);
export const rectCenter = (r: NormRect): [number, number] => [(r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2];

export function rectIoU(a: NormRect, b: NormRect): number {
  const ix = Math.max(0, Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0));
  const iy = Math.max(0, Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0));
  const inter = ix * iy;
  const union = rectWidth(a) * rectHeight(a) + rectWidth(b) * rectHeight(b) - inter;
  return union <= 0 ? 1 : inter / union;
}

export function rectFromPoints(points: [number, number][]): NormRect {
  let x0 = 1;
  let y0 = 1;
  let x1 = 0;
  let y1 = 0;
  for (const [x, y] of points) {
    x0 = Math.min(x0, clamp01(x));
    y0 = Math.min(y0, clamp01(y));
    x1 = Math.max(x1, clamp01(x));
    y1 = Math.max(y1, clamp01(y));
  }
  return { x0, y0, x1, y1 };
}

/* ---------------------------------------------------------------- vocabulary ---- */

export const POLARITIES: Polarity[] = ["like", "dislike", "contrast"];

export const POLARITY_LABEL: Record<Polarity, string> = {
  like: "Like",
  dislike: "Must avoid",
  contrast: "Contrast",
};

/** The colour each polarity wears, in the app and in the plates. */
export const POLARITY_COLOR: Record<Polarity, string> = {
  like: "#248a5c",
  dislike: "#c03a3a",
  contrast: "#4860c8",
};

export const SCOPES: Scope[] = [
  "color",
  "spacing_density",
  "typography",
  "shape_edge",
  "surface_texture",
  "motion_mood",
  "other",
];

export const SCOPE_LABEL: Record<Scope, string> = {
  color: "Color",
  spacing_density: "Space",
  typography: "Type",
  shape_edge: "Shape",
  surface_texture: "Surface",
  motion_mood: "Feel",
  other: "Other",
};

export const SCOPES_HINT: Record<Scope, string> = {
  color: "Hue, saturation, how a colour sits next to another",
  spacing_density: "Room between things: margins, padding, rhythm",
  typography: "Weight, tracking, size, the voice of the letters",
  shape_edge: "Corners, radii, line weight, silhouette",
  surface_texture: "Flatness, grain, gloss, shadow, depth",
  motion_mood: "Tempo and feeling implied by the frame",
  other: "Something the six above do not name",
};

export const MEDIA: Medium[] = ["ui", "web", "video", "illustration"];

export const MEDIUM_LABEL: Record<Medium, string> = {
  ui: "UI",
  web: "Web",
  video: "Video",
  illustration: "Image",
};

export const SPECTRA: Spectrum[] = ["tight_airy", "crisp_soft", "flat_layered", "warm_clinical"];

export const SPECTRUM_ENDS: Record<Spectrum, [string, string]> = {
  tight_airy: ["tight", "airy"],
  crisp_soft: ["crisp", "soft"],
  flat_layered: ["flat", "layered"],
  warm_clinical: ["warm", "clinical"],
};

export const SPECTRUM_LABEL: Record<Spectrum, string> = {
  tight_airy: "Density",
  crisp_soft: "Edge",
  flat_layered: "Depth",
  warm_clinical: "Temperature",
};

/* ------------------------------------------------------------------ project ---- */

export function emptyProject(): Project {
  return { name: "Untitled taste", artifacts: [], next_artifact: 1, next_annotation: 1 };
}

export function newAnnotation(
  id: number,
  polarity: Polarity,
  shape: Shape,
  region: NormRect,
): Annotation {
  return {
    id,
    polarity,
    scope: "other",
    medium: "ui",
    shape,
    region,
    points: [],
    chips: [],
    notes: [],
    partner: null,
    spans: [],
    page: null,
  };
}

export function chipValue(a: Annotation, s: Spectrum): number | undefined {
  return a.chips.find(([which]) => which === s)?.[1];
}

export function setChip(a: Annotation, s: Spectrum, value: number | undefined): Annotation {
  const chips = a.chips.filter(([which]) => which !== s);
  if (value !== undefined) chips.push([s, Math.min(1, Math.max(0, value))]);
  return { ...a, chips };
}

export function noteText(a: Annotation): string {
  return a.notes
    .map((n) => n.text.trim())
    .filter(Boolean)
    .join(" · ");
}

/** Like first, then contrast, then avoid; within a group, reading order. Matches the exporter. */
const RANK: Record<Polarity, number> = { like: 0, contrast: 1, dislike: 2 };

export function orderedAnnotations(project: Project): { artifact: number; annotation: Annotation }[] {
  return project.artifacts
    .flatMap((a) => a.annotations.map((annotation) => ({ artifact: a.id, annotation })))
    .sort((l, r) => {
      const byPolarity = RANK[l.annotation.polarity] - RANK[r.annotation.polarity];
      if (byPolarity) return byPolarity;
      const byY = l.annotation.region.y0 - r.annotation.region.y0;
      if (Math.abs(byY) > 1e-9) return byY;
      const byX = l.annotation.region.x0 - r.annotation.region.x0;
      if (Math.abs(byX) > 1e-9) return byX;
      return l.annotation.id - r.annotation.id;
    });
}

export function counts(project: Project): { like: number; avoid: number; contrast: number } {
  const out = { like: 0, avoid: 0, contrast: 0 };
  for (const a of project.artifacts) {
    for (const n of a.annotations) {
      if (n.polarity === "like") out.like += 1;
      else if (n.polarity === "dislike") out.avoid += 1;
      else out.contrast += 1;
    }
  }
  return out;
}

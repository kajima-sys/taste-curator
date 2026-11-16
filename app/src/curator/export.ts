/** The exporter. In the browser there is no Rust process, so the page builds the same manifest
 *  the PDF carries, shows it, and links the pre-rendered plates from the sample. On a machine
 *  with the engine built, `exportNative` writes the real PDF to disk. */

import type { Project } from "./model";
import { MEDIUM_LABEL, orderedAnnotations, SCOPE_LABEL, SPECTRUM_ENDS } from "./model";

export interface Reading {
  annotation: number;
  artifact: number;
  artifact_name: string;
  polarity: string;
  scope: string;
  medium: string;
  region: [number, number, number, number];
  notes: string[];
  chips: { dial: string; value: number; left: string; right: string }[];
}

export interface ManifestPreview {
  format: string;
  project: string;
  readings: Reading[];
  summary: string[];
  conflicts: string[];
  /** The one-line brief the cold agent gets, so the user can read what will be asked of it. */
  brief: string;
}

export function buildManifest(project: Project): ManifestPreview {
  const readings: Reading[] = orderedAnnotations(project).map(({ artifact, annotation }) => ({
    annotation: annotation.id,
    artifact,
    artifact_name: project.artifacts.find((a) => a.id === artifact)?.name ?? "",
    polarity: annotation.polarity,
    scope: annotation.scope,
    medium: annotation.medium,
    region: [annotation.region.x0, annotation.region.y0, annotation.region.x1, annotation.region.y1],
    notes: annotation.notes.map((n) => n.text).filter((t) => t.trim()),
    chips: annotation.chips.map(([dial, value]) => ({
      dial,
      value,
      left: SPECTRUM_ENDS[dial][0],
      right: SPECTRUM_ENDS[dial][1],
    })),
  }));

  const summary: string[] = [];
  for (const polarity of ["like", "dislike", "contrast"] as const) {
    const group = readings.filter((r) => r.polarity === polarity);
    if (!group.length) continue;
    const scopes = [...new Set(group.map((r) => SCOPE_LABEL[r.scope as keyof typeof SCOPE_LABEL]))];
    const label = polarity === "like" ? "Like" : polarity === "dislike" ? "Must avoid" : "Contrast";
    summary.push(`${label}: ${group.length} marking${group.length === 1 ? "" : "s"} about ${scopes.join(", ")}.`);
  }

  // Same rule as the exporter: same scope and medium, opposite polarity, one dial in opposite
  // halves. Reported, never silently resolved.
  const conflicts: string[] = [];
  for (const like of readings.filter((r) => r.polarity === "like")) {
    for (const avoid of readings.filter((r) => r.polarity === "dislike")) {
      if (like.scope !== avoid.scope || like.medium !== avoid.medium) continue;
      const opposite = like.chips.find((c) =>
        avoid.chips.some((o) => o.dial === c.dial && Math.abs(o.value - 0.5) > 0.15 && Math.abs(c.value - 0.5) > 0.15 && c.value < 0.5 !== o.value < 0.5),
      );
      if (!opposite) continue;
      const other = avoid.chips.find((o) => o.dial === opposite.dial)!;
      const word = (c: { value: number; left: string; right: string }) => (c.value < 0.5 ? c.left : c.right);
      conflicts.push(
        `${SCOPE_LABEL[like.scope as keyof typeof SCOPE_LABEL]} in ${MEDIUM_LABEL[like.medium as keyof typeof MEDIUM_LABEL]}: keep the ${word(opposite)} side (marking ${like.annotation}), but avoid the ${word(other)} side (marking ${avoid.annotation}).`,
      );
    }
  }
  if (!summary.length) summary.push("No markings yet.");

  return {
    format: "taste-curator/manifest",
    project: project.name,
    readings,
    summary,
    conflicts,
    brief:
      "A cold agent gets the PDF and nothing else. It reads the summary, treats the numbers as the truth rather than the crop, and returns a named OKLCH token set plus a specimen sheet — following every observation and avoiding every must-avoid.",
  };
}

/** The exact number of plates the exporter writes for this project. */
export function plateCount(project: Project): { overlays: number; details: number; total: number } {
  const used = [...new Set(project.artifacts.filter((a) => a.annotations.length).map((a) => a.id))];
  const details = project.artifacts.reduce((n, a) => n + a.annotations.length, 0);
  return { overlays: used.length, details, total: 1 + used.length + details };
}

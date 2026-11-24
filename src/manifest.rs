//! Every number the exporter prints, in one serializable manifest. The PDF embeds this file, so
//! a downstream agent can read exact values instead of estimating them from pixels.

use serde::{Deserialize, Serialize};

use crate::calibrate::{self, Envelope};
use crate::model::{Polarity, Project, Shape};
use crate::telemetry::{self, Bitmap, ColorCluster, ContrastReading, GeometryReading, SurroundReading};

/// The measurements taken for one annotation, from the artifact's untouched pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub annotation: u32,
    pub artifact: u32,
    pub artifact_name: String,
    pub polarity: Polarity,
    /// Short labels the curator typed for this marking, verbatim, in the order they added them.
    #[serde(default)]
    pub labels: Vec<String>,
    pub shape: Shape,
    /// Normalized artifact-space region, exactly as stored.
    pub region: [f32; 4],
    /// Normalized freehand polyline, when the marking had one.
    pub points: Vec<[f32; 2]>,
    pub page: Option<u32>,
    pub notes: Vec<String>,
    pub verbatim: Option<String>,
    pub clusters: Vec<ColorCluster>,
    pub contrast: Option<ContrastReading>,
    pub geometry: GeometryReading,
    pub surround: Option<SurroundReading>,
}

/// A contradiction the curator left in place: the same label on two markings of opposite
/// polarity. Only labels they typed can conflict, so the app never invents the argument.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    /// The shared label, exactly as typed.
    pub label: String,
    pub like: u32,
    pub avoid: u32,
    /// Plain-language description, for the human reading the PDF and the agent reading the JSON.
    pub summary: String,
}

/// The whole exported packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub project: String,
    pub artifacts: Vec<ManifestArtifact>,
    /// The Empirical Acceptance Envelope derived from counterfactual calibration and contrastive pairs.
    #[serde(default)]
    pub envelope: Envelope,
    pub readings: Vec<Reading>,
    pub conflicts: Vec<Conflict>,
    /// One line per polarity and label group the agent should act on, written plainly.
    pub summary: Vec<String>,
    /// A literal prompt a cold agent can be handed with this PDF.
    pub cold_agent_prompt: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManifestArtifact {
    pub id: u32,
    pub name: String,
    pub kind: String,
    /// Pixels for an image or a PDF page, or the character count for text.
    pub size: String,
    pub pages: u32,
}

pub const COLD_AGENT_PROMPT: &str = "\
You are given one PDF and its embedded taste-manifest.json. Together they describe a person's \
taste through an Empirical Acceptance Envelope (axes tested by perturbing their references until \
they flinched), contrastive pairs ('this, not that'), and optional marked details.

Read it in this order:
1. Target context & Hard Invariants (envelope.hard_invariants): these are the axes where \
   perturbing a reference caused the curator to flinch ('Lost it'). Preserve the anchor's \
   measured state on these axes and stay well inside the named flinch boundaries.
2. Verified Tolerances / Free Variables (envelope.verified_tolerances): these are axes the \
   curator explicitly tested and did NOT flinch on ('Still has it' or 'Don't care'). You are \
   free to adapt these dimensions to the target's content rather than cloning the reference.
3. Contrastive Pairs (envelope.contrastive_pairs) & Boundary Counterexamples: optimize away \
   from the foils and rejected states toward the anchors.
4. Detail plates & notes: read the curator's words and labels. Use exact OKLCH / contrast \
   measurements ONLY on dimensions that triggered a flinch or are explicitly named in a note; \
   do NOT overfit to drag-box aspect ratios or incidental pixel clusters.

Before generating your solution:
- List the Hard Invariants you are locking, the Verified Tolerances you are adapting freely, \
  and any remaining visual decision you still have to assume.
- Produce a semantic OKLCH token set and a specimen tailored to envelope.target, with one \
  sentence per token citing the invariant, contrastive pair, or observation it came from.

Where two notes conflict, follow the one whose note matches the target more closely.";

/// Build the manifest and every reading for a project.
pub fn build(project: &Project, bitmaps: &dyn Fn(u32, Option<u32>) -> Option<Bitmap>) -> Manifest {
    let mut readings = Vec::new();
    for (artifact_id, annotation) in project.ordered() {
        let Some(artifact) = project.artifact(artifact_id) else { continue };
        let Some(bitmap) = bitmaps(artifact_id, annotation.page) else { continue };
        let clusters = telemetry::dominant_clusters(&bitmap, &annotation.region, 4);
        let contrast = telemetry::contrast_reading(&clusters);
        let geometry = telemetry::geometry_reading(&bitmap, &annotation.region);
        let surround = telemetry::surround_reading(&bitmap, &annotation.region, &clusters);
        let verbatim = if annotation.spans.is_empty() { None } else { Some(annotation.verbatim()) };
        readings.push(Reading {
            annotation: annotation.id,
            artifact: artifact_id,
            artifact_name: artifact.name.clone(),
            polarity: annotation.polarity,
            labels: annotation.labels.clone(),
            shape: annotation.shape,
            region: [annotation.region.x0, annotation.region.y0, annotation.region.x1, annotation.region.y1],
            points: annotation.points.clone(),
            page: annotation.page,
            notes: annotation.notes.iter().map(|n| n.text.clone()).filter(|t| !t.trim().is_empty()).collect(),
            verbatim,
            clusters,
            contrast,
            geometry,
            surround,
        });
    }
    let envelope = calibrate::build_envelope(project, bitmaps);
    let conflicts = find_conflicts(&readings);
    let mut summary = Vec::new();
    if !envelope.target.trim().is_empty() {
        summary.push(format!("Target: {}", envelope.target.trim()));
    }
    if !envelope.hard_invariants.is_empty() {
        let names: Vec<String> = envelope
            .hard_invariants
            .iter()
            .map(|h| format!("{} ({})", h.axis.label(), h.flinch_boundaries.join(", ")))
            .collect();
        summary.push(format!("Hard invariants (flinched when moved): {}.", names.join("; ")));
    }
    if !envelope.verified_tolerances.is_empty() {
        let names: Vec<&str> = envelope
            .verified_tolerances
            .iter()
            .map(|v| v.axis.label())
            .collect();
        summary.push(format!("Verified free to adapt: {}.", names.join(", ")));
    }
    for pair in &envelope.contrastive_pairs {
        let tail = if pair.note.trim().is_empty() {
            pair.measured_delta.clone()
        } else {
            pair.note.trim().to_string()
        };
        summary.push(format!("This, not that: {} over {} — {tail}.", pair.anchor_name, pair.foil_name));
    }
    summary.extend(summarize(&readings, &conflicts));
    let artifacts = project.artifacts.iter().map(|a| {
        let (kind, size, pages) = match &a.source {
            crate::model::ArtifactSource::Image { width, height, .. } => ("image".to_string(), format!("{width}×{height}px"), 1),
            crate::model::ArtifactSource::Text { language, path } => {
                let chars = std::fs::read_to_string(path).map(|s| s.chars().count()).unwrap_or(0);
                (format!("text ({language})"), format!("{chars} chars"), 1)
            }
            crate::model::ArtifactSource::Pdf { pages, .. } => ("pdf".to_string(), format!("{pages} pages"), *pages),
            crate::model::ArtifactSource::Html { width, height, .. } => {
                ("html".to_string(), format!("html page {width}×{height}px"), 1)
            }
        };
        ManifestArtifact { id: a.id, name: a.name.clone(), kind, size, pages }
    }).collect();
    Manifest {
        format: "taste-curator/manifest".into(),
        version: 2,
        project: project.name.clone(),
        artifacts,
        envelope,
        readings,
        conflicts,
        summary,
        cold_agent_prompt: COLD_AGENT_PROMPT.into(),
    }
}

/// Two readings conflict when the curator put the same label on a marking they like and a
/// marking they avoid. Only their own words can make a conflict, so nothing is inferred.
pub fn find_conflicts(readings: &[Reading]) -> Vec<Conflict> {
    let mut out = Vec::new();
    for like in readings.iter().filter(|r| r.polarity == Polarity::Like) {
        for avoid in readings.iter().filter(|r| r.polarity == Polarity::Dislike) {
            let shared = shared_label(&like.labels, &avoid.labels);
            let Some(label) = shared else { continue };
            let summary = format!(
                "“{label}”: marking {} is wanted, marking {} is not. Their notes say why.",
                like.annotation, avoid.annotation
            );
            if out.iter().any(|c: &Conflict| c.label.eq_ignore_ascii_case(&label) && c.like == like.annotation && c.avoid == avoid.annotation) {
                continue;
            }
            out.push(Conflict { label, like: like.annotation, avoid: avoid.annotation, summary });
        }
    }
    out
}

/// The first label both markings carry, compared case-insensitively.
fn shared_label(a: &[String], b: &[String]) -> Option<String> {
    a.iter().find(|x| b.iter().any(|y| y.eq_ignore_ascii_case(x))).cloned()
}

/// Plain sentences the human and the agent both get at the front of the PDF.
pub fn summarize(readings: &[Reading], conflicts: &[Conflict]) -> Vec<String> {
    let mut lines = Vec::new();
    for polarity in [Polarity::Like, Polarity::Dislike, Polarity::Contrast] {
        let group: Vec<&Reading> = readings.iter().filter(|r| r.polarity == polarity).collect();
        if group.is_empty() { continue; }
        let mut labels: Vec<String> = Vec::new();
        for reading in &group {
            for label in &reading.labels {
                if !labels.iter().any(|l| l.eq_ignore_ascii_case(label)) {
                    labels.push(label.clone());
                }
            }
        }
        let about = if labels.is_empty() {
            "with no labels yet".to_string()
        } else {
            format!("about {}", labels.join(", "))
        };
        lines.push(format!(
            "{}: {} marking{} {about}.",
            polarity.label(),
            group.len(),
            if group.len() == 1 { "" } else { "s" },
        ));
    }
    for reading in readings {
        if let Some(contrast) = &reading.contrast {
            if contrast.ratio < 3.0 {
                lines.push(format!(
                    "Marking {} ({}): {} contrast, {} — a fact, not a verdict.",
                    reading.annotation, reading.polarity.label(),
                    format!("{:.1}:1", contrast.ratio), contrast.level
                ));
            }
        }
    }
    for conflict in conflicts {
        lines.push(format!("Conflict: {}", conflict.summary));
    }
    if lines.is_empty() { lines.push("No markings yet.".into()); }
    lines
}

/// The plain-language name of a reading, used in plate headings without inventing a category.
pub fn headline(reading: &Reading) -> String {
    let about = if reading.labels.is_empty() {
        "no labels".to_string()
    } else {
        reading.labels.join(", ")
    };
    format!("{} · {}", reading.polarity.label(), about)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Annotation, ArtifactSource, NormRect};
    use crate::telemetry::Bitmap;

    fn white_plate_with_dark_patch() -> Bitmap {
        let mut b = Bitmap { width: 64, height: 64, rgba: vec![245; 64 * 64 * 4] };
        for p in b.rgba.chunks_exact_mut(4) { p[3] = 255; }
        for y in 16..40 { for x in 16..40 { let i = ((y * 64 + x) * 4) as usize; b.rgba[i..i + 4].copy_from_slice(&[14, 14, 16, 255]); } }
        b
    }

    fn project_with(annotations: Vec<Annotation>) -> Project {
        let mut p = Project::default();
        let id = p.add_artifact("a.png".into(), ArtifactSource::Image { path: "a.png".into(), width: 64, height: 64 });
        for (i, mut a) in annotations.into_iter().enumerate() {
            a.id = i as u32 + 1;
            p.artifact_mut(id).unwrap().annotations.push(a);
        }
        p
    }

    #[test]
    fn manifest_carries_exact_telemetry_and_the_curator_s_words() {
        let b = white_plate_with_dark_patch();
        // The region spans the dark patch and the plate around it, as a real selection would.
        let mut a = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.1, 0.1, 0.9, 0.9));
        a.add_label("this ink");
        a.notes.push(crate::model::Note { text: "this ink feels right".into() });
        let p = project_with(vec![a]);
        let m = build(&p, &|_, _| Some(b.clone()));
        assert_eq!(m.readings.len(), 1);
        let r = &m.readings[0];
        assert!(r.contrast.as_ref().map(|c| c.ratio).unwrap_or(0.0) > 10.0, "{:?}", r.clusters);
        assert!((r.region[2] - 0.9).abs() < 1e-6);
        assert_eq!(r.notes, vec!["this ink feels right".to_string()]);
        assert_eq!(r.labels, vec!["this ink".to_string()]);
        assert!(m.summary.iter().any(|s| s.starts_with("Like: 1 marking about this ink")));
        assert!(m.cold_agent_prompt.contains("OKLCH"));
    }

    #[test]
    fn a_label_on_a_like_and_an_avoid_is_a_conflict() {
        let b = white_plate_with_dark_patch();
        let mut like = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.25, 0.25, 0.6, 0.6));
        like.add_label("margins");
        let mut avoid = Annotation::new(2, Polarity::Dislike, Shape::Rect, NormRect::new(0.0, 0.0, 0.2, 0.2));
        avoid.add_label("Margins");
        let p = project_with(vec![like, avoid]);
        let m = build(&p, &|_, _| Some(b.clone()));
        assert_eq!(m.conflicts.len(), 1);
        let c = &m.conflicts[0];
        assert_eq!(c.label, "margins");
        assert!(c.summary.contains("marking 1") && c.summary.contains("marking 2"), "{}", c.summary);
    }

    #[test]
    fn different_labels_never_conflict() {
        let b = white_plate_with_dark_patch();
        let mut like = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.25, 0.25, 0.6, 0.6));
        like.add_label("margins");
        let mut avoid = Annotation::new(2, Polarity::Dislike, Shape::Rect, NormRect::new(0.0, 0.0, 0.2, 0.2));
        avoid.add_label("line weight");
        let m = build(&project_with(vec![like, avoid]), &|_, _| Some(b.clone()));
        assert!(m.conflicts.is_empty(), "the app only ever compares words the curator typed: {:?}", m.conflicts);
    }

    #[test]
    fn a_marking_with_no_labels_still_reads_plainly() {
        let b = white_plate_with_dark_patch();
        let a = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.25, 0.25, 0.6, 0.6));
        let m = build(&project_with(vec![a]), &|_, _| Some(b.clone()));
        assert_eq!(headline(&m.readings[0]), "Like · no labels");
        assert!(m.summary.iter().any(|s| s.contains("with no labels yet")), "{:?}", m.summary);
    }

    #[test]
    fn missing_bitmap_skips_rather_than_guessing() {
        let a = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.1, 0.1, 0.5, 0.5));
        let m = build(&project_with(vec![a]), &|_, _| None);
        assert!(m.readings.is_empty());
        assert_eq!(m.summary, vec!["No markings yet.".to_string()]);
    }

    #[test]
    fn manifest_round_trips() {
        let b = white_plate_with_dark_patch();
        let a = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.25, 0.25, 0.6, 0.6));
        let m = build(&project_with(vec![a]), &|_, _| Some(b.clone()));
        let json = serde_json::to_string_pretty(&m).unwrap();
        assert_eq!(serde_json::from_str::<Manifest>(&json).unwrap(), m);
    }
}

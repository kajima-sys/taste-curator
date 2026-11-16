//! Every number the exporter prints, in one serializable manifest. The PDF embeds this file, so
//! a downstream agent can read exact values instead of estimating them from pixels.

use serde::{Deserialize, Serialize};

use crate::model::{Medium, Polarity, Project, Scope, Shape};
use crate::telemetry::{self, Bitmap, ColorCluster, ContrastReading, GeometryReading, SurroundReading};

/// The measurements taken for one annotation, from the artifact's untouched pixels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reading {
    pub annotation: u32,
    pub artifact: u32,
    pub artifact_name: String,
    pub polarity: Polarity,
    pub scope: Scope,
    pub medium: Medium,
    pub shape: Shape,
    /// Normalized artifact-space region, exactly as stored.
    pub region: [f32; 4],
    /// Normalized freehand polyline, when the marking had one.
    pub points: Vec<[f32; 2]>,
    pub page: Option<u32>,
    pub chips: Vec<SpectrumReading>,
    pub notes: Vec<String>,
    pub verbatim: Option<String>,
    pub clusters: Vec<ColorCluster>,
    pub contrast: Option<ContrastReading>,
    pub geometry: GeometryReading,
    pub surround: Option<SurroundReading>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpectrumReading {
    pub dial: String,
    /// 0 at the left word, 1 at the right.
    pub value: f32,
    pub left: String,
    pub right: String,
}

/// A contradiction the curator left in place: the same scope and medium, opposite polarity, and
/// perceptual dials that point different ways.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Conflict {
    pub scope: String,
    pub medium: String,
    pub like: u32,
    pub avoid: u32,
    /// Plain-language description, for the human reading the PDF and the agent reading the JSON.
    pub summary: String,
    /// Which dial disagrees, if the two readings name one.
    pub dial: Option<String>,
}

/// The whole exported packet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    pub project: String,
    pub artifacts: Vec<ManifestArtifact>,
    pub readings: Vec<Reading>,
    pub conflicts: Vec<Conflict>,
    /// One line per polarity/scope pair the agent should act on, written plainly.
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
You are given one PDF and nothing else. It describes a person's taste in visual design \
through selections they made on images, code and documents.

Read it this way:
1. The synthesis summary lists what they consistently like and what they must avoid, \
   plus any contradictions already resolved with them.
2. Each detail plate pairs an unoccluded crop with structured facts: polarity \
   (like / must avoid / contrast), what it is about (color, space, type, shape, surface, \
   feel), where it applies (UI, web, video, image), the four perceptual dials, the user's \
   own words, and exact measurements (OKLCH colors, contrast ratio, proportions).
3. Prefer the numbers over your impression of the crop. A color's OKLCH values are the \
   color they want. A contrast ratio tells you how bold to be. Proportions tell you the \
   aspect of the shapes to use.

Then produce:
- a semantic OKLCH token set (named roles such as surface, ink, accent, muted), with each \
  token's L, C and H taken from the like-telemetry and keeping clear of every must-avoid \
  color, at a contrast ratio at least as strong as the like-side readings;
- a specimen sheet that applies those tokens, with the perceptual dials honored \
  (for example tight spacing if the tight-airy dial sits low on like side);
- one sentence per token saying which observation it came from.

Do not add styles that no observation supports. Where the PDF says two things conflict, \
follow the medium that matches your target.";

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
            scope: annotation.scope,
            medium: annotation.medium,
            shape: annotation.shape,
            region: [annotation.region.x0, annotation.region.y0, annotation.region.x1, annotation.region.y1],
            points: annotation.points.clone(),
            page: annotation.page,
            chips: annotation.chips.iter().map(|(dial, value)| {
                let (left, right) = dial.ends();
                SpectrumReading { dial: dial.slug().into(), value: *value, left: left.into(), right: right.into() }
            }).collect(),
            notes: annotation.notes.iter().map(|n| n.text.clone()).filter(|t| !t.trim().is_empty()).collect(),
            verbatim,
            clusters,
            contrast,
            geometry,
            surround,
        });
    }
    let conflicts = find_conflicts(&readings);
    let summary = summarize(&readings, &conflicts);
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
        version: 1,
        project: project.name.clone(),
        artifacts,
        readings,
        conflicts,
        summary,
        cold_agent_prompt: COLD_AGENT_PROMPT.into(),
    }
}

/// Two readings conflict when they share a scope and medium with opposite polarity, and either
/// name the same dial in opposite halves.
pub fn find_conflicts(readings: &[Reading]) -> Vec<Conflict> {
    let mut out = Vec::new();
    for like in readings.iter().filter(|r| r.polarity == Polarity::Like) {
        for avoid in readings.iter().filter(|r| r.polarity == Polarity::Dislike) {
            if like.scope != avoid.scope || like.medium != avoid.medium { continue; }
            let mut dial = None;
            for chip in &like.chips {
                if let Some(other) = avoid.chips.iter().find(|c| c.dial == chip.dial) {
                    if (chip.value - 0.5).abs() > 0.15 && (other.value - 0.5).abs() > 0.15 && (chip.value < 0.5) != (other.value < 0.5) {
                        dial = Some(chip.dial.clone());
                    }
                }
            }
            let scope = like.scope.label();
            let medium = like.medium.label();
            let summary = match &dial {
                Some(d) => format!(
                    "{scope} in {medium}: keep the {} side (marking {}), but avoid the {} side (marking {}).",
                    like.chips.iter().find(|c| &c.dial == d).map(|c| if c.value < 0.5 { c.left.clone() } else { c.right.clone() }).unwrap_or_default(),
                    like.annotation,
                    avoid.chips.iter().find(|c| &c.dial == d).map(|c| if c.value < 0.5 { c.left.clone() } else { c.right.clone() }).unwrap_or_default(),
                    avoid.annotation,
                ),
                None => format!("{scope} in {medium}: marking {} is wanted, marking {} is not, and no dial separates them.", like.annotation, avoid.annotation),
            };
            out.push(Conflict { scope: scope.into(), medium: medium.into(), like: like.annotation, avoid: avoid.annotation, summary, dial });
        }
    }
    out
}

/// Plain sentences the human and the agent both get at the front of the PDF.
pub fn summarize(readings: &[Reading], conflicts: &[Conflict]) -> Vec<String> {
    let mut lines = Vec::new();
    for polarity in [Polarity::Like, Polarity::Dislike, Polarity::Contrast] {
        let group: Vec<&Reading> = readings.iter().filter(|r| r.polarity == polarity).collect();
        if group.is_empty() { continue; }
        let scopes: Vec<&str> = group.iter().map(|r| r.scope.label()).collect::<Vec<_>>();
        let unique = dedupe(&scopes);
        lines.push(format!(
            "{}: {} marking{} about {}.",
            polarity.label(),
            group.len(),
            if group.len() == 1 { "" } else { "s" },
            unique.join(", ")
        ));
    }
    for reading in readings {
        if let Some(contrast) = &reading.contrast {
            if contrast.ratio < 3.0 {
                lines.push(format!(
                    "Marking {} ({} on {}): {} contrast, {} — worth knowing if it is a Like.",
                    reading.annotation, reading.polarity.label(), reading.scope.label(),
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

fn dedupe(items: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !out.iter().any(|o| o == item) { out.push((*item).to_string()); }
    }
    out
}

/// The plain-language name of a reading, used in plate headings: "Like · Color · UI".
pub fn headline(reading: &Reading) -> String {
    format!("{} · {} · {}", reading.polarity.label(), reading.scope.label(), reading.medium.label())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Annotation, ArtifactSource, NormRect, Spectrum};
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
    fn manifest_carries_exact_telemetry() {
        let b = white_plate_with_dark_patch();
        // The region spans the dark patch and the plate around it, as a real selection would.
        let mut a = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.1, 0.1, 0.9, 0.9));
        a.scope = Scope::Color;
        a.set_chip(Spectrum::TightAiry, Some(0.2));
        a.notes.push(crate::model::Note { text: "this ink feels right".into() });
        let p = project_with(vec![a]);
        let m = build(&p, &|_, _| Some(b.clone()));
        assert_eq!(m.readings.len(), 1);
        let r = &m.readings[0];
        assert!(r.contrast.as_ref().map(|c| c.ratio).unwrap_or(0.0) > 10.0, "{:?}", r.clusters);
        assert!((r.region[2] - 0.9).abs() < 1e-6);
        assert_eq!(r.notes, vec!["this ink feels right".to_string()]);
        assert_eq!(r.chips[0].dial, "tight_airy");
        assert!(m.summary.iter().any(|s| s.starts_with("Like: 1 marking")));
        assert!(m.cold_agent_prompt.contains("OKLCH"));
    }

    #[test]
    fn opposite_halves_of_one_dial_in_one_scope_conflict() {
        let b = white_plate_with_dark_patch();
        let mut like = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.25, 0.25, 0.6, 0.6));
        like.scope = Scope::SpacingDensity;
        like.set_chip(Spectrum::TightAiry, Some(0.1));
        let mut avoid = Annotation::new(2, Polarity::Dislike, Shape::Rect, NormRect::new(0.0, 0.0, 0.2, 0.2));
        avoid.scope = Scope::SpacingDensity;
        avoid.set_chip(Spectrum::TightAiry, Some(0.9));
        let p = project_with(vec![like, avoid]);
        let m = build(&p, &|_, _| Some(b.clone()));
        assert_eq!(m.conflicts.len(), 1);
        let c = &m.conflicts[0];
        assert_eq!(c.dial.as_deref(), Some("tight_airy"));
        assert!(c.summary.contains("keep the tight side") && c.summary.contains("avoid the airy side"), "{}", c.summary);
    }

    #[test]
    fn different_media_do_not_conflict() {
        let b = white_plate_with_dark_patch();
        let mut like = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.25, 0.25, 0.6, 0.6));
        like.scope = Scope::Color;
        like.medium = Medium::Ui;
        like.set_chip(Spectrum::WarmClinical, Some(0.1));
        let mut avoid = Annotation::new(2, Polarity::Dislike, Shape::Rect, NormRect::new(0.0, 0.0, 0.2, 0.2));
        avoid.scope = Scope::Color;
        avoid.medium = Medium::Video;
        avoid.set_chip(Spectrum::WarmClinical, Some(0.9));
        let m = build(&project_with(vec![like, avoid]), &|_, _| Some(b.clone()));
        assert!(m.conflicts.is_empty());
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

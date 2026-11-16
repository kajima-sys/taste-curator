//! The annotation data model. Everything a curator says about an artifact is stored in
//! artifact-space normalized `[0..1]` coordinates, plus structured, zero-jargon metadata.
//! Raw pixels and raw source text are never mutated or thrown away: geometry is a separate
//! layer, and extraction reads the original bytes on demand.

use serde::{Deserialize, Serialize};

/// A rectangle in normalized artifact space, after rotation to upright. `x0/y0` is the
/// top-left, `x1/y1` the bottom-right; always `0 <= x0 < x1 <= 1`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NormRect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl NormRect {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> NormRect {
        let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        NormRect { x0: clamp01(x0), y0: clamp01(y0), x1: clamp01(x1), y1: clamp01(y1) }
    }

    pub fn from_points(points: &[[f32; 2]]) -> NormRect {
        let mut r = NormRect { x0: 1.0, y0: 1.0, x1: 0.0, y1: 0.0 };
        for [x, y] in points {
            r.x0 = r.x0.min(clamp01(*x));
            r.y0 = r.y0.min(clamp01(*y));
            r.x1 = r.x1.max(clamp01(*x));
            r.y1 = r.y1.max(clamp01(*y));
        }
        r
    }

    pub fn width(&self) -> f32 { (self.x1 - self.x0).max(0.0) }
    pub fn height(&self) -> f32 { (self.y1 - self.y0).max(0.0) }
    pub fn aspect(&self) -> f32 { if self.height() > 1e-6 { self.width() / self.height() } else { 0.0 } }
    pub fn center(&self) -> [f32; 2] { [(self.x0 + self.x1) * 0.5, (self.y0 + self.y1) * 0.5] }

    /// Grows by `pad` artifact units on every side, clipped to the artifact.
    pub fn padded(&self, pad: f32) -> NormRect {
        NormRect::new(self.x0 - pad, self.y0 - pad, self.x1 + pad, self.y1 + pad)
    }

    /// Intersection-over-union of two rects, `1.0` when both are empty.
    pub fn iou(&self, other: &NormRect) -> f32 {
        let ix = (self.x1.min(other.x1) - self.x0.max(other.x0)).max(0.0);
        let iy = (self.y1.min(other.y1) - self.y0.max(other.y0)).max(0.0);
        let inter = ix * iy;
        let union = self.width() * self.height() + other.width() * other.height() - inter;
        if union <= 0.0 { 1.0 } else { inter / union }
    }
}

fn clamp01(v: f32) -> f32 { if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 } }

/// How the curator feels about a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Polarity {
    /// Keep this.
    Like,
    /// Never do this.
    Dislike,
    /// The "use X from here, not Y from there" pair. A contrast annotation names both sides.
    Contrast,
}

impl Polarity {
    pub const ALL: [Polarity; 3] = [Polarity::Like, Polarity::Dislike, Polarity::Contrast];

    pub fn label(self) -> &'static str {
        match self {
            Polarity::Like => "Like",
            Polarity::Dislike => "Must avoid",
            Polarity::Contrast => "Contrast",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Polarity::Like => "like",
            Polarity::Dislike => "dislike",
            Polarity::Contrast => "contrast",
        }
    }
}

/// What part of the artifact the note is about. Plain words, not design jargon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Color,
    SpacingDensity,
    Typography,
    ShapeEdge,
    SurfaceTexture,
    MotionMood,
    Other,
}

impl Scope {
    pub const ALL: [Scope; 7] = [
        Scope::Color,
        Scope::SpacingDensity,
        Scope::Typography,
        Scope::ShapeEdge,
        Scope::SurfaceTexture,
        Scope::MotionMood,
        Scope::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Scope::Color => "Color",
            Scope::SpacingDensity => "Space",
            Scope::Typography => "Type",
            Scope::ShapeEdge => "Shape",
            Scope::SurfaceTexture => "Surface",
            Scope::MotionMood => "Feel",
            Scope::Other => "Other",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Scope::Color => "color",
            Scope::SpacingDensity => "spacing_density",
            Scope::Typography => "typography",
            Scope::ShapeEdge => "shape_edge",
            Scope::SurfaceTexture => "surface_texture",
            Scope::MotionMood => "motion_mood",
            Scope::Other => "other",
        }
    }
}

/// Where the trait should show up next; keeps contradictions scoped across media.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Medium {
    Ui,
    Web,
    Video,
    Illustration,
}

impl Medium {
    pub const ALL: [Medium; 4] = [Medium::Ui, Medium::Web, Medium::Video, Medium::Illustration];

    pub fn label(self) -> &'static str {
        match self {
            Medium::Ui => "UI",
            Medium::Web => "Web",
            Medium::Video => "Video",
            Medium::Illustration => "Image",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Medium::Ui => "ui",
            Medium::Web => "web",
            Medium::Video => "video",
            Medium::Illustration => "illustration",
        }
    }
}

/// The four perceptual dials. Each is `0..1` where `0` is the left word and `1` the right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Spectrum {
    TightAiry,
    CrispSoft,
    FlatLayered,
    WarmClinical,
}

impl Spectrum {
    pub const ALL: [Spectrum; 4] = [Spectrum::TightAiry, Spectrum::CrispSoft, Spectrum::FlatLayered, Spectrum::WarmClinical];

    pub fn ends(self) -> (&'static str, &'static str) {
        match self {
            Spectrum::TightAiry => ("tight", "airy"),
            Spectrum::CrispSoft => ("crisp", "soft"),
            Spectrum::FlatLayered => ("flat", "layered"),
            Spectrum::WarmClinical => ("warm", "clinical"),
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Spectrum::TightAiry => "tight_airy",
            Spectrum::CrispSoft => "crisp_soft",
            Spectrum::FlatLayered => "flat_layered",
            Spectrum::WarmClinical => "warm_clinical",
        }
    }
}

/// A free-form note, plus an optional machine tag for the downstream agent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub text: String,
}

/// A selected span of text in a code/text artifact or a PDF's text layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Span {
    /// The verbatim characters, never reformatted.
    pub text: String,
    /// Byte offsets into the artifact's raw source text, when it came from a text-like source.
    pub start: usize,
    pub end: usize,
    /// The font size in points and family name for PDF spans; empty for code/text.
    pub font: String,
}

/// What kind of thing the annotation highlights.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Rect,
    Ellipse,
    Freehand,
    /// A range/line selection in code or text: carries `spans`, plus a `loose` bounding box
    /// for the overview plate.
    TextSpan,
    /// A PDF vector or text object picked from the page's own content stream.
    PdfObject,
}

/// One curated observation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub id: u32,
    pub polarity: Polarity,
    pub scope: Scope,
    pub medium: Medium,
    pub shape: Shape,
    /// The selection in normalized artifact space. This is the only geometry the PDF overlay
    /// and the telemetry sampler trust.
    pub region: NormRect,
    /// Freehand points in normalized artifact space, or the corners of another shape.
    pub points: Vec<[f32; 2]>,
    /// The spectrum dials the curator moved, and where they put them.
    pub chips: Vec<(Spectrum, f32)>,
    pub notes: Vec<Note>,
    /// Contrast partner: the annotation this one is contrasted *with*.
    pub partner: Option<u32>,
    /// Verbatim source text, for text/code/PDF selections.
    pub spans: Vec<Span>,
    /// For PDF selections: 1-based page number.
    pub page: Option<u32>,
}

impl Annotation {
    pub fn new(id: u32, polarity: Polarity, shape: Shape, region: NormRect) -> Annotation {
        Annotation {
            id,
            polarity,
            scope: Scope::Other,
            medium: Medium::Ui,
            shape,
            region,
            points: Vec::new(),
            chips: Vec::new(),
            notes: Vec::new(),
            partner: None,
            spans: Vec::new(),
            page: None,
        }
    }

    pub fn chip(&self, s: Spectrum) -> Option<f32> {
        self.chips.iter().find(|(which, _)| *which == s).map(|(_, v)| *v)
    }

    /// Set or clear one dial.
    pub fn set_chip(&mut self, s: Spectrum, value: Option<f32>) {
        self.chips.retain(|(which, _)| *which != s);
        if let Some(v) = value {
            self.chips.push((s, v.clamp(0.0, 1.0)));
        }
    }

    /// The verbatim text of every span joined as it appeared, without adding separators that
    /// were not in the source.
    pub fn verbatim(&self) -> String { self.spans.iter().map(|s| s.text.as_str()).collect() }

    pub fn note_text(&self) -> String {
        self.notes.iter().map(|n| n.text.trim()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" · ")
    }
}

/// What an artifact is made of, so the app and the exporter know what to read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ArtifactSource {
    Image { path: String, width: u32, height: u32 },
    /// Code or plain/markdown text. `language` is a display hint only.
    Text { path: String, language: String },
    Pdf { path: String, pages: u32 },
}

impl ArtifactSource {
    pub fn path(&self) -> &str {
        match self {
            ArtifactSource::Image { path, .. } => path,
            ArtifactSource::Text { path, .. } => path,
            ArtifactSource::Pdf { path, .. } => path,
        }
    }
}

/// One imported file, with every annotation made on it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: u32,
    /// The file's own name, as imported.
    pub name: String,
    pub source: ArtifactSource,
    pub annotations: Vec<Annotation>,
}

impl Artifact {
    pub fn annotation(&self, id: u32) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.id == id)
    }

    pub fn annotation_mut(&mut self, id: u32) -> Option<&mut Annotation> {
        self.annotations.iter_mut().find(|a| a.id == id)
    }
}

/// The whole project: a list of artifacts and the next ids to hand out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub artifacts: Vec<Artifact>,
    pub next_artifact: u32,
    pub next_annotation: u32,
}

impl Default for Project {
    fn default() -> Self {
        Project { name: "Untitled taste".into(), artifacts: Vec::new(), next_artifact: 1, next_annotation: 1 }
    }
}

impl Project {
    pub fn add_artifact(&mut self, name: String, source: ArtifactSource) -> u32 {
        let id = self.next_artifact;
        self.next_artifact += 1;
        self.artifacts.push(Artifact { id, name, source, annotations: Vec::new() });
        id
    }

    pub fn artifact(&self, id: u32) -> Option<&Artifact> { self.artifacts.iter().find(|a| a.id == id) }
    pub fn artifact_mut(&mut self, id: u32) -> Option<&mut Artifact> { self.artifacts.iter_mut().find(|a| a.id == id) }

    pub fn add_annotation(&mut self, artifact: u32, polarity: Polarity, shape: Shape, region: NormRect) -> u32 {
        let id = self.next_annotation;
        self.next_annotation += 1;
        if let Some(a) = self.artifact_mut(artifact) {
            a.annotations.push(Annotation::new(id, polarity, shape, region));
        }
        id
    }

    pub fn annotation_mut(&mut self, artifact: u32, id: u32) -> Option<&mut Annotation> {
        self.artifact_mut(artifact)?.annotation_mut(id)
    }

    pub fn annotation(&self, artifact: u32, id: u32) -> Option<&Annotation> {
        self.artifact(artifact)?.annotation(id)
    }

    /// Every annotation in the project, with its artifact id, polarity first then reading
    /// order, which is the order the PDF plates follow.
    pub fn ordered(&self) -> Vec<(u32, &Annotation)> {
        let mut all: Vec<(u32, &Annotation)> = self.artifacts.iter().flat_map(|a| a.annotations.iter().map(move |n| (a.id, n))).collect();
        all.sort_by(|(_, a), (_, b)| {
            let rank = |p| match p { Polarity::Like => 0, Polarity::Contrast => 1, Polarity::Dislike => 2 };
            rank(a.polarity).cmp(&rank(b.polarity))
                .then(a.region.y0.total_cmp(&b.region.y0))
                .then(a.region.x0.total_cmp(&b.region.x0))
                .then(a.id.cmp(&b.id))
        });
        all
    }

    pub fn counts(&self) -> (usize, usize, usize) {
        let mut counts = (0, 0, 0);
        for a in self.artifacts.iter().flat_map(|a| &a.annotations) {
            match a.polarity {
                Polarity::Like => counts.0 += 1,
                Polarity::Dislike => counts.1 += 1,
                Polarity::Contrast => counts.2 += 1,
            }
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rects_normalize_and_measure() {
        let r = NormRect::new(0.8, 0.9, 0.2, 0.1);
        assert_eq!((r.x0, r.y0, r.x1, r.y1), (0.2, 0.1, 0.8, 0.9));
        assert!((r.width() - 0.6).abs() < 1e-6);
        assert!((r.aspect() - 0.75).abs() < 1e-6);
        assert!((r.padded(0.5).x1 - 1.0).abs() < 1e-6);
        assert!((r.iou(&r) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn iou_of_disjoint_rects_is_zero() {
        let a = NormRect::new(0.0, 0.0, 0.2, 0.2);
        let b = NormRect::new(0.5, 0.5, 0.9, 0.9);
        assert_eq!(a.iou(&b), 0.0);
    }

    #[test]
    fn freehand_bounds_cover_every_point() {
        let r = NormRect::from_points(&[[0.1, 0.2], [0.4, 0.05], [0.3, 0.6]]);
        assert!((r.x0 - 0.1).abs() < 1e-6 && (r.y1 - 0.6).abs() < 1e-6);
    }

    #[test]
    fn annotations_keep_normalized_geometry_and_order_by_polarity() {
        let mut p = Project::default();
        let a = p.add_artifact("a.png".into(), ArtifactSource::Image { path: "a.png".into(), width: 10, height: 10 });
        let like = p.add_annotation(a, Polarity::Like, Shape::Rect, NormRect::new(0.1, 0.5, 0.3, 0.6));
        let avoid = p.add_annotation(a, Polarity::Dislike, Shape::Ellipse, NormRect::new(0.1, 0.1, 0.2, 0.2));
        assert_eq!(p.ordered().iter().map(|(_, n)| n.id).collect::<Vec<_>>(), vec![like, avoid]);
        let n = p.annotation_mut(a, like).unwrap();
        n.set_chip(Spectrum::TightAiry, Some(1.7));
        assert_eq!(n.chip(Spectrum::TightAiry), Some(1.0));
        n.set_chip(Spectrum::TightAiry, None);
        assert_eq!(n.chip(Spectrum::TightAiry), None);
        assert_eq!(p.counts(), (1, 1, 0));
    }

    #[test]
    fn verbatim_spans_do_not_gain_separators() {
        let mut n = Annotation::new(1, Polarity::Like, Shape::TextSpan, NormRect::new(0.0, 0.0, 1.0, 1.0));
        n.spans.push(Span { text: "let x".into(), start: 0, end: 5, font: String::new() });
        n.spans.push(Span { text: " = 1;".into(), start: 5, end: 10, font: String::new() });
        assert_eq!(n.verbatim(), "let x = 1;");
    }

    #[test]
    fn project_round_trips_through_json() {
        let mut p = Project::default();
        let a = p.add_artifact("a.png".into(), ArtifactSource::Image { path: "a.png".into(), width: 4, height: 4 });
        p.add_annotation(a, Polarity::Contrast, Shape::Freehand, NormRect::new(0.0, 0.0, 1.0, 1.0));
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Project>(&json).unwrap(), p);
    }
}

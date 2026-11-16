//! App state: the project, the camera, the tool, the cached substrate of the artifact in view.
//! Every mutation that a curator could regret goes through `remember`, which snapshots the
//! project for undo before changing it.

use std::collections::HashMap;
use std::path::Path;

use taste_curator::camera::Camera;
use taste_curator::model::{Annotation, ArtifactSource, Medium, Note, Polarity, Project, Scope, Shape, Spectrum};
use taste_curator::telemetry::Bitmap;
use taste_curator::tools::Tool;
use rune::{Pixels, TextEdit};

/// The substrate of one artifact page, decoded once and kept: raw pixels plus the handle Rune
/// draws them through.
pub struct Page {
    pub bitmap: Bitmap,
    pub pixels: Pixels,
}

/// What a drag is doing right now. `Idle` means the armed tool decides what a press starts.
#[derive(Clone, Debug, Default)]
pub enum Mode {
    #[default]
    Idle,
    /// Moving the picture; `last` is the pointer in screen points.
    Pan { last: [f32; 2] },
    /// A marking in progress, in normalized artifact space.
    Draw { shape: Shape, start: [f32; 2], current: [f32; 2], points: Vec<[f32; 2]> },
}

pub struct App {
    pub project: Project,
    pub tool: Tool,
    pub mode: Mode,
    pub camera: Camera,
    /// The canvas area in points, from `on_layout`.
    pub viewport: [f32; 2],
    pub active: Option<u32>,
    pub page: u32,
    pub pages: HashMap<(u32, u32), Page>,
    pub texts: HashMap<u32, (String, String)>,
    pub selected: Option<u32>,
    pub note: TextEdit,
    pub note_for: Option<u32>,
    pub status: String,
    pub message: String,
    pub dark: bool,
    pub history: Vec<Project>,
    pub exporting: bool,
    /// A short readout of the selected marking's unannotated pixels.
    pub readout: Vec<String>,
    /// The JSON last written to disk, so saving only happens on a real change.
    pub last_saved: String,
    pub saved_to: Option<std::path::PathBuf>,
}

impl Default for App {
    fn default() -> Self {
        App {
            project: Project::default(),
            tool: Tool::Select,
            mode: Mode::Idle,
            camera: Camera::default(),
            viewport: [800.0, 600.0],
            active: None,
            page: 1,
            pages: HashMap::new(),
            texts: HashMap::new(),
            selected: None,
            note: TextEdit::new(""),
            note_for: None,
            status: Tool::Select.hint().to_string(),
            message: String::new(),
            dark: true,
            history: Vec::new(),
            exporting: false,
            readout: Vec::new(),
            last_saved: String::new(),
            saved_to: None,
        }
    }
}

impl App {
    pub fn remember(&mut self) {
        self.history.push(self.project.clone());
        if self.history.len() > 64 {
            self.history.remove(0);
        }
    }

    pub fn undo(&mut self) {
        if let Some(p) = self.history.pop() {
            self.project = p;
            if self.selected.is_some_and(|id| self.find(id).is_none()) {
                self.selected = None;
            }
            if self.active.is_some_and(|id| self.project.artifact(id).is_none()) {
                self.active = self.project.artifacts.first().map(|a| a.id);
            }
            self.message = "Undone".into();
        }
    }

    pub fn find(&self, annotation: u32) -> Option<(u32, &Annotation)> {
        for a in &self.project.artifacts {
            if let Some(n) = a.annotations.iter().find(|n| n.id == annotation) {
                return Some((a.id, n));
            }
        }
        None
    }

    /// The selected annotation and the artifact it belongs to.
    pub fn selected_annotation(&self) -> Option<(u32, &Annotation)> {
        self.selected.and_then(|id| self.find(id))
    }

    pub fn artifact(&self) -> Option<&taste_curator::model::Artifact> {
        self.project.artifact(self.active?)
    }

    /// The aspect ratio of what is in view: pixels when there are any, otherwise a page shape.
    pub fn aspect(&self) -> f32 {
        match self.active.and_then(|id| self.pages.get(&(id, self.page))) {
            Some(p) if p.bitmap.height > 0 => p.bitmap.width as f32 / p.bitmap.height as f32,
            _ => 0.75,
        }
    }

    pub fn fitted(&self) -> [f32; 2] {
        Camera::fit_size(self.aspect(), self.viewport, 16.0)
    }

    /// Screen points for a normalized artifact point.
    pub fn to_screen(&self, p: [f32; 2]) -> [f32; 2] {
        self.camera.to_screen(p, self.viewport, self.fitted())
    }

    /// The normalized artifact point under a canvas-local screen point.
    pub fn to_artifact(&self, p: [f32; 2]) -> [f32; 2] {
        self.camera.to_artifact(p, self.viewport, self.fitted())
    }

    pub fn zoom_about(&mut self, anchor: [f32; 2], factor: f32) {
        let fitted = self.fitted();
        self.camera.zoom_about(anchor, factor, self.viewport, fitted);
        self.status = format!("Zoom {:.0}%", self.camera.zoom * 100.0);
    }

    pub fn fit(&mut self) {
        self.camera = Camera::default();
        self.status = "Fitted".into();
    }

    /// Move to another artifact, keep its page and clear the selection.
    pub fn open_artifact(&mut self, id: u32) {
        self.active = Some(id);
        self.page = 1;
        self.selected = None;
        self.mode = Mode::Idle;
        self.fit();
        self.ensure_loaded(id, 1);
        self.message.clear();
    }

    /// Decode an artifact page once. Images and PDF pages become pixels; text stays text.
    pub fn ensure_loaded(&mut self, id: u32, page: u32) {
        let Some(artifact) = self.project.artifact(id) else { return };
        let source = artifact.source.clone();
        if matches!(source, ArtifactSource::Text { .. }) && self.texts.contains_key(&id) {
            return;
        }
        if self.pages.contains_key(&(id, page)) {
            return;
        }
        match taste_curator::artifact::load(&source) {
            Ok(substrate) => match substrate {
                taste_curator::artifact::Substrate::Image(b) => self.put_page(id, page, b),
                taste_curator::artifact::Substrate::Pdf { pages } => {
                    for p in pages {
                        self.put_page(id, p.number, p.bitmap);
                    }
                }
                taste_curator::artifact::Substrate::Text { text, language } => {
                    self.texts.insert(id, (text, language));
                }
            },
            Err(e) => self.message = e,
        }
    }

    fn put_page(&mut self, id: u32, page: u32, bitmap: Bitmap) {
        let pixels = Pixels::new();
        pixels.set(bitmap.width, bitmap.height, bitmap.rgba.clone());
        self.pages.insert((id, page), Page { bitmap, pixels });
    }

    /// Re-read the page after edits outside the app (an artifact replaced on disk).
    pub fn forget_pages(&mut self, id: u32) {
        self.pages.retain(|(a, _), _| *a != id);
        self.texts.remove(&id);
    }

    /// Import a file, decode what it needs, and open it.
    pub fn import(&mut self, path: &Path) {
        match taste_curator::artifact::describe(path) {
            Ok((name, source)) => {
                self.remember();
                let id = self.project.add_artifact(name.clone(), source);
                self.open_artifact(id);
                self.message = format!("Added {name}");
            }
            Err(e) => self.message = e,
        }
    }

    pub fn page_count(&self) -> u32 {
        match &self.artifact().map(|a| a.source.clone()) {
            Some(ArtifactSource::Pdf { pages, .. }) => *pages,
            _ => 1,
        }
    }

    pub fn can_page(&self) -> bool {
        matches!(self.artifact().map(|a| a.source.clone()), Some(ArtifactSource::Pdf { .. }))
    }

    pub fn turn_page(&mut self, delta: i32) {
        let count = self.page_count();
        let next = (self.page as i32 + delta).clamp(1, count as i32) as u32;
        if next != self.page {
            self.page = next;
            if let Some(id) = self.active {
                self.ensure_loaded(id, next);
            }

        }
    }

    /// Commit the marking in progress as an annotation with a `Like` reading and open it.
    pub fn commit_draft(&mut self) {
        let Mode::Draw { shape, start, current, points } = self.mode.clone() else { return };
        let region = if shape == Shape::Freehand && points.len() >= 2 {
            taste_curator::model::NormRect::from_points(&points)
        } else {
            taste_curator::model::NormRect::new(start[0], start[1], current[0], current[1])
        };
        if region.width() <= 0.004 || region.height() <= 0.004 {
            self.mode = Mode::Idle;
            return;
        }
        let Some(artifact) = self.active else {
            self.mode = Mode::Idle;
            return;
        };
        let medium = medium_for(self.project.artifact(artifact).map(|a| &a.source));
        let page = self.can_page().then_some(self.page);
        self.remember();
        let id = self.project.add_annotation(artifact, Polarity::Like, shape, region);
        if let Some(n) = self.project.annotation_mut(artifact, id) {
            n.medium = medium;
            n.page = page;
            if shape == Shape::Freehand {
                n.points = points;
            }
        }
        self.select(id);
        self.mode = Mode::Idle;
        self.status = format!("{} — {} marked", shape_label(shape), self.project.artifact(artifact).map(|a| a.name.as_str()).unwrap_or(""));
        self.refresh_readout();
    }

    /// Select an annotation, loading its notes into the field and its artifact into view.
    pub fn select(&mut self, id: u32) {
        let Some((artifact, page, text)) = self
            .find(id)
            .map(|(artifact, n)| (artifact, n.page, n.note_text()))
        else { return };
        self.selected = Some(id);
        if self.active != Some(artifact) {
            self.open_artifact(artifact);
            self.selected = Some(id);
        }
        if let Some(page) = page {
            self.page = page;
            self.ensure_loaded(artifact, page);
        }
        self.note = TextEdit::new(text);
        self.note_for = Some(id);
        self.mode = Mode::Idle;
        self.refresh_readout();
    }

    pub fn deselect(&mut self) {
        self.selected = None;
        self.mode = Mode::Idle;
        self.readout.clear();
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        self.remember();
        if let Some(a) = self.project.artifact_mut(artifact) {
            a.annotations.retain(|n| n.id != id);
        }
        self.deselect();
        self.message = "Marking removed".into();
    }

    /// Copy the note field into the selected annotation; called each frame while editing.
    pub fn sync_note(&mut self) {
        let Some(id) = self.selected else { return };
        let text = self.note.text.clone();
        if let Some((artifact, _)) = self.find(id) {
            if let Some(n) = self.project.annotation_mut(artifact, id) {
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    n.notes.clear();
                } else {
                    n.notes = vec![Note { text: text.clone() }];
                }
            }
        }
    }

    /// A plain-language readout of the selection from its untouched pixels.
    pub fn refresh_readout(&mut self) {
        self.readout.clear();
        let Some((artifact, region, page)) = self
            .selected_annotation()
            .map(|(a, n)| (a, n.region, n.page))
        else { return };
        let Some(bitmap) = self.page_for(artifact, page).map(|p| p.bitmap.clone()) else { return };
        let clusters = taste_curator::telemetry::dominant_clusters(&bitmap, &region, 4);
        let mut lines = Vec::new();
        if let Some(contrast) = taste_curator::telemetry::contrast_reading(&clusters) {
            lines.push(format!("contrast {:.2}:1 ({})", contrast.ratio, contrast.level));
        }
        let g = taste_curator::telemetry::geometry_reading(&bitmap, &region);
        lines.push(format!("{}×{} px · {} · {:.0}% of the frame", g.px_w, g.px_h, g.ratio_name, g.area_share * 100.0));
        let colors: Vec<String> = clusters.iter().take(3).map(|c| format!("#{} {:.0}%", c.hex, c.share * 100.0)).collect();
        if !colors.is_empty() {
            lines.push(colors.join("  "));
        }
        self.readout = lines;
    }

    fn page_for(&self, artifact: u32, page: Option<u32>) -> Option<&Page> {
        self.pages.get(&(artifact, page.unwrap_or(1)))
    }

    pub fn set_polarity(&mut self, polarity: Polarity) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        self.remember();
        if let Some(n) = self.project.annotation_mut(artifact, id) {
            n.polarity = polarity;
        }
    }

    pub fn set_scope(&mut self, scope: Scope) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        self.remember();
        if let Some(n) = self.project.annotation_mut(artifact, id) {
            n.scope = scope;
        }
    }

    pub fn set_medium(&mut self, medium: Medium) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        self.remember();
        if let Some(n) = self.project.annotation_mut(artifact, id) {
            n.medium = medium;
        }
    }

    pub fn set_chip(&mut self, dial: Spectrum, value: Option<f32>) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        self.remember();
        if let Some(n) = self.project.annotation_mut(artifact, id) {
            n.set_chip(dial, value);
        }
    }
}

pub fn medium_for(source: Option<&ArtifactSource>) -> Medium {
    match source {
        Some(ArtifactSource::Text { .. }) => Medium::Web,
        Some(ArtifactSource::Pdf { .. }) => Medium::Ui,
        _ => Medium::Illustration,
    }
}

pub fn shape_label(shape: Shape) -> &'static str {
    match shape {
        Shape::Rect => "Box",
        Shape::Ellipse => "Oval",
        Shape::Freehand => "Freehand",
        Shape::TextSpan => "Text",
        Shape::PdfObject => "Object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taste_curator::model::{NormRect, Polarity, Scope};

    fn seeded() -> App {
        let mut s = App::default();
        let dir = std::env::temp_dir().join("taste-curator-native-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plate.png");
        let img = image::RgbaImage::from_fn(80, 60, |x, y| image::Rgba([(x * 3) as u8, (y * 4) as u8, 120, 255]));
        img.save(&path).unwrap();
        s.import(&path);
        s
    }

    #[test]
    fn a_drawn_box_becomes_a_like_marking_with_measured_telemetry() {
        let mut s = seeded();
        s.tool = Tool::Rect;
        s.mode = Mode::Draw {
            shape: Shape::Rect,
            start: [0.2, 0.2],
            current: [0.6, 0.7],
            points: vec![],
        };
        s.commit_draft();
        let (artifact, n) = s.selected_annotation().expect("selects the new marking");
        assert_eq!(n.polarity, Polarity::Like);
        assert_eq!(n.shape, Shape::Rect);
        assert!((n.region.width() - 0.4).abs() < 1e-5);
        assert_eq!(s.project.artifact(artifact).unwrap().annotations.len(), 1);
        assert!(s.readout.iter().any(|l| l.contains("px")), "{:?}", s.readout);
        assert!(s.readout.iter().any(|l| l.contains("contrast")));
    }

    #[test]
    fn a_click_without_a_drag_makes_nothing() {
        let mut s = seeded();
        s.tool = Tool::Ellipse;
        s.mode = Mode::Draw {
            shape: Shape::Ellipse,
            start: [0.5, 0.5],
            current: [0.5001, 0.5001],
            points: vec![],
        };
        s.commit_draft();
        assert!(s.project.artifacts[0].annotations.is_empty());
        assert!(s.selected.is_none());
    }

    #[test]
    fn freehand_keeps_its_polyline() {
        let mut s = seeded();
        s.tool = Tool::Freehand;
        s.mode = Mode::Draw {
            shape: Shape::Freehand,
            start: [0.1, 0.1],
            current: [0.6, 0.5],
            points: vec![[0.1, 0.1], [0.3, 0.4], [0.6, 0.5]],
        };
        s.commit_draft();
        let (_, n) = s.selected_annotation().unwrap();
        assert_eq!(n.points.len(), 3);
    }

    #[test]
    fn undo_restores_the_markings_and_the_selection_does_not_dangle() {
        let mut s = seeded();
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.1, 0.1], current: [0.5, 0.5], points: vec![] };
        s.commit_draft();
        let id = s.selected.unwrap();
        s.undo();
        assert!(s.selected.is_none(), "the undone marking is not still selected");
        assert!(s.find(id).is_none());
        assert!(s.project.artifacts[0].annotations.is_empty());
    }

    #[test]
    fn metadata_edits_stick_and_undo_rolls_them_back() {
        let mut s = seeded();
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.1, 0.1], current: [0.5, 0.5], points: vec![] };
        s.commit_draft();
        s.set_polarity(Polarity::Dislike);
        s.set_scope(Scope::Color);
        s.set_chip(Spectrum::TightAiry, Some(0.25));
        let (_, n) = s.selected_annotation().unwrap();
        assert_eq!(n.polarity, Polarity::Dislike);
        assert_eq!(n.scope, Scope::Color);
        assert_eq!(n.chip(Spectrum::TightAiry), Some(0.25));
        s.undo();
        let (_, n) = s.selected_annotation().unwrap();
        assert_eq!(n.scope, Scope::Color);
        assert_eq!(n.chip(Spectrum::TightAiry), None);
    }

    #[test]
    fn geometry_is_read_in_pixels_not_normalized_units() {
        let mut s = seeded();
        s.selected = None;
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.0, 0.0], current: [0.5, 0.5], points: vec![] };
        s.commit_draft();
        let (_, n) = s.selected_annotation().unwrap();
        let g = taste_curator::telemetry::geometry_reading(&s.pages[&(1, 1)].bitmap, &n.region);
        assert_eq!((g.px_w, g.px_h), (40, 30));
        assert!((g.aspect - 4.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn export_writes_a_pdf_the_engine_can_parse_again() {
        let mut s = seeded();
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.2, 0.2], current: [0.7, 0.7], points: vec![] };
        s.commit_draft();
        let out = std::env::temp_dir().join("taste-curator-native-tests").join("export.pdf");
        s.export_to(&out);
        assert!(s.message.contains("Exported"), "{}", s.message);
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let (bitmap, _, _) = taste_curator::artifact::render_pdf_page(&bytes, 1, 1.0).expect("page 1 renders");
        assert!(bitmap.width > 100);
    }

    #[test]
    fn region_and_zoom_round_trip_through_the_camera() {
        let s = seeded();
        let p = [0.25, 0.75];
        let back = s.to_artifact(s.to_screen(p));
        assert!((back[0] - p[0]).abs() < 1e-3 && (back[1] - p[1]).abs() < 1e-3);
        let _ = NormRect::new(0.0, 0.0, 1.0, 1.0);
    }
}

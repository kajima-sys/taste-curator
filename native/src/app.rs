//! App state: the project, the camera, the tool, the cached substrate of the artifact in view.
//! Every mutation that a curator could regret goes through `remember`, which snapshots the
//! project for undo before changing it.

use std::collections::HashMap;
use std::path::Path;

use taste_curator::calibrate::{self, DEFAULT_STRENGTH};
use taste_curator::camera::Camera;
use taste_curator::gestures;
use taste_curator::model::{
    Annotation, ArtifactSource, FlinchVerdict, Note, Polarity, ProbeAxis, ProbeDirection,
    ProbeJudgment, Project, Shape,
};
use taste_curator::telemetry::Bitmap;
use taste_curator::tools::Tool;
use rune::{Pixels, TextEdit};

/// Which workspace is open: counterfactual calibration ("Perturb Until They Flinch") or
/// region markup on the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Workspace {
    #[default]
    Calibrate,
    MarkUp,
}

/// Cached side-by-side pixels for the active counterfactual probe.
pub struct ProbePreview {
    pub artifact: u32,
    pub axis: ProbeAxis,
    pub direction: ProbeDirection,
    pub strength: f32,
    pub anchor: Pixels,
    pub perturbed: Pixels,
    pub width: u32,
    pub height: u32,
}

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
    /// A press with the select tool: it picks `hit` on release unless the pointer travels, in
    /// which case the same press pans. `press` is where the pointer went down (screen points),
    /// so a drag that starts under slop doesn't jump when it becomes a pan.
    Select { press: [f32; 2], last: [f32; 2], hit: Option<u32>, moved: bool },
    /// A marking in progress, in normalized artifact space.
    Draw { shape: Shape, start: [f32; 2], current: [f32; 2], points: Vec<[f32; 2]> },
}

pub struct App {
    pub project: Project,
    pub workspace: Workspace,
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
    /// The label being typed in the inspector's "add a word" field.
    pub label_draft: TextEdit,
    pub label_for: Option<u32>,
    /// Target context ("What are you building?"), synced into `project.target`.
    pub target_draft: TextEdit,
    /// Active counterfactual probe `(axis, direction, strength)` for `self.active`.
    pub active_probe: Option<(ProbeAxis, ProbeDirection, f32)>,
    /// Cached anchor + perturbed pixels for the active probe.
    pub probe_preview: Option<ProbePreview>,
    /// Optional note when pairing an anchor against a foil ("This, not that").
    pub pair_note: TextEdit,
    pub status: String,
    pub message: String,
    pub dark: bool,
    pub history: Vec<Project>,
    pub exporting: bool,
    /// A short readout of the selected marking's unannotated pixels, in plain words.
    pub readout: Vec<String>,
    /// The curved readout under "exact values": numbers only, hidden by default.
    pub exact: Vec<String>,
    pub show_values: bool,
    /// The JSON last written to disk, so saving only happens on a real change.
    pub last_saved: String,
    pub saved_to: Option<std::path::PathBuf>,
}

impl Default for App {
    fn default() -> Self {
        App {
            project: Project::default(),
            workspace: Workspace::Calibrate,
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
            label_draft: TextEdit::new(""),
            label_for: None,
            target_draft: TextEdit::new(""),
            active_probe: None,
            probe_preview: None,
            pair_note: TextEdit::new(""),
            status: Tool::Select.hint().to_string(),
            message: String::new(),
            dark: true,
            history: Vec::new(),
            exporting: false,
            readout: Vec::new(),
            exact: Vec::new(),
            show_values: false,
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
            self.target_draft = TextEdit::new(&self.project.target);
            self.active_probe = None;
            self.probe_preview = None;
            self.refresh_probe();
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

    /// A wheel event as the canvas sees it, applied. `dy` negative (a turn away from the user)
    /// zooms in; Ctrl/⌘ fines it, Alt moves the picture, and a sideways wheel moves it sideways.
    pub fn wheel(&mut self, ev: gestures::Wheel) {
        match gestures::wheel_command(ev) {
            gestures::WheelCommand::Zoom { anchor, factor } => self.zoom_about(anchor, factor),
            gestures::WheelCommand::Pan { by } => {
                self.camera.pan_by(by);
                self.status = "Moved".into();
            }
        }
    }

    /// A press on the canvas, in canvas points. The pan tool and the middle button pan at once;
    /// the select tool records a pending pick; a drawing tool starts its draft.
    pub fn canvas_press(&mut self, local: [f32; 2], middle: bool) {
        let at = self.to_artifact(local);
        if middle || self.tool == Tool::Pan {
            self.mode = Mode::Pan { last: local };
            self.status = "Moving the picture".into();
            return;
        }
        if self.tool == Tool::Select {
            let hit = crate::canvas::hit_test(self, at);
            self.mode = Mode::Select { press: local, last: local, hit, moved: false };
            self.status = match hit {
                Some(_) => "Click to open it, drag to move the picture".into(),
                None => "Drag to move the picture".into(),
            };
            return;
        }
        let Some(shape) = self.tool.shape() else {
            self.deselect();
            return;
        };
        self.deselect();
        self.mode = Mode::Draw { shape, start: at, current: at, points: vec![at] };
        self.status = match shape {
            Shape::Rect => "Drawing a box".into(),
            Shape::Ellipse => "Drawing an oval".into(),
            _ => "Drawing around it".into(),
        };
    }

    /// The pressed pointer moved to `local`. A select press turns into a pan once it travels
    /// past [`gestures::CLICK_SLOP`], taking up the travel it already made in the same step, so
    /// nothing jumps. Returns whether the picture moved.
    pub fn canvas_drag(&mut self, local: [f32; 2]) -> bool {
        let at = self.to_artifact(local);
        match &mut self.mode {
            Mode::Pan { last } => {
                let delta = [local[0] - last[0], local[1] - last[1]];
                self.camera.pan_by(delta);
                *last = local;
                true
            }
            Mode::Select { press, last, moved, .. } => {
                if !*moved && gestures::is_click(gestures::travel(*press, local)) {
                    return false;
                }
                let delta = if *moved {
                    [local[0] - last[0], local[1] - last[1]]
                } else {
                    *moved = true;
                    self.status = "Moving the picture".into();
                    [local[0] - press[0], local[1] - press[1]]
                };
                self.camera.pan_by(delta);
                *last = local;
                true
            }
            Mode::Draw { shape, current, points, .. } => {
                *current = at;
                if *shape == Shape::Freehand
                    && points.last().is_none_or(|p| (p[0] - at[0]).abs() + (p[1] - at[1]).abs() > 0.001)
                {
                    points.push(at);
                }
                true
            }
            Mode::Idle => false,
        }
    }

    /// The press released at `local`. A select press that stayed put picks its marking here, and
    /// clears the selection when it found none; a draft commits.
    pub fn canvas_release(&mut self, local: [f32; 2]) {
        if let Mode::Select { press, hit, .. } = self.mode {
            self.mode = Mode::Idle;
            match (gestures::is_click(gestures::travel(press, local)), hit) {
                (true, Some(id)) => self.select(id),
                _ => {
                    self.deselect();
                    self.status = self.tool.hint().to_string();
                }
            }
            return;
        }
        if matches!(self.mode, Mode::Draw { .. }) {
            let at = self.to_artifact(local);
            if let Mode::Draw { current, points, shape, .. } = &mut self.mode {
                *current = at;
                if *shape == Shape::Freehand && points.last() != Some(&at) {
                    points.push(at);
                }
            }
            self.commit_draft();
            return;
        }
        self.mode = Mode::Idle;
        self.status = self.tool.hint().to_string();
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
        self.active_probe = None;
        self.probe_preview = None;
        self.refresh_probe();
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
                // A page renders to pixels, and keeps its source so the curator can read it.
                taste_curator::artifact::Substrate::Html { bitmap, text } => {
                    self.put_page(id, page, bitmap);
                    self.texts.insert(id, (text, "html".into()));
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
        let page = self.can_page().then_some(self.page);
        self.remember();
        let id = self.project.add_annotation(artifact, Polarity::Like, shape, region);
        if let Some(n) = self.project.annotation_mut(artifact, id) {
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
        self.label_draft = TextEdit::new("");
        self.label_for = Some(id);
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

    /// Keep `project.target` synced with the inspector's "What are you building?" field.
    pub fn sync_target(&mut self) {
        let trimmed = self.target_draft.text.trim();
        if self.project.target != trimmed {
            self.project.target = trimmed.to_string();
        }
    }

    /// Ensure the active counterfactual probe has cached anchor + perturbed bitmaps ready.
    pub fn refresh_probe(&mut self) {
        let Some(artifact) = self.active else {
            self.active_probe = None;
            self.probe_preview = None;
            return;
        };
        if self.active_probe.is_none() {
            self.active_probe = calibrate::next_probe(&self.project, artifact);
        }
        let Some((axis, direction, strength)) = self.active_probe else {
            self.probe_preview = None;
            return;
        };
        if let Some(p) = &self.probe_preview {
            if p.artifact == artifact
                && p.axis == axis
                && p.direction == direction
                && (p.strength - strength).abs() < 1e-4
            {
                return;
            }
        }
        let Some(page) = self.pages.get(&(artifact, self.page)) else {
            self.probe_preview = None;
            return;
        };
        let base = calibrate::preview_bitmap(&page.bitmap, 440);
        let perturbed_bm = calibrate::perturb(&base, axis, direction, strength);
        let anchor = Pixels::new();
        anchor.set(base.width, base.height, base.rgba);
        let perturbed = Pixels::new();
        perturbed.set(perturbed_bm.width, perturbed_bm.height, perturbed_bm.rgba);
        self.probe_preview = Some(ProbePreview {
            artifact,
            axis,
            direction,
            strength,
            anchor,
            perturbed,
            width: base.width,
            height: base.height,
        });
    }

    /// Jump directly to a specific perturbation axis and direction on the active reference.
    pub fn select_probe(&mut self, axis: ProbeAxis, direction: ProbeDirection) {
        self.active_probe = Some((axis, direction, DEFAULT_STRENGTH));
        self.probe_preview = None;
        self.refresh_probe();
    }

    /// Record a flinch judgment for the current probe and advance to the next untested axis.
    pub fn judge_probe(&mut self, verdict: FlinchVerdict) {
        let Some(artifact) = self.active else { return };
        let Some((axis, direction, strength)) = self.active_probe else { return };
        self.remember();
        self.project.record_probe(ProbeJudgment {
            artifact,
            axis,
            direction,
            strength,
            verdict,
            note: String::new(),
        });
        let change = axis.change_label(direction);
        self.message = format!("{change}: {}", verdict.label());
        self.active_probe = calibrate::next_probe(&self.project, artifact);
        self.probe_preview = None;
        self.refresh_probe();
    }

    /// Reset all calibration judgments for the active reference.
    pub fn reset_calibration(&mut self) {
        let Some(artifact) = self.active else { return };
        self.remember();
        self.project.clear_probes_for(artifact);
        self.active_probe = None;
        self.probe_preview = None;
        self.refresh_probe();
        self.message = "Calibration reset for this reference".into();
    }

    /// Record a contrastive pair ("This, not that") with the active artifact as Anchor and `foil`
    /// as Foil.
    pub fn commit_pair(&mut self, foil: u32) {
        let Some(anchor) = self.active else { return };
        if anchor == foil {
            return;
        }
        let note = self.pair_note.text.trim().to_string();
        self.remember();
        self.project.add_pair(anchor, foil, note);
        self.pair_note = TextEdit::new("");
        let foil_name = self.project.artifact(foil).map(|a| a.name.clone()).unwrap_or_default();
        self.message = format!("Paired against {foil_name}");
    }

    pub fn remove_pair(&mut self, id: u32) {
        self.remember();
        self.project.remove_pair(id);
        self.message = "Pair removed".into();
    }

    /// A plain-language readout of the selection from its untouched pixels. Facts only: no
    /// value judgement is made, and the numbers live behind `show_values`.
    pub fn refresh_readout(&mut self) {
        self.readout.clear();
        self.exact.clear();
        let Some((artifact, region, page)) = self
            .selected_annotation()
            .map(|(a, n)| (a, n.region, n.page))
        else { return };
        let Some(bitmap) = self.page_for(artifact, page).map(|p| p.bitmap.clone()) else { return };
        let clusters = taste_curator::telemetry::dominant_clusters(&bitmap, &region, 4);
        let g = taste_curator::telemetry::geometry_reading(&bitmap, &region);
        self.readout.push(format!("Size: about {:.0}% of the frame ({})", g.area_share * 100.0, g.ratio_name));
        let colors: Vec<String> = clusters.iter().take(3).map(|c| format!("#{} {:.0}%", c.hex, c.share * 100.0)).collect();
        if !colors.is_empty() {
            self.readout.push(format!("Most of it: {}", colors.join("  ")));
        }
        let mut exact = Vec::new();
        if let Some(contrast) = taste_curator::telemetry::contrast_reading(&clusters) {
            self.readout.push(format!("Separation between the two nearest colours: {:.1}:1", contrast.ratio));
            exact.push(format!("contrast {:.2}:1 ({})", contrast.ratio, contrast.level));
        }
        exact.push(format!("{}×{} px · {:.0}% of the frame", g.px_w, g.px_h, g.area_share * 100.0));
        for c in clusters.iter().take(4) {
            exact.push(format!("#{}  {:.0}%  oklch({:.3} {:.3} {:.1})", c.hex, c.share * 100.0, c.oklch.l, c.oklch.c, c.oklch.h));
        }
        self.exact = exact;
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

    /// Flip one label on the selected marking. A press takes it back off, so a wrong chip costs
    /// one click and nothing has to be cleared out later.
    pub fn toggle_label(&mut self, label: &str) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        if self.find(id).is_some_and(|(_, n)| n.has_label(label)) {
            self.remember();
            if let Some(n) = self.project.annotation_mut(artifact, id) {
                n.remove_label(label);
            }
        } else {
            self.remember();
            if let Some(n) = self.project.annotation_mut(artifact, id) {
                n.add_label(label);
            }
        }
    }

    /// Add the word typed in the inspector's field to the selected marking. The same word on
    /// another marking is one word, not two: `has_label` compares case-insensitively.
    pub fn commit_label(&mut self) {
        let Some(id) = self.selected else { return };
        let Some((artifact, _)) = self.find(id) else { return };
        let word = self.label_draft.text.trim().to_string();
        self.label_draft = TextEdit::new("");
        if word.is_empty() { return; }
        if self.find(id).is_some_and(|(_, n)| n.has_label(&word)) { return; }
        self.remember();
        if let Some(n) = self.project.annotation_mut(artifact, id) {
            n.add_label(&word);
        }
        self.message = format!("“{word}” added — it will be offered back from now on");
    }

    /// Every label in the project, most-used first, to offer back as chips.
    pub fn vocabulary(&self) -> Vec<(String, usize)> {
        Annotation::vocabulary(&self.project.artifacts)
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
    use taste_curator::model::{NormRect, Polarity};

    /// A scratch file unique to this test, so parallel tests never write over each other's
    /// image while another is reading it. Windows made that visible: `img.save` truncates the
    /// file, and a reader in another thread sees a half-written PNG.
    fn scratch(name: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("taste-curator-native-tests-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn seeded() -> App {
        let mut s = App::default();
        let path = scratch("plate.png");
        let img = image::RgbaImage::from_fn(80, 60, |x, y| image::Rgba([(x * 3) as u8, (y * 4) as u8, 120, 255]));
        img.save(&path).unwrap();
        s.import(&path);
        // If the import failed, say so here instead of panicking later on a missing page.
        assert_eq!(s.project.artifacts.len(), 1, "the sample image imported: {}", s.message);
        s
    }

    fn seeded_page() -> App {
        let mut s = App::default();
        let dir = scratch("");
        std::fs::write(dir.join("site.css"), "body{margin:0;font-family:Inter;background:#fff}\n.card{margin:20px;padding:20px;background:#e8e6ff}\nh1{margin:0;color:#2b1a80;font-size:30px}").unwrap();
        let page = dir.join("page.html");
        std::fs::write(&page, r#"<!doctype html><link rel="stylesheet" href="site.css"><div class="card"><h1>Marked up in a window</h1></div>"#).unwrap();
        s.import(&page);
        assert_eq!(s.project.artifacts.len(), 1, "the page imported: {}", s.message);
        s
    }

    #[test]
    fn a_web_page_imports_as_pixels_and_can_be_marked_up() {
        let mut s = seeded_page();
        let artifact = s.artifact().unwrap();
        let ArtifactSource::Html { width, height, .. } = artifact.source else {
            panic!("an .html file is imported as a page, not as plain text");
        };
        assert!(width > 300 && height > 40, "the page has a real laid-out size: {width}x{height}");

        // The rendered page is a normal artifact page, so drawing on it measures its pixels.
        s.tool = Tool::Rect;
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.0, 0.0], current: [0.5, 0.5], points: vec![] };
        s.commit_draft();
        let (_, n) = s.selected_annotation().expect("the marking was created");
        let page = s.pages.values().next().expect("the page rendered to pixels");
        let g = taste_curator::telemetry::geometry_reading(&page.bitmap, &n.region);
        assert_eq!((g.px_w, g.px_h), (width / 2, height / 2), "geometry comes from the rendered page");
        assert!(s.readout.iter().any(|l| l.starts_with("Size:")), "{:?}", s.readout);
        assert!(s.readout.iter().any(|l| l.starts_with("Separation")), "{:?}", s.readout);
    }

    #[test]
    fn a_web_page_exports_into_the_pdf_with_its_rendered_pixels() {
        let mut s = seeded_page();
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.05, 0.1], current: [0.6, 0.8], points: vec![] };
        s.commit_draft();
        let out = scratch("page.pdf");
        s.export_to(&out);
        assert!(s.message.contains("Exported"), "{}", s.message);
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF-"), "a real PDF came out");
        // The page is placed as an image, and the manifest says the artifact is a page.
        assert!(String::from_utf8_lossy(&bytes).contains("taste-manifest.json"));
        let (bitmap, _, _) = taste_curator::artifact::render_pdf_page(&bytes, 1, 1.0).expect("page 1 renders");
        assert!(bitmap.width > 100);
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
        // Plain words by default; the numbers live behind "exact values".
        assert!(s.readout.iter().any(|l| l.starts_with("Size:")), "{:?}", s.readout);
        assert!(s.readout.iter().any(|l| l.starts_with("Separation")), "{:?}", s.readout);
        assert!(!s.readout.iter().any(|l| l.contains("px")), "no bare numbers by default: {:?}", s.readout);
        assert!(s.exact.iter().any(|l| l.contains("px")), "{:?}", s.exact);
        assert!(s.exact.iter().any(|l| l.contains("oklch")), "{:?}", s.exact);
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
        s.toggle_label("breathing");
        s.label_draft = rune::TextEdit::new("margins");
        s.commit_label();
        let (_, n) = s.selected_annotation().unwrap();
        assert_eq!(n.polarity, Polarity::Dislike);
        assert_eq!(n.labels, vec!["breathing".to_string(), "margins".to_string()]);
        s.undo();
        let (_, n) = s.selected_annotation().unwrap();
        assert_eq!(n.labels, vec!["breathing".to_string()], "undo takes back the last label only");
        s.toggle_label("breathing");
        assert!(s.selected_annotation().unwrap().1.labels.is_empty(), "a second press takes it off");
        assert_eq!(s.label_draft.text, "", "the field empties after adding a word");
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
        let out = scratch("export.pdf");
        s.export_to(&out);
        assert!(s.message.contains("Exported"), "{}", s.message);
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        let (bitmap, _, _) = taste_curator::artifact::render_pdf_page(&bytes, 1, 1.0).expect("page 1 renders");
        assert!(bitmap.width > 100);
    }

    /// The reported bug: markings visibly slid off the picture while the wheel zoomed. Both are
    /// placed from one camera rect, so a marking's position *relative to the picture* must not
    /// move at all as the zoom changes, however many wheel steps happen.
    #[test]
    fn markings_stay_locked_to_the_picture_through_a_zoom() {
        let mut s = seeded();
        let marking = NormRect::new(0.25, 0.3, 0.6, 0.7);
        let place = |s: &App| {
            let f = crate::canvas::frame(s);
            let x = (s.to_screen([marking.x0, marking.y0])[0] - f.at[0]) / f.size[0];
            let y = (s.to_screen([marking.x0, marking.y0])[1] - f.at[1]) / f.size[1];
            (x, y, f)
        };
        let (x0, y0, _) = place(&s);
        for dy in [-100.0, -100.0, 40.0, -60.0, 120.0] {
            let anchor = [s.viewport[0] * 0.37, s.viewport[1] * 0.61];
            s.wheel(gestures::Wheel { dx: 0.0, dy, anchor, fine: false, alt: false });
            let (x, y, f) = place(&s);
            assert!((x - x0).abs() < 1e-4 && (y - y0).abs() < 1e-4,
                "after dy {dy} the marking sits at {x:.4},{y:.4} instead of {x0:.4},{y0:.4} ({f:?})");
        }
        assert!(s.camera.zoom > 1.0, "the zoom really did change");
    }

    #[test]
    fn region_and_zoom_round_trip_through_the_camera() {
        let s = seeded();
        let p = [0.25, 0.75];
        let back = s.to_artifact(s.to_screen(p));
        assert!((back[0] - p[0]).abs() < 1e-3 && (back[1] - p[1]).abs() < 1e-3);
        let _ = NormRect::new(0.0, 0.0, 1.0, 1.0);
    }

    #[test]
    fn the_wheel_zooms_about_the_pointer_and_ctrl_fines_it() {
        let mut s = seeded();
        let anchor = [300.0, 220.0];
        let before = s.to_artifact(anchor);
        s.wheel(gestures::Wheel { dx: 0.0, dy: -100.0, anchor, fine: false, alt: false });
        assert!(s.camera.zoom > 1.0, "a wheel up zooms in");
        let after = s.to_artifact(anchor);
        assert!((after[0] - before[0]).abs() < 1e-3 && (after[1] - before[1]).abs() < 1e-3,
            "the point under the pointer stays put: {before:?} then {after:?}");

        s.fit();
        s.wheel(gestures::Wheel { dx: 0.0, dy: 100.0, anchor, fine: false, alt: false });
        assert!(s.camera.zoom < 1.0, "a wheel down zooms out");

        s.fit();
        s.wheel(gestures::Wheel { dx: 0.0, dy: -100.0, anchor, fine: true, alt: false });
        let fine = s.camera.zoom;
        s.fit();
        s.wheel(gestures::Wheel { dx: 0.0, dy: -100.0, anchor, fine: false, alt: false });
        assert!(fine > 1.0 && fine < s.camera.zoom, "Ctrl zooms the same way, further in is further out: {fine} vs {}", s.camera.zoom);
    }

    #[test]
    fn alt_and_a_sideways_wheel_move_the_picture_without_zooming() {
        let mut s = seeded();
        let pan = s.camera.pan;
        s.wheel(gestures::Wheel { dx: 0.0, dy: -40.0, anchor: [100.0, 100.0], fine: false, alt: true });
        assert_eq!(s.camera.zoom, 1.0, "Alt never zooms");
        assert!(s.camera.pan[1] < pan[1], "Alt moves the picture with the wheel");
        s.fit();
        s.wheel(gestures::Wheel { dx: -30.0, dy: 0.0, anchor: [100.0, 100.0], fine: false, alt: false });
        assert_eq!(s.camera.zoom, 1.0, "a sideways wheel does not zoom");
        assert!(s.camera.pan[0] < 0.0, "it moves the picture sideways");
    }

    #[test]
    fn the_middle_button_pans_without_picking_a_marking() {
        let mut s = seeded();
        let id = s.project.artifacts[0].id;
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.2, 0.2], current: [0.6, 0.6], points: vec![] };
        s.commit_draft();
        let marking = s.selected.unwrap();
        s.deselect();
        assert_eq!(s.active, Some(id));

        // The marking is on screen somewhere; press right on it with the middle button.
        let on_it = s.to_screen([0.4, 0.4]);
        s.canvas_press(on_it, true);
        assert!(matches!(s.mode, Mode::Pan { .. }), "the middle button starts a pan");
        s.canvas_drag([on_it[0] + 40.0, on_it[1] + 25.0]);
        s.canvas_release([on_it[0] + 40.0, on_it[1] + 25.0]);
        assert!(s.selected.is_none(), "a middle drag never picks the marking");
        assert!(s.camera.pan[0] > 0.0 && s.camera.pan[1] > 0.0, "the picture moved: {:?}", s.camera.pan);
        assert!(s.project.artifacts[0].annotation(marking).is_some(), "the marking is still there");

        // A plain click afterwards still selects it.
        let on_it = s.to_screen([0.4, 0.4]);
        s.canvas_press(on_it, false);
        s.canvas_release(on_it);
        assert_eq!(s.selected, Some(marking), "a click inside it picks it up");
    }

    #[test]
    fn a_select_drag_pans_past_the_slop_but_a_click_picks() {
        let mut s = seeded();
        s.tool = Tool::Select;
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.2, 0.2], current: [0.6, 0.6], points: vec![] };
        s.commit_draft();
        let marking = s.selected.unwrap();
        s.deselect();
        let press = s.to_screen([0.4, 0.4]);

        // A shaky press inside the marking is still a click.
        s.canvas_press(press, false);
        assert!(matches!(s.mode, Mode::Select { .. }), "the select tool waits for the release");
        assert!(!s.canvas_drag([press[0] + 1.5, press[1] - 1.5]), "inside the slop nothing moves");
        s.canvas_release([press[0] + 1.5, press[1] - 1.5]);
        assert_eq!(s.selected, Some(marking), "a click within the slop picks it");

        // Dragging from the same spot past the slop moves the picture and picks nothing.
        s.deselect();
        let pan = s.camera.pan;
        s.canvas_press(press, false);
        assert!(s.canvas_drag([press[0] + 30.0, press[1] + 20.0]), "past the slop the press pans");
        s.canvas_drag([press[0] + 50.0, press[1] + 40.0]);
        let moved = [s.camera.pan[0] - pan[0], s.camera.pan[1] - pan[1]];
        assert!((moved[0] - 50.0).abs() < 1e-3 && (moved[1] - 40.0).abs() < 1e-3,
            "the picture follows the pointer from where it went down: {moved:?}");
        s.canvas_release([press[0] + 50.0, press[1] + 40.0]);
        assert!(s.selected.is_none(), "panning with select never picks");
    }

    #[test]
    fn clicking_past_a_pan_still_selects() {
        let mut s = seeded();
        s.tool = Tool::Select;
        s.mode = Mode::Draw { shape: Shape::Rect, start: [0.2, 0.2], current: [0.6, 0.6], points: vec![] };
        s.commit_draft();
        let marking = s.selected.unwrap();
        s.deselect();
        // Pan the picture far, then click the marking where it is now.
        let start = s.to_screen([0.4, 0.4]);
        s.canvas_press([start[0] - 200.0, start[1] - 120.0], true);
        s.canvas_drag([start[0] - 40.0, start[1] - 20.0]);
        s.canvas_release([start[0] - 40.0, start[1] - 20.0]);
        assert!(s.selected.is_none());
        let now = s.to_screen([0.4, 0.4]);
        s.canvas_press(now, false);
        s.canvas_release(now);
        assert_eq!(s.selected, Some(marking), "the click after a pan lands where the marking is now");
    }

    #[test]
    fn calibration_records_flinch_advances_probe_and_undoes() {
        let mut s = seeded();
        assert_eq!(s.workspace, Workspace::Calibrate);
        assert!(s.probe_preview.is_some(), "opening an image prepares the first counterfactual probe");
        let first = s.active_probe.unwrap();
        assert_eq!(first.0, ProbeAxis::Warmth);
        assert_eq!(first.1, ProbeDirection::Negative);

        s.judge_probe(FlinchVerdict::LostIt);
        assert_eq!(s.project.probes.len(), 1);
        let second = s.active_probe.unwrap();
        assert_eq!(second.0, ProbeAxis::Warmth);
        assert_eq!(second.1, ProbeDirection::Positive, "opposite direction is bracketed next");

        s.judge_probe(FlinchVerdict::StillHasIt);
        let third = s.active_probe.unwrap();
        assert_eq!(third.0, ProbeAxis::Contrast);

        s.judge_probe(FlinchVerdict::DontCare);
        let fourth = s.active_probe.unwrap();
        assert_eq!(fourth.0, ProbeAxis::Chroma, "DontCare skips the opposite direction");

        s.undo();
        assert_eq!(s.project.probes.len(), 2);
        assert_eq!(s.active_probe.unwrap().0, ProbeAxis::Contrast);
    }

    #[test]
    fn contrastive_pair_and_target_export_into_envelope() {
        let mut s = seeded();
        let first_id = s.active.unwrap();
        // Import a second artifact so we can pair anchor against foil.
        let path2 = scratch("foil.png");
        let img = image::RgbaImage::from_fn(64, 64, |_, _| image::Rgba([30, 30, 38, 255]));
        img.save(&path2).unwrap();
        s.import(&path2);
        let foil_id = s.active.unwrap();
        s.open_artifact(first_id);

        s.target_draft = rune::TextEdit::new("personal reading list page");
        s.sync_target();
        s.pair_note = rune::TextEdit::new("keep the calm ground, not the heavy dark surface");
        s.commit_pair(foil_id);
        s.judge_probe(FlinchVerdict::LostIt);

        let env = calibrate::build_envelope(&s.project, &|id, page| {
            s.pages.get(&(id, page.unwrap_or(1))).map(|p| p.bitmap.clone())
        });
        assert_eq!(env.target, "personal reading list page");
        assert_eq!(env.hard_invariants.len(), 1);
        assert_eq!(env.contrastive_pairs.len(), 1);

        let out = scratch("envelope.pdf");
        s.export_to(&out);
        let bytes = std::fs::read(&out).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        assert!(raw.contains("taste-manifest.json"));
        let (bitmap, _, _) = taste_curator::artifact::render_pdf_page(&bytes, 1, 1.0).expect("page 1 renders");
        assert!(bitmap.width > 100);
    }
}

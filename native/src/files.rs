//! File dialogs and the export. The native dialog is scripted in headless runs, so the same code
//! paths are exercised by `--snap` and by a real window.

use rune::{El, FileDialogOptions, Tx};
use std::path::PathBuf;
use taste_curator::model::ArtifactSource;
use taste_curator::pdf::{self, ExportInput};
use taste_curator::telemetry::Bitmap;

use crate::app::App;

/// Called from the Import button through `Tx`.
pub fn open(s: &mut App, tx: &mut Tx) {
    tx.open_file(
        FileDialogOptions::default().filter(
            "References",
            &["png", "jpg", "jpeg", "gif", "pdf", "rs", "ts", "tsx", "js", "py", "css", "html", "md", "txt", "json", "toml"],
        ),
        |s: &mut App, result: rune::FileDialogResult, _tx| match result {
            Ok(Some(path)) => s.import(&path),
            Ok(None) => {}
            Err(e) => s.message = e.0,
        },
    );
}

pub fn save(s: &mut App, tx: &mut Tx) {
    let name = format!("{}.pdf", slug(&s.project.name));
    tx.save_file(
        FileDialogOptions { file_name: Some(name), ..FileDialogOptions::default().filter("PDF", &["pdf"]) },
        |s: &mut App, result: rune::FileDialogResult, _tx| match result {
            Ok(Some(path)) => s.export_to(&path),
            Ok(None) => {}
            Err(e) => s.message = e.0,
        },
    );
}

fn slug(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() { "taste".into() } else { trimmed }
}

/// Every page of every artifact, decoded fresh: the exporter needs full-resolution pixels and
/// must not trust anything cached for drawing.
fn bitmaps(project: &taste_curator::model::Project) -> impl Fn(u32, Option<u32>) -> Option<Bitmap> + '_ {
    move |id, page| {
        let artifact = project.artifact(id)?;
        let source = match &artifact.source {
            ArtifactSource::Image { path, .. } => ArtifactSource::Image { path: path.clone(), width: 0, height: 0 },
            other => other.clone(),
        };
        if !std::path::Path::new(source.path()).exists() {
            return None;
        }
        let substrate = taste_curator::artifact::load(&source).ok()?;
        substrate.page_bitmap(page.unwrap_or(1)).cloned()
    }
}

impl App {
    pub fn export_to(&mut self, path: &std::path::Path) {
        let bitmaps = bitmaps(&self.project);
        let manifest = taste_curator::manifest::build(&self.project, &bitmaps);
        match pdf::export(&ExportInput { manifest: &manifest, bitmaps: &bitmaps }) {
            Ok(result) => match std::fs::write(path, &result.bytes) {
                Ok(()) => {
                    self.message = format!(
                        "Exported {} readings and {} conflicts to {}",
                        manifest.readings.len(),
                        manifest.conflicts.len(),
                        path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
                    );
                }
                Err(e) => self.message = format!("{}: {e}", path.display()),
            },
            Err(e) => self.message = e,
        }
    }
}

/// The export button in the top bar.
pub fn export_button(s: &App, t: rune::tok::Theme) -> El {
    use rune::{text, tok::{radius, type_}};
    text(if s.exporting { "Exporting…" } else { "Export PDF" })
        .key("export")
        .role("button")
        .focusable(true)
        .font(type_::LABEL)
        .color(rune::Color::BLACK)
        .px(10.0)
        .py(4.0)
        .radius(radius::SM)
        .fill(if s.project.artifacts.is_empty() { t.control_off } else { t.knob })
        .hover(t.hover)
        .on_press_tx(|s: &mut App, tx: &mut Tx| {
            if s.project.artifacts.is_empty() {
                s.message = "Import a reference first.".into();
                return;
            }
            save(s, tx);
        })
}

/// Kept for headless runs: export without a dialog.
pub fn export_to_path(s: &mut App, path: PathBuf) {
    s.export_to(&path);
}

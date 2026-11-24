//! Where the project lives between launches. One JSON file in the app's data folder, written
//! after each change and read at startup, so closing the window is never a surprise.

use std::path::PathBuf;

use taste_curator::model::Project;

use crate::app::App;

const APP_ID: &str = "dev.tastecurator.app";

fn path() -> Option<PathBuf> {
    let dirs = rune::dirs::AppDirs::new(APP_ID)?;
    Some(dirs.data().join("project.json"))
}

/// Load the last project, if there is one. Artifacts whose files went away are dropped.
pub fn load(s: &mut App) {
    let Some(path) = path() else { return };
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let Ok(mut project) = serde_json::from_str::<Project>(&text) else { return };
    let before = project.artifacts.len();
    project.artifacts.retain(|a| std::path::Path::new(a.source.path()).exists());
    s.project = project;
    s.target_draft = rune::TextEdit::new(&s.project.target);
    if let Some(id) = s.project.artifacts.first().map(|a| a.id) {
        s.open_artifact(id);
    }
    if s.project.artifacts.len() != before {
        s.message = "Some references moved or were deleted; they were left out.".into();
    }
}

/// Write the project out. Called from `poll`, so it only writes when the JSON changed.
pub fn save(s: &mut App) {
    let Some(path) = path() else { return };
    let Ok(text) = serde_json::to_string_pretty(&s.project) else { return };
    if text == s.last_saved {
        return;
    }
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&path, &text) {
        Ok(()) => {
            s.last_saved = text;
            s.saved_to = Some(path);
        }
        Err(e) => s.message = format!("Could not save your work: {e}"),
    }
}

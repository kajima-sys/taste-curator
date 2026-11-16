//! The engine the UI talks to. Reads a project JSON, writes the PDF, and prints the telemetry
//! manifest on stdout so the page can show exactly what will be embedded.
//!
//!   taste-engine export <project.json> <out.pdf>
//!   taste-engine manifest <project.json>
//!
//! It never mutates the project: paths in the JSON are read, nothing is written back.

use std::path::Path;

use taste_curator::manifest;
use taste_curator::model::{ArtifactSource, Project};
use taste_curator::pdf::{self, ExportInput};
use taste_curator::telemetry::Bitmap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("manifest") => manifest_cmd(args.get(1)),
        Some("export") => export_cmd(args.get(1), args.get(2)),
        _ => Err("usage: taste-engine manifest <project.json> | export <project.json> <out.pdf>".into()),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn read_project(path: Option<&String>) -> Result<Project, String> {
    let path = path.ok_or("missing project path")?;
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// Every artifact's bitmap, loaded once from its file on disk.
fn load_bitmaps(project: &Project) -> impl Fn(u32, Option<u32>) -> Option<Bitmap> + '_ {
    move |id, page| {
        let artifact = project.artifact(id)?;
        let source = match &artifact.source {
            ArtifactSource::Image { path, .. } => {
                ArtifactSource::Image { path: path.clone(), width: 0, height: 0 }
            }
            other => other.clone(),
        };
        if !Path::new(source.path()).exists() { return None; }
        let substrate = taste_curator::artifact::load(&source).ok()?;
        substrate.page_bitmap(page.unwrap_or(1)).cloned()
    }
}

fn manifest_cmd(path: Option<&String>) -> Result<(), String> {
    let project = read_project(path)?;
    let bitmaps = load_bitmaps(&project);
    let m = manifest::build(&project, &bitmaps);
    println!("{}", serde_json::to_string(&m).map_err(|e| e.to_string())?);
    Ok(())
}

fn export_cmd(path: Option<&String>, out: Option<&String>) -> Result<(), String> {
    let project = read_project(path)?;
    let out = out.ok_or("missing output path")?;
    let bitmaps = load_bitmaps(&project);
    let m = manifest::build(&project, &bitmaps);
    let result = pdf::export(&ExportInput { manifest: &m, bitmaps: &bitmaps })?;
    std::fs::write(out, &result.bytes).map_err(|e| format!("{out}: {e}"))?;
    let stats = serde_json::json!({
        "bytes": result.bytes.len(),
        "plates": result.crops.len() + result.overlays.len() + 1,
        "readings": m.readings.len(),
        "conflicts": m.conflicts.len(),
        "summary": m.summary,
    });
    println!("{stats}");
    Ok(())
}

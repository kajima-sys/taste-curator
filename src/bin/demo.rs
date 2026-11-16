//! Builds a sample curated project, exports its PDF, and renders every plate to PNG for the
//! static preview. Run with `cargo run --bin demo`.

use std::path::{Path, PathBuf};

use taste_curator::manifest;
use taste_curator::model::{Annotation, ArtifactSource, Medium, Note, NormRect, Polarity, Project, Scope, Shape, Span, Spectrum};
use taste_curator::pdf::{self, ExportInput};
use taste_curator::telemetry::Bitmap;

const OUT: &str = "seed";
const ARTIFACTS: &str = "seed/artifacts";

fn main() {
    let started = std::time::Instant::now();
    std::fs::create_dir_all(ARTIFACTS).expect("create artifact dir");
    let dashboard = draw_dashboard();
    let poster = draw_poster();
    let swatches = draw_swatches();
    for (name, bitmap) in [("dashboard.png", &dashboard), ("poster.png", &poster), ("swatches.png", &swatches)] {
        save_png(bitmap, &Path::new(ARTIFACTS).join(name));
    }

    let project = sample_project(&dashboard, &poster, &swatches);
    let lookup = |id: u32, page: Option<u32>| -> Option<Bitmap> {
        let _ = page;
        match id {
            1 => Some(dashboard.clone()),
            2 => Some(poster.clone()),
            3 => Some(swatches.clone()),
            _ => None,
        }
    };
    let m = manifest::build(&project, &lookup);
    let result = pdf::export(&ExportInput { manifest: &m, bitmaps: &lookup }).expect("export");
    std::fs::write(format!("{OUT}/taste-curator.pdf"), &result.bytes).expect("write pdf");
    // The project JSON ships beside the images so the app can open the sample on first launch.
    // Paths are written relative to the seed folder's parent, which is what the app rebases from.
    let text = serde_json::to_string_pretty(&project).expect("json");
    std::fs::write(format!("{OUT}/project.json"), text).expect("write project");

    // Start from a clean page directory so a shorter export never leaves stale plates.
    let _ = std::fs::remove_dir_all(Path::new(OUT).join("plates"));
    // Render each plate at 1.6x so the PNGs stay readable when zoomed.
    let mut pages = Vec::new();
    let mut n = 1u32;
    while let Ok((bitmap, _, _)) = taste_curator::artifact::render_pdf_page(&result.bytes, n, 1.6) {
        let name = format!("page-{n:02}.png");
        save_png(&bitmap, &Path::new(OUT).join("plates").join(&name));
        pages.push(name);
        n += 1;
    }

    // The plate order the exporter uses: synthesis, one overview per artifact in first-seen
    // order, then one detail per reading in the manifest's order.
    let mut titles = vec!["Synthesis — the whole read".to_string()];
    let mut seen: Vec<u32> = Vec::new();
    for r in &m.readings {
        if seen.contains(&r.artifact) { continue; }
        seen.push(r.artifact);
        let count = m.readings.iter().filter(|x| x.artifact == r.artifact).count();
        titles.push(format!("Overview — {} · {} marking{}", r.artifact_name, count, if count == 1 { "" } else { "s" }));
    }
    for r in &m.readings {
        titles.push(format!("Detail {} — {} · {} · {}", r.annotation, r.polarity.label(), r.scope.label(), r.medium.label()));
    }
    assert_eq!(titles.len(), pages.len(), "plate titles must match the rendered pages");

    let plates: Vec<String> = pages.iter().map(|name| format!("{OUT}/plates/{name}")).collect();
    let index = serde_json::json!({
        "titles": titles,
        "summary": m.summary,
        "conflicts": m.conflicts.iter().map(|c| c.summary.clone()).collect::<Vec<_>>(),
        "pdf": format!("{OUT}/taste-curator.pdf"),
        "plates": plates,
    });
    std::fs::write(format!("{OUT}/plates.json"), serde_json::to_vec_pretty(&index).expect("json")).expect("write plates.json");
    println!("{} plates, {} readings, {} bytes of PDF, {:.1}s", pages.len(), m.readings.len(), result.bytes.len(), started.elapsed().as_secs_f32());
}

fn note(text: &str) -> Vec<Note> { vec![Note { text: text.into() }] }

fn sample_project(dashboard: &Bitmap, poster: &Bitmap, swatches: &Bitmap) -> Project {
    let mut p = Project { name: "Studio handoff — spring".into(), ..Project::default() };
    let dash = p.add_artifact("dashboard.png".into(), ArtifactSource::Image { path: format!("{ARTIFACTS}/dashboard.png"), width: dashboard.width, height: dashboard.height });
    let post = p.add_artifact("poster.png".into(), ArtifactSource::Image { path: format!("{ARTIFACTS}/poster.png"), width: poster.width, height: poster.height });
    let swat = p.add_artifact("swatches.png".into(), ArtifactSource::Image { path: format!("{ARTIFACTS}/swatches.png"), width: swatches.width, height: swatches.height });

    // 4. A real web page, rendered to pixels in pure Rust. Its size is the laid-out page height,
    //    measured the same way the app measures it when the file is opened.
    let page = Path::new(OUT).join("landing.html");
    if page.is_file() {
        let size = taste_curator::html::render_file(&page, 0, &taste_curator::html::bundled_fonts())
            .map(|b| (b.width, b.height))
            .unwrap_or((1280, 258));
        p.add_artifact(
            "landing.html".into(),
            ArtifactSource::Html { path: format!("{OUT}/landing.html"), width: size.0, height: size.1 },
        );
    }

    // 1. The accent green, liked for colour.
    let mut a = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.655, 0.60, 0.775, 0.665));
    a.scope = Scope::Color;
    a.medium = Medium::Ui;
    a.set_chip(Spectrum::WarmClinical, Some(0.28));
    a.set_chip(Spectrum::CrispSoft, Some(0.35));
    a.notes = note("This green exactly — not the minty one. It holds up on white without shouting.");
    p.artifact_mut(dash).unwrap().annotations.push(a);

    // 2. Airy card spacing, liked. Same scope and medium as the dislike below, opposite half of
    //    the tight-airy dial: an honest conflict the summary has to name.
    let mut a = Annotation::new(2, Polarity::Like, Shape::Rect, NormRect::new(0.255, 0.175, 0.60, 0.53));
    a.scope = Scope::SpacingDensity;
    a.medium = Medium::Ui;
    a.set_chip(Spectrum::TightAiry, Some(0.86));
    a.notes = note("Room between the cards and around their content. I want this everywhere in the app shell.");
    p.artifact_mut(dash).unwrap().annotations.push(a);

    // 3. The cramped sidebar table, disliked.
    let mut a = Annotation::new(3, Polarity::Dislike, Shape::Rect, NormRect::new(0.02, 0.30, 0.165, 0.92));
    a.scope = Scope::SpacingDensity;
    a.medium = Medium::Ui;
    a.set_chip(Spectrum::TightAiry, Some(0.14));
    a.notes = note("Rows stacked with no air — every label fights the next one.");
    p.artifact_mut(dash).unwrap().annotations.push(a);

    // 4. Dense data table, liked (freehand outline).
    let mut a = Annotation::new(4, Polarity::Like, Shape::Freehand, NormRect::new(0.265, 0.66, 0.615, 0.86));
    a.medium = Medium::Web;
    a.scope = Scope::SpacingDensity;
    a.points = vec![
        [0.268, 0.668], [0.40, 0.662], [0.545, 0.671], [0.610, 0.700],
        [0.607, 0.812], [0.470, 0.852], [0.320, 0.845], [0.270, 0.805], [0.268, 0.668],
    ];
    a.set_chip(Spectrum::TightAiry, Some(0.22));
    a.notes = note("But a dense table is fine when the rhythm is regular. Tight is not the problem; ragged is.");
    p.artifact_mut(dash).unwrap().annotations.push(a);

    // 5. Poster headline type, liked.
    let mut a = Annotation::new(5, Polarity::Like, Shape::Rect, NormRect::new(0.075, 0.185, 0.615, 0.325));
    a.scope = Scope::Typography;
    a.medium = Medium::Illustration;
    a.set_chip(Spectrum::CrispSoft, Some(0.18));
    a.notes = note("Heavy, tight tracking, no softness in the letterforms. This is the tone for editorial pieces.");
    p.artifact_mut(post).unwrap().annotations.push(a);

    // 6. Glossy gradient, disliked.
    let mut a = Annotation::new(6, Polarity::Dislike, Shape::Ellipse, NormRect::new(0.60, 0.42, 0.93, 0.66));
    a.scope = Scope::SurfaceTexture;
    a.medium = Medium::Illustration;
    a.set_chip(Spectrum::FlatLayered, Some(0.82));
    a.notes = note("Plasticky sheen over a flat field. Reads as a template.");
    p.artifact_mut(post).unwrap().annotations.push(a);

    // 7. Verbatim code selection from a text artifact's span, carried as text.
    let mut a = Annotation::new(7, Polarity::Like, Shape::TextSpan, NormRect::new(0.075, 0.60, 0.60, 0.645));
    a.scope = Scope::Typography;
    a.medium = Medium::Web;
    a.spans = vec![Span {
        text: "letter-spacing: -0.02em;\ntext-wrap: balance;".into(),
        start: 0,
        end: 47,
        font: "IBM Plex Mono 13".into(),
    }];
    a.notes = note("Keep this pairing: slightly negative tracking with balanced wraps.");
    p.artifact_mut(post).unwrap().annotations.push(a);

    // 8. Contrast marking: the clinical swatch against the warm one.
    let mut a = Annotation::new(8, Polarity::Contrast, Shape::Rect, NormRect::new(0.09, 0.10, 0.53, 0.46));
    a.scope = Scope::Color;
    a.medium = Medium::Web;
    a.set_chip(Spectrum::WarmClinical, Some(0.76));
    a.notes = note("Take the restraint of the top row, not its coldness — warm the neutrals by about ten degrees.");
    p.artifact_mut(swat).unwrap().annotations.push(a);

    // Annotations above are built with explicit ids rather than `add_annotation`, so the counter
    // has to be advanced here; otherwise the app would hand a new marking an id already in use.
    let highest = p.artifacts.iter().flat_map(|a| a.annotations.iter().map(|n| n.id)).max().unwrap_or(0);
    p.next_annotation = highest + 1;

    p
}

// ---- sample artwork -------------------------------------------------------------------

fn save_png(bitmap: &Bitmap, path: &Path) {
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent).expect("mkdir"); }
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), bitmap.width, bitmap.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().expect("png header").write_image_data(&bitmap.rgba).expect("png data");
}

struct Canvas { w: u32, h: u32, px: Vec<u8> }

impl Canvas {
    fn new(w: u32, h: u32, bg: [u8; 3]) -> Canvas {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) { px.extend_from_slice(&[bg[0], bg[1], bg[2], 255]); }
        Canvas { w, h, px }
    }

    fn rect(&mut self, x: f32, y: f32, rw: f32, rh: f32, color: [u8; 3]) {
        let (x0, y0) = (x as i64, y as i64);
        let (x1, y1) = ((x + rw) as i64, (y + rh) as i64);
        for yy in y0.max(0)..y1.min(self.h as i64) {
            for xx in x0.max(0)..x1.min(self.w as i64) {
                let i = ((yy as u32 * self.w + xx as u32) * 4) as usize;
                self.px[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
            }
        }
    }

    fn rounded(&mut self, x: f32, y: f32, rw: f32, rh: f32, r: f32, color: [u8; 3]) {
        let (x0, y0, x1, y1) = (x as i64, y as i64, (x + rw) as i64, (y + rh) as i64);
        for yy in y0.max(0)..y1.min(self.h as i64) {
            for xx in x0.max(0)..x1.min(self.w as i64) {
                let cx = xx as f32 + 0.5;
                let cy = yy as f32 + 0.5;
                let dx = (x + r - cx).max(0.0).max(cx - (x + rw - r));
                let dy = (y + r - cy).max(0.0).max(cy - (y + rh - r));
                if dx * dx + dy * dy <= r * r {
                    let i = ((yy as u32 * self.w + xx as u32) * 4) as usize;
                    self.px[i..i + 4].copy_from_slice(&[color[0], color[1], color[2], 255]);
                }
            }
        }
    }

    fn gradient(&mut self, top: [u8; 3], bottom: [u8; 3], sheen: bool) {
        for y in 0..self.h {
            let t = y as f32 / self.h as f32;
            for x in 0..self.w {
                let glow = if sheen {
                    let dx = x as f32 / self.w as f32 - 0.72;
                    let dy = y as f32 / self.h as f32 - 0.36;
                    (1.0 - (dx * dx + dy * dy).sqrt() * 2.6).clamp(0.0, 1.0)
                } else { 0.0 };
                let i = ((y * self.w + x) * 4) as usize;
                for c in 0..3 {
                    let base = top[c] as f32 + (bottom[c] as f32 - top[c] as f32) * t;
                    self.px[i + c] = (base + glow * 42.0).clamp(0.0, 255.0) as u8;
                }
                self.px[i + 3] = 255;
            }
        }
    }

    fn bitmap(self) -> Bitmap { Bitmap { width: self.w, height: self.h, rgba: self.px } }
}

fn draw_dashboard() -> Bitmap {
    let mut c = Canvas::new(1200, 780, [247, 247, 245]);
    // Sidebar.
    c.rect(0.0, 0.0, 210.0, 780.0, [20, 22, 26]);
    // Cramped rows in the sidebar: 14px type at 22px pitch, no padding.
    for i in 0..22 {
        let y = 236.0 + i as f32 * 24.0;
        c.rect(18.0, y, 96.0 + (i % 4) as f32 * 18.0, 7.0, if i % 7 == 0 { [150, 156, 168] } else { [86, 92, 104] });
        c.rect(150.0, y, 30.0, 7.0, [60, 66, 76]);
    }
    // Header.
    c.rect(210.0, 0.0, 990.0, 62.0, [255, 255, 255]);
    c.rect(210.0, 61.0, 990.0, 1.0, [228, 228, 232]);
    c.rect(240.0, 24.0, 132.0, 12.0, [30, 32, 38]);
    // Cards with generous spacing.
    let cards = [(300.0f32, 132.0, [255, 255, 255]), (640.0, 132.0, [255, 255, 255]), (300.0, 400.0, [255, 255, 255])];
    for (x, y, fill) in cards {
        c.rounded(x, y, 300.0, 228.0, 14.0, fill);
        c.rect(x + 28.0, y + 32.0, 74.0, 8.0, [172, 176, 186]);
        c.rect(x + 28.0, y + 54.0, 160.0, 16.0, [28, 30, 36]);
        c.rect(x + 28.0, y + 150.0, 244.0, 6.0, [206, 208, 214]);
        c.rect(x + 28.0, y + 164.0, 200.0, 6.0, [206, 208, 214]);
    }
    // The accent green: a wide button on the first card.
    c.rounded(328.0, 468.0, 144.0, 50.0, 10.0, [42, 138, 92]);
    c.rect(352.0, 488.0, 96.0, 10.0, [240, 248, 244]);
    // A second accent, for contrast in the telemetry.
    c.rounded(640.0, 132.0, 300.0, 92.0, 14.0, [86, 74, 214]);
    // Dense data table with a regular rhythm.
    c.rounded(300.0, 500.0, 900.0, 250.0, 14.0, [255, 255, 255]);
    for row in 0..6 {
        let y = 528.0 + row as f32 * 34.0;
        c.rect(324.0, y, 180.0, 8.0, [60, 64, 74]);
        c.rect(600.0, y, 120.0, 8.0, [140, 144, 154]);
        c.rect(860.0, y, 90.0, 8.0, [140, 144, 154]);
        c.rect(1080.0, y, 60.0, 8.0, [42, 138, 92]);
    }
    c.bitmap()
}

fn draw_poster() -> Bitmap {
    let mut c = Canvas::new(900, 1200, [255, 255, 255]);
    c.gradient([246, 238, 226], [223, 206, 186], true);
    // Heavy headline blocks: the liked type, tight tracking.
    for (i, width) in [520.0f32, 470.0, 560.0, 300.0].iter().enumerate() {
        c.rect(68.0, 222.0 + i as f32 * 62.0, *width, 46.0, [26, 24, 22]);
    }
    // Smaller caption type.
    c.rect(68.0, 512.0, 300.0, 10.0, [92, 84, 74]);
    c.rect(68.0, 532.0, 240.0, 10.0, [92, 84, 74]);
    // A clinical white field, disliked for its sheen handled below.
    c.rounded(540.0, 504.0, 300.0, 240.0, 8.0, [250, 250, 252]);
    // The glossy highlight that the ellipse marks.
    for r in 0..90 {
        let t = 1.0 - r as f32 / 90.0;
        let v = (255.0 * (1.0 - t * 0.55)).clamp(0.0, 255.0) as u8;
        c.rounded(700.0 - r as f32, 560.0 - r as f32 * 0.6, (r * 2) as f32, (r as f32 * 1.2) as f32, 4.0, [v, v, (v as f32 * 1.02).min(255.0) as u8]);
    }
    // A quiet caption near the bottom.
    c.rect(68.0, 880.0, 420.0, 8.0, [120, 110, 98]);
    c.rect(68.0, 900.0, 360.0, 8.0, [120, 110, 98]);
    c.bitmap()
}

fn draw_swatches() -> Bitmap {
    let mut c = Canvas::new(720, 460, [252, 252, 250]);
    let colors = [
        [232, 84, 62], [238, 158, 66], [216, 196, 88], [92, 168, 118],
        [58, 118, 196], [96, 82, 186], [168, 92, 168], [58, 60, 66],
    ];
    for (i, color) in colors.iter().enumerate() {
        let (x, y) = (i % 4, i / 4);
        c.rounded(36.0 + x as f32 * 166.0, 46.0 + y as f32 * 186.0, 150.0, 170.0, 6.0, *color);
    }
    c.bitmap()
}

#[allow(dead_code)]
fn _unused(_: PathBuf) {}

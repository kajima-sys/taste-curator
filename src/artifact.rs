//! Loading a curator's files. Images decode to their original pixels; code and text keep their
//! verbatim bytes; PDFs are parsed and each page is rasterized in pure Rust with `hayro` while
//! the page's own text and vector content stays readable from the content stream. Nothing here
//! ever modifies a source file.

use std::path::{Path, PathBuf};

use crate::model::ArtifactSource;
use crate::telemetry::Bitmap;

/// A rasterized page of a PDF, at a fixed 2× device scale so crops and telemetry are stable.
#[derive(Clone, Debug)]
pub struct PdfPage {
    pub number: u32,
    /// The page's bitmap, at `RASTER_SCALE` relative to the page's point size.
    pub bitmap: Bitmap,
    /// Page size in points, upright (rotation applied).
    pub width_pt: f32,
    pub height_pt: f32,
    /// Every text-showing operator's verbatim string on this page, in content order.
    pub text: String,
    /// The page's drawn vector segments in normalized upright page space, for inspectability
    /// and for the object-pick tool.
    pub vectors: Vec<Vec<[f32; 2]>>,
}

pub const RASTER_SCALE: f32 = 2.0;

/// A loaded artifact's unaltered substrate.
#[derive(Clone, Debug)]
pub enum Substrate {
    /// Full-resolution pixels, exactly as decoded from the imported file.
    Image(Bitmap),
    /// Verbatim source bytes as UTF-8 text (lossy only for genuinely non-UTF8 bytes).
    Text { text: String, language: String },
    Pdf { pages: Vec<PdfPage> },
}

impl Substrate {
    /// The bitmap for a page of this artifact (`page` is 1-based and ignored for images).
    pub fn page_bitmap(&self, page: u32) -> Option<&Bitmap> {
        match self {
            Substrate::Image(b) => Some(b),
            Substrate::Text { .. } => None,
            Substrate::Pdf { pages } => pages.iter().find(|p| p.number == page).map(|p| &p.bitmap),
        }
    }

    pub fn page_count(&self) -> u32 {
        match self {
            Substrate::Pdf { pages } => pages.len().max(1) as u32,
            _ => 1,
        }
    }
}

/// The importer's judgement about a file, from its bytes rather than its extension.
pub fn sniff(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"%PDF-") { return "pdf"; }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { return "png"; }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { return "jpeg"; }
    if bytes.starts_with(b"GIF8") { return "gif"; }
    "text"
}

/// The artifact's kind, without loading it.
pub fn describe(path: &Path) -> Result<(String, ArtifactSource), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "artifact".into());
    let abs = path.display().to_string();
    let source = match sniff(&bytes) {
        "pdf" => {
            let pages = pdf_page_count(&bytes)?;
            ArtifactSource::Pdf { path: abs, pages }
        }
        "png" | "jpeg" | "gif" => {
            let b = Bitmap::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            ArtifactSource::Image { path: abs, width: b.width, height: b.height }
        }
        _ => ArtifactSource::Text { path: abs, language: guess_language(path) },
    };
    Ok((name, source))
}

fn guess_language(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "swift" => "swift",
        "css" => "css",
        "html" => "html",
        "json" => "json",
        "md" | "markdown" => "markdown",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "sh" => "shell",
        _ => "text",
    }
    .to_string()
}

/// Load everything an artifact needs to be shown and measured.
pub fn load(source: &ArtifactSource) -> Result<Substrate, String> {
    let path = PathBuf::from(source.path());
    let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    match source {
        ArtifactSource::Image { .. } => Ok(Substrate::Image(Bitmap::decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))?)),
        ArtifactSource::Text { language, .. } => {
            Ok(Substrate::Text { text: String::from_utf8_lossy(&bytes).into_owned(), language: language.clone() })
        }
        ArtifactSource::Pdf { .. } => Ok(Substrate::Pdf { pages: load_pdf(&bytes)? }),
    }
}

fn pdf_page_count(bytes: &[u8]) -> Result<u32, String> {
    let pdf = hayro::hayro_syntax::Pdf::new(bytes.to_vec()).map_err(|e| format!("pdf: {e:?}"))?;
    Ok(pdf.pages().len() as u32)
}

/// Rasterize every page and read its verbatim text and vector segments.
pub fn load_pdf(bytes: &[u8]) -> Result<Vec<PdfPage>, String> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::hayro_syntax::Pdf;
    use hayro::{render, PixmapSettings, RenderCache, RenderSettings};

    let pdf = Pdf::new(bytes.to_vec()).map_err(|e| format!("pdf: {e:?}"))?;
    let cache = RenderCache::new();
    let settings = InterpreterSettings::default();
    let render_settings = RenderSettings::default();
    let mut out = Vec::new();
    for (i, page) in pdf.pages().iter().enumerate() {
        let (pw, ph) = page.render_dimensions();
        let pixmap_settings = PixmapSettings { x_scale: RASTER_SCALE, y_scale: RASTER_SCALE, ..PixmapSettings::default() };
        let pix = render(page, &cache, &settings, &render_settings, &pixmap_settings);
        let bitmap = pixmap_to_bitmap(&pix);
        let (text, vectors) = read_page_content(page);
        out.push(PdfPage { number: i as u32 + 1, bitmap, width_pt: pw, height_pt: ph, text, vectors });
    }
    Ok(out)
}

fn pixmap_to_bitmap(pix: &hayro::vello_cpu::Pixmap) -> Bitmap {
    let (w, h) = (pix.width() as u32, pix.height() as u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    // vello_cpu stores premultiplied RGBA8; hayro renders onto a transparent target, so
    // unpremultiply to the straight-alpha form the rest of the app expects.
    for p in pix.data() {
        let a = p.a as f32 / 255.0;
        let un = |v: u8| if a > 0.0 { (v as f32 / a).clamp(0.0, 255.0) as u8 } else { 0 };
        rgba.extend_from_slice(&[un(p.r), un(p.g), un(p.b), p.a]);
    }
    Bitmap { width: w, height: h, rgba }
}

/// The page's text-showing operators in order, and the segments of its path operators, both in
/// normalized upright page space. This is what makes a PDF selection's source inspectable
/// instead of only its raster.
fn read_page_content(page: &hayro::hayro_syntax::page::Page<'_>) -> (String, Vec<Vec<[f32; 2]>>) {
    use hayro::hayro_syntax::content::ops::TypedInstruction as Op;

    let (pw, ph) = page.render_dimensions();
    let mut text = String::new();
    let mut vectors: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut current: Vec<[f32; 2]> = Vec::new();
    let inv = |x: f32, y: f32| -> [f32; 2] {
        [(x / pw.max(1e-6)).clamp(0.0, 1.0), (1.0 - y / ph.max(1e-6)).clamp(0.0, 1.0)]
    };
    let flush = |current: &mut Vec<[f32; 2]>, vectors: &mut Vec<Vec<[f32; 2]>>| {
        if current.len() >= 2 { vectors.push(std::mem::take(current)); } else { current.clear(); }
    };
    let mut ops = page.typed_operations();
    while let Some(op) = ops.next() {
        match op {
            Op::MoveTo(m) => {
                flush(&mut current, &mut vectors);
                current.push(inv(m.0.as_f32(), m.1.as_f32()));
            }
            Op::LineTo(l) => current.push(inv(l.0.as_f32(), l.1.as_f32())),
            Op::CubicTo(c) => current.push(inv(c.4.as_f32(), c.5.as_f32())),
            Op::ClosePath(_) => {
                if let (Some(first), Some(last)) = (current.first().copied(), current.last().copied()) {
                    if first != last { current.push(first); }
                }
            }
            Op::RectPath(r) => {
                flush(&mut current, &mut vectors);
                let (x, y, w, h) = (r.0.as_f32(), r.1.as_f32(), r.2.as_f32(), r.3.as_f32());
                current.push(inv(x, y));
                current.push(inv(x + w, y));
                current.push(inv(x + w, y + h));
                current.push(inv(x, y + h));
                current.push(inv(x, y));
                flush(&mut current, &mut vectors);
            }
            Op::ShowText(s) => text.push_str(&String::from_utf8_lossy(s.0.as_bytes())),
            Op::NextLineAndShowText(s) => {
                text.push('\n');
                text.push_str(&String::from_utf8_lossy(s.0.as_bytes()));
            }
            Op::ShowTextWithParameters(s) => {
                text.push('\n');
                text.push_str(&String::from_utf8_lossy(s.2.as_bytes()));
            }
            Op::ShowTexts(a) => {
                for item in a.0.iter::<hayro::hayro_syntax::object::Object>() {
                    if let Ok(s) = hayro::hayro_syntax::object::String::try_from(item) {
                        text.push_str(&String::from_utf8_lossy(s.as_bytes()));
                    }
                }
            }
            _ => {}
        }
    }
    flush(&mut current, &mut vectors);
    // Keep the plate readable: a busy page can have thousands of tiny segments.
    vectors.truncate(4000);
    (text, vectors)
}

/// Rasterize one page of an already-loaded PDF file at an arbitrary scale, for crops that must
/// stay sharp when printed.
pub fn render_pdf_page(bytes: &[u8], page_number: u32, scale: f32) -> Result<(Bitmap, f32, f32), String> {
    use hayro::hayro_interpret::InterpreterSettings;
    use hayro::{render, PixmapSettings, RenderCache, RenderSettings};
    let pdf = hayro::hayro_syntax::Pdf::new(bytes.to_vec()).map_err(|e| format!("pdf: {e:?}"))?;
    let page = pdf.pages().get(page_number.saturating_sub(1) as usize).ok_or("no such page")?;
    let cache = RenderCache::new();
    let settings = InterpreterSettings::default();
    let (pw, ph) = page.render_dimensions();
    let pix = render(page, &cache, &settings, &RenderSettings::default(), &PixmapSettings { x_scale: scale, y_scale: scale, ..PixmapSettings::default() });
    Ok((pixmap_to_bitmap(&pix), pw, ph))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffing_reads_bytes_not_extensions() {
        assert_eq!(sniff(b"%PDF-1.7 something"), "pdf");
        assert_eq!(sniff(&[0x89, b'P', b'N', b'G', 0, 0]), "png");
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpeg");
        assert_eq!(sniff(b"fn main() {}"), "text");
    }

    #[test]
    fn text_loads_verbatim_including_trailing_space() {
        let dir = std::env::temp_dir().join("taste-curator-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("snippet.rs");
        std::fs::write(&path, "let a = 1;\n\nfn f() {}\n").unwrap();
        let (_, source) = describe(&path).unwrap();
        let sub = load(&source).unwrap();
        match sub {
            Substrate::Text { text, language } => {
                assert_eq!(text, "let a = 1;\n\nfn f() {}\n");
                assert_eq!(language, "rust");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_png_keeps_its_exact_pixels() {
        let dir = std::env::temp_dir().join("taste-curator-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("swatch.png");
        let img = image::RgbaImage::from_pixel(7, 5, image::Rgba([12, 200, 90, 255]));
        img.save(&path).unwrap();
        let (name, source) = describe(&path).unwrap();
        assert_eq!(name, "swatch.png");
        assert!(matches!(source, ArtifactSource::Image { width: 7, height: 5, .. }));
        match load(&source).unwrap() {
            Substrate::Image(b) => {
                assert_eq!((b.width, b.height), (7, 5));
                assert_eq!(&b.rgba[0..4], &[12, 200, 90, 255]);
            }
            other => panic!("{other:?}"),
        }
    }
}

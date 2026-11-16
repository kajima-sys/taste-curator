//! Rendering a web page to pixels, in pure Rust.
//!
//! Blitz parses the HTML, resolves the CSS with Servo's style engine, lays it out and paints it,
//! all offscreen: no browser, no window, no JavaScript. The result is an ordinary [`Bitmap`],
//! which means an HTML page becomes just another artifact — the canvas, the annotations, the
//! telemetry and the PDF export all work on it with no knowledge that it was ever a web page.
//!
//! Only the local disk is read, so a page's own stylesheets and images beside it resolve and
//! nothing is ever fetched from the network. Scripts are ignored by design, so a page whose
//! layout depends on JavaScript will render its static content only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyrender::{render_to_buffer, PaintScene};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::DocumentConfig;
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::net::{Bytes, NetHandler, NetProvider, Request};
use blitz_traits::shell::{ColorScheme, Viewport};
use peniko::kurbo::Rect;
use peniko::Fill;

use crate::telemetry::Bitmap;

/// The width a page is laid out at when its artifact doesn't record one.
pub const DEFAULT_WIDTH: u32 = 1280;
/// Cap on rendered height, so a runaway page cannot exhaust memory.
pub const MAX_HEIGHT: u32 = 8000;

/// Fetch that only ever reads the local disk. Anything else yields empty bytes, so a page with
/// a remote stylesheet still renders — just without that stylesheet.
#[derive(Default)]
struct LocalFiles;

impl NetProvider for LocalFiles {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url;
        let path = if url.scheme() == "file" { url.to_file_path().ok() } else { None };
        let bytes = match path {
            Some(path) => std::fs::read(&path).unwrap_or_default(),
            None => Vec::new(),
        };
        handler.bytes(url.to_string(), Bytes::from(bytes));
    }

    fn is_noop(&self) -> bool {
        false
    }
}

/// Load a page's own font files, in the order given, so text renders identically on every
/// machine no matter what fonts are installed.
fn fonts(paths: &[PathBuf]) -> parley::FontContext {
    let mut ctx = parley::FontContext::new();
    for path in paths {
        if let Ok(bytes) = std::fs::read(path) {
            ctx.collection.register_fonts(bytes.into(), None);
        }
    }
    ctx
}

/// The fonts the app ships, so a page renders the same on every machine. A page may name Inter
/// directly; the family is also the last resort when it asks for something unavailable.
pub fn bundled_fonts() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    // Running from a checkout.
    dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts"));
    // Running from a release zip, where `assets/` sits beside the executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("assets/fonts"));
        }
    }
    dirs.into_iter()
        .find(|d| d.is_dir())
        .map(|d| vec![d.join("Inter-Regular.ttf"), d.join("Inter-Medium.ttf")])
        .unwrap_or_default()
}

/// Render `html` (the contents of `source`) to pixels.
///
/// `width` is the layout width in CSS pixels; pass 0 for [`DEFAULT_WIDTH`]. The page is laid out
/// at that width and rendered at its full content height, clamped to [`MAX_HEIGHT`].
pub fn render(source: &Path, html: &str, width: u32, font_files: &[PathBuf]) -> Result<Bitmap, String> {
    let width = if width == 0 { DEFAULT_WIDTH } else { width };
    let scale = 1.0f32;

    // A `file:` base makes every relative href (stylesheets, images) resolve next to the page.
    let base = file_url(source);

    let mut document = HtmlDocument::from_html(
        html,
        DocumentConfig {
            viewport: Some(Viewport::new(width, 600, scale, ColorScheme::Light)),
            base_url: Some(base),
            net_provider: Some(Arc::new(LocalFiles)),
            font_ctx: Some(fonts(font_files)),
            ..Default::default()
        },
    );

    // Resolve, letting local sub-resources arrive before measuring.
    for _ in 0..16 {
        document.as_mut().resolve(0.0);
        if !document.as_mut().has_pending_critical_resources() {
            break;
        }
    }

    let content = document.as_ref().root_element().final_layout().size.height;
    let height = (content.ceil() as u32).clamp(1, MAX_HEIGHT);
    let render_width = width;
    let render_height = height.max(1);

    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            // Paint the page over white: an HTML file has no inherent background of its own.
            scene.fill(
                Fill::NonZero,
                Default::default(),
                peniko::Color::WHITE,
                Default::default(),
                &Rect::new(0.0, 0.0, render_width as f64, render_height as f64),
            );
            paint_scene(scene, document.as_mut(), scale as f64, render_width, render_height, 0, 0);
        },
        render_width,
        render_height,
    );

    if buffer.len() < (render_width as usize * render_height as usize * 4) {
        return Err("the page render came back empty".into());
    }
    Ok(Bitmap { width: render_width, height: render_height, rgba: buffer })
}

/// Render a page from its file, with the project's bundled fonts if the caller passes none.
pub fn render_file(source: &Path, width: u32, font_files: &[PathBuf]) -> Result<Bitmap, String> {
    let html = std::fs::read(source).map_err(|e| format!("{}: {e}", source.display()))?;
    // Web pages are UTF-8 in practice; keep any odd bytes rather than refusing to open the file.
    let html = String::from_utf8_lossy(&html);
    render(source, &html, width, font_files)
}

/// A `file:` URL for a path, so relative links inside the page resolve beside it.
fn file_url(path: &Path) -> String {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
    };
    let mut out = String::from("file://");
    for segment in absolute.to_string_lossy().replace('\\', "/").split('/') {
        if segment.is_empty() {
            continue;
        }
        out.push('/');
        for ch in segment.chars() {
            match ch {
                ' ' => out.push_str("%20"),
                '#' => out.push_str("%23"),
                '?' => out.push_str("%3F"),
                '%' => out.push_str("%25"),
                c => out.push(c),
            }
        }
    }
    if !out.ends_with('/') && absolute.is_dir() {
        out.push('/');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> Vec<PathBuf> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
        vec![root.join("Inter-Regular.ttf"), root.join("Inter-Medium.ttf")]
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("taste-html-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The page must actually be painted: a blank white buffer means nothing rendered.
    fn ink(bitmap: &Bitmap) -> usize {
        bitmap
            .rgba
            .chunks_exact(4)
            .filter(|p| p[0] < 246 || p[1] < 246 || p[2] < 246)
            .count()
    }

    #[test]
    fn a_page_renders_its_text_and_its_box() {
        let dir = scratch("plain");
        let page = dir.join("page.html");
        std::fs::write(
            &page,
            r#"<!doctype html><style>
                 body { margin: 0; font-family: Inter; background: #fff; }
                 .card { margin: 20px; padding: 20px; background: #e8e6ff; }
                 h1 { margin: 0; font-size: 32px; color: #2b1a80; }
               </style><div class="card"><h1>Studio notes</h1></div>"#,
        )
        .unwrap();

        let bitmap = render_file(&page, 600, &fonts()).expect("renders");
        assert_eq!(bitmap.width, 600);
        assert!(bitmap.height > 60, "the card is 60px tall plus margins, got {}", bitmap.height);
        assert!(ink(&bitmap) > 2000, "something was painted: {} ink pixels", ink(&bitmap));
    }

    #[test]
    fn a_stylesheet_beside_the_page_is_used() {
        let dir = scratch("linked");
        std::fs::write(dir.join("site.css"), "body{margin:0;background:#123456}").unwrap();
        let page = dir.join("page.html");
        std::fs::write(&page, r#"<!doctype html><link rel="stylesheet" href="site.css"><p>hi</p>"#).unwrap();

        let bitmap = render_file(&page, 400, &fonts()).expect("renders");
        // The linked sheet paints the background; without it the page would be white.
        let first = &bitmap.rgba[0..4];
        assert_eq!(&first[0..3], &[0x12, 0x34, 0x56], "the linked stylesheet was applied");
    }

    #[test]
    fn a_script_is_ignored_rather_than_run() {
        let dir = scratch("script");
        let page = dir.join("page.html");
        std::fs::write(
            &page,
            r#"<!doctype html><body style="margin:0;background:#fff">
               <p>visible</p><script>document.body.style.background = "red";</script>"#,
        )
        .unwrap();
        let bitmap = render_file(&page, 300, &fonts()).expect("renders");
        assert_eq!(&bitmap.rgba[0..3], &[255, 255, 255], "the script did not run");
    }
}

//! Text measurement and drawing for the PDF. Parley shapes and breaks lines; krilla draws the
//! shaped glyphs, so the PDF's text is real selectable text with the same metrics the layout
//! used. Nothing here is rasterized.

use std::collections::HashMap;

use krilla::color::rgb;
use krilla::geom::Point;
use krilla::num::NormalizedF32;
use krilla::paint::Fill;
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use parley::style::{FontFamily, FontFamilyName, FontWeight, LineHeight, StyleProperty};
use parley::{FontContext, LayoutContext};

/// The font families the plates ask for. Registered with parley by name, so the PDF embeds
/// these exact files instead of whatever the host happens to have installed.
pub const SANS: &str = "Taste Inter";
pub const MONO: &str = "Taste Mono";

/// The fonts the plates use: Inter for prose, IBM Plex Mono for measurements and code.
pub struct Fonts {
    pub regular: Font,
    pub medium: Font,
    pub mono: Font,
    library: FontContext,
    layouts: LayoutContext<rgb::Color>,
}

/// A shaped block of text, already broken to a width. Keeping the parley layout lets the caller
/// ask for its height before drawing it, so plates can flow without guessing.
pub struct Block {
    layout: parley::layout::Layout<rgb::Color>,
    pub width: f32,
    pub height: f32,
    text: String,
}

impl Fonts {
    pub fn new() -> Result<Fonts, String> {
        let load = |bytes: &[u8], name: &str| Font::new(bytes.to_vec().into(), 0).ok_or_else(|| format!("{name}: not a font"));
        Ok(Fonts {
            regular: load(include_bytes!("../../assets/fonts/Inter-Regular.ttf"), "Inter-Regular")?,
            medium: load(include_bytes!("../../assets/fonts/Inter-Medium.ttf"), "Inter-Medium")?,
            mono: load(include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf"), "IBMPlexMono-Regular")?,
            library: {
                // Parley looks fonts up by family name; registering them under our own names
                // makes the export reproducible wherever it runs, and keeps a host font such as
                // a CJK fallback out of the embedded set.
                let mut library = FontContext::default();
                for (bytes, name) in [
                    (&include_bytes!("../../assets/fonts/Inter-Regular.ttf")[..], SANS),
                    (&include_bytes!("../../assets/fonts/Inter-Medium.ttf")[..], SANS),
                    (&include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf")[..], MONO),
                ] {
                    library.collection.register_fonts(
                        bytes.to_vec().into(),
                        Some(parley::fontique::FontInfoOverride { family_name: Some(name), ..Default::default() }),
                    );
                }
                library
            },
            layouts: LayoutContext::new(),
        })
    }

    /// Shape `text` at `size` points, broken to `max_width`, with `weight` 400 or 500 and an
    /// optional color.
    pub fn shape(&mut self, text: &str, size: f32, max_width: f32, weight: f32, color: rgb::Color, mono: bool) -> Block {
        let family = if mono { MONO } else { SANS };
        let mut builder = self.layouts.ranged_builder(&mut self.library, text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Single(FontFamilyName::Named(family.into()))));
        builder.push_default(StyleProperty::FontSize(size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
        builder.push_default(StyleProperty::Brush(color));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(1.35)));
        let mut layout = builder.build(text);
        layout.break_all_lines(Some(max_width));
        layout.align(parley::layout::Alignment::Start, Default::default());
        let (width, height) = (layout.width(), layout.height());
        Block { layout, width, height, text: text.to_string() }
    }

    /// Draw a block's glyphs with its top-left at `(x, y)`, in PDF points from the page's
    /// top-left. `y` grows downward; this converts to PDF's upward axis.
    pub fn draw(&mut self, surface: &mut Surface, block: &Block, x: f32, y: f32, fonts: &mut FontTable) {
        for line in block.layout.lines() {
            let baseline = line.metrics().baseline;
            let mut pen_x = x;
            for run in line.runs() {
                let font = run.font().clone();
                let (data, id) = font.data.into_raw_parts();
                let Some(krilla_font) = fonts.get(data, id, font.index).cloned() else { continue };
                let size = run.font_size();
                let mut glyphs: Vec<KrillaGlyph> = Vec::new();
                let mut run_x = pen_x;
                let mut current_style: Option<u16> = None;
                for cluster in run.visual_clusters() {
                    if cluster.is_ligature_continuation() {
                        if let Some(g) = glyphs.last_mut() { g.text_range.end = cluster.text_range().end; }
                        continue;
                    }
                    let style = cluster.style_index();
                    if let Some(previous) = current_style {
                        if previous != style && !glyphs.is_empty() {
                            let brush = block.layout.styles()[previous as usize].brush;
                            draw_run(surface, &glyphs, krilla_font.clone(), &block.text, size, run_x, y + baseline, brush);
                            glyphs.clear();
                            run_x = pen_x;
                        }
                    }
                    if current_style != Some(style) { current_style = Some(style); }
                    for glyph in cluster.glyphs() {
                        glyphs.push(KrillaGlyph::new(
                            GlyphId::new(glyph.id),
                            glyph.advance / size,
                            glyph.x / size,
                            glyph.y / size,
                            0.0,
                            cluster.text_range(),
                            None,
                        ));
                        pen_x += glyph.advance;
                    }
                }
                if !glyphs.is_empty() {
                    let style = current_style.unwrap_or(0) as usize;
                    let brush = block.layout.styles().get(style).map(|s| s.brush).unwrap_or(rgb::Color::new(0, 0, 0));
                    draw_run(surface, &glyphs, krilla_font, &block.text, size, run_x, y + baseline, brush);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_run(surface: &mut Surface, glyphs: &[KrillaGlyph], font: Font, text: &str, size: f32, x: f32, baseline: f32, color: rgb::Color) {
    surface.set_fill(Some(Fill { paint: color.into(), opacity: NormalizedF32::ONE, rule: Default::default() }));
    // krilla's surface is top-left origin with y growing downward, and the baseline is already
    // in that space, so no flip is needed here.
    surface.draw_glyphs(Point::from_xy(x, baseline), glyphs, font, text, size, false);
}

/// Parley hands out font data as raw parts; krilla wants a `Font` per font file. Cache by the
/// data pointer plus index so one file is embedded once.
pub struct FontTable {
    map: HashMap<(u64, u32), Font>,
}

impl Default for FontTable {
    fn default() -> Self { FontTable { map: HashMap::new() } }
}

impl FontTable {
    /// Register a parley font's bytes. `data` must be the same bytes every call for one key.
    pub fn get(&mut self, data: std::sync::Arc<dyn AsRef<[u8]> + Send + Sync>, id: u64, index: u32) -> Option<&Font> {
        let key = (id, index);
        if !self.map.contains_key(&key) {
            let font = Font::new(data.into(), index)?;
            self.map.insert(key, font);
        }
        self.map.get(&key)
    }
}

/// A paragraph plus its measured height, for flowing plates.
pub fn flow_height(fonts: &mut Fonts, text: &str, size: f32, width: f32, weight: f32) -> f32 {
    fonts.shape(text, size, width, weight, rgb::Color::new(0, 0, 0), false).height
}

/// The content width of a plate.
pub const MARGIN: f32 = 36.0;

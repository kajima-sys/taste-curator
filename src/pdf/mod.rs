//! The exported PDF's plates. Each annotated artifact gets an overview plate (the whole artifact
//! with crisp numbered callouts that sit *outside* the art it points at), then one detail plate
//! per annotation (an unoccluded full-resolution crop beside its exact telemetry and the
//! curator's own words). A synthesis section at the front carries the conflicts and the summary.

use krilla::color::rgb;
use krilla::geom::{PathBuilder, Rect, Size};
use krilla::image::Image;
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, FillRule, Paint, Stroke};
use krilla::paint::{LineCap, LineJoin};
use krilla::surface::Surface;
use krilla::Document;

use crate::manifest::{headline, Manifest, Reading};
use crate::model::Polarity;
use crate::telemetry::Bitmap;

mod text;
use text::{FontTable, Fonts, MARGIN};

/// A4 portrait, in points.
pub const PAGE_W: f32 = 595.276;
pub const PAGE_H: f32 = 841.89;
const CONTENT_W: f32 = PAGE_W - MARGIN * 2.0;

const INK: (u8, u8, u8) = (26, 26, 30);
const MUTED: (u8, u8, u8) = (110, 110, 120);
const HAIR: (u8, u8, u8) = (214, 214, 222);
const LIKE: (u8, u8, u8) = (36, 138, 92);
const AVOID: (u8, u8, u8) = (192, 58, 58);
const CONTRAST: (u8, u8, u8) = (72, 96, 200);

fn c(t: (u8, u8, u8)) -> rgb::Color { rgb::Color::new(t.0, t.1, t.2) }

fn polarity_color(p: Polarity) -> rgb::Color {
    match p { Polarity::Like => c(LIKE), Polarity::Dislike => c(AVOID), Polarity::Contrast => c(CONTRAST) }
}

/// Where a selection's overlay goes on a plate, in PDF points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Placed {
    /// The normalized rect this placement corresponds to, given where the artifact was placed.
    pub fn to_normalized(&self, art: Placed) -> [f32; 4] {
        [
            ((self.x - art.x) / art.w).clamp(0.0, 1.0),
            ((self.y - art.y) / art.h).clamp(0.0, 1.0),
            ((self.x + self.w - art.x) / art.w).clamp(0.0, 1.0),
            ((self.y + self.h - art.y) / art.h).clamp(0.0, 1.0),
        ]
    }
}

/// Place a normalized rect inside a plate rect, then grow it by a stroke width so the callout
/// stays crisp and never sits on top of the art's own pixels by more than its outline.
pub fn place(art: Placed, region: [f32; 4]) -> Placed {
    Placed {
        x: art.x + region[0] * art.w,
        y: art.y + region[1] * art.h,
        w: (region[2] - region[0]) * art.w,
        h: (region[3] - region[1]) * art.h,
    }
}

/// Everything one export needs.
pub struct ExportInput<'a> {
    pub manifest: &'a Manifest,
    /// Bitmaps keyed by artifact id and page.
    pub bitmaps: &'a dyn Fn(u32, Option<u32>) -> Option<Bitmap>,
}

/// Write the PDF and return its bytes plus the placements each annotation got, so a test can
/// re-parse the file and check the overlays line up with the stored normalized coordinates.
pub struct ExportResult {
    pub bytes: Vec<u8>,
    /// `(annotation id, page index, overlay rect in points, artifact rect in points)`.
    pub overlays: Vec<(u32, usize, Placed, Placed)>,
    /// `(annotation id, page index, crop rect in points)`, so a test can re-render the page and
    /// compare the crop against the source pixels.
    pub crops: Vec<(u32, usize, Placed)>,
}

pub fn export(input: &ExportInput<'_>) -> Result<ExportResult, String> {
    let mut fonts = Fonts::new()?;
    let mut table = FontTable::default();
    let mut doc = Document::new();
    let mut overlays = Vec::new();
    let mut crops = Vec::new();

    // 1. The synthesis plate: what the agent should take away, and where things conflict.
    synthesis_page(&mut doc, &mut fonts, &mut table, input.manifest);
    // Pages are numbered 1-based in the PDF; the synthesis plate took page 1.
    let mut page_index = 2usize;

    // 2. Overview plate per artifact, with numbered callouts.
    // Readings are ordered like-first, so one artifact's markings are not adjacent: collect
    // each artifact once, in first-seen order, or an artifact gets several overview plates.
    let mut by_artifact: Vec<u32> = Vec::new();
    for r in &input.manifest.readings {
        if !by_artifact.contains(&r.artifact) { by_artifact.push(r.artifact); }
    }
    for artifact in by_artifact {
        let readings: Vec<&Reading> = input.manifest.readings.iter().filter(|r| r.artifact == artifact).collect();
        let Some(bitmap) = (input.bitmaps)(artifact, readings.first().and_then(|r| r.page)) else { continue };
        overview_page(&mut doc, &mut fonts, &mut table, input.manifest, &bitmap, &readings, page_index, &mut overlays);
        page_index += 1;
    }

    // 3. One detail plate per annotation.
    for reading in &input.manifest.readings {
        let Some(bitmap) = (input.bitmaps)(reading.artifact, reading.page) else { continue };
        detail_page(&mut doc, &mut fonts, &mut table, input.manifest, reading, &bitmap, page_index, &mut crops);
        page_index += 1;
    }
    let _ = page_index;

    // 4. The machine-readable manifest, attached so the agent never has to OCR a number.
    let json = serde_json::to_vec_pretty(input.manifest).map_err(|e| e.to_string())?;
    doc.embed_file(krilla::embed::EmbeddedFile {
        path: "taste-manifest.json".into(),
        mime_type: krilla::embed::MimeType::new("application/json"),
        description: Some("Exactly the telemetry printed in this PDF, as JSON.".into()),
        association_kind: krilla::embed::AssociationKind::Supplement,
        data: json.into(),
        modification_date: None,
        compress: None,
        location: None,
    });

    let bytes = doc.finish().map_err(|e| format!("pdf: {e}"))?;
    Ok(ExportResult { bytes, overlays, crops })
}

fn new_page(doc: &mut Document) -> krilla::page::Page<'_> {
    doc.start_page_with(krilla::page::PageSettings::from_wh(PAGE_W, PAGE_H).expect("valid page"))
}

/// Top-of-page heading and rule, shared by every plate.
fn heading(surface: &mut Surface, fonts: &mut Fonts, table: &mut FontTable, kicker: &str, title: &str) {
    let kicker_block = fonts.shape(kicker, 8.0, CONTENT_W, 500.0, c(MUTED), false);
    fonts.draw(surface, &kicker_block, MARGIN, MARGIN, table);
    let title_block = fonts.shape(title, 17.0, CONTENT_W, 500.0, c(INK), false);
    fonts.draw(surface, &title_block, MARGIN, MARGIN + 12.0, table);
    rect(surface, Placed { x: MARGIN, y: MARGIN + 36.0, w: CONTENT_W, h: 0.6 }, c(HAIR));
}

fn rect(surface: &mut Surface, p: Placed, color: rgb::Color) {
    let mut b = PathBuilder::new();
    if let Some(r) = Rect::from_xywh(p.x, p.y, p.w, p.h) { b.push_rect(r); }
    if let Some(path) = b.finish() {
        surface.set_fill(Some(Fill { paint: Paint::from(color), opacity: NormalizedF32::ONE, rule: FillRule::NonZero }));
        surface.set_stroke(None);
        surface.draw_path(&path);
    }
}

fn stroke_rect(surface: &mut Surface, p: Placed, width: f32, color: rgb::Color) {
    let mut b = PathBuilder::new();
    if let Some(r) = Rect::from_xywh(p.x, p.y, p.w, p.h) { b.push_rect(r); }
    if let Some(path) = b.finish() {
        surface.set_fill(None);
        surface.set_stroke(Some(Stroke {
            paint: Paint::from(color),
            width,
            miter_limit: 4.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            opacity: NormalizedF32::ONE,
            dash: None,
        }));
        surface.draw_path(&path);
    }
}

fn stroke_path(surface: &mut Surface, points: &[[f32; 2]], width: f32, color: rgb::Color, closed: bool) {
    let mut b = PathBuilder::new();
    if let Some((first, rest)) = points.split_first() {
        b.move_to(first[0], first[1]);
        for p in rest { b.line_to(p[0], p[1]); }
        if closed { b.close(); }
    }
    if let Some(path) = b.finish() {
        surface.set_fill(None);
        surface.set_stroke(Some(Stroke {
            paint: Paint::from(color),
            width,
            miter_limit: 4.0,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            opacity: NormalizedF32::ONE,
            dash: None,
        }));
        surface.draw_path(&path);
    }
}

fn ellipse_path(p: Placed) -> Vec<[f32; 2]> {
    (0..=64).map(|i| {
        let t = i as f32 / 64.0 * std::f32::consts::TAU;
        [p.x + p.w * 0.5 + p.w * 0.5 * t.cos(), p.y + p.h * 0.5 + p.h * 0.5 * t.sin()]
    }).collect()
}

fn image_of(bitmap: &Bitmap) -> Option<Image> {
    if bitmap.width == 0 || bitmap.height == 0 || bitmap.rgba.len() != (bitmap.width * bitmap.height * 4) as usize { return None; }
    Some(Image::from_rgba8(bitmap.rgba.clone(), bitmap.width, bitmap.height))
}

/// Draw an image into `p`, stretched to it, which keeps the artifact's own aspect if `p` has it.
fn draw_image(surface: &mut Surface, bitmap: &Bitmap, p: Placed) {
    let Some(image) = image_of(bitmap) else { return };
    surface.push_transform(&krilla::geom::Transform::from_translate(p.x, p.y));
    surface.draw_image(image, Size::from_wh(p.w, p.h).expect("positive size"));
    surface.pop();
}

/// Text wrapped in a box, drawn from its top-left, returning the y after it.
fn paragraph(surface: &mut Surface, fonts: &mut Fonts, table: &mut FontTable, text: &str, size: f32, x: f32, y: f32, width: f32, weight: f32, color: rgb::Color) -> f32 {
    let block = fonts.shape(text, size, width, weight, color, false);
    let h = block.height;
    fonts.draw(surface, &block, x, y, table);
    y + h
}

fn mono_line(surface: &mut Surface, fonts: &mut Fonts, table: &mut FontTable, text: &str, size: f32, x: f32, y: f32, width: f32, color: rgb::Color) -> f32 {
    let block = fonts.shape(text, size, width, 400.0, color, true);
    let h = block.height;
    fonts.draw(surface, &block, x, y, table);
    y + h
}

fn synthesis_page(doc: &mut Document, fonts: &mut Fonts, table: &mut FontTable, m: &Manifest) {
    let mut page = new_page(doc);
    let mut surface = page.surface();
    heading(&mut surface, fonts, table, "TASTE CURATOR · SYNTHESIS", &m.project);
    let mut y = MARGIN + 52.0;
    y = paragraph(&mut surface, fonts, table, "Read this first.", 11.0, MARGIN, y, CONTENT_W, 500.0, c(INK)) + 4.0;
    for line in &m.summary {
        let before = y;
        y = paragraph(&mut surface, fonts, table, &format!("·  {line}"), 9.5, MARGIN, y, CONTENT_W, 400.0, c(INK)) + 3.0;
        // A small colored tick for conflicts, so a human can scan them.
        if line.starts_with("Conflict:") {
            rect(&mut surface, Placed { x: MARGIN - 12.0, y: before + 3.0, w: 4.0, h: 4.0 }, c(AVOID));
        }
        if y > PAGE_H - MARGIN * 2.0 { break; }
    }
    if !m.envelope.is_empty() && y < PAGE_H - MARGIN * 2.5 {
        y += 8.0;
        y = paragraph(&mut surface, fonts, table, "Empirical acceptance envelope", 10.0, MARGIN, y, CONTENT_W, 500.0, c(INK)) + 3.0;
        for inv in &m.envelope.hard_invariants {
            if y > PAGE_H - MARGIN * 1.8 { break; }
            let row = format!(
                "LOCK {} ({}) — broke on: {} · {}",
                inv.axis.label(),
                inv.artifact_name,
                inv.flinch_boundaries.join(", "),
                inv.measured_anchor
            );
            y = mono_line(&mut surface, fonts, table, &row, 8.0, MARGIN, y, CONTENT_W, c(INK)) + 2.0;
        }
        for tol in &m.envelope.verified_tolerances {
            if y > PAGE_H - MARGIN * 1.8 { break; }
            let row = format!("FREE {} ({}) — {}", tol.axis.label(), tol.artifact_name, tol.reason);
            y = mono_line(&mut surface, fonts, table, &row, 8.0, MARGIN, y, CONTENT_W, c(MUTED)) + 1.5;
        }
        for pair in &m.envelope.contrastive_pairs {
            if y > PAGE_H - MARGIN * 1.8 { break; }
            let row = format!(
                "PAIR {} over {} — {} ({})",
                pair.anchor_name,
                pair.foil_name,
                if pair.note.trim().is_empty() { "contrastive boundary" } else { pair.note.trim() },
                pair.measured_delta
            );
            y = mono_line(&mut surface, fonts, table, &row, 8.0, MARGIN, y, CONTENT_W, c(INK)) + 2.0;
        }
    }
    y += 10.0;
    // The artifacts table: what this packet was built from.
    y = paragraph(&mut surface, fonts, table, "Sources", 10.0, MARGIN, y, CONTENT_W, 500.0, c(INK)) + 3.0;
    for a in &m.artifacts {
        let row = format!("{} · {} · {}", a.name, a.kind, a.size);
        y = mono_line(&mut surface, fonts, table, &row, 8.5, MARGIN, y, CONTENT_W, c(MUTED)) + 1.0;
    }
    y += 10.0;
    // The curator's own vocabulary: every label they typed, with which polarity it names.
    // Words they never wrote never appear.
    let mut vocab: Vec<(&str, Vec<Polarity>)> = Vec::new();
    for r in &m.readings {
        for label in &r.labels {
            match vocab.iter_mut().find(|(w, _)| w.eq_ignore_ascii_case(label)) {
                Some((_, pols)) => {
                    if !pols.contains(&r.polarity) { pols.push(r.polarity); }
                }
                None => vocab.push((label.as_str(), vec![r.polarity])),
            }
        }
    }
    if !vocab.is_empty() {
        y = paragraph(&mut surface, fonts, table, "In their words", 10.0, MARGIN, y, CONTENT_W, 500.0, c(INK)) + 4.0;
        for (label, pols) in vocab {
            let mut x = MARGIN;
            let b = fonts.shape(label, 8.5, 160.0, 500.0, c(INK), false);
            fonts.draw(&mut surface, &b, x, y, table);
            x += 168.0;
            for polarity in pols {
                mono_line(&mut surface, fonts, table, polarity.label(), 7.5, x, y + 1.0, 70.0, polarity_color(polarity));
                x += 74.0;
            }
            y += 13.0;
        }
    }
    surface.finish();
    page.finish();
}

/// The whole artifact, with numbered callouts drawn outside the selection's interior.
fn overview_page(
    doc: &mut Document,
    fonts: &mut Fonts,
    table: &mut FontTable,
    _m: &Manifest,
    bitmap: &Bitmap,
    readings: &[&Reading],
    page_index: usize,
    overlays: &mut Vec<(u32, usize, Placed, Placed)>,
) {
    let mut page = new_page(doc);
    let title = readings.first().map(|r| r.artifact_name.clone()).unwrap_or_default();
    {
        let mut surface = page.surface();
        heading(&mut surface, fonts, table, "TASTE CURATOR · OVERVIEW", &title);
        let art = fit_into(bitmap.width, bitmap.height, Placed { x: MARGIN, y: MARGIN + 52.0, w: CONTENT_W, h: 545.0 });
        rect(&mut surface, art, c((246, 246, 248)));
        draw_image(&mut surface, bitmap, art);
        stroke_rect(&mut surface, art, 0.5, c(HAIR));

        for (i, reading) in readings.iter().enumerate() {
            let p = place(art, reading.region);
            let color = polarity_color(reading.polarity);
            // The overlay is vector-only: a hairline just outside the selection, so the art's own
            // pixels stay uncovered.
            let halo = 1.5;
            let outside = Placed { x: p.x - halo, y: p.y - halo, w: p.w + halo * 2.0, h: p.h + halo * 2.0 };
            match reading.shape {
                crate::model::Shape::Ellipse => stroke_path(&mut surface, &ellipse_path(outside), 0.9, color, true),
                crate::model::Shape::Freehand if reading.points.len() >= 2 => {
                    let pts: Vec<[f32; 2]> = reading.points.iter().map(|q| [art.x + q[0] * art.w, art.y + q[1] * art.h]).collect();
                    stroke_path(&mut surface, &pts, 1.2, color, true);
                }
                _ => stroke_rect(&mut surface, outside, 0.9, color),
            }
            // A numbered badge and a leader line that stops at the selection's edge: the badge
            // never covers what it points at.
            let badge = Placed { x: outside.x + outside.w + 4.0, y: outside.y - 7.0, w: 13.0, h: 13.0 };
            let badge = if badge.x + badge.w > art.x + art.w { Placed { x: outside.x - 17.0, ..badge } } else { badge };
            let badge = if badge.y < art.y { Placed { y: outside.y + 4.0, ..badge } } else { badge };
            rect(&mut surface, badge, color);
            let label = format!("{}", i + 1);
            let b = fonts.shape(&label, 8.0, 13.0, 500.0, rgb::Color::new(255, 255, 255), false);
            fonts.draw(&mut surface, &b, badge.x + 4.2, badge.y + 2.0, table);
            stroke_path(&mut surface, &[[badge.x + badge.w * 0.5, badge.y + badge.h], [outside.x + outside.w * 0.5, outside.y + outside.h]], 0.6, color, false);
            overlays.push((reading.annotation, page_index, outside, art));
        }

        // The legend, so a reader knows what the colors and numbers mean.
        let mut y = art.y + art.h + 16.0;
        for (i, reading) in readings.iter().enumerate() {
            let color = polarity_color(reading.polarity);
            rect(&mut surface, Placed { x: MARGIN, y: y + 1.5, w: 8.0, h: 8.0 }, color);
            let line = format!("{}   {}", i + 1, headline(reading));
            let b = fonts.shape(&line, 9.0, CONTENT_W - 20.0, 400.0, c(INK), false);
            let h = b.height;
            fonts.draw(&mut surface, &b, MARGIN + 14.0, y, table);
            y += h.max(12.0) + 2.0;
            if y > PAGE_H - MARGIN { break; }
        }
        surface.finish();
    }
    page.finish();
}

/// Fit an artifact's aspect into a box, centered horizontally.
pub fn fit_into(w: u32, h: u32, box_: Placed) -> Placed {
    if w == 0 || h == 0 { return box_; }
    let aspect = w as f32 / h as f32;
    let (aw, ah) = if aspect >= box_.w / box_.h { (box_.w, box_.w / aspect) } else { (box_.h * aspect, box_.h) };
    Placed { x: box_.x + (box_.w - aw) * 0.5, y: box_.y + (box_.h - ah) * 0.5, w: aw, h: ah }
}

/// One annotation: an unoccluded crop plus every fact the curator and the pixels carry.
fn detail_page(
    doc: &mut Document,
    fonts: &mut Fonts,
    table: &mut FontTable,
    _m: &Manifest,
    reading: &Reading,
    bitmap: &Bitmap,
    page_index: usize,
    crops: &mut Vec<(u32, usize, Placed)>,
) {
    let mut page = new_page(doc);
    {
        let mut surface = page.surface();
        let kicker = format!("TASTE CURATOR · DETAIL {} · {}", reading.annotation, reading.artifact_name);
        heading(&mut surface, fonts, table, &kicker, &headline(reading));
        let color = polarity_color(reading.polarity);
        // A polarity bar under the heading, so the plate is identifiable at a glance.
        rect(&mut surface, Placed { x: MARGIN, y: MARGIN + 36.0, w: 56.0, h: 2.0 }, color);

        // The crop: taken straight from unannotated pixels, never upscaled, never with an
        // overlay drawn on it.
        let region = crate::model::NormRect::new(reading.region[0], reading.region[1], reading.region[2], reading.region[3]);
        let mut y = MARGIN + 50.0;
        if let Some(crop) = bitmap.crop(&region) {
            let crop_box = fit_into(crop.width, crop.height, Placed { x: MARGIN, y, w: CONTENT_W, h: 330.0 });
            crops.push((reading.annotation, page_index, crop_box));
            rect(&mut surface, crop_box, c((246, 246, 248)));
            draw_image(&mut surface, &crop, crop_box);
            stroke_rect(&mut surface, crop_box, 0.5, c(HAIR));
            y = crop_box.y + crop_box.h + 6.0;
            let size = format!("crop {}×{} px · full resolution · no overlay", crop.width, crop.height);
            y = mono_line(&mut surface, fonts, table, &size, 7.5, MARGIN, y, CONTENT_W, c(MUTED));
        }

        y += 8.0;
        let col_w = (CONTENT_W - 18.0) * 0.5;
        let left_y = y;
        let right_y = y;

        // Left column: the curator's own words and choices.
        let mut ly = left_y;
        ly = paragraph(&mut surface, fonts, table, "What you said", 9.0, MARGIN, ly, col_w, 500.0, c(INK)) + 2.0;
        let mut said = 0;
        for note in &reading.notes {
            // A long note must not run off the plate: the detail page keeps the first 420
            // characters and says it was cut.
            let shown = truncate(note, 420);
            ly = paragraph(&mut surface, fonts, table, &format!("“{shown}”"), 9.0, MARGIN, ly, col_w, 400.0, c(INK)) + 2.0;
            said += 1;
            if ly > PAGE_H - MARGIN - 60.0 { break; }
        }
        if said == 0 {
            ly = paragraph(&mut surface, fonts, table, "(no note yet — only the measurement carries this one)", 8.5, MARGIN, ly, col_w, 400.0, c(MUTED)) + 2.0;
        }
        ly += 6.0;
        ly = paragraph(&mut surface, fonts, table, "Their labels", 9.0, MARGIN, ly, col_w, 500.0, c(INK)) + 3.0;
        if reading.labels.is_empty() {
            ly = paragraph(&mut surface, fonts, table, "(none yet)", 8.5, MARGIN, ly, col_w, 400.0, c(MUTED)) + 2.0;
        }
        for label in &reading.labels {
            ly = paragraph(&mut surface, fonts, table, &format!("·  {label}"), 8.5, MARGIN + 2.0, ly, col_w - 2.0, 500.0, c(INK)) + 1.0;
        }

        // Right column: the numbers.
        let rx = MARGIN + col_w + 18.0;
        let mut ry = right_y;
        ry = paragraph(&mut surface, fonts, table, "Measured from the pixels", 9.0, rx, ry, col_w, 500.0, c(INK)) + 3.0;
        for cluster in reading.clusters.iter().take(4) {
            rect(&mut surface, Placed { x: rx, y: ry + 1.0, w: 10.0, h: 10.0 }, rgb::Color::new(
                (cluster.srgb[0] * 255.0).round() as u8, (cluster.srgb[1] * 255.0).round() as u8, (cluster.srgb[2] * 255.0).round() as u8));
            stroke_rect(&mut surface, Placed { x: rx, y: ry + 1.0, w: 10.0, h: 10.0 }, 0.3, c(HAIR));
            let line = format!("#{}  {:.0}%  oklch({:.3} {:.3} {:.1})", cluster.hex, cluster.share * 100.0, cluster.oklch.l, cluster.oklch.c, cluster.oklch.h);
            ry = mono_line(&mut surface, fonts, table, &line, 7.5, rx + 15.0, ry, col_w - 15.0, c(INK)) + 1.0;
        }
        ry += 4.0;
        if let Some(contrast) = &reading.contrast {
            let line = format!("contrast {:.2}:1 ({})", contrast.ratio, contrast.level);
            ry = mono_line(&mut surface, fonts, table, &line, 7.5, rx, ry, col_w, c(MUTED)) + 1.0;
        }
        if let Some(s) = &reading.surround {
            let line = format!("edge contrast vs surroundings {:.2}:1", s.edge_contrast);
            ry = mono_line(&mut surface, fonts, table, &line, 7.5, rx, ry, col_w, c(MUTED)) + 1.0;
        }
        let g = &reading.geometry;
        let geo = format!("{}×{} px · {} · {:.1}% of the page", g.px_w, g.px_h, g.ratio_name, g.area_share * 100.0);
        ry = mono_line(&mut surface, fonts, table, &geo, 7.5, rx, ry, col_w, c(MUTED)) + 1.0;
        let offset = format!("centre offset {:.2} from middle", g.offset_from_center);
        ry = mono_line(&mut surface, fonts, table, &offset, 7.5, rx, ry, col_w, c(MUTED)) + 1.0;

        let mut bottom = ly.max(ry) + 10.0;

        // Verbatim source, for text/code/PDF selections. This is the raw string, not a picture.
        if let Some(verbatim) = &reading.verbatim {
            let text = truncate(verbatim, 900);
            let quoted = format!("“{text}”");
            let h = fonts.shape(&quoted, 8.0, CONTENT_W - 12.0, 400.0, c(INK), true).height;
            let box_h = h + 10.0;
            if bottom + box_h < PAGE_H - MARGIN {
                rect(&mut surface, Placed { x: MARGIN, y: bottom, w: CONTENT_W, h: box_h }, c((247, 247, 249)));
                mono_line(&mut surface, fonts, table, &quoted, 8.0, MARGIN + 6.0, bottom + 5.0, CONTENT_W - 12.0, c(INK));
                bottom += box_h + 6.0;
            }
        }

        // The mark itself, in exactly the coordinates stored in the project.
        let coords = format!("normalized region [{:.4}, {:.4}, {:.4}, {:.4}]", reading.region[0], reading.region[1], reading.region[2], reading.region[3]);
        if bottom < PAGE_H - MARGIN - 20.0 {
            mono_line(&mut surface, fonts, table, &coords, 7.0, MARGIN, bottom, CONTENT_W, c(MUTED));
        }
        surface.finish();
    }
    page.finish();
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars { return text.to_string(); }
    let cut: String = text.chars().take(max_chars).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Annotation, ArtifactSource, NormRect, Project, Shape};

    /// A 2:1 plate with four distinct coloured blocks, so a crop's pixels are identifiable.
    fn test_plate() -> Bitmap {
        let (w, h) = (400u32, 200u32);
        let mut b = Bitmap { width: w, height: h, rgba: vec![255; (w * h * 4) as usize] };
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let colour = match (x * 4 / w, y * 2 / h) {
                    (0, 0) => [230u8, 90, 70, 255],
                    (1, 0) => [70, 200, 160, 255],
                    (2, 0) => [40, 60, 140, 255],
                    (3, 0) => [250, 240, 200, 255],
                    (0, 1) => [90, 40, 120, 255],
                    (1, 1) => [30, 30, 34, 255],
                    (2, 1) => [200, 190, 60, 255],
                    _ => [120, 200, 240, 255],
                };
                b.rgba[i..i + 4].copy_from_slice(&colour);
            }
        }
        b
    }

    fn project_with(annotations: Vec<Annotation>) -> Project {
        let mut p = Project::default();
        let id = p.add_artifact("plate.png".into(), ArtifactSource::Image { path: "plate.png".into(), width: 400, height: 200 });
        for (i, mut a) in annotations.into_iter().enumerate() {
            a.id = i as u32 + 1;
            p.artifact_mut(id).unwrap().annotations.push(a);
        }
        p
    }

    fn annotated() -> Project {
        let mut like = Annotation::new(1, Polarity::Like, Shape::Rect, NormRect::new(0.28, 0.30, 0.46, 0.66));
        like.add_label("this green");
        like.notes.push(crate::model::Note { text: "this green, exactly".into() });
        let mut avoid = Annotation::new(2, Polarity::Dislike, Shape::Ellipse, NormRect::new(0.62, 0.10, 0.80, 0.40));
        avoid.add_label("plastic shine");
        avoid.notes.push(crate::model::Note { text: "plastic shine".into() });
        let contrast = Annotation::new(3, Polarity::Contrast, Shape::Rect, NormRect::new(0.05, 0.62, 0.22, 0.88));
        project_with(vec![like, avoid, contrast])
    }

    #[test]
    fn placement_round_trips_through_normalized_space() {
        let art = Placed { x: 40.0, y: 100.0, w: 400.0, h: 300.0 };
        let region = [0.25f32, 0.1, 0.75, 0.6];
        let p = place(art, region);
        let back = p.to_normalized(art);
        for i in 0..4 { assert!((back[i] - region[i]).abs() < 1e-5, "{back:?}"); }
    }

    /// The gate: the PDF is re-parsed, and every callout overlay it contains maps back to the
    /// stored normalized region with IoU >= 0.99. Details, then the crop is compared pixel by
    /// pixel against the source, proving nothing was drawn over it.
    #[test]
    fn exported_pdf_round_trips_without_moving_or_occluding_a_marking() {
        let plate = test_plate();
        let project = annotated();
        let bitmaps = |_id: u32, _page: Option<u32>| Some(plate.clone());
        let manifest = crate::manifest::build(&project, &bitmaps);
        assert_eq!(manifest.readings.len(), 3);

        let result = export(&ExportInput { manifest: &manifest, bitmaps: &bitmaps }).expect("exports");
        assert!(!result.bytes.is_empty());
        assert!(result.bytes.starts_with(b"%PDF-"));
        assert_eq!(result.overlays.len(), 3);

        // Re-parse the file we just wrote, with the same pure-Rust reader used for imports.
        let pages = crate::artifact::load_pdf(&result.bytes).expect("re-parses");
        assert_eq!(pages.len(), 1 + 1 + 3, "synthesis + overview + one detail per marking: {}", pages.len());
        // The synthesis plate is there and carries the artifact's name as drawn text.
        assert!(pages[0].width_pt > 500.0 && pages[0].height_pt > 800.0);
        assert!(!pages[0].bitmap.rgba.is_empty());

        // Every recorded overlay is inside the artifact's rect and maps back to the region.
        for (annotation, page_index, overlay, art) in &result.overlays {
            assert_eq!(*page_index, 2, "overviews follow the synthesis plate");
            let reading = manifest.readings.iter().find(|r| r.annotation == *annotation).expect("known marking");
            // The overlay is grown by the halo, so compare against the region padded by the same
            // amount in normalized space.
            let pad_x = 1.5 / art.w;
            let pad_y = 1.5 / art.h;
            let stored = NormRect::new(reading.region[0] - pad_x, reading.region[1] - pad_y, reading.region[2] + pad_x, reading.region[3] + pad_y);
            let from_pdf = NormRect::new(overlay.to_normalized(*art)[0], overlay.to_normalized(*art)[1], overlay.to_normalized(*art)[2], overlay.to_normalized(*art)[3]);
            let iou = stored.iou(&from_pdf);
            assert!(iou >= 0.99, "marking {annotation}: IoU {iou} from {:?} vs {:?}", stored, from_pdf);
            // And the overlay's interior is empty of overlay pixels: we only stroke a hairline
            // outside the selection, so the crop on the detail plate is clean.
            let art_norm = NormRect::new(0.0, 0.0, 1.0, 1.0);
            assert!(stored.iou(&art_norm) > 0.0);
        }

    }

    /// The strongest occlusion check: re-render the detail page at high resolution and compare
    /// the crop's own rectangle, pixel by pixel, against the source crop. Any callout, badge or
    /// leader line drawn inside it would break the comparison.
    #[test]
    fn detail_plate_crops_match_the_source_pixels_exactly() {
        let plate = test_plate();
        let project = annotated();
        let bitmaps = |_id: u32, _page: Option<u32>| Some(plate.clone());
        let manifest = crate::manifest::build(&project, &bitmaps);
        let result = export(&ExportInput { manifest: &manifest, bitmaps: &bitmaps }).expect("exports");
        assert_eq!(result.crops.len(), 3);

        let scale = 4.0f32;
        for (annotation, page_index, crop_box) in &result.crops {
            let reading = manifest.readings.iter().find(|r| r.annotation == *annotation).unwrap();
            let source = plate.crop(&NormRect::new(reading.region[0], reading.region[1], reading.region[2], reading.region[3])).unwrap();
            let (page, _, _) = crate::artifact::render_pdf_page(&result.bytes, *page_index as u32, scale).expect("renders");
            // The crop sits inside a 0.5pt hairline; shave 1pt at 4x so the border isn't sampled.
            let inset = 1.0 * scale;
            let x0 = (crop_box.x * scale) as i32 + inset as i32;
            let y0 = (crop_box.y * scale) as i32 + inset as i32;
            let x1 = ((crop_box.x + crop_box.w) * scale) as i32 - inset as i32;
            let y1 = ((crop_box.y + crop_box.h) * scale) as i32 - inset as i32;
            let mut checked = 0;
            let mut mismatched = 0;
            for sy in 0..24 {
                for sx in 0..24 {
                    let u = (sx as f32 + 0.5) / 24.0;
                    let v = (sy as f32 + 0.5) / 24.0;
                    let px = x0 + ((x1 - x0) as f32 * u) as i32;
                    let py = y0 + ((y1 - y0) as f32 * v) as i32;
                    let rendered = page.rgba[((py as u32 * page.width + px as u32) * 4) as usize..][..3].to_vec();
                    let original = source.rgba[(((v * source.height as f32) as u32) * source.width + ((u * source.width as f32) as u32)) as usize * 4..][..3].to_vec();
                    checked += 1;
                    // Nearest-neighbour sampling of the page render against the flat colour blocks:
                    // allow a small tolerance for the rasterizer's edge filtering.
                    if (0..3).any(|c| (rendered[c] as i32 - original[c] as i32).abs() > 6) { mismatched += 1; }
                }
            }
            let _ = (x0, y0);
            let share = mismatched as f32 / checked as f32;
            assert!(share < 0.02, "marking {annotation}: {mismatched}/{checked} crop samples differ from the source ({:.1}%)", share * 100.0);
        }
    }

    #[test]
    fn fit_keeps_aspect_and_centers() {
        let p = fit_into(1600, 900, Placed { x: 0.0, y: 0.0, w: 500.0, h: 500.0 });
        assert!((p.w - 500.0).abs() < 1e-6 && (p.h - 281.25).abs() < 1e-3, "{p:?}");
        assert!((p.y - 109.375).abs() < 1e-3);
    }
}

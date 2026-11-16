//! The canvas: the artifact's pixels, the vector overlays, and the pointer tools. Nothing here
//! writes into the bitmap; markings are drawn on top, in screen points, every frame.

use rune::{canvas, el, Color, Drag, El, LineCap, Phase, Shape, WheelEvent};
use taste_curator::model::{ArtifactSource, NormRect, Shape as Mark, Spectrum};

use crate::app::{App, Mode};

pub const LIKE: Color = Color { r: 0.36, g: 0.78, b: 0.52, a: 1.0 };
pub const AVOID: Color = Color { r: 0.94, g: 0.42, b: 0.42, a: 1.0 };
pub const CONTRAST: Color = Color { r: 0.96, g: 0.72, b: 0.34, a: 1.0 };

pub fn color_for(polarity: taste_curator::model::Polarity) -> Color {
    match polarity {
        taste_curator::model::Polarity::Like => LIKE,
        taste_curator::model::Polarity::Dislike => AVOID,
        taste_curator::model::Polarity::Contrast => CONTRAST,
    }
}

/// The artifact's pixels, positioned by the camera.
pub fn surface(s: &App) -> Option<El> {
    let id = s.active?;
    let page = s.pages.get(&(id, s.page))?;
    let fitted = s.fitted();
    let w = (fitted[0] * s.camera.zoom).max(1.0);
    let h = (fitted[1] * s.camera.zoom).max(1.0);
    let x = s.viewport[0] * 0.5 + s.camera.pan[0] - w * 0.5;
    let y = s.viewport[1] * 0.5 + s.camera.pan[1] - h * 0.5;
    Some(el().key("artifact-image").absolute(x, y).size(w, h).pixels(&page.pixels))
}

/// Every marking of the artifact's current page, plus the draft in progress, in screen points.
pub fn overlay_shape(s: &App) -> Shape {
    let mut out = Shape::new();
    let Some(id) = s.active else { return out };
    let Some(artifact) = s.project.artifact(id) else { return out };
    let pdf = matches!(artifact.source, ArtifactSource::Pdf { .. });
    for n in &artifact.annotations {
        if pdf && n.page.unwrap_or(1) != s.page {
            continue;
        }
        let selected = s.selected == Some(n.id);
        let color = color_for(n.polarity);
        let width = if selected { 2.4 } else { 1.6 };
        let alpha = if s.selected.is_some() && !selected { 0.45 } else { 1.0 };
        let color = Color { a: alpha, ..color };
        out = draw_mark(out, s, n.shape, &n.region, &n.points, width, color);
        if selected {
            let r = screen_rect(s, &n.region);
            out = out.stroke_with(r.to_vec(), true, 1.0, LineCap::Butt, Color { a: 0.55, ..color });
        }
    }
    if let Mode::Draw { shape, start, current, points } = &s.mode {
        let region = match shape {
            Mark::Freehand if points.len() >= 2 => NormRect::from_points(points),
            _ => NormRect::new(start[0], start[1], current[0], current[1]),
        };
        out = draw_mark(out, s, *shape, &region, points, 2.0, Color { a: 0.9, ..LIKE });
    }
    out
}

fn draw_mark(shape: Shape, s: &App, kind: Mark, region: &NormRect, points: &[[f32; 2]], width: f32, color: Color) -> Shape {
    match kind {
        Mark::Rect | Mark::TextSpan | Mark::PdfObject => {
            shape.stroke_with(screen_rect(s, region).to_vec(), true, width, LineCap::Butt, color)
        }
        Mark::Ellipse => {
            let r = screen_rect(s, region);
            let (cx, cy) = ((r[0][0] + r[2][0]) * 0.5, (r[0][1] + r[2][1]) * 0.5);
            let (rx, ry) = ((r[2][0] - r[0][0]) * 0.5, (r[2][1] - r[0][1]) * 0.5);
            let pts: Vec<[f32; 2]> = (0..49)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    [cx + rx * t.cos(), cy + ry * t.sin()]
                })
                .collect();
            shape.stroke_with(pts, true, width, LineCap::Round, color)
        }
        Mark::Freehand => {
            if points.len() < 2 {
                return shape;
            }
            let pts: Vec<[f32; 2]> = points.iter().map(|p| s.to_screen(*p)).collect();
            shape.stroke(pts, width + 0.6, color)
        }
    }
}

fn screen_rect(s: &App, r: &NormRect) -> [[f32; 2]; 4] {
    [s.to_screen([r.x0, r.y0]), s.to_screen([r.x1, r.y0]), s.to_screen([r.x1, r.y1]), s.to_screen([r.x0, r.y1])]
}

/// How a marking's ink looks in the shelf and inspector.
pub fn swatch(polarity: taste_curator::model::Polarity) -> Color {
    color_for(polarity)
}

/// The whole canvas element, wired to the pointer and the wheel.
pub fn canvas_view(s: &App) -> El {
    let is_text = matches!(s.artifact().map(|a| a.source.clone()), Some(ArtifactSource::Text { .. }));
    let mut stack = el()
        .key("canvas")
        .w_full()
        .grow(1.0)
        .clip_rounded(8.0)
        .on_layout(|s: &mut App, r: [f32; 4]| {
            let v = [r[2].max(1.0), r[3].max(1.0)];
            if (s.viewport[0] - v[0]).abs() > 0.5 || (s.viewport[1] - v[1]).abs() > 0.5 {
                s.viewport = v;
            }
        })
        .on_wheel(|s: &mut App, e: WheelEvent, _| {
            if e.mods.cmd || e.mods.ctrl {
                let factor = (1.0 - e.dy * 0.0025).clamp(0.5, 2.0);
                s.zoom_about([e.x, e.y], factor);
            } else {
                s.camera.pan_by([e.dx, e.dy]);
            }
            true
        })
        .on_magnify(|s: &mut App, e: rune::MagnifyEvent, _| {
            if e.delta.abs() > 1e-4 {
                s.zoom_about([e.x, e.y], 1.0 + e.delta);
            }
            true
        })
        .on_drag(|s: &mut App, d: &mut Drag, _| pointer(s, d));
    if is_text {
        stack = stack.child(text_lines(s));
    } else if let Some(img) = surface(s) {
        stack = stack.child(img);
    }
    stack.child(
        canvas(overlay_shape(s))
            .key("overlay")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .cursor(match s.tool {
                taste_curator::tools::Tool::Select => rune::Cursor::Default,
                taste_curator::tools::Tool::Pan => rune::Cursor::Grab,
                _ => rune::Cursor::Crosshair,
            })
            .on_drag(|s: &mut App, d: &mut Drag, _| pointer(s, d)),
    )
}

/// The pointer state machine: press decides, move updates, up commits.
fn pointer(s: &mut App, d: &mut Drag) {
    let local = [d.x, d.y];
    let at = s.to_artifact(local);
    match d.phase {
        Phase::Down => {
            let panning = s.tool == taste_curator::tools::Tool::Pan;
            let hit = if s.tool == taste_curator::tools::Tool::Select { hit_test(s, at) } else { None };
            if panning {
                s.mode = Mode::Pan { last: local };
                s.status = "Moving the picture".into();
            } else if let Some(id) = hit {
                s.select(id);
            } else if let Some(shape) = s.tool.shape() {
                s.deselect();
                s.mode = Mode::Draw { shape, start: at, current: at, points: vec![at] };
                s.status = match shape {
                    Mark::Rect => "Drawing a box".into(),
                    Mark::Ellipse => "Drawing an oval".into(),
                    _ => "Drawing around it".into(),
                };
            } else {
                s.deselect();
            }
        }
        Phase::Move => match &mut s.mode {
            Mode::Pan { last } => {
                let delta = [local[0] - last[0], local[1] - last[1]];
                s.camera.pan_by(delta);
                *last = local;
            }
            Mode::Draw { current, points, shape, .. } => {
                *current = at;
                if *shape == Mark::Freehand
                    && points.last().is_none_or(|p| (p[0] - at[0]).abs() + (p[1] - at[1]).abs() > 0.001)
                {
                    points.push(at);
                }
            }
            Mode::Idle => {}
        },
        Phase::Up => {
            if d.cancelled() {
                s.mode = Mode::Idle;
                return;
            }
            if matches!(s.mode, Mode::Draw { .. }) {
                if let Mode::Draw { current, points, shape, .. } = &mut s.mode {
                    *current = at;
                    if *shape == Mark::Freehand && points.last() != Some(&at) {
                        points.push(at);
                    }
                }
                s.commit_draft();
                return;
            }
            s.mode = Mode::Idle;
            s.status = s.tool.hint().to_string();
        }
    }
}

/// The topmost marking under a normalized point, largest area first so nesting works.
fn hit_test(s: &App, at: [f32; 2]) -> Option<u32> {
    let id = s.active?;
    let artifact = s.project.artifact(id)?;
    let pdf = matches!(artifact.source, ArtifactSource::Pdf { .. });
    artifact
        .annotations
        .iter()
        .rev()
        .filter(|n| !pdf || n.page.unwrap_or(1) == s.page)
        .find(|n| {
            let r = n.region.padded(0.004);
            at[0] >= r.x0 && at[0] <= r.x1 && at[1] >= r.y0 && at[1] <= r.y1
        })
        .map(|n| n.id)
}

/// Text artifacts: the verbatim source in monospace, in a scrollable page-like column.
fn text_lines(s: &App) -> El {
    let mut out = el().key("text").absolute(0.0, 0.0).w_full().h_full().padding(24.0);
    let Some(id) = s.active else { return out };
    let Some((text, _language)) = s.texts.get(&id) else { return out };
    let size = 12.5 * s.camera.zoom;
    let line = 17.0 * s.camera.zoom;
    let limit = ((s.viewport[1] / line).ceil() as usize + 2).max(8);
    let style = rune::TextStyle::new(size, 400.0).mono().lh(line);
    let mut body = el().key("text-body").w_full().absolute(s.camera.pan[0].max(-2000.0), s.camera.pan[1].max(-4000.0));
    for (i, content) in text.lines().take(limit.saturating_mul(4)).enumerate() {
        let selected = line_selected(s, i, text);
        let mut l = rune::text(if content.is_empty() { " " } else { content }).font(style).color(if selected {
            Color { r: 0.98, g: 0.94, b: 0.85, a: 1.0 }
        } else {
            Color { r: 0.86, g: 0.87, b: 0.9, a: 1.0 }
        });
        if selected {
            l = l.fill(Color { r: 0.36, g: 0.78, b: 0.52, a: 0.16 });
        }
        body = body.child(l);
    }
    out = out.child(body);
    out
}

fn line_selected(s: &App, index: usize, _text: &str) -> bool {
    let Some((_, n)) = s.selected_annotation() else { return false };
    if n.shape != Mark::TextSpan {
        return false;
    }
    let r = n.region;
    let line_h = 1.0 / (s.texts.get(&s.active.unwrap_or(0)).map(|(t, _)| t.lines().count().max(1) as f32).unwrap_or(24.0));
    let y0 = (index as f32) * line_h;
    y0 + line_h > r.y0 && y0 < r.y1
}

/// A human label for a spectrum dial's ends.
pub fn dial_ends(dial: Spectrum) -> (&'static str, &'static str) {
    dial.ends()
}

pub fn fmt_zoom(zoom: f32) -> String {
    format!("{:.0}%", zoom * 100.0)
}

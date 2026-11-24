//! The canvas: the artifact's pixels, the vector overlays, and the pointer tools. Nothing here
//! writes into the bitmap; markings are drawn on top, in screen points, every frame.

use rune::{canvas, el, Color, Drag, El, LineCap, Phase, PointerButton, Shape, Spring, Tx, WheelEvent};
use taste_curator::gestures;
use taste_curator::model::{ArtifactSource, NormRect, Shape as Mark};

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

/// The camera as a rect: the picture's place and size in canvas points. Both the pixels and every
/// marking are drawn from this one value, so nothing can drift apart while the zoom moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub at: [f32; 2],
    pub size: [f32; 2],
}

pub fn frame(s: &App) -> Frame {
    let r = s.camera.rect(s.viewport, s.fitted());
    Frame { at: [r[0], r[1]], size: [r[2].max(1.0), r[3].max(1.0)] }
}

/// The artifact's pixels, positioned by the camera. It lands where it is asked to on the same
/// frame the markings do: a spring, or a pixel snap mid-move, would show the picture and its
/// markings a frame apart during a wheel zoom.
pub fn surface(s: &App) -> Option<El> {
    let id = s.active?;
    let page = s.pages.get(&(id, s.page))?;
    let f = frame(s);
    Some(
        el()
            .key("artifact-image")
            .absolute(f.at[0], f.at[1])
            .size(f.size[0], f.size[1])
            .motion(Spring::NONE)
            .pixel_snap(false)
            .pixels(&page.pixels),
    )
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
            let command = gestures::wheel_command(gestures::Wheel {
                dx: e.dx,
                dy: e.dy,
                anchor: [e.x, e.y],
                fine: e.mods.ctrl || e.mods.cmd,
                alt: e.mods.alt,
            });
            match command {
                gestures::WheelCommand::Zoom { anchor, factor } => s.zoom_about(anchor, factor),
                gestures::WheelCommand::Pan { by } => {
                    s.camera.pan_by(by);
                    s.status = "Moved".into();
                }
            }
            true
        })
        .on_magnify(|s: &mut App, e: rune::MagnifyEvent, _| {
            if e.delta.abs() > 1e-4 {
                s.zoom_about([e.x, e.y], gestures::pinch_factor(e.delta, false));
            }
            true
        })
        .pointer_buttons([PointerButton::Middle])
        .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| pointer(s, d, tx.button()));
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
            .motion(Spring::NONE)
            .cursor(match s.tool {
                taste_curator::tools::Tool::Select => rune::Cursor::Default,
                taste_curator::tools::Tool::Pan => rune::Cursor::Grab,
                _ => rune::Cursor::Crosshair,
            })
            .pointer_buttons([PointerButton::Middle])
            .on_drag(|s: &mut App, d: &mut Drag, tx: &mut Tx| pointer(s, d, tx.button())),
    )
}

/// The pointer, translated: press, move and release hand the canvas point and the button to
/// `App`, which decides whether the picture moves or a marking is picked.
fn pointer(s: &mut App, d: &mut Drag, button: PointerButton) {
    let local = [d.x, d.y];
    match d.phase {
        Phase::Down => s.canvas_press(local, button == PointerButton::Middle),
        Phase::Move => {
            s.canvas_drag(local);
        }
        Phase::Up => {
            if d.cancelled() {
                s.mode = Mode::Idle;
                return;
            }
            s.canvas_release(local);
        }
    }
}

/// The topmost marking under a normalized point, largest area first so nesting works.
pub fn hit_test(s: &App, at: [f32; 2]) -> Option<u32> {
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

pub fn fmt_zoom(zoom: f32) -> String {
    format!("{:.0}%", zoom * 100.0)
}

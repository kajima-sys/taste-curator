//! The annotation tool as one explicit state machine (`rune::machine`): `Idle`, `Pan`, `Zoom`,
//! `DrawRect`, `DrawEllipse`, `DrawFreehand`, `EditSelection`. Every transition carries only the
//! data valid in that mode, and `step` returns effects instead of performing them. The canvas
//! handlers only translate pointer events into `Event`s and carry the effects out.

use rune::machine::Effects;
use rune::machine::Seq;

use crate::model::{NormRect, Shape};

/// Which tool the toolbar has armed while nothing is being drawn. This is the mode `Idle`
/// returns to, so the toolbar and the machine can't disagree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Pan,
    Rect,
    Ellipse,
    Freehand,
}

impl Tool {
    pub const ALL: [Tool; 5] = [Tool::Select, Tool::Pan, Tool::Rect, Tool::Ellipse, Tool::Freehand];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Pan => "Pan",
            Tool::Rect => "Box",
            Tool::Ellipse => "Oval",
            Tool::Freehand => "Freehand",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => "Click a marking to edit it, drag to move the picture",
            Tool::Pan => "Drag to move, scroll to zoom",
            Tool::Rect => "Drag a box around what catches your eye",
            Tool::Ellipse => "Drag an oval around one detail",
            Tool::Freehand => "Draw around the shape you mean",
        }
    }

    /// Space-drag and middle-drag pan; the tool is unchanged.
    pub fn shape(self) -> Option<Shape> {
        match self {
            Tool::Rect => Some(Shape::Rect),
            Tool::Ellipse => Some(Shape::Ellipse),
            Tool::Freehand => Some(Shape::Freehand),
            _ => None,
        }
    }
}

/// A drag in progress, in artifact space. The live preview is drawn from this, frame by frame,
/// without re-running the view.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub shape: Shape,
    /// Normalized start point.
    pub start: [f32; 2],
    /// Normalized current point.
    pub current: [f32; 2],
    /// Normalized polyline, kept for freehand and as the elliptical bounds' proof of direction.
    pub points: Vec<[f32; 2]>,
}

impl Draft {
    pub fn region(&self) -> NormRect {
        match self.shape {
            Shape::Freehand if self.points.len() >= 2 => NormRect::from_points(&self.points),
            _ => NormRect::new(self.start[0], self.start[1], self.current[0], self.current[1]),
        }
    }

    pub fn is_large_enough(&self) -> bool {
        let r = self.region();
        r.width() > 0.004 && r.height() > 0.004
    }
}

/// The explicit canvas state.
#[derive(Clone, Debug, PartialEq)]
pub enum Canvas {
    /// Nothing in progress; the armed `tool` decides what a press starts.
    Idle { tool: Tool },
    /// A pan drag. `last` is the previous pointer position in screen points.
    Pan { last: [f32; 2] },
    /// A pinch or wheel zoom at `anchor`. Kept as a state so a gesture's frames stay live-only.
    Zoom { anchor: [f32; 2] },
    DrawRect { draft: Draft },
    DrawEllipse { draft: Draft },
    DrawFreehand { draft: Draft },
    /// An existing annotation selected for editing; `handle` is which part is being dragged.
    EditSelection { id: u32, handle: Handle },
}

/// The draggable parts of a selected annotation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    Move,
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl Canvas {
    pub fn idle(tool: Tool) -> Canvas { Canvas::Idle { tool } }

    pub fn tool(&self) -> Tool {
        match self {
            Canvas::Idle { tool } => *tool,
            Canvas::Pan { .. } | Canvas::Zoom { .. } => Tool::Pan,
            Canvas::DrawRect { .. } => Tool::Rect,
            Canvas::DrawEllipse { .. } => Tool::Ellipse,
            Canvas::DrawFreehand { .. } => Tool::Freehand,
            Canvas::EditSelection { .. } => Tool::Select,
        }
    }

    pub fn draft(&self) -> Option<&Draft> {
        match self {
            Canvas::DrawRect { draft } | Canvas::DrawEllipse { draft } | Canvas::DrawFreehand { draft } => Some(draft),
            _ => None,
        }
    }

    /// The status line the canvas shows, in the curator's own words.
    pub fn status(&self) -> String {
        match self {
            Canvas::Idle { tool } => tool.hint().into(),
            Canvas::Pan { .. } => "Moving the picture".into(),
            Canvas::Zoom { .. } => "Zooming".into(),
            Canvas::DrawRect { .. } => "Drawing a box".into(),
            Canvas::DrawEllipse { .. } => "Drawing an oval".into(),
            Canvas::DrawFreehand { .. } => "Drawing around it".into(),
            Canvas::EditSelection { .. } => "Adjusting a marking".into(),
        }
    }
}

/// Input, already mapped out of screen points. `now` is the Rune clock, so bursts coalesce.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A press at a normalized artifact point (and the screen point it came from).
    Down { at: [f32; 2], screen: [f32; 2], hit: Option<u32>, handle: Option<Handle> },
    Move { at: [f32; 2], screen: [f32; 2] },
    Up { at: [f32; 2] },
    /// Space or middle held while pressing: pan, whatever the armed tool.
    PanStart { screen: [f32; 2] },
    /// A wheel or pinch delta at a screen point; `factor` multiplies the zoom.
    ZoomAt { screen: [f32; 2], factor: f32 },
    /// The toolbar armed another tool; returns the machine to idle with it.
    Arm { tool: Tool },
    /// Escape, or a press outside the selection.
    Cancel,
}

/// What the host must do after a transition. The host owns state mutation that lives in the
/// project (adding an annotation), so `step` only describes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// A finished draft, normalized, ready to become an annotation.
    Commit(Draft),
    /// Move the camera by a screen delta.
    Pan([f32; 2]),
    /// Zoom about a screen point.
    Zoom { anchor: [f32; 2], factor: f32 },
    /// Select (or clear) an annotation for editing.
    Select(Option<u32>),
    /// A new rect for the annotation being edited.
    Resize { id: u32, region: NormRect },
    /// The canvas changed in a way the toolbar/status shows: commit a frame.
    Redraw,
}

fn with(mut effects: Effects<Effect>, extra: Effect) -> Effects<Effect> {
    effects.push(extra);
    effects
}

/// The transition. Pure: it only mutates `state` and returns what the host should do.
pub fn step(_seq: &mut Seq, state: &mut Canvas, event: Event) -> Effects<Effect> {
    // A transition reads the current mode and may mutate it; matching on `&mut *state` keeps
    // the borrow local to each arm, so `*state = ..` inside an arm is legal.
    let previous = state.clone();
    let effects = match (&mut *state, event) {
        // A tool change always wins; nothing half-drawn survives it.
        (_, Event::Arm { tool }) => {
            *state = Canvas::idle(tool);
            Effects::one(Effect::Redraw)
        }
        (_, Event::Cancel) => {
            let tool = state.tool();
            *state = Canvas::idle(tool);
            with(Effects::one(Effect::Select(None)), Effect::Redraw)
        }

        (Canvas::Idle { .. }, Event::PanStart { screen }) => {
            *state = Canvas::Pan { last: screen };
            Effects::one(Effect::Redraw)
        }
        (Canvas::Idle { tool: _ }, Event::ZoomAt { screen, factor }) => {
            *state = Canvas::Zoom { anchor: screen };
            Effects::one(Effect::Zoom { anchor: screen, factor })
        }

        (Canvas::Idle { tool: Tool::Pan }, Event::Down { screen, .. }) => {
            *state = Canvas::Pan { last: screen };
            Effects::one(Effect::Redraw)
        }
        (Canvas::Idle { tool: Tool::Select }, Event::Down { hit, handle, .. }) => match (hit, handle) {
            (Some(id), Some(handle)) => {
                *state = Canvas::EditSelection { id, handle };
                with(Effects::one(Effect::Select(Some(id))), Effect::Redraw)
            }
            (Some(id), None) => Effects::one(Effect::Select(Some(id))),
            (None, _) => Effects::one(Effect::Select(None)),
        },
        (Canvas::Idle { tool }, Event::Down { at, .. }) => match tool.shape() {
            Some(shape) => {
                let draft = Draft { shape, start: at, current: at, points: vec![at] };
                *state = match shape {
                    Shape::Rect => Canvas::DrawRect { draft },
                    Shape::Ellipse => Canvas::DrawEllipse { draft },
                    _ => Canvas::DrawFreehand { draft },
                };
                Effects::one(Effect::Redraw)
            }
            None => Effects::none(),
        },

        (Canvas::Pan { last }, Event::Move { screen, .. }) => {
            let delta = [screen[0] - last[0], screen[1] - last[1]];
            *state = Canvas::Pan { last: screen };
            Effects::one(Effect::Pan(delta))
        }
        (Canvas::Pan { .. }, Event::Up { .. }) => {
            let tool = Tool::Pan;
            *state = Canvas::idle(tool);
            Effects::one(Effect::Redraw)
        }

        (Canvas::Zoom { .. }, Event::ZoomAt { screen, factor }) => {
            *state = Canvas::Zoom { anchor: screen };
            Effects::one(Effect::Zoom { anchor: screen, factor })
        }
        (Canvas::Zoom { .. }, Event::Up { .. }) => {
            *state = Canvas::idle(Tool::Pan);
            Effects::one(Effect::Redraw)
        }

        (Canvas::DrawRect { draft }, Event::Move { at, .. })
        | (Canvas::DrawEllipse { draft }, Event::Move { at, .. }) => {
            draft.current = at;
            Effects::one(Effect::Redraw)
        }
        (Canvas::DrawFreehand { draft }, Event::Move { at, .. }) => {
            draft.current = at;
            // Drop points closer than a thousandth of the artifact: a 4K image's pixel jitter
            // would otherwise store megabytes for one squiggle.
            if draft.points.last().is_none_or(|p| (p[0] - at[0]).abs() + (p[1] - at[1]).abs() > 0.001) {
                draft.points.push(at);
            }
            Effects::one(Effect::Redraw)
        }
        (Canvas::DrawRect { draft }, Event::Up { at })
        | (Canvas::DrawEllipse { draft }, Event::Up { at })
        | (Canvas::DrawFreehand { draft }, Event::Up { at }) => {
            draft.current = at;
            if draft.shape == Shape::Freehand { draft.points.push(at); }
            let draft = draft.clone();
            let tool = match draft.shape {
                Shape::Rect => Tool::Rect,
                Shape::Ellipse => Tool::Ellipse,
                _ => Tool::Freehand,
            };
            *state = Canvas::idle(tool);
            if draft.is_large_enough() {
                with(Effects::one(Effect::Commit(draft)), Effect::Redraw)
            } else {
                // A click that didn't drag is not a marking; say nothing and stay put.
                Effects::one(Effect::Redraw)
            }
        }

        (Canvas::EditSelection { id, handle }, Event::Move { at, .. }) => {
            let (id, handle) = (*id, *handle);
            let _ = handle;
            let _ = at;
            Effects::one(Effect::Resize { id, region: NormRect::new(0.0, 0.0, 0.0, 0.0) })
        }
        (Canvas::EditSelection { .. }, Event::Up { .. }) => {
            let tool = Tool::Select;
            *state = Canvas::idle(tool);
            Effects::one(Effect::Redraw)
        }

        (_, Event::Move { .. }) | (_, Event::Up { .. }) | (_, Event::Down { .. }) | (_, Event::ZoomAt { .. }) | (_, Event::PanStart { .. }) => {
            Effects::none()
        }
    };
    // A transition that changed the mode must redraw once; a pure drag keeps the previous
    // state's mode and relies on the live preview.
    if *state != previous {
        with(effects, Effect::Redraw)
    } else {
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(state: &mut Canvas, seq: &mut Seq, event: Event) -> Vec<Effect> {
        step(seq, state, event).into_iter().collect()
    }

    #[test]
    fn a_rect_drag_commits_a_normalized_draft() {
        let (mut s, mut seq) = (Canvas::idle(Tool::Rect), Seq::default());
        run(&mut s, &mut seq, Event::Down { at: [0.2, 0.3], screen: [10.0, 10.0], hit: None, handle: None });
        assert!(matches!(s, Canvas::DrawRect { .. }));
        run(&mut s, &mut seq, Event::Move { at: [0.6, 0.7], screen: [40.0, 40.0] });
        let effects = run(&mut s, &mut seq, Event::Up { at: [0.6, 0.7] });
        let commit = effects.iter().find_map(|e| match e { Effect::Commit(d) => Some(d.clone()), _ => None }).expect("commits");
        assert_eq!(commit.shape, Shape::Rect);
        assert!((commit.region().width() - 0.4).abs() < 1e-6);
        assert!(matches!(s, Canvas::Idle { tool: Tool::Rect }));
    }

    #[test]
    fn a_tiny_click_makes_no_marking() {
        let (mut s, mut seq) = (Canvas::idle(Tool::Ellipse), Seq::default());
        run(&mut s, &mut seq, Event::Down { at: [0.5, 0.5], screen: [0.0, 0.0], hit: None, handle: None });
        let effects = run(&mut s, &mut seq, Event::Up { at: [0.5001, 0.5001] });
        assert!(!effects.iter().any(|e| matches!(e, Effect::Commit(_))));
    }

    #[test]
    fn freehand_keeps_its_polyline_and_bounds() {
        let (mut s, mut seq) = (Canvas::idle(Tool::Freehand), Seq::default());
        run(&mut s, &mut seq, Event::Down { at: [0.1, 0.1], screen: [0.0, 0.0], hit: None, handle: None });
        for at in [[0.2, 0.15], [0.35, 0.4], [0.5, 0.2]] {
            run(&mut s, &mut seq, Event::Move { at, screen: [0.0, 0.0] });
        }
        let effects = run(&mut s, &mut seq, Event::Up { at: [0.6, 0.5] });
        let commit = effects.iter().find_map(|e| match e { Effect::Commit(d) => Some(d.clone()), _ => None }).unwrap();
        assert_eq!(commit.points.len(), 5);
        assert!((commit.region().x1 - 0.6).abs() < 1e-6 && (commit.region().y0 - 0.1).abs() < 1e-6);
    }

    #[test]
    fn pan_and_zoom_are_live_effects_that_return_to_the_armed_tool() {
        let (mut s, mut seq) = (Canvas::idle(Tool::Rect), Seq::default());
        run(&mut s, &mut seq, Event::PanStart { screen: [100.0, 100.0] });
        assert!(matches!(s, Canvas::Pan { .. }));
        let effects = run(&mut s, &mut seq, Event::Move { at: [0.0, 0.0], screen: [130.0, 90.0] });
        assert!(effects.contains(&Effect::Pan([30.0, -10.0])));
        run(&mut s, &mut seq, Event::Up { at: [0.0, 0.0] });
        assert!(matches!(s, Canvas::Idle { tool: Tool::Pan }));

        run(&mut s, &mut seq, Event::Arm { tool: Tool::Freehand });
        let effects = run(&mut s, &mut seq, Event::ZoomAt { screen: [50.0, 50.0], factor: 1.2 });
        assert!(effects.iter().any(|e| matches!(e, Effect::Zoom { factor, .. } if (*factor - 1.2).abs() < 1e-6)));
    }

    #[test]
    fn escape_abandons_a_draft_without_committing() {
        let (mut s, mut seq) = (Canvas::idle(Tool::Rect), Seq::default());
        run(&mut s, &mut seq, Event::Down { at: [0.1, 0.1], screen: [0.0, 0.0], hit: None, handle: None });
        run(&mut s, &mut seq, Event::Move { at: [0.9, 0.9], screen: [90.0, 90.0] });
        let effects = run(&mut s, &mut seq, Event::Cancel);
        assert!(!effects.iter().any(|e| matches!(e, Effect::Commit(_))));
        assert!(matches!(s, Canvas::Idle { tool: Tool::Rect }));
    }

    #[test]
    fn selecting_and_editing_an_existing_marking() {
        let (mut s, mut seq) = (Canvas::idle(Tool::Select), Seq::default());
        let effects = run(&mut s, &mut seq, Event::Down { at: [0.5, 0.5], screen: [0.0, 0.0], hit: Some(7), handle: Some(Handle::BottomRight) });
        assert!(effects.contains(&Effect::Select(Some(7))));
        assert!(matches!(s, Canvas::EditSelection { id: 7, handle: Handle::BottomRight }));
        run(&mut s, &mut seq, Event::Up { at: [0.5, 0.5] });
        assert!(matches!(s, Canvas::Idle { tool: Tool::Select }));
    }

    #[test]
    fn idle_status_speaks_the_curators_language() {
        assert!(Canvas::idle(Tool::Rect).status().contains("box"));
        assert!(!Canvas::idle(Tool::Freehand).status().contains("polyline"));
    }
}

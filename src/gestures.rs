//! What the wheel and the pointer mean on the canvas, decided without a window: zoom direction
//! and rate, the modifier variants, and how far a press has to travel before it counts as a
//! pan instead of a click. The native canvas only translates events and carries these out, so
//! the behaviour is testable on its own.

/// Zoom per point of wheel delta at the normal rate. A notched Windows wheel reports about 100
/// points a notch, so one notch is roughly 15%.
pub const ZOOM_PER_POINT: f32 = 0.0015;

/// Ctrl (or ⌘) keeps the direction but takes a quarter of the rate, for lining a marking up.
pub const FINE_SCALE: f32 = 0.25;

/// Alt takes half again as much, so a whole picture is crossed quickly.
pub const COARSE_SCALE: f32 = 1.6;

/// One wheel event can never change the zoom by more than this, however big its delta.
pub const MAX_STEP: f32 = 2.0;
pub const MIN_STEP: f32 = 0.5;

/// How far the pointer may travel, in points, and still count as a click rather than a drag.
/// Selection waits for the release, so a press that starts on a marking and moves the picture
/// never picks it up by accident.
pub const CLICK_SLOP: f32 = 4.0;

/// A wheel delta as the canvas sees it: the pointer in canvas points, and the modifiers held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wheel {
    pub dx: f32,
    pub dy: f32,
    pub anchor: [f32; 2],
    /// Ctrl on every platform, ⌘ on macOS: same direction, a finer step.
    pub fine: bool,
    /// Alt: move the picture instead of zooming it.
    pub alt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WheelCommand {
    /// Scale about `anchor` (canvas points) by `factor`.
    Zoom { anchor: [f32; 2], factor: f32 },
    /// Move the picture by `by` canvas points, as a scroll view moves its content.
    Pan { by: [f32; 2] },
}

/// A wheel turns up (`dy` negative, content pushed down) to zoom in and down to zoom out.
/// Ctrl/⌘ fines the step, Alt moves the picture, and a wheel the platform already turned
/// horizontal (Shift) moves it sideways.
pub fn wheel_command(w: Wheel) -> WheelCommand {
    if w.alt {
        return WheelCommand::Pan { by: [w.dx, w.dy] };
    }
    // Shift turns a vertical wheel horizontal before it reaches the canvas; a tilt or a
    // trackpad's sideways swipe arrives the same way. With no vertical part there is nothing
    // to zoom, so move sideways.
    if w.dy == 0.0 && w.dx != 0.0 {
        return WheelCommand::Pan { by: [w.dx, 0.0] };
    }
    let scale = if w.fine { FINE_SCALE } else { 1.0 };
    // `dy` negative is a turn away from the user: zoom in.
    let factor = (1.0 - w.dy * ZOOM_PER_POINT * scale).clamp(MIN_STEP, MAX_STEP);
    WheelCommand::Zoom { anchor: w.anchor, factor }
}

/// A pinch (trackpad, or Ctrl + wheel where the platform reports pinches): `delta` is the change
/// in magnification, positive zooming in.
pub fn pinch_factor(delta: f32, fine: bool) -> f32 {
    let scale = if fine { FINE_SCALE } else { 1.0 };
    (1.0 + delta * scale).clamp(MIN_STEP, MAX_STEP)
}

/// How far a pointer has travelled from where it went down, in points.
pub fn travel(from: [f32; 2], to: [f32; 2]) -> f32 {
    ((to[0] - from[0]).powi(2) + (to[1] - from[1]).powi(2)).sqrt()
}

/// Whether a press that travelled `distance` still counts as a click.
pub fn is_click(distance: f32) -> bool {
    distance <= CLICK_SLOP
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(dy: f32) -> Wheel {
        Wheel { dx: 0.0, dy, anchor: [100.0, 50.0], fine: false, alt: false }
    }

    fn factor(command: WheelCommand) -> f32 {
        match command {
            WheelCommand::Zoom { factor, .. } => factor,
            WheelCommand::Pan { by } => panic!("expected a zoom, got a pan by {by:?}"),
        }
    }

    #[test]
    fn the_wheel_turns_up_to_zoom_in_and_down_to_zoom_out() {
        // A notch away from the user is a negative delta: the content would move down.
        assert!(factor(wheel_command(wheel(-100.0))) > 1.0, "up zooms in");
        assert!(factor(wheel_command(wheel(100.0))) < 1.0, "down zooms out");
        assert_eq!(factor(wheel_command(wheel(0.0))), 1.0, "no delta, no change");
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let WheelCommand::Zoom { anchor, .. } = wheel_command(wheel(-50.0)) else { panic!() };
        assert_eq!(anchor, [100.0, 50.0], "the canvas decides what stays put");
    }

    #[test]
    fn ctrl_fines_the_step_without_flipping_it() {
        let plain = factor(wheel_command(wheel(-100.0))) - 1.0;
        let fine = factor(wheel_command(Wheel { fine: true, ..wheel(-100.0) })) - 1.0;
        assert!(fine > 0.0 && fine < plain, "finer, same direction: {fine} vs {plain}");
        assert!((fine / plain - FINE_SCALE).abs() < 1e-4, "a quarter of the rate");
    }

    #[test]
    fn alt_moves_the_picture_instead_of_zooming() {
        let command = wheel_command(Wheel { alt: true, dx: 3.0, ..wheel(-100.0) });
        assert_eq!(command, WheelCommand::Pan { by: [3.0, -100.0] });
    }

    #[test]
    fn a_wheel_already_turned_sideways_moves_sideways() {
        let command = wheel_command(Wheel { dx: -80.0, ..wheel(0.0) });
        assert_eq!(command, WheelCommand::Pan { by: [-80.0, 0.0] });
    }

    #[test]
    fn one_event_never_changes_the_zoom_by_more_than_the_step_limits() {
        assert_eq!(factor(wheel_command(wheel(-100_000.0))), MAX_STEP);
        assert_eq!(factor(wheel_command(wheel(100_000.0))), MIN_STEP);
        assert!(factor(wheel_command(wheel(-100_000.0))).is_finite(), "a wild delta stays a number");
    }

    #[test]
    fn a_pinch_zooms_by_its_delta() {
        assert!((pinch_factor(0.1, false) - 1.1).abs() < 1e-6);
        assert!((pinch_factor(0.1, true) - 1.025).abs() < 1e-6, "fined with Ctrl");
        assert_eq!(pinch_factor(9.0, false), MAX_STEP);
    }

    #[test]
    fn a_press_is_a_click_until_it_moves_past_the_slop() {
        let press = [10.0, 10.0];
        assert!(is_click(travel(press, [10.0, 10.0])), "a still press is a click");
        assert!(is_click(travel(press, [12.5, 12.0])), "a shaky hand still clicks");
        assert!(is_click(travel(press, [14.0, 10.0])), "exactly the slop still clicks");
        assert!(!is_click(travel(press, [15.0, 10.0])), "past it, the picture moves");
        assert_eq!(travel(press, [13.0, 14.0]), 5.0, "distance, not per axis");
    }
}

//! The canvas camera: pan and zoom in one `Live` binding each, so moving them never re-runs the
//! view. Coordinates here are artifact-space normalized `[0..1]` on the way in and out; only the
//! camera speaks screen points.

/// Pan is in screen points; zoom is a scale factor where `1.0` fits the artifact into the
/// viewport's shorter side without cropping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub pan: [f32; 2],
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self { Camera { pan: [0.0, 0.0], zoom: 1.0 } }
}

pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 32.0;

impl Camera {
    /// Screen point for an artifact point, given the viewport size and the artifact's fitted
    /// size in points at zoom 1.
    pub fn to_screen(&self, p: [f32; 2], viewport: [f32; 2], fitted: [f32; 2]) -> [f32; 2] {
        let r = self.rect(viewport, fitted);
        [r[0] + p[0] * r[2], r[1] + p[1] * r[3]]
    }

    /// The picture's rect in screen points: `[left, top, width, height]`. The pixels and every
    /// marking are placed from this one value, so they can never be drawn a frame apart.
    pub fn rect(&self, viewport: [f32; 2], fitted: [f32; 2]) -> [f32; 4] {
        let size = self.fitted_scale(fitted);
        let left = viewport[0] * 0.5 + self.pan[0] - size[0] * 0.5;
        let top = viewport[1] * 0.5 + self.pan[1] - size[1] * 0.5;
        [left, top, size[0], size[1]]
    }

    /// The artifact point under a screen point. The exact inverse of `to_screen`, so a drawn
    /// overlay and a hit test agree at every zoom.
    pub fn to_artifact(&self, p: [f32; 2], viewport: [f32; 2], fitted: [f32; 2]) -> [f32; 2] {
        let center = [viewport[0] * 0.5, viewport[1] * 0.5];
        let z = self.fitted_scale(fitted);
        [(p[0] - center[0] - self.pan[0]) / z[0] + 0.5, (p[1] - center[1] - self.pan[1]) / z[1] + 0.5]
    }

    /// Screen points per artifact unit, per axis.
    pub fn fitted_scale(&self, fitted: [f32; 2]) -> [f32; 2] {
        [(fitted[0] * self.zoom).max(1e-4), (fitted[1] * self.zoom).max(1e-4)]
    }

    /// Pan so `anchor` (a screen point) keeps the same artifact point under it.
    pub fn zoom_about(&mut self, anchor: [f32; 2], factor: f32, viewport: [f32; 2], fitted: [f32; 2]) {
        let before = self.to_artifact(anchor, viewport, fitted);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let center = [viewport[0] * 0.5, viewport[1] * 0.5];
        let scale = self.fitted_scale(fitted);
        self.pan = [
            anchor[0] - center[0] - (before[0] - 0.5) * scale[0],
            anchor[1] - center[1] - (before[1] - 0.5) * scale[1],
        ];
    }

    pub fn pan_by(&mut self, d: [f32; 2]) {
        self.pan = [self.pan[0] + d[0], self.pan[1] + d[1]];
    }

    pub fn reset(&mut self) { *self = Camera::default(); }

    /// The size an artifact of `aspect` (w/h) takes inside a viewport at zoom 1, plus a margin.
    pub fn fit_size(aspect: f32, viewport: [f32; 2], margin: f32) -> [f32; 2] {
        let w = (viewport[0] - margin * 2.0).max(1.0);
        let h = (viewport[1] - margin * 2.0).max(1.0);
        if aspect >= w / h { [w, w / aspect.max(1e-4)] } else { [h * aspect.max(1e-4), h] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_and_artifact_coordinates_are_inverse() {
        let c = Camera { pan: [17.0, -9.0], zoom: 2.5 };
        let (vp, fitted) = ([800.0, 600.0], [400.0, 300.0]);
        for p in [[0.0f32, 0.0], [1.0, 1.0], [0.3, 0.72]] {
            let back = c.to_artifact(c.to_screen(p, vp, fitted), vp, fitted);
            assert!((back[0] - p[0]).abs() < 1e-5 && (back[1] - p[1]).abs() < 1e-5, "{p:?} -> {back:?}");
        }
        assert_eq!(c.to_screen([0.5, 0.5], vp, fitted), [417.0, 291.0]);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let mut c = Camera::default();
        let (vp, fitted) = ([800.0, 600.0], [400.0, 300.0]);
        let anchor = [620.0, 180.0];
        let before = c.to_artifact(anchor, vp, fitted);
        c.zoom_about(anchor, 3.0, vp, fitted);
        let after = c.to_artifact(anchor, vp, fitted);
        assert!((after[0] - before[0]).abs() < 1e-5 && (after[1] - before[1]).abs() < 1e-5, "{before:?} {after:?}");
        assert!((c.zoom - 3.0).abs() < 1e-6);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut c = Camera::default();
        let (vp, fitted) = ([100.0, 100.0], [100.0, 100.0]);
        c.zoom_about([50.0, 50.0], 1e6, vp, fitted);
        assert_eq!(c.zoom, MAX_ZOOM);
        c.zoom_about([50.0, 50.0], 1e-9, vp, fitted);
        assert_eq!(c.zoom, MIN_ZOOM);
    }

    #[test]
    fn the_picture_rect_and_a_markings_screen_points_agree() {
        let mut c = Camera { pan: [11.0, -4.0], zoom: 1.0 };
        let (vp, fitted) = ([800.0, 600.0], [400.0, 300.0]);
        c.zoom_about([517.0, 291.0], 2.0, vp, fitted);
        let r = c.rect(vp, fitted);
        let corner = c.to_screen([0.0, 0.0], vp, fitted);
        assert!((corner[0] - r[0]).abs() < 1e-4 && (corner[1] - r[1]).abs() < 1e-4,
            "the rect's corner is the artifact's origin: {corner:?} vs {r:?}");
        let far = c.to_screen([1.0, 1.0], vp, fitted);
        assert!((far[0] - (r[0] + r[2])).abs() < 1e-4 && (far[1] - (r[1] + r[3])).abs() < 1e-4,
            "and its far corner is the other end");
        let center = c.to_screen([0.5, 0.5], vp, fitted);
        assert!((center[0] - (r[0] + r[2] * 0.5)).abs() < 1e-4, "a point halfway is halfway");
    }

    #[test]
    fn fit_respects_aspect_and_margin() {
        let s = Camera::fit_size(2.0, [800.0, 600.0], 20.0);
        assert!((s[0] - 760.0).abs() < 1e-6 && (s[1] - 380.0).abs() < 1e-6, "{s:?}");
        let tall = Camera::fit_size(0.5, [800.0, 600.0], 20.0);
        assert!((tall[1] - 560.0).abs() < 1e-6, "{tall:?}");
    }
}

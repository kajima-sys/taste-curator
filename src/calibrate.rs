//! Active Counterfactual Calibration ("Perturb Until They Flinch") and the Empirical Acceptance
//! Envelope.
//!
//! Instead of asking a non-designer to name design concepts or trusting raw pixel dumps of a
//! bounding box, we hold a reference steady, perturb one perceptual axis at a time, and record
//! whether the curator flinches (`LostIt`), accepts (`StillHasIt`), or skips (`DontCare`).
//!
//! From those judgments we derive an `Envelope`:
//! - `hard_invariants`: axes where a perturbation triggered a flinch, paired with only the
//!   anchor measurement relevant to that axis.
//! - `verified_tolerances`: axes where perturbations did not trigger a flinch — explicitly
//!   marking them as free variables the downstream builder agent can adapt.
//! - `boundary_counterexamples`: concrete rejected perturbation states.
//! - `contrastive_pairs`: "This, not that" pairs comparing an anchor artifact against a foil.

use serde::{Deserialize, Serialize};

use crate::model::{FlinchVerdict, NormRect, ProbeAxis, ProbeDirection, ProbeJudgment, Project};
use crate::telemetry::{
    self, oklab_from_srgb, oklch_from_oklab, srgb_from_oklab, srgb_from_oklch, Bitmap, Oklab,
    Oklch,
};

/// Default perturbation strength for a clear, non-designer-perceptible first step.
pub const DEFAULT_STRENGTH: f32 = 0.65;

/// Summary state of one axis on one artifact during calibration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisStatus {
    Untested,
    Partial,
    HardInvariant,
    Tolerance,
}

/// An axis where perturbing the reference caused the curator to flinch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HardInvariant {
    pub axis: ProbeAxis,
    pub artifact: u32,
    pub artifact_name: String,
    /// Human-readable description of what broke (e.g. "Pushed cooler").
    pub flinch_boundaries: Vec<String>,
    /// What held when pushed the other way, if tested and accepted.
    pub passing_directions: Vec<String>,
    /// Measurement taken from the anchor ONLY for this axis (never unrelated pixel dumps).
    pub measured_anchor: String,
}

/// An axis the curator tested and did not flinch on: explicitly free to adapt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifiedTolerance {
    pub axis: ProbeAxis,
    pub artifact: u32,
    pub artifact_name: String,
    pub reason: String,
}

/// A concrete rejected counterfactual state sitting just outside the curator's taste.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundaryCounterexample {
    pub artifact: u32,
    pub artifact_name: String,
    pub axis: ProbeAxis,
    pub direction: ProbeDirection,
    pub strength: f32,
    pub change: String,
    #[serde(default)]
    pub note: String,
}

/// A "This, not that" boundary rule between an anchor artifact and a foil artifact.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContrastiveRule {
    pub id: u32,
    pub anchor: u32,
    pub anchor_name: String,
    pub foil: u32,
    pub foil_name: String,
    pub note: String,
    /// Automatic plain-language visual delta between the two references.
    pub measured_delta: String,
}

/// The Empirical Acceptance Envelope: what a cold builder agent actually needs to transfer
/// taste without forced guesses or incidental over-copying.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Envelope {
    pub target: String,
    pub hard_invariants: Vec<HardInvariant>,
    pub verified_tolerances: Vec<VerifiedTolerance>,
    pub boundary_counterexamples: Vec<BoundaryCounterexample>,
    pub contrastive_pairs: Vec<ContrastiveRule>,
}

impl Envelope {
    pub fn is_empty(&self) -> bool {
        self.hard_invariants.is_empty()
            && self.verified_tolerances.is_empty()
            && self.boundary_counterexamples.is_empty()
            && self.contrastive_pairs.is_empty()
    }
}

/// Downscale a bitmap to fit within `max_side` so interactive perturbation stays instant.
pub fn preview_bitmap(source: &Bitmap, max_side: u32) -> Bitmap {
    let max_side = max_side.max(16);
    if source.width <= max_side && source.height <= max_side {
        return source.clone();
    }
    let scale = (max_side as f32 / source.width.max(source.height) as f32).min(1.0);
    let w = ((source.width as f32 * scale).round() as u32).max(1);
    let h = ((source.height as f32 * scale).round() as u32).max(1);
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let sy = ((y as f32 + 0.5) * source.height as f32 / h as f32)
            .floor()
            .clamp(0.0, (source.height - 1) as f32) as u32;
        for x in 0..w {
            let sx = ((x as f32 + 0.5) * source.width as f32 / w as f32)
                .floor()
                .clamp(0.0, (source.width - 1) as f32) as u32;
            let si = ((sy * source.width + sx) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&source.rgba[si..si + 4]);
        }
    }
    Bitmap::from_rgba8(w, h, out)
}

/// Apply one controlled single-axis counterfactual perturbation to `source`.
pub fn perturb(
    source: &Bitmap,
    axis: ProbeAxis,
    direction: ProbeDirection,
    strength: f32,
) -> Bitmap {
    let s = strength.clamp(0.05, 1.0) * direction.sign();
    match axis {
        ProbeAxis::Warmth => perturb_warmth(source, s),
        ProbeAxis::Contrast => perturb_contrast(source, s),
        ProbeAxis::Chroma => perturb_chroma(source, s),
        ProbeAxis::Lightness => perturb_lightness(source, s),
        ProbeAxis::Crispness => perturb_crispness(source, s),
        ProbeAxis::Breathing => perturb_breathing(source, s),
    }
}

fn perturb_warmth(source: &Bitmap, signed_s: f32) -> Bitmap {
    // Shift OKLab a/b warmer (+a, +b) or cooler (-a, -b) while preserving lightness L.
    let da = 0.022 * signed_s;
    let db = 0.055 * signed_s;
    map_pixels(source, |rgba| {
        let mut lab = oklab_from_srgb(rgba);
        lab.a += da;
        lab.b += db;
        let out = srgb_from_oklab(lab);
        [out[0], out[1], out[2], rgba[3]]
    })
}

fn perturb_contrast(source: &Bitmap, signed_s: f32) -> Bitmap {
    let mean_l = mean_lightness(source);
    // Negative compresses toward mean_l (0.45x at -1.0); Positive expands (1.55x at +1.0).
    let factor = (1.0 + 0.58 * signed_s).clamp(0.35, 1.85);
    map_pixels(source, |rgba| {
        let mut lab = oklab_from_srgb(rgba);
        lab.l = (mean_l + (lab.l - mean_l) * factor).clamp(0.0, 1.0);
        let out = srgb_from_oklab(lab);
        [out[0], out[1], out[2], rgba[3]]
    })
}

fn perturb_chroma(source: &Bitmap, signed_s: f32) -> Bitmap {
    let scale = (1.0 + 0.90 * signed_s).max(0.05);
    map_pixels(source, |rgba| {
        let lch = oklch_from_oklab(oklab_from_srgb(rgba));
        let out = srgb_from_oklch(Oklch {
            l: lch.l,
            c: (lch.c * scale).min(0.42),
            h: lch.h,
        });
        [out[0], out[1], out[2], rgba[3]]
    })
}

fn perturb_lightness(source: &Bitmap, signed_s: f32) -> Bitmap {
    let dl = 0.16 * signed_s;
    map_pixels(source, |rgba| {
        let mut lab = oklab_from_srgb(rgba);
        lab.l = (lab.l + dl).clamp(0.0, 1.0);
        let out = srgb_from_oklab(lab);
        [out[0], out[1], out[2], rgba[3]]
    })
}

fn perturb_crispness(source: &Bitmap, signed_s: f32) -> Bitmap {
    let radius = if signed_s.abs() > 0.5 { 2i32 } else { 1i32 };
    let blurred = box_blur(source, radius);
    if signed_s < 0.0 {
        return blurred;
    }
    // Unsharp mask: original + amount * (original - blurred).
    let amount = 1.6 * signed_s;
    let mut out = source.rgba.clone();
    for i in (0..out.len()).step_by(4) {
        for c in 0..3 {
            let orig = source.rgba[i + c] as f32;
            let blur = blurred.rgba[i + c] as f32;
            let v = (orig + amount * (orig - blur)).round().clamp(0.0, 255.0) as u8;
            out[i + c] = v;
        }
    }
    Bitmap::from_rgba8(source.width, source.height, out)
}

fn perturb_breathing(source: &Bitmap, signed_s: f32) -> Bitmap {
    let (w, h) = (source.width, source.height);
    if w < 8 || h < 8 {
        return source.clone();
    }
    let frac = (0.16 * signed_s.abs()).clamp(0.04, 0.22);
    if signed_s > 0.0 {
        // More outer room: frame the scaled-down reference in its dominant edge colour.
        let bg = edge_fill_color(source);
        let mut out = vec![0u8; (w * h * 4) as usize];
        for p in out.chunks_exact_mut(4) {
            p.copy_from_slice(&bg);
        }
        let pad_x = ((w as f32 * frac).round() as u32).min(w / 3);
        let pad_y = ((h as f32 * frac).round() as u32).min(h / 3);
        let inner_w = (w - pad_x * 2).max(1);
        let inner_h = (h - pad_y * 2).max(1);
        for y in 0..inner_h {
            let sy = ((y as f32 + 0.5) * h as f32 / inner_h as f32)
                .floor()
                .clamp(0.0, (h - 1) as f32) as u32;
            for x in 0..inner_w {
                let sx = ((x as f32 + 0.5) * w as f32 / inner_w as f32)
                    .floor()
                    .clamp(0.0, (w - 1) as f32) as u32;
                let si = ((sy * w + sx) * 4) as usize;
                let di = (((y + pad_y) * w + (x + pad_x)) * 4) as usize;
                out[di..di + 4].copy_from_slice(&source.rgba[si..si + 4]);
            }
        }
        Bitmap::from_rgba8(w, h, out)
    } else {
        // Tighter / cramped: crop inward and scale back up.
        let crop_x = ((w as f32 * frac).round() as u32).min(w / 3);
        let crop_y = ((h as f32 * frac).round() as u32).min(h / 3);
        let inner_w = (w - crop_x * 2).max(1);
        let inner_h = (h - crop_y * 2).max(1);
        let mut out = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            let sy = crop_y
                + ((y as f32 + 0.5) * inner_h as f32 / h as f32)
                    .floor()
                    .clamp(0.0, (inner_h - 1) as f32) as u32;
            for x in 0..w {
                let sx = crop_x
                    + ((x as f32 + 0.5) * inner_w as f32 / w as f32)
                        .floor()
                        .clamp(0.0, (inner_w - 1) as f32) as u32;
                let si = ((sy * w + sx) * 4) as usize;
                let di = ((y * w + x) * 4) as usize;
                out[di..di + 4].copy_from_slice(&source.rgba[si..si + 4]);
            }
        }
        Bitmap::from_rgba8(w, h, out)
    }
}

fn edge_fill_color(b: &Bitmap) -> [u8; 4] {
    let mut sum = [0u64; 3];
    let mut count = 0u64;
    for x in 0..b.width {
        for y in [0, b.height.saturating_sub(1)] {
            let i = ((y * b.width + x) * 4) as usize;
            sum[0] += b.rgba[i] as u64;
            sum[1] += b.rgba[i + 1] as u64;
            sum[2] += b.rgba[i + 2] as u64;
            count += 1;
        }
    }
    if count == 0 {
        return [240, 240, 240, 255];
    }
    [(sum[0] / count) as u8, (sum[1] / count) as u8, (sum[2] / count) as u8, 255]
}

fn box_blur(source: &Bitmap, radius: i32) -> Bitmap {
    let (w, h) = (source.width as i32, source.height as i32);
    let mut tmp = vec![0u8; source.rgba.len()];
    let mut out = vec![0u8; source.rgba.len()];
    let span = (radius * 2 + 1) as u32;
    // Horizontal pass.
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dx in -radius..=radius {
                let sx = (x + dx).clamp(0, w - 1);
                let i = ((y * w + sx) * 4) as usize;
                for c in 0..4 {
                    acc[c] += source.rgba[i + c] as u32;
                }
            }
            let di = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                tmp[di + c] = (acc[c] / span) as u8;
            }
        }
    }
    // Vertical pass.
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            for dy in -radius..=radius {
                let sy = (y + dy).clamp(0, h - 1);
                let i = ((sy * w + x) * 4) as usize;
                for c in 0..4 {
                    acc[c] += tmp[i + c] as u32;
                }
            }
            let di = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                out[di + c] = (acc[c] / span) as u8;
            }
        }
    }
    Bitmap::from_rgba8(source.width, source.height, out)
}

fn map_pixels(source: &Bitmap, f: impl Fn([f32; 4]) -> [f32; 4]) -> Bitmap {
    let mut out = vec![0u8; source.rgba.len()];
    for i in (0..source.rgba.len()).step_by(4) {
        let p = [
            source.rgba[i] as f32 / 255.0,
            source.rgba[i + 1] as f32 / 255.0,
            source.rgba[i + 2] as f32 / 255.0,
            source.rgba[i + 3] as f32 / 255.0,
        ];
        let q = f(p);
        out[i] = (q[0].clamp(0.0, 1.0) * 255.0).round() as u8;
        out[i + 1] = (q[1].clamp(0.0, 1.0) * 255.0).round() as u8;
        out[i + 2] = (q[2].clamp(0.0, 1.0) * 255.0).round() as u8;
        out[i + 3] = (q[3].clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    Bitmap::from_rgba8(source.width, source.height, out)
}

fn mean_lightness(b: &Bitmap) -> f32 {
    let total = (b.width * b.height) as usize;
    if total == 0 {
        return 0.5;
    }
    let step = (total / 4096).max(1);
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for idx in (0..total).step_by(step) {
        let i = idx * 4;
        let lab = oklab_from_srgb([
            b.rgba[i] as f32 / 255.0,
            b.rgba[i + 1] as f32 / 255.0,
            b.rgba[i + 2] as f32 / 255.0,
            1.0,
        ]);
        sum += lab.l as f64;
        n += 1;
    }
    (sum / n.max(1) as f64) as f32
}

/// Summary statistics for targeted anchor measurement (only exposed when an axis flinches).
struct AnchorStats {
    mean_l: f32,
    mean_a: f32,
    mean_b: f32,
    mean_c: f32,
    contrast_ratio: Option<f32>,
    edge_energy: f32,
    border_share: f32,
}

fn measure_anchor(b: &Bitmap) -> AnchorStats {
    let total = (b.width * b.height) as usize;
    let step = (total / 4096).max(1);
    let (mut l, mut a, mut bb, mut c) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut n = 0usize;
    for idx in (0..total).step_by(step) {
        let i = idx * 4;
        let lab: Oklab = oklab_from_srgb([
            b.rgba[i] as f32 / 255.0,
            b.rgba[i + 1] as f32 / 255.0,
            b.rgba[i + 2] as f32 / 255.0,
            1.0,
        ]);
        l += lab.l as f64;
        a += lab.a as f64;
        bb += lab.b as f64;
        c += (lab.a * lab.a + lab.b * lab.b).sqrt() as f64;
        n += 1;
    }
    let nf = n.max(1) as f64;
    let full = NormRect::new(0.0, 0.0, 1.0, 1.0);
    let clusters = telemetry::dominant_clusters(b, &full, 4);
    let contrast_ratio = telemetry::contrast_reading(&clusters).map(|r| r.ratio);

    // Simple horizontal gradient energy for edge crispness.
    let mut edge_sum = 0.0f64;
    let mut edge_n = 0usize;
    let stride_y = (b.height / 64).max(1);
    let stride_x = (b.width / 64).max(1);
    for y in (0..b.height).step_by(stride_y as usize) {
        for x in (1..b.width).step_by(stride_x as usize) {
            let i0 = ((y * b.width + x - 1) * 4) as usize;
            let i1 = ((y * b.width + x) * 4) as usize;
            let d = (b.rgba[i1] as f32 - b.rgba[i0] as f32).abs()
                + (b.rgba[i1 + 1] as f32 - b.rgba[i0 + 1] as f32).abs()
                + (b.rgba[i1 + 2] as f32 - b.rgba[i0 + 2] as f32).abs();
            edge_sum += (d / (3.0 * 255.0)) as f64;
            edge_n += 1;
        }
    }

    // Share of pixels matching the outer edge fill colour (proxy for breathing / ground).
    let edge = edge_fill_color(b);
    let mut bg_hits = 0usize;
    let mut bg_n = 0usize;
    for idx in (0..total).step_by(step) {
        let i = idx * 4;
        let d = (b.rgba[i] as i32 - edge[0] as i32).abs()
            + (b.rgba[i + 1] as i32 - edge[1] as i32).abs()
            + (b.rgba[i + 2] as i32 - edge[2] as i32).abs();
        if d < 36 {
            bg_hits += 1;
        }
        bg_n += 1;
    }

    AnchorStats {
        mean_l: (l / nf) as f32,
        mean_a: (a / nf) as f32,
        mean_b: (bb / nf) as f32,
        mean_c: (c / nf) as f32,
        contrast_ratio,
        edge_energy: (edge_sum / edge_n.max(1) as f64) as f32,
        border_share: bg_hits as f32 / bg_n.max(1) as f32,
    }
}

fn axis_measurement(axis: ProbeAxis, stats: &AnchorStats) -> String {
    match axis {
        ProbeAxis::Warmth => {
            let lean = if stats.mean_b > 0.012 {
                "warm-leaning"
            } else if stats.mean_b < -0.012 {
                "cool-leaning"
            } else {
                "near-neutral"
            };
            format!("anchor mean OKLab a={:+.3}, b={:+.3} ({lean})", stats.mean_a, stats.mean_b)
        }
        ProbeAxis::Contrast => match stats.contrast_ratio {
            Some(r) => format!("anchor primary separation {:.1}:1", r),
            None => format!("anchor mean lightness L={:.2}", stats.mean_l),
        },
        ProbeAxis::Chroma => format!("anchor mean OKLCH chroma C={:.3}", stats.mean_c),
        ProbeAxis::Lightness => format!("anchor mean OKLab lightness L={:.2}", stats.mean_l),
        ProbeAxis::Crispness => format!("anchor edge sharpness index {:.3}", stats.edge_energy),
        ProbeAxis::Breathing => {
            format!("anchor open ground share ~{:.0}%", stats.border_share * 100.0)
        }
    }
}

/// Adaptive next probe for `artifact`:
/// - Walks through all 6 axes.
/// - If an axis got `DontCare`, skips the opposite direction (respecting curator attention).
/// - Otherwise brackets both directions (`Negative` then `Positive`).
pub fn next_probe(project: &Project, artifact: u32) -> Option<(ProbeAxis, ProbeDirection, f32)> {
    for axis in ProbeAxis::ALL {
        let judged: Vec<&ProbeJudgment> = project
            .probes
            .iter()
            .filter(|p| p.artifact == artifact && p.axis == axis)
            .collect();
        if judged.iter().any(|p| p.verdict == FlinchVerdict::DontCare) {
            continue;
        }
        let has_neg = judged.iter().any(|p| p.direction == ProbeDirection::Negative);
        let has_pos = judged.iter().any(|p| p.direction == ProbeDirection::Positive);
        if !has_neg {
            return Some((axis, ProbeDirection::Negative, DEFAULT_STRENGTH));
        }
        if !has_pos {
            return Some((axis, ProbeDirection::Positive, DEFAULT_STRENGTH));
        }
    }
    None
}

/// Status of one axis on one artifact for the UI strip.
pub fn axis_status(project: &Project, artifact: u32, axis: ProbeAxis) -> AxisStatus {
    let judged: Vec<&ProbeJudgment> = project
        .probes
        .iter()
        .filter(|p| p.artifact == artifact && p.axis == axis)
        .collect();
    if judged.is_empty() {
        return AxisStatus::Untested;
    }
    if judged.iter().any(|p| p.verdict == FlinchVerdict::LostIt) {
        return AxisStatus::HardInvariant;
    }
    if judged.iter().any(|p| p.verdict == FlinchVerdict::DontCare) || judged.len() >= 2 {
        return AxisStatus::Tolerance;
    }
    AxisStatus::Partial
}

/// Build the Empirical Acceptance Envelope from all recorded probes and contrastive pairs.
pub fn build_envelope(
    project: &Project,
    bitmaps: &dyn Fn(u32, Option<u32>) -> Option<Bitmap>,
) -> Envelope {
    let mut hard_invariants = Vec::new();
    let mut verified_tolerances = Vec::new();
    let mut boundary_counterexamples = Vec::new();
    let mut contrastive_pairs = Vec::new();

    // Group probes by (artifact, axis) in artifact order then ProbeAxis::ALL order.
    for artifact in &project.artifacts {
        let art_probes: Vec<&ProbeJudgment> = project
            .probes
            .iter()
            .filter(|p| p.artifact == artifact.id)
            .collect();
        if art_probes.is_empty() {
            continue;
        }
        let stats = bitmaps(artifact.id, Some(1)).map(|b| measure_anchor(&b));

        for axis in ProbeAxis::ALL {
            let group: Vec<&ProbeJudgment> =
                art_probes.iter().copied().filter(|p| p.axis == axis).collect();
            if group.is_empty() {
                continue;
            }
            let lost: Vec<&ProbeJudgment> = group
                .iter()
                .copied()
                .filter(|p| p.verdict == FlinchVerdict::LostIt)
                .collect();
            let held: Vec<&ProbeJudgment> = group
                .iter()
                .copied()
                .filter(|p| p.verdict == FlinchVerdict::StillHasIt)
                .collect();
            let dont_care = group.iter().any(|p| p.verdict == FlinchVerdict::DontCare);

            if !lost.is_empty() {
                for p in &lost {
                    boundary_counterexamples.push(BoundaryCounterexample {
                        artifact: artifact.id,
                        artifact_name: artifact.name.clone(),
                        axis,
                        direction: p.direction,
                        strength: p.strength,
                        change: axis.change_label(p.direction).into(),
                        note: p.note.clone(),
                    });
                }
                let measured_anchor = stats
                    .as_ref()
                    .map(|s| axis_measurement(axis, s))
                    .unwrap_or_else(|| "no raster substrate".into());
                hard_invariants.push(HardInvariant {
                    axis,
                    artifact: artifact.id,
                    artifact_name: artifact.name.clone(),
                    flinch_boundaries: lost
                        .iter()
                        .map(|p| axis.change_label(p.direction).to_string())
                        .collect(),
                    passing_directions: held
                        .iter()
                        .map(|p| axis.change_label(p.direction).to_string())
                        .collect(),
                    measured_anchor,
                });
            } else if dont_care || !held.is_empty() {
                let reason = if dont_care {
                    "Curator marked don't care — explicitly free to adapt.".into()
                } else {
                    let dirs: Vec<&str> = held
                        .iter()
                        .map(|p| axis.change_label(p.direction))
                        .collect();
                    format!("Held under perturbation ({}) — safe to adapt.", dirs.join(", "))
                };
                verified_tolerances.push(VerifiedTolerance {
                    axis,
                    artifact: artifact.id,
                    artifact_name: artifact.name.clone(),
                    reason,
                });
            }
        }
    }

    // Contrastive pairs ("This, not that").
    for pair in &project.pairs {
        let Some(anchor) = project.artifact(pair.anchor) else { continue };
        let Some(foil) = project.artifact(pair.foil) else { continue };
        let measured_delta = match (bitmaps(anchor.id, Some(1)), bitmaps(foil.id, Some(1))) {
            (Some(a), Some(f)) => describe_pair_delta(&measure_anchor(&a), &measure_anchor(&f)),
            _ => "visual comparison without raster pixels".into(),
        };
        contrastive_pairs.push(ContrastiveRule {
            id: pair.id,
            anchor: anchor.id,
            anchor_name: anchor.name.clone(),
            foil: foil.id,
            foil_name: foil.name.clone(),
            note: pair.note.clone(),
            measured_delta,
        });
    }

    Envelope {
        target: project.target.clone(),
        hard_invariants,
        verified_tolerances,
        boundary_counterexamples,
        contrastive_pairs,
    }
}

fn describe_pair_delta(a: &AnchorStats, f: &AnchorStats) -> String {
    let mut diffs = Vec::new();
    let dl = a.mean_l - f.mean_l;
    if dl.abs() > 0.06 {
        diffs.push(if dl > 0.0 { "anchor is brighter" } else { "anchor is darker" });
    }
    let dc = a.mean_c - f.mean_c;
    if dc.abs() > 0.015 {
        diffs.push(if dc > 0.0 { "anchor has stronger colour" } else { "anchor is more muted" });
    }
    let db = a.mean_b - f.mean_b;
    if db.abs() > 0.015 {
        diffs.push(if db > 0.0 { "anchor is warmer" } else { "anchor is cooler" });
    }
    let dg = a.border_share - f.border_share;
    if dg.abs() > 0.08 {
        diffs.push(if dg > 0.0 { "anchor has more open ground" } else { "anchor is denser" });
    }
    if diffs.is_empty() {
        "similar global tone; difference is structural".into()
    } else {
        diffs.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ArtifactSource;

    fn sample_plate() -> Bitmap {
        let (w, h) = (32u32, 32u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                let col = if x >= 8 && x < 24 && y >= 8 && y < 24 {
                    [40, 120, 90, 255]
                } else {
                    [235, 232, 224, 255]
                };
                rgba[i..i + 4].copy_from_slice(&col);
            }
        }
        Bitmap::from_rgba8(w, h, rgba)
    }

    #[test]
    fn every_axis_perturbation_changes_the_pixels_deterministically() {
        let src = sample_plate();
        for axis in ProbeAxis::ALL {
            for dir in [ProbeDirection::Negative, ProbeDirection::Positive] {
                let a = perturb(&src, axis, dir, DEFAULT_STRENGTH);
                let b = perturb(&src, axis, dir, DEFAULT_STRENGTH);
                assert_eq!(a.rgba, b.rgba, "{axis:?} {dir:?} must be deterministic");
                assert_ne!(a.rgba, src.rgba, "{axis:?} {dir:?} must visibly change pixels");
            }
        }
    }

    #[test]
    fn dont_care_skips_the_opposite_direction_on_that_axis() {
        let mut p = Project::default();
        let a = p.add_artifact(
            "a.png".into(),
            ArtifactSource::Image { path: "a.png".into(), width: 32, height: 32 },
        );
        let first = next_probe(&p, a).unwrap();
        assert_eq!(first.0, ProbeAxis::Warmth);
        p.record_probe(ProbeJudgment {
            artifact: a,
            axis: first.0,
            direction: first.1,
            strength: first.2,
            verdict: FlinchVerdict::DontCare,
            note: String::new(),
        });
        let second = next_probe(&p, a).unwrap();
        assert_eq!(
            second.0,
            ProbeAxis::Contrast,
            "DontCare must skip the opposite direction and advance to the next axis"
        );
    }

    #[test]
    fn envelope_separates_hard_invariants_from_verified_tolerances() {
        let mut p = Project { target: "reading list page".into(), ..Project::default() };
        let a = p.add_artifact(
            "a.png".into(),
            ArtifactSource::Image { path: "a.png".into(), width: 32, height: 32 },
        );
        let b = p.add_artifact(
            "b.png".into(),
            ArtifactSource::Image { path: "b.png".into(), width: 32, height: 32 },
        );
        p.record_probe(ProbeJudgment {
            artifact: a,
            axis: ProbeAxis::Warmth,
            direction: ProbeDirection::Negative,
            strength: 0.65,
            verdict: FlinchVerdict::LostIt,
            note: "too clinical".into(),
        });
        p.record_probe(ProbeJudgment {
            artifact: a,
            axis: ProbeAxis::Chroma,
            direction: ProbeDirection::Negative,
            strength: 0.65,
            verdict: FlinchVerdict::StillHasIt,
            note: String::new(),
        });
        p.add_pair(a, b, "keep a's calm, avoid b's clutter".into());

        let plate = sample_plate();
        let env = build_envelope(&p, &|_, _| Some(plate.clone()));
        assert_eq!(env.target, "reading list page");
        assert_eq!(env.hard_invariants.len(), 1);
        assert_eq!(env.hard_invariants[0].axis, ProbeAxis::Warmth);
        assert!(env.hard_invariants[0].measured_anchor.contains("OKLab"));
        assert_eq!(env.verified_tolerances.len(), 1);
        assert_eq!(env.verified_tolerances[0].axis, ProbeAxis::Chroma);
        assert_eq!(env.boundary_counterexamples.len(), 1);
        assert_eq!(env.contrastive_pairs.len(), 1);
    }
}

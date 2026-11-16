//! Deterministic perceptual measurements taken from the *unannotated* pixels inside a selection
//! (and in a ring around it). The downstream agent is handed numbers, never asked to eyeball a
//! compressed crop. Everything here is pure: same bytes in, same numbers out.

use serde::{Deserialize, Serialize};

/// sRGB in `0..=1`, straight alpha.
pub type Rgba = [f32; 4];

/// OKLab, as published by Björn Ottosson. `l` is 0..1 lightness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

/// OKLCH: OKLab in polar form, `h` in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Oklch {
    pub l: f32,
    pub c: f32,
    pub h: f32,
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
}

pub fn oklab_from_srgb(c: Rgba) -> Oklab {
    let r = srgb_to_linear(c[0].clamp(0.0, 1.0));
    let g = srgb_to_linear(c[1].clamp(0.0, 1.0));
    let b = srgb_to_linear(c[2].clamp(0.0, 1.0));
    let l = 0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_99 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l_, m_, s_) = (l.cbrt(), m.cbrt(), s.cbrt());
    Oklab {
        l: 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
        a: 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        b: 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    }
}

pub fn srgb_from_oklab(c: Oklab) -> Rgba {
    let l_ = c.l + 0.396_337_78 * c.a + 0.215_803_76 * c.b;
    let m_ = c.l - 0.105_561_346 * c.a - 0.063_854_17 * c.b;
    let s_ = c.l - 0.089_484_18 * c.a - 1.291_485_5 * c.b;
    let (l, m, s) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s;
    let g = -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s;
    let b = -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s;
    [linear_to_srgb(r.clamp(0.0, 1.0)), linear_to_srgb(g.clamp(0.0, 1.0)), linear_to_srgb(b.clamp(0.0, 1.0)), 1.0]
}

pub fn oklch_from_oklab(c: Oklab) -> Oklch {
    let c_ = (c.a * c.a + c.b * c.b).sqrt();
    let mut h = c.b.atan2(c.a).to_degrees();
    if h < 0.0 { h += 360.0; }
    if c_ < 1e-6 { h = 0.0; }
    Oklch { l: c.l, c: c_, h }
}

pub fn oklch_from_srgb(c: Rgba) -> Oklch { oklch_from_oklab(oklab_from_srgb(c)) }

pub fn srgb_from_oklch(c: Oklch) -> Rgba {
    let h = c.h.to_radians();
    srgb_from_oklab(Oklab { l: c.l, a: c.c * h.cos(), b: c.c * h.sin() })
}

/// WCAG 2 relative luminance from sRGB, straight alpha composited over white when needed.
pub fn relative_luminance(c: Rgba) -> f32 {
    let a = c[3].clamp(0.0, 1.0);
    let over_white = |v: f32| v * a + (1.0 - a);
    0.2126 * srgb_to_linear(over_white(c[0]).clamp(0.0, 1.0))
        + 0.7152 * srgb_to_linear(over_white(c[1]).clamp(0.0, 1.0))
        + 0.0722 * srgb_to_linear(over_white(c[2]).clamp(0.0, 1.0))
}

/// WCAG contrast ratio, `(1.05) / (0.05)` at the extremes, and always `>= 1`.
pub fn contrast_ratio(a: Rgba, b: Rgba) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

/// Perceptual distance in OKLab; roughly `0.02` is a just-noticeable difference.
pub fn delta_e(a: Oklab, b: Oklab) -> f32 {
    ((a.l - b.l).powi(2) + (a.a - b.a).powi(2) + (a.b - b.b).powi(2)).sqrt()
}

/// One dominant colour cluster in a selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ColorCluster {
    /// Share of the sampled pixels this cluster covers, `0..1`, all clusters sum to 1.
    pub share: f32,
    pub srgb: [f32; 3],
    pub oklch: Oklch,
    /// Hex, uppercase without `#`, for humans and quick machine use.
    pub hex: String,
}

impl ColorCluster {
    fn from_oklab(share: f32, lab: Oklab) -> ColorCluster {
        let srgb = srgb_from_oklab(lab);
        ColorCluster {
            share,
            srgb: [srgb[0], srgb[1], srgb[2]],
            oklch: oklch_from_oklab(lab),
            hex: hex_of(srgb),
        }
    }
}

pub fn hex_of(c: Rgba) -> String {
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("{:02X}{:02X}{:02X}", to(c[0]), to(c[1]), to(c[2]))
}

/// A straight-alpha RGBA8 bitmap, rows top first. This is the unaltered decoded artifact, or a
/// raster of a PDF page; annotations never write into it.
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, straight sRGB.
    pub rgba: Vec<u8>,
}

impl Bitmap {
    pub fn pixel(&self, x: u32, y: u32) -> Rgba {
        let i = ((y.min(self.height.saturating_sub(1)) * self.width + x.min(self.width.saturating_sub(1))) * 4) as usize;
        match self.rgba.get(i..i + 4) {
            Some(p) => [p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0, p[3] as f32 / 255.0],
            None => [0.0, 0.0, 0.0, 0.0],
        }
    }

    /// Pixels inside a normalized rect, as a bounding box in integer pixel coordinates.
    pub fn rect_px(&self, r: &crate::model::NormRect) -> (u32, u32, u32, u32) {
        let x0 = (r.x0 * self.width as f32).floor().clamp(0.0, self.width as f32) as u32;
        let y0 = (r.y0 * self.height as f32).floor().clamp(0.0, self.height as f32) as u32;
        let x1 = (r.x1 * self.width as f32).ceil().clamp(0.0, self.width as f32) as u32;
        let y1 = (r.y1 * self.height as f32).ceil().clamp(0.0, self.height as f32) as u32;
        (x0, y0, x1.max(x0 + 1).min(self.width), y1.max(y0 + 1).min(self.height))
    }

    /// A crop at full resolution, never resampled.
    pub fn crop(&self, r: &crate::model::NormRect) -> Option<Bitmap> {
        let (x0, y0, x1, y1) = self.rect_px(r);
        let (w, h) = (x1 - x0, y1 - y0);
        if w == 0 || h == 0 { return None; }
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in y0..y1 {
            let start = ((y * self.width + x0) * 4) as usize;
            out.extend_from_slice(&self.rgba[start..start + (w * 4) as usize]);
        }
        Some(Bitmap { width: w, height: h, rgba: out })
    }

    pub fn from_rgba8(width: u32, height: u32, rgba: Vec<u8>) -> Bitmap { Bitmap { width, height, rgba } }

    /// Decode a PNG or JPEG from bytes, as imported.
    pub fn decode(bytes: &[u8]) -> Result<Bitmap, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("decode: {e}"))?;
        let rgba = img.to_rgba8();
        Ok(Bitmap { width: rgba.width(), height: rgba.height(), rgba: rgba.into_raw() })
    }
}

/// Deterministic k-means in OKLab, seeded by a fixed stride (no RNG, so results are stable and
/// reproducible across machines and runs).
pub fn dominant_clusters(bitmap: &Bitmap, region: &crate::model::NormRect, k: usize) -> Vec<ColorCluster> {
    dominant_clusters_in(bitmap, region, k, 0)
}

/// The same, but skipping a border of `inset` pixels so a selection's own stroke colour doesn't
/// dominate its interior.
pub fn dominant_clusters_in(bitmap: &Bitmap, region: &crate::model::NormRect, k: usize, inset: u32) -> Vec<ColorCluster> {
    let (x0, y0, x1, y1) = bitmap.rect_px(region);
    let (ix0, iy0) = (x0 + inset, y0 + inset);
    let (ix1, iy1) = (x1.saturating_sub(inset).max(ix0 + 1), y1.saturating_sub(inset).max(iy0 + 1));
    let (w, h) = (ix1.saturating_sub(ix0), iy1.saturating_sub(iy0));
    if w == 0 || h == 0 || k == 0 { return Vec::new(); }

    // Sample at most ~40k pixels, in a fixed grid, and weight each by alpha so transparent
    // areas don't create phantom clusters.
    let target = 40_000usize;
    let stride = (((w as usize * h as usize) as f64 / target as f64).sqrt().ceil() as usize).max(1);
    let mut samples: Vec<(Oklab, f32)> = Vec::new();
    let mut y = iy0 as usize;
    while y < iy1 as usize {
        let mut x = ix0 as usize;
        while x < ix1 as usize {
            let c = bitmap.pixel(x as u32, y as u32);
            if c[3] > 0.02 { samples.push((oklab_from_srgb(c), c[3])); }
            x += stride;
        }
        y += stride;
    }
    if samples.is_empty() {
        // Fully transparent: one cluster of transparent black so the plate still says something.
        return vec![ColorCluster::from_oklab(1.0, Oklab::default())];
    }
    kmeans(&samples, k.min(samples.len()).max(1))
}

fn kmeans(samples: &[(Oklab, f32)], k: usize) -> Vec<ColorCluster> {
    // Deterministic farthest-point seeding: start at a fixed sample, then repeatedly take the
    // sample furthest from every centre so far. A small dark patch inside a large light plate is
    // then a centre of its own instead of being averaged away, and the result is stable.
    let mut centres: Vec<Oklab> = vec![samples[0].0];
    let mut nearest: Vec<f32> = samples.iter().map(|(p, _)| delta_e(*p, centres[0])).collect();
    while centres.len() < k {
        let (idx, d) = nearest.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, d)| (i, *d)).unwrap_or((0, 0.0));
        if d < 1e-5 { break; }
        let next = samples[idx].0;
        centres.push(next);
        for (i, (p, _)) in samples.iter().enumerate() {
            let d = delta_e(*p, next);
            if d < nearest[i] { nearest[i] = d; }
        }
    }
    let mut assign = vec![0usize; samples.len()];
    for _ in 0..12 {
        let mut changed = false;
        for (i, (p, _)) in samples.iter().enumerate() {
            let mut best = 0;
            let mut best_d = f32::INFINITY;
            for (j, c) in centres.iter().enumerate() {
                let d = delta_e(*p, *c);
                if d < best_d { best_d = d; best = j; }
            }
            if assign[i] != best { assign[i] = best; changed = true; }
        }
        let mut sum = vec![(0.0f64, 0.0f64, 0.0f64, 0.0f64); k];
        for (i, (p, w)) in samples.iter().enumerate() {
            let s = &mut sum[assign[i]];
            let w = *w as f64;
            s.0 += p.l as f64 * w;
            s.1 += p.a as f64 * w;
            s.2 += p.b as f64 * w;
            s.3 += w;
        }
        for (j, s) in sum.iter().enumerate() {
            if s.3 > 0.0 { centres[j] = Oklab { l: (s.0 / s.3) as f32, a: (s.1 / s.3) as f32, b: (s.2 / s.3) as f32 }; }
        }
        if !changed { break; }
    }
    let mut weight = vec![0.0f64; k];
    let mut sum = vec![(0.0f64, 0.0f64, 0.0f64); k];
    let mut total = 0.0f64;
    for (i, (p, w)) in samples.iter().enumerate() {
        let w = *w as f64;
        weight[assign[i]] += w;
        let s = &mut sum[assign[i]];
        s.0 += p.l as f64 * w;
        s.1 += p.a as f64 * w;
        s.2 += p.b as f64 * w;
        total += w;
    }
    let mut out: Vec<ColorCluster> = (0..k)
        .filter(|j| weight[*j] > 0.0)
        .map(|j| {
            let s = sum[j];
            let w = weight[j];
            ColorCluster::from_oklab((w / total) as f32, Oklab { l: (s.0 / w) as f32, a: (s.1 / w) as f32, b: (s.2 / w) as f32 })
        })
        .collect();
    out.sort_by(|a, b| b.share.total_cmp(&a.share));
    out
}

/// The foreground/background pair of a selection: background is the largest cluster, foreground
/// the largest cluster far enough from it in OKLab to be a distinct thing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContrastReading {
    pub background: [f32; 3],
    pub foreground: [f32; 3],
    pub ratio: f32,
    /// WCAG level for body text at this ratio.
    pub level: String,
}

pub fn contrast_reading(clusters: &[ColorCluster]) -> Option<ContrastReading> {
    let bg = clusters.first()?;
    let bg_lab = oklab_from_srgb([bg.srgb[0], bg.srgb[1], bg.srgb[2], 1.0]);
    let fg = clusters.iter().skip(1)
        .filter(|c| delta_e(oklab_from_srgb([c.srgb[0], c.srgb[1], c.srgb[2], 1.0]), bg_lab) > 0.08)
        .max_by(|a, b| a.share.total_cmp(&b.share))
        .or_else(|| clusters.get(1))?;
    let ratio = contrast_ratio([bg.srgb[0], bg.srgb[1], bg.srgb[2], 1.0], [fg.srgb[0], fg.srgb[1], fg.srgb[2], 1.0]);
    let level = if ratio >= 7.0 { "AAA" } else if ratio >= 4.5 { "AA" } else if ratio >= 3.0 { "AA-large" } else { "below AA" };
    Some(ContrastReading {
        background: bg.srgb,
        foreground: fg.srgb,
        ratio,
        level: level.into(),
    })
}

/// Geometry of a selection, in artifact pixels and proportion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeometryReading {
    /// Normalized size of the selection.
    pub norm_w: f32,
    pub norm_h: f32,
    /// Pixel width/height at full resolution.
    pub px_w: u32,
    pub px_h: u32,
    /// Width divided by height.
    pub aspect: f32,
    /// Nearest simple ratio, such as `16:9`, when one is within 2%.
    pub ratio_name: String,
    /// How far the region's centre sits from the artifact's centre, `0..~1.41`.
    pub offset_from_center: f32,
    /// Share of the artifact's area the selection covers.
    pub area_share: f32,
}

pub fn geometry_reading(bitmap: &Bitmap, region: &crate::model::NormRect) -> GeometryReading {
    let (x0, y0, x1, y1) = bitmap.rect_px(region);
    let (px_w, px_h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    // Proportions are read in pixels: a square selection on a 2:1 page is not 1:1 to the eye.
    let aspect = if px_h > 0 { px_w as f32 / px_h as f32 } else { 0.0 };
    let (cx, cy) = (region.center()[0] - 0.5, region.center()[1] - 0.5);
    GeometryReading {
        norm_w: region.width(),
        norm_h: region.height(),
        px_w,
        px_h,
        aspect,
        ratio_name: simple_ratio(aspect),
        offset_from_center: (cx * cx + cy * cy).sqrt(),
        area_share: region.width() * region.height(),
    }
}

/// A ratio name for an aspect within 2%, from a small table of common ones.
pub fn simple_ratio(aspect: f32) -> String {
    if !aspect.is_finite() || aspect <= 0.0 { return String::new(); }
    const COMMON: [(u32, u32); 10] = [(1, 1), (4, 3), (3, 2), (16, 9), (21, 9), (2, 1), (3, 4), (9, 16), (1, 2), (5, 4)];
    for (w, h) in COMMON {
        let target = w as f32 / h as f32;
        if (aspect - target).abs() / target <= 0.02 { return format!("{w}:{h}"); }
    }
    format!("{aspect:.2}:1")
}

/// The ring just outside a selection: how its surroundings contrast with it. Used to say things
/// like "this chip sits on a much lighter plate" without the agent guessing from a crop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurroundReading {
    pub ring_share: f32,
    pub ring_background: [f32; 3],
    pub ring_foreground: [f32; 3],
    pub ring_contrast: f32,
    /// Contrast between the selection's background and the ring's background.
    pub edge_contrast: f32,
}

pub fn surround_reading(bitmap: &Bitmap, region: &crate::model::NormRect, inside: &[ColorCluster]) -> Option<SurroundReading> {
    let pad = (0.04f32).max(region.width().min(region.height()) * 0.5);
    let outer = region.padded(pad);
    let (x0, y0, x1, y1) = outer_px(bitmap, &outer);
    let (ix0, iy0, ix1, iy1) = bitmap.rect_px(region);
    let (w, h) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    if w == 0 || h == 0 { return None; }
    let mut ring: Vec<(Oklab, f32)> = Vec::new();
    let stride = (((w as usize * h as usize) as f64 / 20_000.0).sqrt().ceil() as usize).max(1);
    let mut y = y0 as usize;
    while y < y1 as usize {
        let mut x = x0 as usize;
        while x < x1 as usize {
            let inside_rect = x >= ix0 as usize && x < ix1 as usize && y >= iy0 as usize && y < iy1 as usize;
            if !inside_rect {
                let c = bitmap.pixel(x as u32, y as u32);
                if c[3] > 0.02 { ring.push((oklab_from_srgb(c), c[3])); }
            }
            x += stride;
        }
        y += stride;
    }
    if ring.is_empty() { return None; }
    let clusters = kmeans(&ring, 3);
    // A flat ring is one cluster: its background is the ring colour and it has no distinct
    // foreground, so its own contrast is 1.
    let reading = contrast_reading(&clusters).unwrap_or(ContrastReading {
        background: clusters[0].srgb,
        foreground: clusters[0].srgb,
        ratio: 1.0,
        level: "below AA".into(),
    });
    let share = clusters.iter().map(|c| c.share).sum::<f32>() * pad.min(0.5);
    let inner_bg = inside.first().map(|c| [c.srgb[0], c.srgb[1], c.srgb[2], 1.0]);
    let edge_contrast = inner_bg.map(|b| contrast_ratio(b, [reading.background[0], reading.background[1], reading.background[2], 1.0])).unwrap_or(1.0);
    Some(SurroundReading {
        ring_share: share.clamp(0.0, 1.0),
        ring_background: reading.background,
        ring_foreground: reading.foreground,
        ring_contrast: reading.ratio,
        edge_contrast,
    })
}

fn outer_px(bitmap: &Bitmap, r: &crate::model::NormRect) -> (u32, u32, u32, u32) { bitmap.rect_px(r) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NormRect;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> Bitmap {
        Bitmap { width: w, height: h, rgba: c.iter().copied().cycle().take((w * h * 4) as usize).collect() }
    }

    #[test]
    fn oklch_known_values() {
        let white = oklch_from_srgb([1.0, 1.0, 1.0, 1.0]);
        assert!((white.l - 1.0).abs() < 0.002, "{white:?}");
        assert!(white.c < 0.002, "{white:?}");
        let black = oklch_from_srgb([0.0, 0.0, 0.0, 1.0]);
        assert!(black.l.abs() < 0.002, "{black:?}");
        let red = oklch_from_srgb([1.0, 0.0, 0.0, 1.0]);
        assert!((red.h - 29.23).abs() < 1.0, "{red:?}");
        assert!((red.c - 0.2577).abs() < 0.01, "{red:?}");
    }

    #[test]
    fn oklab_round_trips() {
        for c in [[0.2f32, 0.4, 0.8], [0.9, 0.7, 0.1], [0.5, 0.5, 0.5], [0.05, 0.9, 0.3]] {
            let back = srgb_from_oklab(oklab_from_srgb([c[0], c[1], c[2], 1.0]));
            for i in 0..3 { assert!((back[i] - c[i]).abs() < 0.002, "{c:?} -> {back:?}"); }
        }
        let back = srgb_from_oklch(oklch_from_srgb([0.3, 0.6, 0.9, 1.0]));
        assert!((back[2] - 0.9).abs() < 0.01, "{back:?}");
    }

    #[test]
    fn contrast_matches_wcag() {
        let ratio = contrast_ratio([0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
        assert!((ratio - 21.0).abs() < 0.01, "{ratio}");
        assert!((contrast_ratio([1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn clusters_find_the_two_colours_in_a_split_region() {
        let mut b = solid(100, 100, [255, 255, 255, 255]);
        for y in 0..100 { for x in 0..50 { let i = ((y * 100 + x) * 4) as usize; b.rgba[i..i + 4].copy_from_slice(&[20, 20, 20, 255]); } }
        let clusters = dominant_clusters(&b, &NormRect::new(0.0, 0.0, 1.0, 1.0), 3);
        assert!(clusters.len() >= 2);
        assert!((clusters[0].share - 0.5).abs() < 0.06, "{clusters:?}");
        let reading = contrast_reading(&clusters).unwrap();
        assert!(reading.ratio > 10.0, "{reading:?}");
        assert_eq!(reading.level, "AAA");
    }

    #[test]
    fn extraction_is_deterministic() {
        let mut b = solid(64, 64, [200, 120, 40, 255]);
        for i in (0..b.rgba.len()).step_by(24) { b.rgba[i] = 12; }
        let a = dominant_clusters(&b, &NormRect::new(0.1, 0.1, 0.9, 0.9), 4);
        let c = dominant_clusters(&b, &NormRect::new(0.1, 0.1, 0.9, 0.9), 4);
        assert_eq!(a, c);
    }

    #[test]
    fn crop_keeps_full_resolution_pixels() {
        let mut b = solid(40, 20, [10, 20, 30, 255]);
        b.rgba[0] = 99;
        let c = b.crop(&NormRect::new(0.0, 0.0, 0.5, 0.5)).unwrap();
        assert_eq!((c.width, c.height), (20, 10));
        assert_eq!(&c.rgba[0..4], &[99, 20, 30, 255]);
    }

    #[test]
    fn aspect_reads_as_a_common_ratio() {
        assert_eq!(simple_ratio(16.0 / 9.0), "16:9");
        assert_eq!(simple_ratio(1.0), "1:1");
        assert!(simple_ratio(1.777).starts_with("16:9"));
    }

    #[test]
    fn geometry_reports_pixels_and_area() {
        let b = solid(200, 100, [0, 0, 0, 255]);
        let g = geometry_reading(&b, &NormRect::new(0.25, 0.25, 0.75, 0.75));
        assert_eq!((g.px_w, g.px_h), (100, 50));
        assert!((g.area_share - 0.25).abs() < 1e-6);
        assert!((g.aspect - 2.0).abs() < 1e-6);
        assert!(g.offset_from_center < 1e-6);
    }

    #[test]
    fn a_light_selection_on_a_dark_plate_reports_edge_contrast() {
        let mut b = solid(200, 200, [20, 20, 24, 255]);
        for y in 60..140 { for x in 60..140 { let i = ((y * 200 + x) * 4) as usize; b.rgba[i..i + 4].copy_from_slice(&[240, 240, 235, 255]); } }
        let region = NormRect::new(0.3, 0.3, 0.7, 0.7);
        let inside = dominant_clusters(&b, &region, 2);
        let s = surround_reading(&b, &region, &inside).unwrap();
        assert!(s.edge_contrast > 8.0, "{s:?} {inside:?}");
        assert!(s.ring_background[0] < 0.2, "{s:?}");
    }

    #[test]
    fn transparent_pixels_do_not_create_clusters() {
        let b = solid(20, 20, [0, 0, 0, 0]);
        let clusters = dominant_clusters(&b, &NormRect::new(0.0, 0.0, 1.0, 1.0), 3);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].share, 1.0);
    }
}

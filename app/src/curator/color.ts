/** Colour maths for the live readout under the canvas. Mirrors `src/telemetry.rs`: OKLab and
 * OKLCH conversion, WCAG contrast, and a small deterministic k-means so a selection's colours
 * can be shown the instant it is drawn, before the exporter runs. */

export type Rgba = [number, number, number, number];

export interface Oklch {
  l: number;
  c: number;
  h: number;
}

export interface Cluster {
  share: number;
  rgb: [number, number, number];
  oklch: Oklch;
  hex: string;
}

const srgbToLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

export function oklab([r, g, b]: Rgba): [number, number, number] {
  const lr = srgbToLinear(r);
  const lg = srgbToLinear(g);
  const lb = srgbToLinear(b);
  const l = 0.412_221_47 * lr + 0.536_332_55 * lg + 0.051_445_99 * lb;
  const m = 0.211_903_5 * lr + 0.680_699_5 * lg + 0.107_396_96 * lb;
  const s = 0.088_302_46 * lr + 0.281_718_85 * lg + 0.629_978_7 * lb;
  const l_ = Math.cbrt(l);
  const m_ = Math.cbrt(m);
  const s_ = Math.cbrt(s);
  return [
    0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_,
    1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
    0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
  ];
}

export function oklch(c: Rgba): Oklch {
  const [l, a, b] = oklab(c);
  const chroma = Math.hypot(a, b);
  let h = (Math.atan2(b, a) * 180) / Math.PI;
  if (h < 0) h += 360;
  if (chroma < 1e-6) h = 0;
  return { l, c: chroma, h };
}

export function hexOf([r, g, b]: Rgba): string {
  const to = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0");
  return `${to(r)}${to(g)}${to(b)}`.toUpperCase();
}

const linearToSrgb = (c: number) => (c <= 0.0031308 ? c * 12.92 : 1.055 * c ** (1 / 2.4) - 0.055);

export function srgbFromOklch({ l, c, h }: Oklch): Rgba {
  const rad = (h * Math.PI) / 180;
  const a = c * Math.cos(rad);
  const b = c * Math.sin(rad);
  const l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
  const m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
  const s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
  const [L, M, S] = [l_ ** 3, m_ ** 3, s_ ** 3];
  return [
    Math.min(1, Math.max(0, linearToSrgb(4.076_741_7 * L - 3.307_711_6 * M + 0.230_969_94 * S))),
    Math.min(1, Math.max(0, linearToSrgb(-1.268_438 * L + 2.609_757_4 * M - 0.341_319_38 * S))),
    Math.min(1, Math.max(0, linearToSrgb(-0.004_196_086_3 * L - 0.703_418_6 * M + 1.707_614_7 * S))),
    1,
  ];
}

export function luminance([r, g, b, a = 1]: Rgba): number {
  const over = (v: number) => v * a + (1 - a);
  return (
    0.2126 * srgbToLinear(Math.min(1, Math.max(0, over(r)))) +
    0.7152 * srgbToLinear(Math.min(1, Math.max(0, over(g)))) +
    0.0722 * srgbToLinear(Math.min(1, Math.max(0, over(b))))
  );
}

export function contrast(a: Rgba, b: Rgba): number {
  const la = luminance(a);
  const lb = luminance(b);
  const hi = Math.max(la, lb);
  const lo = Math.min(la, lb);
  return (hi + 0.05) / (lo + 0.05);
}

export function contrastLevel(ratio: number): string {
  if (ratio >= 7) return "AAA";
  if (ratio >= 4.5) return "AA";
  if (ratio >= 3) return "AA large";
  return "below AA";
}

/** Every pixel layer the canvas can sample: an image drawn into a canvas we own. */
export interface Sample {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

export function readPixels(image: HTMLImageElement | HTMLCanvasElement): Sample | null {
  const width = "naturalWidth" in image ? image.naturalWidth : image.width;
  const height = "naturalHeight" in image ? image.naturalHeight : image.height;
  if (!width || !height) return null;
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return null;
  ctx.drawImage(image, 0, 0);
  return { width, height, data: ctx.getImageData(0, 0, width, height).data };
}

function pixelAt(s: Sample, x: number, y: number): Rgba {
  const xi = Math.min(s.width - 1, Math.max(0, Math.round(x)));
  const yi = Math.min(s.height - 1, Math.max(0, Math.round(y)));
  const i = (yi * s.width + xi) * 4;
  return [s.data[i] / 255, s.data[i + 1] / 255, s.data[i + 2] / 255, s.data[i + 3] / 255];
}

/** Dominant colours inside a normalized region, farthest-point seeded so a small dark patch
 *  inside a large light plate keeps its own cluster. */
export function clusters(sample: Sample, region: { x0: number; y0: number; x1: number; y1: number }, k = 4): Cluster[] {
  const x0 = Math.floor(region.x0 * sample.width);
  const y0 = Math.floor(region.y0 * sample.height);
  const x1 = Math.ceil(region.x1 * sample.width);
  const y1 = Math.ceil(region.y1 * sample.height);
  const step = Math.max(1, Math.floor(Math.hypot(x1 - x0, y1 - y0) / 64));
  const samples: [number, number, number][] = [];
  for (let y = y0; y < y1; y += step) {
    for (let x = x0; x < x1; x += step) {
      const [l, a, b] = oklab(pixelAt(sample, x, y));
      samples.push([l, a, b]);
    }
  }
  if (!samples.length) return [];
  const dist = (p: [number, number, number], q: [number, number, number]) =>
    Math.hypot(p[0] - q[0], p[1] - q[1], p[2] - q[2]);

  const centres: [number, number, number][] = [samples[0]];
  const nearest = samples.map((p) => dist(p, centres[0]));
  while (centres.length < k) {
    let best = -1;
    let bestD = 1e-5;
    nearest.forEach((d, i) => {
      if (d > bestD) {
        bestD = d;
        best = i;
      }
    });
    if (best < 0) break;
    const next = samples[best];
    centres.push(next);
    samples.forEach((p, i) => {
      const d = dist(p, next);
      if (d < nearest[i]) nearest[i] = d;
    });
  }

  const assign = new Array(samples.length).fill(0);
  for (let iter = 0; iter < 8; iter += 1) {
    samples.forEach((p, i) => {
      let best = 0;
      let bestD = Infinity;
      centres.forEach((c, j) => {
        const d = dist(p, c);
        if (d < bestD) {
          bestD = d;
          best = j;
        }
      });
      assign[i] = best;
    });
    centres.forEach((_, j) => {
      let n = 0;
      const sum = [0, 0, 0];
      samples.forEach((p, i) => {
        if (assign[i] === j) {
          n += 1;
          sum[0] += p[0];
          sum[1] += p[1];
          sum[2] += p[2];
        }
      });
      if (n) centres[j] = [sum[0] / n, sum[1] / n, sum[2] / n];
    });
  }

  const out: Cluster[] = centres.map((centre, j) => {
    const share = assign.filter((a) => a === j).length / samples.length;
    const rgb = srgbFromOklch({
      l: centre[0],
      c: Math.hypot(centre[1], centre[2]),
      h: Math.atan2(centre[2], centre[1]) * (180 / Math.PI),
    });
    const straight: Rgba = [rgb[0], rgb[1], rgb[2], 1];
    return { share, rgb: [straight[0], straight[1], straight[2]], oklch: oklch(straight), hex: hexOf(straight) };
  });
  return out.filter((c) => c.share > 0.01).sort((a, b) => b.share - a.share);
}

/** The strongest contrast between any two clusters, which is the reading the exporter prints. */
export function pairContrast(list: Cluster[]): { ratio: number; a: Cluster; b: Cluster } | null {
  let best: { ratio: number; a: Cluster; b: Cluster } | null = null;
  for (let i = 0; i < list.length; i += 1) {
    for (let j = i + 1; j < list.length; j += 1) {
      const a = list[i];
      const b = list[j];
      const ratio = contrast([...a.rgb, 1], [...b.rgb, 1]);
      if (!best || ratio > best.ratio) best = { ratio, a, b };
    }
  }
  return best;
}

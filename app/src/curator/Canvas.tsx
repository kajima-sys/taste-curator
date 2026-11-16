/** The canvas. Camera in a ref so pan and zoom never re-render the annotation layer's DOM;
 *  the live stage is repainted with `requestAnimationFrame`-throttled draws. Coordinates in and
 *  out of the model are normalized artifact space, exactly like the Rust camera. */

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import type { NormRect, Project, Shape } from "./model";
import { POLARITY_COLOR, rectFromPoints, rectHeight, rectWidth } from "./model";

export type Tool = "select" | "pan" | "rect" | "ellipse" | "freehand";

export const TOOLS: { id: Tool; label: string; key: string; hint: string }[] = [
  { id: "select", label: "Select", key: "V", hint: "Click a marking to edit it" },
  { id: "pan", label: "Pan", key: "H", hint: "Drag to move, scroll to zoom" },
  { id: "rect", label: "Box", key: "R", hint: "Drag a box around what catches your eye" },
  { id: "ellipse", label: "Oval", key: "O", hint: "Drag an oval around one detail" },
  { id: "freehand", label: "Freehand", key: "F", hint: "Draw around the shape you mean" },
];

export const TOOL_SHAPE: Record<Tool, Shape | null> = {
  select: null,
  pan: null,
  rect: "rect",
  ellipse: "ellipse",
  freehand: "freehand",
};

const MIN_ZOOM = 0.1;
const MAX_ZOOM = 24;

export interface Camera {
  panX: number;
  panY: number;
  zoom: number;
}

export interface StageSize {
  width: number;
  height: number;
  fittedW: number;
  fittedH: number;
  originX: number;
  originY: number;
}

/** The artifact's drawn rect in screen points, at the current camera. */
export function stageOf(
  camera: Camera,
  viewport: { width: number; height: number },
  aspect: number,
  margin = 28,
): StageSize {
  const availW = Math.max(1, viewport.width - margin * 2);
  const availH = Math.max(1, viewport.height - margin * 2);
  const fittedW = aspect >= availW / availH ? availW : availH * aspect;
  const fittedH = aspect >= availW / availH ? availW / aspect : availH;
  const width = fittedW * camera.zoom;
  const height = fittedH * camera.zoom;
  return {
    width,
    height,
    fittedW,
    fittedH,
    originX: viewport.width / 2 - width / 2 + camera.panX,
    originY: viewport.height / 2 - height / 2 + camera.panY,
  };
}

export function toScreen(p: [number, number], stage: StageSize): [number, number] {
  return [stage.originX + p[0] * stage.width, stage.originY + p[1] * stage.height];
}

export function toArtifact(p: [number, number], stage: StageSize): [number, number] {
  return [
    Math.min(1, Math.max(0, (p[0] - stage.originX) / Math.max(1e-6, stage.width))),
    Math.min(1, Math.max(0, (p[1] - stage.originY) / Math.max(1e-6, stage.height))),
  ];
}

export interface Draft {
  shape: Shape;
  start: [number, number];
  current: [number, number];
  points: [number, number][];
}

export interface CanvasProps {
  project: Project;
  artifactId: number;
  tool: Tool;
  selected: number | null;
  polarity: "like" | "dislike" | "contrast";
  camera: Camera;
  onCamera: (camera: Camera) => void;
  onDraftCommit: (draft: Draft) => void;
  onSelect: (id: number | null) => void;
  onStatus: (status: string) => void;
  onStrokeColor: (color: string) => void;
  draftColor: string;
  imageUrl: string | null;
  showMarks: boolean;
}

export function Canvas({
  project,
  artifactId,
  tool,
  selected,
  polarity,
  camera,
  onCamera,
  onDraftCommit,
  onSelect,
  onStatus,
  onStrokeColor,
  draftColor,
  imageUrl,
  showMarks,
}: CanvasProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const [viewport, setViewport] = useState({ width: 0, height: 0 });
  const [draft, setDraft] = useState<Draft | null>(null);
  const [hover, setHover] = useState<number | null>(null);
  const dragRef = useRef<
    | { kind: "pan"; last: [number, number] }
    | { kind: "draw"; draft: Draft }
    | null
  >(null);
  const artRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const [imageVersion, setImageVersion] = useState(0);

  useLayoutEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => {
      const rect = el.getBoundingClientRect();
      setViewport({ width: Math.round(rect.width), height: Math.round(rect.height) });
    });
    observer.observe(el);
    const rect = el.getBoundingClientRect();
    setViewport({ width: Math.round(rect.width), height: Math.round(rect.height) });
    return () => observer.disconnect();
  }, []);

  const artifact = project.artifacts.find((a) => a.id === artifactId);
  const annotations = artifact?.annotations ?? [];
  const source = artifact?.source;
  const aspect = source?.kind === "image" && source.height > 0 ? source.width / source.height : 1;
  const stage = stageOf(camera, viewport, aspect);
  const dpr = typeof window === "undefined" ? 1 : Math.min(2, window.devicePixelRatio || 1);

  // The artifact, drawn into its own buffer whenever the camera or viewport changes.
  useEffect(() => {
    const canvas = artRef.current;
    const img = imageRef.current;
    if (!canvas) return;
    canvas.width = Math.max(1, Math.round(viewport.width * dpr));
    canvas.height = Math.max(1, Math.round(viewport.height * dpr));
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, viewport.width, viewport.height);
    if (!img || !imageUrl) return;
    // Checkerboard behind transparent art, so an alpha PNG is legible.
    const tile = 8;
    for (let y = 0; y < stage.height; y += tile) {
      for (let x = 0; x < stage.width; x += tile) {
        const even = ((x / tile) | 0) % 2 === ((y / tile) | 0) % 2;
        ctx.fillStyle = even ? "#f3f2ef" : "#e9e8e4";
        ctx.fillRect(stage.originX + x, stage.originY + y, Math.min(tile, stage.width - x), Math.min(tile, stage.height - y));
      }
    }
    ctx.imageSmoothingEnabled = camera.zoom < 3;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, stage.originX, stage.originY, stage.width, stage.height);
    ctx.strokeStyle = "rgba(22,22,26,.16)";
    ctx.lineWidth = 1;
    ctx.strokeRect(stage.originX - 0.5, stage.originY - 0.5, stage.width + 1, stage.height + 1);
  }, [camera, viewport, stage.height, stage.originX, stage.originY, stage.width, imageUrl, imageVersion, dpr]);

  // The marking layer, on top, redrawn on state changes only.
  useEffect(() => {
    const canvas = overlayRef.current;
    if (!canvas) return;
    canvas.width = Math.max(1, Math.round(viewport.width * dpr));
    canvas.height = Math.max(1, Math.round(viewport.height * dpr));
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, viewport.width, viewport.height);
    if (!showMarks) return;

    const drawRegion = (region: NormRect, shape: Shape, points: [number, number][], color: string, weight: number, glow: boolean) => {
      ctx.save();
      ctx.strokeStyle = color;
      ctx.lineWidth = weight;
      ctx.lineJoin = "round";
      ctx.lineCap = "round";
      if (glow) {
        ctx.shadowColor = "rgba(255,255,255,.9)";
        ctx.shadowBlur = 3;
      }
      if (shape === "freehand" && points.length >= 2) {
        ctx.beginPath();
        points.forEach((p, i) => {
          const [x, y] = toScreen(p, stage);
          if (i === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        });
        ctx.closePath();
        ctx.stroke();
      } else if (shape === "ellipse") {
        const [cx, cy] = toScreen([(region.x0 + region.x1) / 2, (region.y0 + region.y1) / 2], stage);
        ctx.beginPath();
        ctx.ellipse(cx, cy, Math.max(1, (rectWidth(region) * stage.width) / 2), Math.max(1, (rectHeight(region) * stage.height) / 2), 0, 0, Math.PI * 2);
        ctx.stroke();
      } else {
        const [x, y] = toScreen([region.x0, region.y0], stage);
        ctx.strokeRect(x, y, rectWidth(region) * stage.width, rectHeight(region) * stage.height);
      }
      ctx.restore();
    };

    annotations.forEach((a) => {
      const color = POLARITY_COLOR[a.polarity];
      const isSelected = a.id === selected;
      const isHovered = a.id === hover;
      drawRegion(a.region, a.shape, a.points, color, isSelected ? 2.5 : isHovered ? 2 : 1.5, true);
      // A numbered chip at the region's top-left, outside the marking itself.
      const [x, y] = toScreen([a.region.x0, a.region.y0], stage);
      const r = isSelected ? 9 : 7.5;
      ctx.save();
      ctx.beginPath();
      ctx.arc(x, y, r, 0, Math.PI * 2);
      ctx.fillStyle = color;
      ctx.fill();
      ctx.strokeStyle = "rgba(255,255,255,.85)";
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.fillStyle = "#fff";
      ctx.font = `500 ${r}px Inter, ui-sans-serif, system-ui`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(String(a.id), x, y + 0.5);
      ctx.restore();
      if (isSelected) {
        const corners: [number, number][] = [
          [a.region.x0, a.region.y0],
          [a.region.x1, a.region.y0],
          [a.region.x1, a.region.y1],
          [a.region.x0, a.region.y1],
        ];
        ctx.save();
        ctx.fillStyle = "#fff";
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.5;
        corners.forEach((c) => {
          const [hx, hy] = toScreen(c, stage);
          ctx.beginPath();
          ctx.rect(hx - 3.5, hy - 3.5, 7, 7);
          ctx.fill();
          ctx.stroke();
        });
        ctx.restore();
      }
    });

    if (draft) {
      const region = draft.shape === "freehand" && draft.points.length >= 2 ? rectFromPoints(draft.points) : rectFromPoints([draft.start, draft.current]);
      drawRegion(region, draft.shape, draft.points.length ? draft.points : [draft.start, draft.current], draftColor, 2, true);
      ctx.save();
      ctx.setLineDash([4, 3]);
      ctx.strokeStyle = "rgba(22,22,26,.5)";
      ctx.lineWidth = 1;
      const [x, y] = toScreen([region.x0, region.y0], stage);
      ctx.strokeRect(x, y, rectWidth(region) * stage.width, rectHeight(region) * stage.height);
      ctx.restore();
    }
  }, [annotations, dpr, draft, draftColor, hover, selected, showMarks, stage, viewport]);

  const hitTest = useCallback(
    (p: [number, number]): number | null => {
      const hits = annotations
        .filter((a) => {
          if (a.shape === "freehand" && a.points.length >= 3) {
            const ctx = overlayRef.current?.getContext("2d");
            if (!ctx) return false;
            ctx.beginPath();
            a.points.forEach((q, i) => {
              const [x, y] = toScreen(q, stage);
              if (i === 0) ctx.moveTo(x, y);
              else ctx.lineTo(x, y);
            });
            ctx.closePath();
            const [sx, sy] = toScreen(p, stage);
            return ctx.isPointInPath(sx, sy);
          }
          return p[0] >= a.region.x0 && p[0] <= a.region.x1 && p[1] >= a.region.y0 && p[1] <= a.region.y1;
        })
        .map((a) => a.id);
      return hits.length ? hits[hits.length - 1] : null;
    },
    [annotations, stage],
  );

  const local = (e: React.PointerEvent): [number, number] => {
    const rect = e.currentTarget.getBoundingClientRect();
    return [e.clientX - rect.left, e.clientY - rect.top];
  };

  const zoomAbout = (anchor: [number, number], factor: number) => {
    const nextZoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, camera.zoom * factor));
    const before = toArtifact(anchor, stage);
    const nextStage = stageOf({ ...camera, zoom: nextZoom }, viewport, aspect);
    const after = toScreen(before, nextStage);
    onCamera({
      zoom: nextZoom,
      panX: camera.panX + (anchor[0] - after[0]),
      panY: camera.panY + (anchor[1] - after[1]),
    });
  };

  const onPointerDown = (e: React.PointerEvent) => {
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    const p = local(e);
    const at = toArtifact(p, stage);
    const isPan = tool === "pan" || e.button === 1 || e.altKey;
    if (isPan) {
      dragRef.current = { kind: "pan", last: p };
      onStatus("Moving the picture");
      return;
    }
    if (tool === "select") {
      const hit = hitTest(at);
      onSelect(hit);
      onStatus(hit ? "Marking selected" : "Nothing under the pointer");
      return;
    }
    const shape = TOOL_SHAPE[tool];
    if (!shape) return;
    const next: Draft = { shape, start: at, current: at, points: [at] };
    dragRef.current = { kind: "draw", draft: next };
    setDraft(next);
    onStrokeColor(POLARITY_COLOR[polarity]);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const p = local(e);
    const at = toArtifact(p, stage);
    const drag = dragRef.current;
    if (!drag) {
      if (tool === "select") setHover(hitTest(at));
      return;
    }
    if (drag.kind === "pan") {
      onCamera({ ...camera, panX: camera.panX + (p[0] - drag.last[0]), panY: camera.panY + (p[1] - drag.last[1]) });
      drag.last = p;
      return;
    }
    const next: Draft =
      drag.draft.shape === "freehand"
        ? {
            ...drag.draft,
            current: at,
            points:
              Math.abs(at[0] - drag.draft.current[0]) + Math.abs(at[1] - drag.draft.current[1]) > 0.0015
                ? [...drag.draft.points, at]
                : drag.draft.points,
          }
        : { ...drag.draft, current: at };
    drag.draft = next;
    setDraft(next);
  };

  const finish = (e: React.PointerEvent) => {
    const drag = dragRef.current;
    dragRef.current = null;
    if (!drag) return;
    if (drag.kind === "pan") {
      onStatus(TOOLS.find((t) => t.id === tool)?.hint ?? "");
      return;
    }
    const at = toArtifact(local(e), stage);
    const final: Draft =
      drag.draft.shape === "freehand"
        ? { ...drag.draft, current: at, points: [...drag.draft.points, at] }
        : { ...drag.draft, current: at };
    setDraft(null);
    const region = final.shape === "freehand" ? rectFromPoints(final.points) : rectFromPoints([final.start, final.current]);
    if (rectWidth(region) > 0.004 && rectHeight(region) > 0.004) onDraftCommit(final);
    else onStatus("That was a click, not a marking — drag to draw one");
  };

  const onWheel = (e: React.WheelEvent) => {
    if (!e.ctrlKey && !e.metaKey && Math.abs(e.deltaY) < 1 && Math.abs(e.deltaX) < 1) return;
    e.preventDefault();
    const rect = e.currentTarget.getBoundingClientRect();
    const anchor: [number, number] = [e.clientX - rect.left, e.clientY - rect.top];
    if (e.ctrlKey || e.metaKey) {
      zoomAbout(anchor, Math.exp(-e.deltaY * 0.0025));
      return;
    }
    if (e.shiftKey) onCamera({ ...camera, panX: camera.panX - e.deltaY });
    else onCamera({ ...camera, panX: camera.panX - e.deltaX, panY: camera.panY - e.deltaY });
  };

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const handler = (e: WheelEvent) => {
      if (e.ctrlKey || e.metaKey) e.preventDefault();
    };
    el.addEventListener("wheel", handler, { passive: false });
    return () => el.removeEventListener("wheel", handler);
  }, []);

  return (
    <div
      ref={wrapRef}
      className="relative h-full w-full overflow-hidden bg-[#fafaf8] touch-none select-none"
      style={{ cursor: tool === "pan" ? "grab" : tool === "select" ? "default" : "crosshair" }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={finish}
      onPointerCancel={finish}
      onWheel={onWheel}
    >
      {imageUrl ? (
        <img
          ref={imageRef}
          src={imageUrl}
          alt={artifact?.name ?? ""}
          className="pointer-events-none absolute h-0 w-0 opacity-0"
          onLoad={() => setImageVersion((v) => v + 1)}
        />
      ) : null}
      <canvas ref={artRef} className="absolute inset-0 h-full w-full" />
      <canvas ref={overlayRef} className="absolute inset-0 h-full w-full" />
      {!imageUrl ? (
        <div className="absolute inset-0 grid place-items-center">
          <div className="max-w-xs text-center">
            <p className="text-[13px] font-medium text-neutral-700">Nothing loaded in this view</p>
            <p className="mt-1 text-[12.5px] leading-relaxed text-neutral-500">
              Add an image from the shelf, then drag a box around the detail worth keeping.
            </p>
          </div>
        </div>
      ) : null}
      <div className="pointer-events-none absolute bottom-3 left-1/2 -translate-x-1/2 rounded-full border border-neutral-200/80 bg-white/85 px-3 py-1 text-[11.5px] text-neutral-500 backdrop-blur">
        {Math.round(camera.zoom * 100)}% · {Math.round(stage.width)}×{Math.round(stage.height)} pt
      </div>
    </div>
  );
}


import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ChevronDown,
  Eye,
  EyeOff,
  FileDown,
  Focus,
  MousePointer2,
  Pencil,
  Redo2,
  RotateCcw,
  Undo2,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { Canvas, TOOLS, type Camera, type Draft, type Tool } from "./Canvas";
import { ExportDialog } from "./ExportDialog";
import { Inspector } from "./Inspector";
import { Shelf } from "./Shelf";
import { clusters, pairContrast, readPixels, type Cluster, type Sample } from "./color";
import { useProject } from "./useProject";
import {
  POLARITY_COLOR,
  newAnnotation,
  type Annotation,
  type Polarity,
  type Project,
  type Shape,
  type Spectrum,
} from "./model";

interface SeedPlates {
  plates: string[];
  summary: string[];
  conflicts: string[];
  pdf: string;
}

type Snapshot = Project;

export default function Workspace() {
  const { project, setProject, error } = useProject();
  const [activeArtifact, setActiveArtifact] = useState(1);
  const [selected, setSelected] = useState<number | null>(null);
  const [tool, setTool] = useState<Tool>("rect");
  const [polarity, setPolarity] = useState<Polarity>("like");
  const [camera, setCamera] = useState<Camera>({ panX: 0, panY: 0, zoom: 1 });
  const [status, setStatus] = useState(TOOLS.find((t) => t.id === "rect")!.hint);
  const [draftColor, setDraftColor] = useState(POLARITY_COLOR.like);
  const [showMarks, setShowMarks] = useState(true);
  const [seed, setSeed] = useState<SeedPlates | null>(null);
  const [exportOpen, setExportOpen] = useState(false);
  const [sample, setSample] = useState<Sample | null>(null);

  const history = useRef<Snapshot[]>([]);
  const future = useRef<Snapshot[]>([]);
  const [historyTick, setHistoryTick] = useState(0);

  useEffect(() => {
    if (!project) return;
    if (!project.artifacts.some((a) => a.id === activeArtifact)) {
      setActiveArtifact(project.artifacts[0]?.id ?? 1);
      setSelected(null);
    }
  }, [project, activeArtifact]);

  useEffect(() => {
    fetch("seed/plates.json")
      .then((r) => r.json())
      .then(setSeed)
      .catch(() => setSeed(null));
  }, []);

  const active = project?.artifacts.find((a) => a.id === activeArtifact) ?? null;
  const annotation = active?.annotations.find((a) => a.id === selected) ?? null;
  const imageUrl = active ? active.source.path : null;

  // Sample the source once, so the inspector can show the colours a selection contains the
  // moment it is drawn rather than waiting for an export.
  const imageFor = useRef<HTMLImageElement | null>(null);
  useEffect(() => {
    if (!imageUrl) {
      setSample(null);
      return;
    }
    setSample(null);
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => {
      imageFor.current = img;
      setSample(readPixels(img));
    };
    img.onerror = () => setSample(null);
    img.src = imageUrl;
  }, [imageUrl]);

  const live = useMemo(() => {
    if (!annotation || !sample) return { clusters: null as Cluster[] | null, edge: null as number | null };
    const list = clusters(sample, annotation.region, 4);
    const pair = pairContrast(list);
    return { clusters: list, edge: pair ? pair.ratio : null };
  }, [annotation, sample]);

  const pushHistory = useCallback((before: Project) => {
    history.current.push(before);
    if (history.current.length > 60) history.current.shift();
    future.current = [];
    setHistoryTick((t) => t + 1);
  }, []);

  const edit = useCallback(
    (fn: (p: Project) => Project) => {
      setProject((current) => {
        if (!current) return current;
        const next = fn(current);
        if (next === current) return current;
        pushHistory(current);
        return next;
      });
    },
    [setProject, pushHistory],
  );

  const undo = useCallback(() => {
    setProject((current) => {
      const previous = history.current.pop();
      if (!previous || !current) return current;
      future.current.push(current);
      setHistoryTick((t) => t + 1);
      return previous;
    });
  }, [setProject]);

  const redo = useCallback(() => {
    setProject((current) => {
      const next = future.current.pop();
      if (!next || !current) return current;
      history.current.push(current);
      setHistoryTick((t) => t + 1);
      return next;
    });
  }, [setProject]);

  const updateAnnotation = useCallback(
    (artifactId: number, id: number, patch: Partial<Annotation>) => {
      edit((p) => ({
        ...p,
        artifacts: p.artifacts.map((a) =>
          a.id !== artifactId
            ? a
            : { ...a, annotations: a.annotations.map((n) => (n.id === id ? { ...n, ...patch } : n)) },
        ),
      }));
    },
    [edit],
  );

  const commitDraft = useCallback(
    (draft: Draft) => {
      if (!project) return;
      const shape: Shape = draft.shape;
      const region =
        shape === "freehand"
          ? boundsOf(draft.points)
          : {
              x0: Math.min(draft.start[0], draft.current[0]),
              y0: Math.min(draft.start[1], draft.current[1]),
              x1: Math.max(draft.start[0], draft.current[0]),
              y1: Math.max(draft.start[1], draft.current[1]),
            };
      const id = project.next_annotation;
      const fresh = {
        ...newAnnotation(id, polarity, shape, region),
        points: shape === "freehand" ? draft.points : [],
      };
      edit((p) => ({
        ...p,
        next_annotation: id + 1,
        artifacts: p.artifacts.map((a) =>
          a.id !== activeArtifact ? a : { ...a, annotations: [...a.annotations, fresh] },
        ),
      }));
      setSelected(id);
      setStatus(`Marking ${id} — say what it is about, on the right`);
      setTool("select");
    },
    [project, polarity, activeArtifact, edit],
  );

  const deleteAnnotation = useCallback(
    (id: number) => {
      edit((p) => ({
        ...p,
        artifacts: p.artifacts.map((a) => ({
          ...a,
          annotations: a.annotations.filter((n) => n.id !== id),
        })),
      }));
      setSelected((s) => (s === id ? null : s));
    },
    [edit],
  );

  const addArtifact = useCallback(() => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/webp";
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file || !project) return;
      const url = URL.createObjectURL(file);
      const img = new Image();
      img.onload = () => {
        const width = img.naturalWidth;
        const height = img.naturalHeight;
        const id = project.next_artifact;
        edit((p) => ({
          ...p,
          next_artifact: id + 1,
          artifacts: [
            ...p.artifacts,
            {
              id,
              name: file.name,
              source: { kind: "image", path: url, width, height },
              annotations: [],
            },
          ],
        }));
        setActiveArtifact(id);
        setSelected(null);
        setCamera({ panX: 0, panY: 0, zoom: 1 });
        setStatus(`${file.name} added — drag a box around what catches your eye`);
      };
      img.src = url;
    };
    input.click();
  }, [project, edit]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
      if (typing) return;
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "e") {
        e.preventDefault();
        setExportOpen(true);
        return;
      }
      const match = TOOLS.find((t) => t.key.toLowerCase() === e.key.toLowerCase());
      if (match) {
        setTool(match.id);
        setStatus(match.hint);
        return;
      }
      if (e.key === "Escape") {
        setSelected(null);
        setStatus(TOOLS.find((t) => t.id === tool)?.hint ?? "");
      }
      if ((e.key === "Backspace" || e.key === "Delete") && selected !== null) {
        e.preventDefault();
        deleteAnnotation(selected);
      }
      if (e.key === " ") {
        e.preventDefault();
        setTool((t) => (t === "pan" ? "rect" : "pan"));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [deleteAnnotation, redo, selected, tool, undo]);

  if (!project) {
    return (
      <div className="grid h-screen place-items-center bg-[#fafaf8]">
        <div className="flex flex-col items-center gap-2">
          <div className="size-5 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-800" />
          <p className="text-[12.5px] text-muted-foreground">Opening the sample project…</p>
        </div>
      </div>
    );
  }

  const toolDef = TOOLS.find((t) => t.id === tool)!;

  return (
    <TooltipProvider delayDuration={350}>
      <div className="flex h-screen min-h-0 flex-col overflow-hidden bg-[#fafaf8] text-foreground">
        <header className="flex h-12 shrink-0 items-center gap-3 border-b bg-white/80 px-4 backdrop-blur">
          <div className="flex items-center gap-2">
            <span className="grid size-6 place-items-center rounded-md bg-foreground text-white">
              <Focus size={13} strokeWidth={2.2} />
            </span>
            <span className="text-[13px] font-medium tracking-tight">Taste Curator</span>
          </div>
          <Separator orientation="vertical" className="h-4" />
          <ProjectTitle
            name={project.name}
            onRename={(name) => edit((p) => (p.name === name ? p : { ...p, name }))}
          />
          <div className="ml-auto flex items-center gap-1.5">
            <Badge variant="secondary" className="h-6 rounded-full px-2 text-[10.5px] font-normal text-muted-foreground">
              Local only
            </Badge>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon" className="size-7" onClick={undo} disabled={!history.current.length}>
                  <Undo2 size={14} />
                  <span className="sr-only">Undo</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>Undo · ⌘Z</TooltipContent>
            </Tooltip>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button variant="ghost" size="icon" className="size-7" onClick={redo} disabled={!future.current.length} data-tick={historyTick}>
                  <Redo2 size={14} />
                  <span className="sr-only">Redo</span>
                </Button>
              </TooltipTrigger>
              <TooltipContent>Redo · ⇧⌘Z</TooltipContent>
            </Tooltip>
            <Separator orientation="vertical" className="mx-1 h-4" />
            <Button size="sm" className="h-7 gap-1.5 text-[12px]" onClick={() => setExportOpen(true)}>
              <FileDown size={13} />
              Export
              <kbd className="ml-0.5 hidden rounded bg-white/15 px-1 font-sans text-[9.5px] font-normal sm:inline">⌘E</kbd>
            </Button>
          </div>
        </header>

        <div className="grid min-h-0 flex-1 grid-cols-[248px_minmax(0,1fr)_284px] max-[1100px]:grid-cols-[220px_minmax(0,1fr)] max-[820px]:grid-cols-1">
          <aside className="min-h-0 border-r bg-[#fbfaf8] max-[820px]:hidden">
            <Shelf
              project={project}
              activeArtifact={activeArtifact}
              selected={selected}
              onArtifact={(id) => {
                setActiveArtifact(id);
                setSelected(null);
                setCamera({ panX: 0, panY: 0, zoom: 1 });
              }}
              onSelect={(id) => {
                setSelected(id);
                if (id !== null) setTool("select");
              }}
              onDelete={deleteAnnotation}
              onAddArtifact={addArtifact}
            />
          </aside>

          <main className="flex min-h-0 min-w-0 flex-col">
            <div className="flex h-11 shrink-0 items-center gap-2 border-b bg-white/70 px-3">
              <div className="flex items-center gap-0.5 rounded-lg border bg-white p-0.5">
                {TOOLS.map((t) => {
                  const Icon = t.id === "select" ? MousePointer2 : t.id === "pan" ? Eye : Pencil;
                  return (
                    <Tooltip key={t.id}>
                      <TooltipTrigger asChild>
                        <button
                          onClick={() => {
                            setTool(t.id);
                            setStatus(t.hint);
                          }}
                          aria-pressed={tool === t.id}
                          className={cn(
                            "flex h-6 items-center gap-1.5 rounded-md px-2 text-[11.5px] transition-colors",
                            tool === t.id ? "bg-foreground text-background" : "text-muted-foreground hover:bg-muted",
                          )}
                        >
                          <Icon size={12} strokeWidth={1.9} />
                          <span className="max-[1250px]:hidden">{t.label}</span>
                        </button>
                      </TooltipTrigger>
                      <TooltipContent className="text-[11.5px]">
                        {t.label} · {t.key}
                      </TooltipContent>
                    </Tooltip>
                  );
                })}
              </div>

              <Separator orientation="vertical" className="h-4" />

              <div className="flex items-center gap-1">
                {(["like", "dislike", "contrast"] as Polarity[]).map((p) => (
                  <button
                    key={p}
                    onClick={() => setPolarity(p)}
                    aria-pressed={polarity === p}
                    className={cn(
                      "flex items-center gap-1.5 rounded-full border px-2 py-1 text-[11.5px] transition-colors",
                      polarity === p ? "font-medium text-white" : "bg-white text-muted-foreground hover:bg-muted/60",
                    )}
                    style={
                      polarity === p
                        ? { background: POLARITY_COLOR[p], borderColor: POLARITY_COLOR[p] }
                        : undefined
                    }
                  >
                    <span
                      className="size-1.5 rounded-full"
                      style={{ background: polarity === p ? "currentColor" : POLARITY_COLOR[p] }}
                    />
                    {p === "dislike" ? "Avoid" : p === "like" ? "Like" : "Contrast"}
                  </button>
                ))}
              </div>

              <div className="ml-auto flex items-center gap-1">
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={() => setShowMarks((v) => !v)}
                      aria-pressed={showMarks}
                    >
                      {showMarks ? <Eye size={14} /> : <EyeOff size={14} />}
                      <span className="sr-only">{showMarks ? "Hide markings" : "Show markings"}</span>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{showMarks ? "Hide markings" : "Show markings"}</TooltipContent>
                </Tooltip>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-7"
                      onClick={() => setCamera({ panX: 0, panY: 0, zoom: 1 })}
                    >
                      <RotateCcw size={13.5} />
                      <span className="sr-only">Fit the picture</span>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>Fit the picture</TooltipContent>
                </Tooltip>
              </div>
            </div>

            <div className="relative min-h-0 flex-1">
              <Canvas
                project={project}
                artifactId={activeArtifact}
                tool={tool}
                selected={selected}
                polarity={polarity}
                camera={camera}
                onCamera={setCamera}
                onDraftCommit={commitDraft}
                onSelect={setSelected}
                onStatus={setStatus}
                onStrokeColor={setDraftColor}
                draftColor={draftColor}
                imageUrl={imageUrl}
                showMarks={showMarks}
              />
            </div>

            <div className="flex h-8 shrink-0 items-center gap-2 border-t bg-white/70 px-3 text-[11px] text-muted-foreground">
              <span className="font-medium text-foreground/70">{toolDef.label}</span>
              <span className="text-muted-foreground/50">·</span>
              <span className="truncate">{status}</span>
              <span className="ml-auto flex items-center gap-3 tabular-nums">
                <span className="hidden sm:inline">{active ? `${active.source.kind === "image" ? `${active.source.width}×${active.source.height}` : "text"}` : ""}</span>
                <span>{Math.round(camera.zoom * 100)}%</span>
              </span>
            </div>
          </main>

          <aside className="min-h-0 border-l bg-white max-[1100px]:hidden">
            {error ? <p className="p-4 text-[12px] text-destructive">{error}</p> : null}
            <Inspector
              annotation={annotation}
              clusters={live.clusters}
              edgeContrast={live.edge}
              onChange={(patch) => {
                if (annotation) updateAnnotation(activeArtifact, annotation.id, patch);
              }}
              onChip={(dial: Spectrum, value: number | undefined) => {
                if (!annotation) return;
                const chips = annotation.chips.filter(([d]) => d !== dial);
                if (value !== undefined) chips.push([dial, value]);
                updateAnnotation(activeArtifact, annotation.id, { chips });
              }}
            />
          </aside>
        </div>

        <ExportDialog project={project} seed={seed} open={exportOpen} onOpenChange={setExportOpen} />
      </div>
    </TooltipProvider>
  );
}

function ProjectTitle({ name, onRename }: { name: string; onRename: (name: string) => void }) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(name);
  useEffect(() => setValue(name), [name]);
  if (editing) {
    return (
      <input
        autoFocus
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onBlur={() => {
          setEditing(false);
          const next = value.trim();
          if (next && next !== name) onRename(next);
          else setValue(name);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          if (e.key === "Escape") {
            setValue(name);
            setEditing(false);
          }
        }}
        className="w-56 rounded-md border bg-white px-2 py-1 text-[12.5px] outline-none focus-visible:border-foreground/30"
        aria-label="Project name"
      />
    );
  }
  return (
    <button
      onClick={() => setEditing(true)}
      className="group flex items-center gap-1 rounded-md px-1.5 py-1 text-[12.5px] text-muted-foreground hover:bg-muted"
    >
      {name}
      <ChevronDown size={11} className="opacity-0 transition-opacity group-hover:opacity-60" />
    </button>
  );
}

function boundsOf(points: [number, number][]) {
  let x0 = 1;
  let y0 = 1;
  let x1 = 0;
  let y1 = 0;
  for (const [x, y] of points) {
    x0 = Math.min(x0, x);
    y0 = Math.min(y0, y);
    x1 = Math.max(x1, x);
    y1 = Math.max(y1, y);
  }
  return { x0, y0, x1, y1 };
}


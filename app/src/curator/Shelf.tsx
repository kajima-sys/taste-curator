import { useMemo, useState } from "react";
import { ChevronRight, Image as ImageIcon, Sparkles, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import {
  MEDIUM_LABEL,
  POLARITIES,
  POLARITY_COLOR,
  POLARITY_LABEL,
  SCOPE_LABEL,
  counts,
  noteText,
  type Annotation,
  type Polarity,
  type Project,
} from "./model";

interface Props {
  project: Project;
  activeArtifact: number;
  selected: number | null;
  onArtifact: (id: number) => void;
  onSelect: (id: number | null) => void;
  onDelete: (id: number) => void;
  onAddArtifact: () => void;
}

export function Shelf({ project, activeArtifact, selected, onArtifact, onSelect, onDelete, onAddArtifact }: Props) {
  const [filter, setFilter] = useState<Polarity | "all">("all");
  const tally = counts(project);
  const total = tally.like + tally.avoid + tally.contrast;

  const annotations = useMemo(() => {
    const artifact = project.artifacts.find((a) => a.id === activeArtifact);
    if (!artifact) return [];
    return artifact.annotations
      .filter((a) => filter === "all" || a.polarity === filter)
      .sort((a, b) => a.region.y0 - b.region.y0);
  }, [project, activeArtifact, filter]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="px-4 pt-4 pb-3">
        <div className="flex items-baseline justify-between">
          <p className="text-[10.5px] font-medium tracking-[0.14em] text-muted-foreground uppercase">Sources</p>
          <Button variant="ghost" size="sm" className="h-6 px-1.5 text-[11.5px]" onClick={onAddArtifact}>
            Add
          </Button>
        </div>
        <div className="mt-2 flex flex-col gap-0.5">
          {project.artifacts.map((artifact) => {
            const active = artifact.id === activeArtifact;
            return (
              <button
                key={artifact.id}
                onClick={() => onArtifact(artifact.id)}
                className={cn(
                  "flex items-center gap-2.5 rounded-lg px-2 py-1.5 text-left transition-colors",
                  active ? "bg-primary/8" : "hover:bg-muted/70",
                )}
              >
                <span
                  className={cn(
                    "grid size-7 shrink-0 place-items-center rounded-md border bg-white",
                    active ? "border-primary/25" : "border-border/70",
                  )}
                >
                  <ImageIcon size={13} strokeWidth={1.7} className="text-neutral-500" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[12.5px] font-medium text-foreground">{artifact.name}</span>
                  <span className="block text-[11px] text-muted-foreground">
                    {artifact.annotations.length} marking{artifact.annotations.length === 1 ? "" : "s"}
                  </span>
                </span>
                {active ? <ChevronRight size={12} className="text-muted-foreground" /> : null}
              </button>
            );
          })}
          {!project.artifacts.length ? (
            <p className="px-2 py-3 text-[12px] leading-relaxed text-muted-foreground">
              Nothing imported yet. Add an image to start curating.
            </p>
          ) : null}
        </div>
      </div>

      <Separator />

      <div className="flex items-center gap-1 px-3 pt-3 pb-2">
        <FilterPill active={filter === "all"} onClick={() => setFilter("all")} label="All" count={total} />
        {POLARITIES.map((p) => (
          <FilterPill
            key={p}
            active={filter === p}
            onClick={() => setFilter(p)}
            label={p === "dislike" ? "Avoid" : POLARITY_LABEL[p]}
            count={p === "like" ? tally.like : p === "dislike" ? tally.avoid : tally.contrast}
            color={POLARITY_COLOR[p]}
          />
        ))}
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <ul className="flex flex-col gap-0.5 px-2 pb-4">
          {annotations.map((a) => (
            <MarkingRow
              key={a.id}
              annotation={a}
              selected={a.id === selected}
              onSelect={() => onSelect(a.id)}
              onDelete={() => onDelete(a.id)}
            />
          ))}
          {!annotations.length ? (
            <li className="px-2 py-6 text-center text-[12px] leading-relaxed text-muted-foreground">
              {filter === "all"
                ? "No markings on this source yet."
                : `No ${filter === "dislike" ? "must-avoid" : filter} markings here.`}
            </li>
          ) : null}
        </ul>
      </ScrollArea>

      <div className="flex items-center gap-1.5 border-t px-4 py-3 text-[11.5px] text-muted-foreground">
        <Sparkles size={12} className="text-neutral-400" />
        <span>
          {total} marking{total === 1 ? "" : "s"} across {project.artifacts.length} source
          {project.artifacts.length === 1 ? "" : "s"}
        </span>
      </div>
    </div>
  );
}

function FilterPill({
  active,
  onClick,
  label,
  count,
  color,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  count: number;
  color?: string;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center gap-1.5 rounded-full px-2 py-1 text-[11.5px] transition-colors",
        active ? "bg-foreground text-background" : "text-muted-foreground hover:bg-muted",
      )}
    >
      {color ? <span className="size-1.5 rounded-full" style={{ background: active ? "currentColor" : color }} /> : null}
      {label}
      <span className={cn("tabular-nums", active ? "opacity-70" : "opacity-60")}>{count}</span>
    </button>
  );
}

function MarkingRow({
  annotation,
  selected,
  onSelect,
  onDelete,
}: {
  annotation: Annotation;
  selected: boolean;
  onSelect: () => void;
  onDelete: () => void;
}) {
  const note = noteText(annotation);
  return (
    <li>
      <div
        className={cn(
          "group relative flex cursor-pointer gap-2 rounded-lg px-2 py-2 transition-colors",
          selected ? "bg-muted" : "hover:bg-muted/60",
        )}
        onClick={onSelect}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect();
          }
        }}
      >
        <span className="mt-0.5 flex w-4 shrink-0 justify-center">
          <span
            className="mt-0.5 grid size-4 place-items-center rounded-[4px] text-[9px] font-semibold text-white"
            style={{ background: POLARITY_COLOR[annotation.polarity] }}
          >
            {annotation.id}
          </span>
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span className="text-[12px] font-medium">{SCOPE_LABEL[annotation.scope]}</span>
            <span className="text-[11px] text-muted-foreground">· {MEDIUM_LABEL[annotation.medium]}</span>
            {annotation.polarity === "dislike" ? (
              <Badge
                variant="outline"
                className="ml-auto h-4 rounded px-1 text-[9.5px] font-medium tracking-wide text-destructive"
              >
                AVOID
              </Badge>
            ) : null}
          </div>
          <p
            className={cn(
              "mt-0.5 line-clamp-2 text-[11.5px] leading-snug",
              note ? "text-muted-foreground" : "text-muted-foreground/70 italic",
            )}
          >
            {note || "No note — the dials carry it"}
          </p>
        </div>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="absolute top-1 right-1 size-6 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
              onClick={(e) => {
                e.stopPropagation();
                onDelete();
              }}
            >
              <Trash2 size={12} />
              <span className="sr-only">Delete marking {annotation.id}</span>
            </Button>
          </TooltipTrigger>
          <TooltipContent>Delete this marking</TooltipContent>
        </Tooltip>
      </div>
    </li>
  );
}

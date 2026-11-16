import { useEffect, useMemo, useState } from "react";
import { Info, MousePointerClick, MoveHorizontal } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Slider } from "@/components/ui/slider";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import type { Cluster } from "./color";
import { contrastLevel } from "./color";
import {
  MEDIA,
  MEDIUM_LABEL,
  POLARITIES,
  POLARITY_COLOR,
  POLARITY_LABEL,
  SCOPES,
  SCOPE_LABEL,
  SCOPES_HINT,
  SPECTRA,
  SPECTRUM_ENDS,
  SPECTRUM_LABEL,
  chipValue,
  type Annotation,
  type Medium,
  type Polarity,
  type Scope,
  type Spectrum,
} from "./model";

interface Props {
  annotation: Annotation | null;
  clusters: Cluster[] | null;
  edgeContrast: number | null;
  onChange: (patch: Partial<Annotation>) => void;
  onChip: (dial: Spectrum, value: number | undefined) => void;
}

export function Inspector({ annotation, clusters, edgeContrast, onChange, onChip }: Props) {
  const [note, setNote] = useState(annotation?.notes.map((n) => n.text).join("\n") ?? "");

  useEffect(() => {
    setNote(annotation?.notes.map((n) => n.text).join("\n") ?? "");
  }, [annotation?.id, annotation?.notes]);

  const pair = useMemo(() => {
    if (!clusters || clusters.length < 2) return null;
    return { a: clusters[0], b: clusters[1] };
  }, [clusters]);

  if (!annotation) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-8 text-center">
        <span className="grid size-9 place-items-center rounded-full border bg-muted/40">
          <MousePointerClick size={15} className="text-neutral-400" />
        </span>
        <p className="text-[12.5px] font-medium text-foreground">Nothing selected</p>
        <p className="max-w-[15rem] text-[11.5px] leading-relaxed text-muted-foreground">
          Pick a marking on the canvas or in the shelf, or drag a new box around the detail worth
          keeping.
        </p>
      </div>
    );
  }

  return (
    <ScrollArea className="h-full">
      <div className="px-4 pt-4 pb-8">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span
              className="grid size-5 place-items-center rounded-[5px] text-[10px] font-semibold text-white"
              style={{ background: POLARITY_COLOR[annotation.polarity] }}
            >
              {annotation.id}
            </span>
            <span className="text-[12.5px] font-medium">Marking {annotation.id}</span>
          </div>
          <Badge variant="secondary" className="h-5 rounded px-1.5 text-[10px] font-normal text-muted-foreground">
            {annotation.shape === "text_span" ? "text span" : annotation.shape}
          </Badge>
        </div>

        <Group label="Is this worth keeping?">
          <div className="grid grid-cols-3 gap-1">
            {POLARITIES.map((p) => (
              <PolarityButton
                key={p}
                polarity={p}
                active={annotation.polarity === p}
                onClick={() => onChange({ polarity: p })}
              />
            ))}
          </div>
        </Group>

        <Group label="What is it about?">
          <div className="grid grid-cols-2 gap-1">
            {SCOPES.map((s) => (
              <ScopeButton
                key={s}
                scope={s}
                active={annotation.scope === s}
                onClick={() => onChange({ scope: s })}
              />
            ))}
          </div>
        </Group>

        <Group label="Where should it show up?">
          <div className="grid grid-cols-4 gap-1">
            {MEDIA.map((m) => (
              <MediumButton
                key={m}
                medium={m}
                active={annotation.medium === m}
                onClick={() => onChange({ medium: m })}
              />
            ))}
          </div>
        </Group>

        <Separator className="my-4" />

        <Group label="How does it feel?" hint>
          <div className="flex flex-col gap-3.5">
            {SPECTRA.map((s) => (
              <Dial
                key={s}
                dial={s}
                value={chipValue(annotation, s)}
                onChange={(v) => onChip(s, v)}
              />
            ))}
          </div>
        </Group>

        <Group label="In your own words">
          <Textarea
            value={note}
            placeholder="What catches your eye here? Write it the way you would say it out loud."
            className="min-h-[76px] resize-none bg-white text-[12.5px] leading-relaxed placeholder:text-[12px] placeholder:leading-relaxed"
            onChange={(e) => setNote(e.target.value)}
            onBlur={() => {
              const text = note.trim();
              const current = annotation.notes.map((n) => n.text).join("\n").trim();
              if (text !== current) onChange({ notes: text ? [{ text }] : [] });
            }}
          />
        </Group>

        <Separator className="my-4" />

        <div className="flex flex-col gap-2.5">
          <p className="text-[10.5px] font-medium tracking-[0.13em] text-muted-foreground uppercase">
            Read from the pixels
          </p>
          {clusters?.length ? (
            <div className="flex items-center gap-1.5">
              {clusters.slice(0, 4).map((c) => (
                <Tooltip key={c.hex}>
                  <TooltipTrigger asChild>
                    <div className="flex flex-1 flex-col gap-1">
                      <div
                        className="h-7 w-full rounded border border-black/5"
                        style={{ background: `#${c.hex}` }}
                      />
                      <span className="font-mono text-[9.5px] text-muted-foreground">
                        {Math.round(c.share * 100)}%
                      </span>
                    </div>
                  </TooltipTrigger>
                  <TooltipContent>
                    <span className="font-mono text-[11px]">
                      #{c.hex} · oklch({c.oklch.l.toFixed(3)} {c.oklch.c.toFixed(3)}{" "}
                      {c.oklch.h.toFixed(0)})
                    </span>
                  </TooltipContent>
                </Tooltip>
              ))}
            </div>
          ) : (
            <p className="text-[11.5px] leading-relaxed text-muted-foreground">
              Loading the source pixels…
            </p>
          )}

          {pair ? (
            <dl className="grid grid-cols-2 gap-x-3 gap-y-1.5 text-[11.5px]">
              <Stat
                label="Contrast"
                value={`${edgeContrast?.toFixed(1) ?? "—"}:1`}
                hint={edgeContrast ? contrastLevel(edgeContrast) : ""}
              />
              <Stat label="Aspect" value={aspectLabel(annotation)} />
              <Stat
                label="Oklch L"
                value={pair.a.oklch.l.toFixed(2)}
                hint={`to ${pair.b.oklch.l.toFixed(2)}`}
              />
              <Stat
                label="Chroma"
                value={pair.a.oklch.c.toFixed(2)}
                hint={`to ${pair.b.oklch.c.toFixed(2)}`}
              />
            </dl>
          ) : null}

          {annotation.spans.length ? (
            <div className="rounded-md border bg-muted/40 p-2">
              <p className="mb-1 text-[9.5px] font-medium tracking-[0.12em] text-muted-foreground uppercase">
                Verbatim
              </p>
              <pre className="max-h-32 overflow-auto font-mono text-[10.5px] leading-relaxed whitespace-pre-wrap">
                {annotation.spans.map((s) => s.text).join("")}
              </pre>
            </div>
          ) : null}
        </div>
      </div>
    </ScrollArea>
  );
}

function Group({ label, hint, children }: { label: string; hint?: boolean; children: React.ReactNode }) {
  return (
    <div className="mt-4">
      <div className="mb-1.5 flex items-center gap-1">
        <p className="text-[10.5px] font-medium tracking-[0.13em] text-muted-foreground uppercase">{label}</p>
        {hint ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <button className="grid size-3.5 place-items-center text-muted-foreground/70">
                <Info size={10} />
              </button>
            </TooltipTrigger>
            <TooltipContent className="max-w-[15rem] text-[11.5px] leading-relaxed">
              Leave a dial alone unless it says something. Only the dials you move reach the PDF.
            </TooltipContent>
          </Tooltip>
        ) : null}
      </div>
      {children}
    </div>
  );
}

function PolarityButton({ polarity, active, onClick }: { polarity: Polarity; active: boolean; onClick: () => void }) {
  const color = POLARITY_COLOR[polarity];
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        "flex items-center justify-center gap-1.5 rounded-md border px-1.5 py-1.5 text-[11.5px] transition-colors",
        active ? "font-medium text-white" : "bg-white text-muted-foreground hover:bg-muted/60",
      )}
      style={active ? { background: color, borderColor: color } : undefined}
    >
      <span className="size-1.5 rounded-full" style={{ background: active ? "currentColor" : color }} />
      {polarity === "dislike" ? "Avoid" : POLARITY_LABEL[polarity]}
    </button>
  );
}

function ScopeButton({ scope, active, onClick }: { scope: Scope; active: boolean; onClick: () => void }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          onClick={onClick}
          aria-pressed={active}
          className={cn(
            "truncate rounded-md border px-2 py-1.5 text-left text-[11.5px] transition-colors",
            active ? "border-foreground/25 bg-foreground/[.055] font-medium text-foreground" : "bg-white text-muted-foreground hover:bg-muted/60",
          )}
        >
          {SCOPE_LABEL[scope]}
        </button>
      </TooltipTrigger>
      <TooltipContent className="max-w-[15rem] text-[11.5px] leading-relaxed">{SCOPES_HINT[scope]}</TooltipContent>
    </Tooltip>
  );
}

function MediumButton({ medium, active, onClick }: { medium: Medium; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      aria-pressed={active}
      className={cn(
        "rounded-md border py-1.5 text-[11px] transition-colors",
        active ? "border-foreground/25 bg-foreground/[.055] font-medium text-foreground" : "bg-white text-muted-foreground hover:bg-muted/60",
      )}
    >
      {MEDIUM_LABEL[medium]}
    </button>
  );
}

function Dial({
  dial,
  value,
  onChange,
}: {
  dial: Spectrum;
  value: number | undefined;
  onChange: (v: number | undefined) => void;
}) {
  const [left, right] = SPECTRUM_ENDS[dial];
  const moved = value !== undefined;
  return (
    <div>
      <div className="mb-1 flex items-baseline justify-between">
        <span className="text-[11.5px] font-medium">{SPECTRUM_LABEL[dial]}</span>
        <span className="font-mono text-[10.5px] text-muted-foreground">
          {moved ? `${value! < 0.5 ? "–" : "+"}${Math.abs(value! * 2 - 1).toFixed(2)}` : "not set"}
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="w-9 text-right text-[10.5px] text-muted-foreground">{left}</span>
        <Slider
          value={[value ?? 0.5]}
          min={0}
          max={1}
          step={0.01}
          className="flex-1"
          onValueChange={([v]) => onChange(v)}
          onDoubleClick={() => onChange(undefined)}
          aria-label={`${left} to ${right}`}
        />
        <span className="w-9 text-[10.5px] text-muted-foreground">{right}</span>
      </div>
      {!moved ? (
        <p className="mt-0.5 flex items-center gap-1 text-[10px] text-muted-foreground/70">
          <MoveHorizontal size={9} /> dragged to the middle means nothing; leave it
        </p>
      ) : null}
    </div>
  );
}

function Stat({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="flex items-baseline justify-between gap-2">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-mono text-[10.5px] tabular-nums">
        {value}
        {hint ? <span className="ml-1 text-muted-foreground/70">{hint}</span> : null}
      </dd>
    </div>
  );
}

function aspectLabel(a: Annotation): string {
  const w = a.region.x1 - a.region.x0;
  const h = a.region.y1 - a.region.y0;
  if (h < 1e-6) return "—";
  return `${(w / h).toFixed(2)}:1`;
}

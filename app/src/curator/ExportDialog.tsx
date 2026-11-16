import { useMemo, useState } from "react";
import { Check, Copy, Download, FileText, Printer } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { buildManifest, plateCount } from "./export";
import { MEDIUM_LABEL, POLARITY_COLOR, POLARITY_LABEL, SCOPE_LABEL, type Project } from "./model";

interface Seed {
  plates: string[];
  summary: string[];
  conflicts: string[];
  pdf: string;
}

export function ExportDialog({
  project,
  seed,
  open,
  onOpenChange,
}: {
  project: Project;
  seed: Seed | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const manifest = useMemo(() => buildManifest(project), [project]);
  const plates = useMemo(() => plateCount(project), [project]);
  const [copied, setCopied] = useState(false);
  const json = useMemo(() => JSON.stringify(manifest, null, 2), [manifest]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(json);
      setCopied(true);
      setTimeout(() => setCopied(false), 1800);
    } catch {
      setCopied(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[88vh] gap-0 overflow-hidden p-0 sm:max-w-2xl">
        <DialogHeader className="border-b px-5 py-4">
          <DialogTitle className="text-[15px]">What the agent will receive</DialogTitle>
          <DialogDescription className="text-[12.5px] leading-relaxed">
            One PDF, {plates.total} plates — a summary, {plates.overlays} overview
            {plates.overlays === 1 ? "" : "s"} and {plates.details} detail
            {plates.details === 1 ? "" : "s"} — with the exact numbers embedded as JSON. Nothing in
            it is a screenshot: the crops are unaltered pixels and the overlays are vector.
          </DialogDescription>
        </DialogHeader>

        <Tabs defaultValue="read" className="flex min-h-0 flex-1 flex-col">
          <div className="flex items-center justify-between gap-3 border-b px-5 py-2.5">
            <TabsList className="h-7">
              <TabsTrigger value="read" className="text-[11.5px]">
                The read
              </TabsTrigger>
              <TabsTrigger value="plates" className="text-[11.5px]">
                Plates
              </TabsTrigger>
              <TabsTrigger value="json" className="text-[11.5px]">
                Embedded data
              </TabsTrigger>
            </TabsList>
            <div className="flex items-center gap-1.5">
              {seed ? (
                <Button size="sm" variant="outline" className="h-7 gap-1.5 text-[11.5px]" asChild>
                  <a href={seed.pdf} download>
                    <Download size={12} /> The sample PDF
                  </a>
                </Button>
              ) : null}
              <Button size="sm" className="h-7 gap-1.5 text-[11.5px]" onClick={copy}>
                {copied ? <Check size={12} /> : <Copy size={12} />}
                {copied ? "Copied" : "Copy JSON"}
              </Button>
            </div>
          </div>

          <TabsContent value="read" className="mt-0 min-h-0 overflow-y-auto px-5 py-4">
            <Section title="Synthesis">
              <List lines={manifest.summary} />
            </Section>
            <Section title="Conflicts kept on purpose">
              {manifest.conflicts.length ? (
                <ul className="flex flex-col gap-2">
                  {manifest.conflicts.map((c) => (
                    <li
                      key={c}
                      className="rounded-md border-l-2 bg-muted/40 py-1.5 pr-2.5 pl-2.5 text-[12px] leading-relaxed"
                      style={{ borderColor: POLARITY_COLOR.dislike }}
                    >
                      {c}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-[12px] text-muted-foreground">
                  None. Every like and dislike sits in a different scope or medium, so the agent gets
                  no doubled-back instructions.
                </p>
              )}
            </Section>
            <Section title="The brief">
              <p className="text-[12px] leading-relaxed text-muted-foreground">{manifest.brief}</p>
            </Section>
            <Section title={`${manifest.readings.length} readings`}>
              <ul className="divide-y rounded-md border bg-white">
                {manifest.readings.map((r) => (
                  <li key={r.annotation} className="flex items-start gap-2.5 px-2.5 py-2">
                    <span
                      className="mt-0.5 grid size-4 shrink-0 place-items-center rounded-[3px] text-[9px] font-semibold text-white"
                      style={{ background: POLARITY_COLOR[r.polarity as keyof typeof POLARITY_COLOR] }}
                    >
                      {r.annotation}
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="text-[11.5px]">
                        <span className="font-medium">{SCOPE_LABEL[r.scope as keyof typeof SCOPE_LABEL]}</span>
                        <span className="text-muted-foreground">
                          {" "}
                          · {MEDIUM_LABEL[r.medium as keyof typeof MEDIUM_LABEL]} · {r.artifact_name}
                        </span>
                      </p>
                      {r.notes.length ? (
                        <p className="mt-0.5 line-clamp-1 text-[11px] text-muted-foreground">
                          “{r.notes.join(" ")}”
                        </p>
                      ) : null}
                    </div>
                    <Badge variant="outline" className="h-4 shrink-0 rounded px-1 text-[9.5px] font-normal">
                      {POLARITY_LABEL[r.polarity as keyof typeof POLARITY_LABEL]}
                    </Badge>
                  </li>
                ))}
                {!manifest.readings.length ? (
                  <li className="px-3 py-6 text-center text-[12px] text-muted-foreground">No markings yet.</li>
                ) : null}
              </ul>
            </Section>
          </TabsContent>

          <TabsContent value="plates" className="mt-0 min-h-0 overflow-y-auto px-5 py-4">
            {seed ? (
              <PlateStrip seed={seed} />
            ) : (
              <p className="text-[12px] text-muted-foreground">Export the project to render its plates.</p>
            )}
          </TabsContent>

          <TabsContent value="json" className="mt-0 min-h-0 overflow-hidden px-5 py-4">
            <pre className="max-h-[46vh] overflow-auto rounded-md border bg-muted/40 p-3 font-mono text-[10.5px] leading-relaxed">
              {json}
            </pre>
          </TabsContent>
        </Tabs>

        <div className="flex items-center justify-between gap-3 border-t bg-muted/30 px-5 py-3">
          <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
            <FileText size={12} /> Built by the Rust exporter, re-checked before it ships.
          </p>
          <Button size="sm" variant="outline" className="h-7 gap-1.5 text-[11.5px]" asChild disabled={!seed}>
            <a href={seed?.pdf ?? "#"} target="_blank" rel="noreferrer">
              <Printer size={12} /> Open the PDF
            </a>
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="mb-5">
      <p className="mb-2 text-[10.5px] font-medium tracking-[0.13em] text-muted-foreground uppercase">{title}</p>
      {children}
    </div>
  );
}

function List({ lines }: { lines: string[] }) {
  return (
    <ul className="flex flex-col gap-1">
      {lines.map((line) => (
        <li key={line} className="flex gap-2 text-[12px] leading-relaxed">
          <span className="mt-[7px] size-1 shrink-0 rounded-full bg-neutral-400" />
          <span>{line}</span>
        </li>
      ))}
    </ul>
  );
}

function PlateStrip({ seed }: { seed: Seed }) {
  const [active, setActive] = useState(0);
  return (
    <div>
      <div className="overflow-hidden rounded-md border bg-white shadow-sm">
        <img src={seed.plates[active]} alt={`Plate ${active + 1}`} className="w-full" />
      </div>
      <div className="mt-3 flex gap-1.5 overflow-x-auto pb-1">
        {seed.plates.map((p, i) => (
          <button
            key={p}
            onClick={() => setActive(i)}
            className={cn(
              "w-12 shrink-0 overflow-hidden rounded border transition-opacity",
              i === active ? "border-foreground/40 opacity-100" : "border-border opacity-55 hover:opacity-85",
            )}
            aria-label={`Plate ${i + 1}`}
          >
            <img src={p} alt="" className="w-full" />
          </button>
        ))}
      </div>
      <Separator className="my-3" />
      <p className="text-[11px] leading-relaxed text-muted-foreground">
        These are the sample project's plates, rendered at 1.6×. Yours are written into the PDF you
        export.
      </p>
    </div>
  );
}

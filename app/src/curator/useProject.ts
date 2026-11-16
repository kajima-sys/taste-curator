import { useEffect, useState } from "react";
import type { Project } from "./model";

/** The curated project, persisted to localStorage so a reload never loses work. */
const KEY = "taste-curator/project/v1";

export function useProject() {
  const [project, setProject] = useState<Project | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      const saved = localStorage.getItem(KEY);
      if (saved) {
        try {
          const parsed = JSON.parse(saved) as Project;
          if (parsed?.artifacts) {
            if (!cancelled) setProject(parsed);
            return;
          }
        } catch {
          // A corrupt save is replaced by the sample rather than leaving a blank app.
        }
      }
      try {
        const res = await fetch("seed/project.json");
        const seed = (await res.json()) as Project;
        if (!cancelled) setProject(seed);
      } catch {
        if (!cancelled) {
          setError("Could not load the sample project.");
          setProject({ name: "Untitled taste", artifacts: [], next_artifact: 1, next_annotation: 1 });
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (project) localStorage.setItem(KEY, JSON.stringify(project));
  }, [project]);

  return { project, setProject, error };
}

export function resetSaved() {
  localStorage.removeItem(KEY);
}

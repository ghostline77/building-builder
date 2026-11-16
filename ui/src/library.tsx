import { useEffect, useState } from "react";
import { Button, Input, Select } from "@ghostline/ui-core";
import { api, response, building as b } from "./store";
import type { LibraryModule } from "./model";
import type { Pending } from "./plan";
export function Library({ place }: { place: (v: Pending) => void }) {
  const [query, setQuery] = useState(""),
    [role, setRole] = useState(""),
    [kind, setKind] = useState(""),
    [offset, setOffset] = useState(0),
    [result, setResult] = useState<{ entries: LibraryModule[]; total: number }>(
      { entries: [], total: 0 },
    );
  useEffect(() => {
    const abort = new AbortController(),
      timer = setTimeout(() => {
        api<typeof result>(
          "/api/library?" +
            new URLSearchParams({
              q: query,
              role,
              kind,
              offset: String(offset),
            }),
          undefined,
          abort.signal,
        )
          .then((value) => {
            if (!abort.signal.aborted) setResult(value);
          })
          .catch((e) => {
            if (!abort.signal.aborted) b.report(e);
          });
      }, 200);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [query, role, kind, offset]);
  return (
    <div className="space-y-4">
      <h2 className="text-xl font-semibold">Catalogue</h2>
      <Input
        aria-label="Search building catalogue"
        type="search"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setOffset(0);
        }}
        placeholder="Furniture, architecture, props…"
      />
      <label className="grid gap-2 text-sm">
        Role
        <Select
          value={role}
          onChange={(e) => {
            setRole(e.target.value);
            setOffset(0);
          }}
        >
          <option value="">All roles</option>
          {[
            "furniture",
            "chair",
            "table",
            "storage",
            "device",
            "wall",
            "floor",
            "ceiling",
            "doorway",
            "stairs",
            "roof",
            "pillar",
            "railing",
          ].map((v) => (
            <option key={v}>{v}</option>
          ))}
        </Select>
      </label>
      <label className="grid gap-2 text-sm">
        Representation
        <Select
          value={kind}
          onChange={(e) => {
            setKind(e.target.value);
            setOffset(0);
          }}
        >
          <option value="">Assemblies and meshes</option>
          <option value="assembly">Complete assemblies</option>
          <option value="mesh">Individual meshes</option>
          <option value="part">Separate parts</option>
        </Select>
      </label>
      <p role="status" className="text-xs text-muted">
        {result.total} matches · {Math.min(offset + 1, result.total)}–
        {Math.min(offset + 60, result.total)}
      </p>
      <div className="flex gap-2">
        <Button
          disabled={!offset}
          onClick={() => setOffset(Math.max(0, offset - 60))}
        >
          Previous
        </Button>
        <Button
          disabled={offset + 60 >= result.total}
          onClick={() => setOffset(offset + 60)}
        >
          Next
        </Button>
      </div>
      {result.entries.map((module) => (
        <ModuleCard key={module.id} module={module} place={place} />
      ))}
    </div>
  );
}
function ModuleCard({
  module,
  place,
}: {
  module: LibraryModule;
  place: (v: Pending) => void;
}) {
  const [appearance, setAppearance] = useState(module.appearances[0] || ""),
    [view, setView] = useState("hero"),
    [preview, setPreview] = useState(""),
    [busy, setBusy] = useState(false),
    [failed, setFailed] = useState(false);
  const images = module.previews?.[appearance] || [],
    media =
      images.find((m) => m.view === view) ||
      images.find((m) => m.view === "hero") ||
      images[0],
    state = module.states?.[appearance],
    blocked =
      !module.appearances.length ||
      (!!state &&
        (state.resolution !== "resolved" ||
          state.preview === "not_applicable"));
  useEffect(
    () => () => {
      if (preview) URL.revokeObjectURL(preview);
    },
    [preview],
  );
  return (
    <article className="space-y-3 rounded-lg border border-line bg-panel p-3">
      <strong className="text-sm">{module.name.replaceAll("_", " ")}</strong>
      <p className="text-xs text-muted">
        {module.bounds?.dimensions.map((v) => v.toFixed(2)).join(" × ")} m ·{" "}
        {module.role} · {module.kind}
      </p>
      <Select
        aria-label={`Appearance for ${module.name}`}
        value={appearance}
        onChange={(e) => {
          setAppearance(e.target.value);
          setPreview("");
          setFailed(false);
        }}
      >
        {module.appearances.map((v) => (
          <option key={v} value={v}>
            {v || "Default"}
          </option>
        ))}
      </Select>
      <Select
        aria-label={`Preview view for ${module.name}`}
        value={view}
        onChange={(e) => {
          setView(e.target.value);
          setPreview("");
          setFailed(false);
        }}
      >
        {["hero", "front", "back", "left", "right"].map((v) => (
          <option key={v}>{v}</option>
        ))}
      </Select>
      {!failed && (preview || media) && (
        <a
          href={
            preview ||
            "/api/catalogue-media?path=" + encodeURIComponent(media!.path)
          }
          target="_blank"
          rel="noopener noreferrer"
        >
          <img
            loading="lazy"
            className="aspect-square w-full object-contain"
            src={
              preview ||
              "/api/catalogue-media?path=" + encodeURIComponent(media!.path)
            }
            alt={"Preview of " + module.name}
            onError={() => setFailed(true)}
          />
        </a>
      )}
      <p className="text-xs text-muted">
        {blocked
          ? state?.diagnostics.join(" · ") || "No placeable geometry"
          : failed
            ? "Published image missing; native preview available"
            : state?.preview === "failed"
              ? "Catalogue preview failed; native preview available"
              : !media
                ? "Catalogue preview not rendered yet"
                : ""}
      </p>
      <div className="flex flex-wrap gap-2">
        <Button
          size="sm"
          disabled={blocked}
          onClick={() => place({ module, appearance, yaw: 0 })}
        >
          + Place on plan
        </Button>
        <Button
          size="sm"
          disabled={blocked || busy}
          onClick={() => {
            setBusy(true);
            b.run(async () => {
              try {
                const isolated = structuredClone(b.project);
                isolated.rooms = [];
                isolated.openings = {};
                isolated.stairs = [];
                isolated.objects = [
                  {
                    id: "module_preview",
                    asset: module.id,
                    appearance,
                    position: [0, 0, 0],
                    yaw: 0,
                    anchor: "base_center",
                  },
                ];
                await api("/api/compile", isolated);
                const result = await response("/api/preview", {
                  project: isolated,
                  options: {
                    size: 512,
                    cutaway: false,
                    level: null,
                    view: "hero",
                    camera: { frame_projected: view },
                  },
                });
                setPreview(URL.createObjectURL(await result.blob()));
                setFailed(false);
              } finally {
                setBusy(false);
              }
            });
          }}
        >
          Preview module
        </Button>
      </div>
    </article>
  );
}

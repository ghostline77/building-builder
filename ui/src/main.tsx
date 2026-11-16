import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { AppShell, Button, Select } from "@ghostline/ui-core";
import { Viewport, nativeMaterials } from "@ghostline/ui-core/viewport";
import type {
  ViewportHandle,
  ViewportSource,
} from "@ghostline/ui-core/viewport";
import { DraftEditor, JsonFields } from "./forms";
import {
  array,
  at,
  human,
  nameOf,
  newEntry,
  object,
  text,
  uid,
  write,
} from "./model";
import type { Json, JsonObject, Path, PreviewOptions, Project } from "./model";
import { api, building as b, useBuilding } from "./store";
import { Plan } from "./plan";
import type { Pending } from "./plan";
import { Library } from "./library";
import { Modal, Retrofit, WorldSurvey } from "./retrofit";
import "./style.css";
let booted = false;
function BuildingBuilder() {
  const state = useBuilding(),
    { bootstrap, compiled, selection, busy, pending: pendingDrafts } = state,
    project = compiled?.project;
  const [panel, setPanel] = useState("build"),
    [level, setLevel] = useState(0),
    [tool, setTool] = useState("select"),
    [snap, setSnap] = useState(0.25),
    [pending, setPending] = useState<Pending | null>(null),
    [wall, setWall] = useState(""),
    [collision, setCollision] = useState(false),
    [cutaway, setCutaway] = useState(true),
    [whole, setWhole] = useState(true),
    [comparison, setComparison] = useState("proposed"),
    [hidden, setHidden] = useState<string[]>([]),
    [view, setView] = useState("hero"),
    [saved, setSaved] = useState(""),
    [start, setStart] = useState(false),
    [exporting, setExporting] = useState(false),
    [exportResult, setExportResult] = useState<Json | undefined>(),
    [exportOpen, setExportOpen] = useState(false),
    [route, setRoute] = useState<JsonObject>({
      start: [1.5, 1.5, 0],
      end: [1.5, 1.5, 4.2],
    }),
    [routeTriangles, setRouteTriangles] = useState<Json[]>([]),
    [routeMessage, setRouteMessage] = useState(""),
    [inside, setInside] = useState(false),
    [camera, setCamera] = useState<JsonObject>({
      eye: [1.5, 2, 1.7],
      target: [1.5, 14, 1.7],
    });
  const viewport = useRef<ViewportHandle>(null),
    file = useRef<HTMLInputElement>(null),
    lastRequested = useRef("");
  const kit =
      bootstrap?.kits.find((k) => k.id === project?.kit) || bootstrap?.kit,
    height = kit?.storey_height || 4.2;
  const source = useMemo<ViewportSource | undefined>(
    () => (state.preview ? { kind: "gltf", url: state.preview } : undefined),
    [state.preview],
  );
  const options = useMemo<PreviewOptions>(
    () => ({
      browser_scene: true,
      size: 1024,
      cutaway: !inside && cutaway,
      level: whole && array(project?.shells).length ? null : level,
      view,
      comparison,
      hidden_layers: hidden,
      ...(inside
        ? {
            camera: {
              perspective: {
                eye: [
                  Number(array(camera.eye)[0]),
                  Number(array(camera.eye)[2]) + level * height,
                  -Number(array(camera.eye)[1]),
                ],
                target: [
                  Number(array(camera.target)[0]),
                  Number(array(camera.target)[2]) + level * height,
                  -Number(array(camera.target)[1]),
                ],
                up: [0, 1, 0],
                vertical_fov: 1.15,
                near: 0.05,
                far: 200,
              },
            },
          }
        : {}),
    }),
    [
      inside,
      cutaway,
      whole,
      level,
      view,
      comparison,
      hidden,
      camera,
      height,
      project?.shells,
    ],
  );
  const sceneKey = project ? JSON.stringify([project, options]) : "";
  useEffect(() => {
    if (!booted) {
      booted = true;
      b.run(() => b.boot());
    }
  }, []);
  useEffect(() => {
    if (
      !project ||
      busy ||
      state.previewing ||
      pendingDrafts.length ||
      lastRequested.current === sceneKey
    )
      return;
    const timer = setTimeout(() => {
      lastRequested.current = sceneKey;
      b.run(() => b.preview(options));
    }, 450);
    return () => clearTimeout(timer);
  }, [sceneKey, busy, state.previewing, pendingDrafts.length]);
  const removeSelection = (): Promise<void> =>
    b.transact((p) => {
      const value = at(p, selection);
      if (object(value).reference_only)
        throw Error(
          "Imported references are locked. Edit their shell operation instead.",
        );
      write(p, selection, undefined);
      b.update({ selection: [] });
    });
  const duplicate = (): Promise<void> =>
    b.transact((p) => {
      const v = at(p, selection);
      if (!v || typeof v !== "object" || object(v).reference_only)
        throw Error("Select an authored object or room first.");
      const next = structuredClone(object(v));
      next.id = uid(String(selection[0] || "copy"));
      if (Array.isArray(next.position)) {
        next.position[0] = Number(next.position[0]) + snap;
        next.position[1] = Number(next.position[1]) + snap;
      } else if (typeof next.x === "number") next.x += 1;
      array(p[String(selection[0])]).push(next);
    });
  useEffect(() => {
    const unload = (e: BeforeUnloadEvent) => {
      const current = b.getSnapshot();
      if (
        current.pending.length ||
        (current.compiled &&
          JSON.stringify(current.compiled.project) !== current.saved)
      ) {
        e.preventDefault();
        e.returnValue = "";
      }
    };
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        b.run(() => b.save());
      }
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement
      )
        return;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
        e.preventDefault();
        b.run(() => b.undo(e.shiftKey));
      }
      if (e.key === "Escape") {
        setPending(null);
        setTool("select");
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "d") {
        e.preventDefault();
        if (selection.length) b.run(duplicate);
      }
      if (e.key === "Delete" && selection.length) b.run(removeSelection);
    };
    addEventListener("beforeunload", unload);
    addEventListener("keydown", key);
    return () => {
      removeEventListener("beforeunload", unload);
      removeEventListener("keydown", key);
    };
  }, [selection, snap]);
  const exportNative = async (): Promise<void> => {
    b.checkDrafts();
    setExportOpen(true);
    setExporting(true);
    setExportResult(undefined);
    try {
      setExportResult(await b.export());
      b.notify("Native export verified");
    } finally {
      setExporting(false);
    }
  };
  const download = useMemo(
    () =>
      project
        ? URL.createObjectURL(
            new Blob([JSON.stringify(project, null, 2) + "\n"], {
              type: "application/json",
            }),
          )
        : "",
    [project],
  );
  useEffect(
    () => () => {
      if (download) URL.revokeObjectURL(download);
    },
    [download],
  );
  const add = (key: string): void =>
    b.run(async () => {
      if (!project) return;
      const value = newEntry(key, project, level, height);
      if (key === "stairs") {
        const room =
          project.rooms.find(
            (r) =>
              r.id === object(at(project, selection)).id && r.level === level,
          ) || project.rooms.find((r) => r.level === level);
        if (!room) throw Error("Select a grid room before adding stairs.");
        await b.transact((p) => {
          const next = object(value);
          next.x = room.x * 3 + 0.2;
          next.y = room.y * 3 + 1.2;
          next.run = Math.max(
            3,
            Math.ceil((height / 2 / Math.tan((35 * Math.PI) / 180)) * 20) / 20,
          );
          const rows = array(p.stairs);
          p.stairs = rows;
          rows.push(next);
          if (
            !p.rooms.some(
              (r) =>
                r.level === level + 1 &&
                r.x === room.x &&
                r.y === room.y &&
                r.width === room.width &&
                r.depth === room.depth,
            )
          )
            p.rooms.push({ ...room, id: uid("upper"), level: level + 1 });
          b.update({ selection: ["stairs", rows.length - 1] });
        });
      } else {
        await b.add(key, value);
        const rows = at(b.project, [key]);
        b.update({
          selection: Array.isArray(rows)
            ? [key, rows.length - 1]
            : [key, Object.keys(object(rows)).at(-1) || ""],
        });
      }
    });
  const newProject = async (): Promise<void> => {
    if (!bootstrap) return;
    if (
      !confirm(
        "Start a new building? Save or download the current project first.",
      )
    )
      return;
    const next = structuredClone(bootstrap.starter);
    next.id = uid("building");
    next.name = "New building";
    next.rooms = [];
    next.openings = {};
    next.objects = [];
    await b.load(next);
    setPanel("build");
    setLevel(0);
  };
  const selected = project ? at(project, selection) : undefined;
  return (
    <AppShell
      product="Building builder"
      project={<strong>{project?.name || "Loading project…"}</strong>}
      actions={
        <>
          <Button onClick={() => setStart(true)}>Start project</Button>
          <Button
            disabled={!state.history.length || busy}
            onClick={() => b.run(() => b.undo())}
          >
            Undo
          </Button>
          <Button
            disabled={!state.future.length || busy}
            onClick={() => b.run(() => b.undo(true))}
          >
            Redo
          </Button>
          <Button
            disabled={!project || busy}
            onClick={() => b.run(() => b.save())}
          >
            Save project
          </Button>
          <Button
            variant="primary"
            disabled={!project || busy || exporting}
            onClick={() => b.run(exportNative)}
          >
            Export native archive
          </Button>
        </>
      }
    >
      <main className="grid min-h-0 flex-1 grid-cols-1 lg:grid-cols-[300px_minmax(400px,1fr)_310px]">
        <aside className="space-y-5 border-r border-line p-4">
          <nav aria-label="Editor panels" className="flex flex-wrap gap-1">
            {["build", "library", "project", "retrofit"].map((tab) => (
              <Button
                key={tab}
                size="sm"
                variant={panel === tab ? "primary" : "ghost"}
                onClick={() => setPanel(tab)}
              >
                {human(tab)}
              </Button>
            ))}
          </nav>
          {!project ? (
            <p>Loading building…</p>
          ) : panel === "library" ? (
            <Library
              place={(value) => {
                setPending(value);
                setTool("select");
              }}
            />
          ) : panel === "retrofit" ? (
            <Retrofit level={level} hidden={hidden} setHidden={setHidden} />
          ) : panel === "project" ? (
            <>
              <Panel title="Project" open>
                <DraftEditor
                  name="Project identity"
                  value={{ name: project.name, id: project.id }}
                  apply={(v) => b.transact((p) => Object.assign(p, object(v)))}
                />
                <label className="grid gap-2 text-sm">
                  Saved project
                  <Select
                    value={saved}
                    onChange={(e) => setSaved(e.target.value)}
                  >
                    <option value="">Choose a saved project</option>
                    <optgroup label="Examples">
                      {[
                        ["workshop", "Workshop"],
                        ["stairs-and-devices", "Stairs, doors and navigation"],
                        ["motel-modules", "Original motel module lab"],
                      ].map(([id, name]) => (
                        <option key={id} value={"example:" + id}>
                          {name}
                        </option>
                      ))}
                    </optgroup>
                    <optgroup label="Saved projects">
                      {bootstrap?.projects.map((id) => (
                        <option key={id}>{id}</option>
                      ))}
                    </optgroup>
                  </Select>
                </label>
                <Button
                  disabled={!saved || busy}
                  onClick={() =>
                    b.run(async () => {
                      const next = await api<Project>(
                        saved.startsWith("example:")
                          ? "/api/example?id=" +
                              encodeURIComponent(saved.slice(8))
                          : "/api/open?id=" + encodeURIComponent(saved),
                      );
                      await b.load(next, !saved.startsWith("example:"));
                      setLevel(0);
                    })
                  }
                >
                  Open project
                </Button>
                <Button onClick={() => b.run(newProject)}>
                  New empty project
                </Button>
                <a
                  className="block rounded border border-line p-3 text-sm"
                  href={download}
                  download={project.id + ".building.json"}
                >
                  Download project JSON
                </a>
                <Button onClick={() => file.current?.click()}>
                  Import project JSON
                </Button>
                <p className="text-xs text-muted">
                  {bootstrap?.catalogue_count.toLocaleString()} catalogue
                  objects. {bootstrap?.library_source}.
                </p>
              </Panel>
              <Panel title="Existing world">
                <WorldSurvey />
              </Panel>
              <Panel title="All project settings">
                <DraftEditor
                  name="All project settings"
                  value={project}
                  apply={(v) => b.transact((p) => Object.assign(p, object(v)))}
                />
              </Panel>
            </>
          ) : (
            <>
              <Panel title="Rooms" open>
                {project.rooms.map((r, i) => (
                  <Button
                    key={r.id}
                    className="w-full justify-start"
                    onClick={() => b.update({ selection: ["rooms", i] })}
                  >
                    {r.id} · floor {r.level}
                  </Button>
                ))}
                <Button onClick={() => add("rooms")}>+ Add room</Button>
              </Panel>
              <Panel title="Selected wall" open>
                {wall ? (
                  <>
                    <p className="text-xs text-muted">{wall}</p>
                    <label className="grid gap-2 text-sm">
                      Opening
                      <Select
                        value={project.openings[wall] || "solid"}
                        onChange={(e) => {
                          const nextValue = e.target.value;
                          return b.run(() =>
                            b.transact((p) => {
                              if (nextValue === "solid")
                                delete p.openings[wall];
                              else p.openings[wall] = nextValue;
                              if (
                                !["door_auto", "door_manual"].includes(
                                  nextValue,
                                )
                              ) {
                                delete object(p.door_devices)[wall];
                                delete object(p.door_states)[wall];
                              }
                            }),
                          );
                        }}
                      >
                        {[
                          "solid",
                          "doorway",
                          "passage",
                          "door_auto",
                          "door_manual",
                        ].map((o) => (
                          <option key={o}>{o}</option>
                        ))}
                      </Select>
                    </label>
                    {["door_auto", "door_manual"].includes(
                      project.openings[wall] || "",
                    ) && (
                      <>
                        <label className="grid gap-2 text-sm">
                          Initial door state
                          <Select
                            value={text(
                              object(project.door_states)[wall],
                              "CLOSED",
                            )}
                            onChange={(e) => {
                              const nextValue = e.target.value;
                              return b.run(() =>
                                b.transact((p) => {
                                  const states = object(p.door_states);
                                  p.door_states = states;
                                  states[wall] = nextValue;
                                }),
                              );
                            }}
                          >
                            {["CLOSED", "OPENED", "LOCKED"].map((o) => (
                              <option key={o}>{o}</option>
                            ))}
                          </Select>
                        </label>
                        <label className="grid gap-2 text-sm">
                          Native door preset
                          <Select
                            value={text(
                              object(object(project.door_devices)[wall]).preset,
                              "sliding_entropy_c",
                            )}
                            onChange={(e) => {
                              const nextValue = e.target.value;
                              return b.run(() =>
                                b.transact((p) => {
                                  const devices = object(p.door_devices);
                                  p.door_devices = devices;
                                  devices[wall] = {
                                    ...object(devices[wall]),
                                    preset: nextValue,
                                  };
                                }),
                              );
                            }}
                          >
                            {[
                              "sliding_entropy_c",
                              "hinged_h",
                              "hinged_e",
                              "sliding_kitsch_double_f",
                              "garage_w300_h250",
                            ].map((o) => (
                              <option key={o}>{o}</option>
                            ))}
                          </Select>
                        </label>
                      </>
                    )}
                  </>
                ) : (
                  <p className="text-sm text-muted">
                    Select a wall in the plan to edit its opening.
                  </p>
                )}
              </Panel>
              <Panel title="Construction kit">
                <label className="grid gap-2 text-sm">
                  Measured kit
                  <Select
                    value={project.kit}
                    onChange={(e) => {
                      const nextValue = e.target.value;
                      return b.run(() =>
                        b.transact((p) => {
                          const kit = bootstrap?.kits.find(
                            (k) => k.id === nextValue,
                          );
                          if (!kit) return;
                          p.kit = kit.id;
                          p.wall_appearance = kit.wall_appearances[0] || "";
                          p.floor_appearance = kit.floor_appearances[0] || "";
                        }),
                      );
                    }}
                  >
                    {bootstrap?.kits.map((k) => (
                      <option key={k.id} value={k.id}>
                        {k.name}
                      </option>
                    ))}
                  </Select>
                </label>
              </Panel>
              {[
                "objects",
                "shell_rooms",
                "partitions",
                "stairs",
                "door_devices",
                "workstations",
                "lights",
                "audio_areas",
              ].map((key) => (
                <Panel key={key} title={human(key)}>
                  {(Array.isArray(project[key])
                    ? array(project[key]).map((v, i) => ({
                        value: v,
                        path: [key, i] as Path,
                      }))
                    : Object.entries(object(project[key])).map(
                        ([id, value]) => ({
                          value: value!,
                          path: [key, id] as Path,
                        }),
                      )
                  ).map(({ value, path }) => (
                    <Button
                      key={JSON.stringify(path)}
                      className="w-full justify-start"
                      onClick={() => b.update({ selection: path })}
                    >
                      {nameOf(value)}
                    </Button>
                  ))}
                  {key !== "objects" && (
                    <Button onClick={() => add(key)}>
                      + Add {human(key.replace(/s$/, ""))}
                    </Button>
                  )}
                </Panel>
              ))}
              <label className="flex gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={project.interior === true}
                  onChange={(e) => {
                    const nextChecked = e.target.checked;
                    return b.run(() => b.set(["interior"], nextChecked));
                  }}
                />
                Classify grid rooms as interiors
              </label>
            </>
          )}
        </aside>
        <section className="min-w-0 space-y-5 p-4">
          {project && (
            <>
              <div className="flex flex-wrap items-center gap-3">
                <span className="flex-1 text-xs text-muted">
                  {compiled?.placements.length} placements ·{" "}
                  {compiled?.solids.length} new colliders
                </span>
                <label className="flex items-center gap-2 text-sm">
                  Floor
                  <Select
                    value={level}
                    onChange={(e) => setLevel(Number(e.target.value))}
                  >
                    {["Ground", "First", "Second", "Third"].map((v, i) => (
                      <option key={i} value={i}>
                        {v}
                      </option>
                    ))}
                  </Select>
                </label>
              </div>
              <div className="flex flex-wrap gap-3">
                <label className="grid gap-1 text-xs">
                  Plan tool
                  <Select
                    value={tool}
                    onChange={(e) => setTool(e.target.value)}
                  >
                    {[
                      ["select", "Select"],
                      ["draw", "Draw room"],
                      ["move", "Move room"],
                      ["resize", "Resize room"],
                      ["partition", "Draw partition"],
                      ["object-move", "Move object"],
                      ["object-rotate", "Rotate object"],
                    ].map(([v, t]) => (
                      <option key={v} value={v}>
                        {t}
                      </option>
                    ))}
                  </Select>
                </label>
                <label className="grid gap-1 text-xs">
                  Snap · metres
                  <Select
                    value={snap}
                    onChange={(e) => setSnap(Number(e.target.value))}
                  >
                    {[0.05, 0.1, 0.25, 0.5, 1].map((v) => (
                      <option key={v}>{v}</option>
                    ))}
                  </Select>
                </label>
                <label className="flex items-center gap-2 text-xs">
                  <input
                    type="checkbox"
                    checked={collision}
                    onChange={(e) => setCollision(e.target.checked)}
                  />
                  Collision overlay
                </label>
              </div>
              <Plan
                level={level}
                tool={tool}
                snap={snap}
                pending={pending}
                setPending={setPending}
                setWall={setWall}
                collision={collision}
                route={routeTriangles}
              />
              <div className="flex flex-wrap items-center gap-3">
                <h2 className="mr-auto text-lg font-semibold">
                  Material preview
                </h2>
                <label className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={cutaway}
                    onChange={(e) => setCutaway(e.target.checked)}
                  />
                  Cutaway
                </label>
                <Select
                  aria-label="Preview view"
                  value={view}
                  onChange={(e) => {
                    setView(e.target.value);
                    viewport.current?.fit(
                      e.target.value === "hero"
                        ? "perspective"
                        : (e.target.value as
                            "front" | "back" | "left" | "right"),
                    );
                  }}
                >
                  {["hero", "front", "back", "left", "right"].map((v) => (
                    <option key={v}>{v}</option>
                  ))}
                </Select>
                <Button
                  disabled={state.previewing}
                  onClick={() => b.run(() => b.preview(options))}
                >
                  Refresh preview
                </Button>
              </div>
              {array(project.shells).length > 0 && (
                <div className="flex flex-wrap gap-3">
                  <label className="flex items-center gap-2 text-sm">
                    Building comparison
                    <Select
                      value={comparison}
                      onChange={(e) => setComparison(e.target.value)}
                    >
                      {["existing", "proposed", "changes"].map((v) => (
                        <option key={v}>{v}</option>
                      ))}
                    </Select>
                  </label>
                  <label className="flex items-center gap-2 text-sm">
                    <input
                      type="checkbox"
                      checked={whole}
                      onChange={(e) => setWhole(e.target.checked)}
                    />
                    Whole building
                  </label>
                </div>
              )}
              <Viewport
                ref={viewport}
                label="Building 3D preview"
                className="h-[500px] rounded-xl border border-line"
                {...(source ? { source } : {})}
                materialAdapter={nativeMaterials}
                preserveCamera
                onError={b.report}
                onSelect={(value) => {
                  if (!value) return;
                  const index = project.objects.findIndex(
                    (o) =>
                      value.name === o.id || value.name.startsWith(o.id + ":"),
                  );
                  if (index >= 0) b.update({ selection: ["objects", index] });
                }}
              />
              <div className="flex flex-wrap gap-2">
                <Button onClick={() => viewport.current?.fit("perspective")}>
                  Fit
                </Button>
                <Button onClick={() => viewport.current?.zoom(0.8)}>
                  Zoom in
                </Button>
                <Button onClick={() => viewport.current?.zoom(1.2)}>
                  Zoom out
                </Button>
                <Button
                  onClick={() => viewport.current?.fit("perspective", true)}
                >
                  Focus selection
                </Button>
                <Button onClick={() => viewport.current?.orbit(0.3)}>
                  Orbit
                </Button>
              </div>
              {state.previewing && (
                <p className="text-muted">Preparing interactive scene…</p>
              )}
              {state.previewKey !== sceneKey && state.preview && (
                <p className="text-xs text-warning">Preview needs refresh</p>
              )}
            </>
          )}
        </section>
        <aside className="space-y-5 border-l border-line p-4">
          {project && (
            <>
              <Panel title="Selection" open>
                {selection.length > 0 && selected !== undefined ? (
                  <>
                    <p className="break-all text-xs text-muted">
                      {selection.join(" / ")}
                    </p>
                    {object(selected).reference_only ? (
                      <div className="space-y-3">
                        <p className="text-sm text-muted">
                          Locked native reference. Use Retrofit to change its
                          source geometry.
                        </p>
                        <pre className="overflow-auto whitespace-pre-wrap break-all text-xs">
                          {JSON.stringify(selected, null, 2)}
                        </pre>
                      </div>
                    ) : (
                      <DraftEditor
                        key={JSON.stringify(selection)}
                        name={"Selection " + JSON.stringify(selection)}
                        value={selected}
                        apply={(v) => b.set(selection, v)}
                        remove={removeSelection}
                      />
                    )}
                    {!object(selected).reference_only && (
                      <div className="flex flex-wrap gap-2">
                        <Button size="sm" onClick={() => b.run(duplicate)}>
                          Duplicate
                        </Button>
                        {selection[0] === "objects" && (
                          <Button
                            size="sm"
                            onClick={() =>
                              b.run(() =>
                                b.transact((p) => {
                                  p.objects[Number(selection[1])]!.yaw += 15;
                                }),
                              )
                            }
                          >
                            Rotate 15°
                          </Button>
                        )}
                      </div>
                    )}
                  </>
                ) : (
                  <p className="text-sm text-muted">
                    Select a room or object to edit its dimensions and
                    placement.
                  </p>
                )}
              </Panel>
              <Panel title="Finishes">
                {(["wall", "floor"] as const).map((kind) => (
                  <label key={kind} className="grid gap-2 text-sm">
                    {human(kind)} finish
                    <Select
                      value={project[kind + "_appearance"] as string}
                      onChange={(e) => {
                        const nextValue = e.target.value;
                        return b.run(() =>
                          b.set([kind + "_appearance"], nextValue),
                        );
                      }}
                    >
                      {(kind === "wall"
                        ? kit?.wall_appearances
                        : kit?.floor_appearances
                      )?.map((v) => (
                        <option key={v}>{v}</option>
                      ))}
                    </Select>
                  </label>
                ))}
              </Panel>
              <Panel title="World anchor">
                <DraftEditor
                  name="World anchor"
                  value={{ origin: project.origin, yaw: project.yaw }}
                  apply={(v) => b.transact((p) => Object.assign(p, object(v)))}
                />
              </Panel>
              <Panel title="Human navigation">
                <DraftEditor
                  name="Navigation"
                  value={
                    project.navigation || {
                      export: false,
                      radius: 0.3,
                      height: 1.8,
                      climb: 0.6,
                      layer: 200,
                      connections: [],
                    }
                  }
                  apply={(v) => b.set(["navigation"], v)}
                />
                <h3 className="text-sm font-semibold">Route probe</h3>
                <JsonFields
                  value={route}
                  onChange={(v) => setRoute(object(v))}
                />
                <Button
                  onClick={() =>
                    b.run(async () => {
                      const result = await api<JsonObject>("/api/route", {
                        project,
                        ...route,
                      });
                      setRouteTriangles(array(result.triangles));
                      setRouteMessage(
                        result.connected
                          ? "Connected · " +
                              array(result.faces).length +
                              " polygons"
                          : "No connected route",
                      );
                    })
                  }
                >
                  Check route
                </Button>
                <p role="status" className="text-xs text-muted">
                  {routeMessage}
                </p>
              </Panel>
              <Panel title="Interior camera">
                <label className="flex gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={inside}
                    onChange={(e) => setInside(e.target.checked)}
                  />
                  View from inside the building
                </label>
                <JsonFields
                  value={camera}
                  onChange={(v) => setCamera(object(v))}
                />
                <Button
                  onClick={() => {
                    setInside(true);
                    const eye = array(camera.eye),
                      target = array(camera.target);
                    viewport.current?.lookAt(
                      [
                        Number(eye[0]),
                        Number(eye[2]) + level * height,
                        -Number(eye[1]),
                      ],
                      [
                        Number(target[0]),
                        Number(target[2]) + level * height,
                        -Number(target[1]),
                      ],
                    );
                  }}
                >
                  Walk inside
                </Button>
              </Panel>
              <Panel title="Validation" open>
                {compiled?.diagnostics.length ? (
                  compiled.diagnostics.map((d, i) => (
                    <p className="text-xs text-warning" key={i}>
                      {d}
                    </p>
                  ))
                ) : (
                  <p className="text-xs text-muted">No compiler diagnostics.</p>
                )}
              </Panel>
              <Panel title="Technical layers">
                {[
                  "Architecture",
                  "Collision",
                  "Door devices",
                  "Workstations",
                  "Practical lights",
                  "Ambience areas",
                ].map((name, i) => (
                  <p key={name} className="flex justify-between gap-3 text-xs">
                    <span>{name}</span>
                    <span>
                      {i === 0
                        ? compiled?.placements.length
                        : i === 1
                          ? compiled?.solids.length
                          : i === 2
                            ? Object.keys(object(project.door_devices)).length
                            : i === 3
                              ? array(project.workstations).length
                              : i === 4
                                ? array(project.lights).length
                                : array(project.audio_areas).length}
                    </span>
                  </p>
                ))}
              </Panel>
            </>
          )}
        </aside>
      </main>
      <footer className="flex flex-wrap justify-between gap-3 border-t border-line p-4 text-xs">
        <p
          role={state.error ? "alert" : "status"}
          className={state.error ? "text-danger" : "text-muted"}
        >
          {state.message}
        </p>
        <span className="text-muted">
          {project
            ? `Revision ${project.revision} · ${JSON.stringify(project) === state.saved ? "saved" : "unsaved"}`
            : ""}
          {pendingDrafts.length
            ? ` · ${pendingDrafts.length} pending settings`
            : ""}
        </span>
      </footer>
      {start && (
        <Modal title="Start a building project" close={() => setStart(false)}>
          <div className="flex flex-wrap gap-3">
            <Button
              onClick={() =>
                b.run(async () => {
                  await newProject();
                  setStart(false);
                })
              }
            >
              New building
            </Button>
            <Button
              onClick={() => {
                setPanel("retrofit");
                setStart(false);
              }}
            >
              Retrofit existing building
            </Button>
            <Button
              onClick={() => {
                setPanel("project");
                setStart(false);
              }}
            >
              Open saved project
            </Button>
          </div>
        </Modal>
      )}
      {exportOpen && (
        <Modal
          title="Native export"
          close={() => {
            if (!exporting) setExportOpen(false);
          }}
        >
          <p className="mb-4 text-sm text-muted">
            Exports the complete building. Cutaway and floor filters affect only
            the preview.
          </p>
          {exporting ? (
            <p role="status">
              Writing native resources, checking readback and packing archive…
            </p>
          ) : exportResult !== undefined ? (
            <div className="space-y-3">
              <p className="text-sm">
                {text(object(exportResult).meshes)} meshes ·{" "}
                {text(object(exportResult).entities, "0")} entities ·{" "}
                {text(object(exportResult).colliders)} colliders ·{" "}
                {text(object(exportResult).verified_resources)} native resources
                verified
              </p>
              <p className="break-all text-xs">
                SHA-256 {text(object(exportResult).archive_sha256)}
              </p>
              {array(object(exportResult).diagnostics).map((d, i) => (
                <p key={i} className="text-xs text-muted">
                  {text(d)}
                </p>
              ))}
              {array(object(exportResult).files).map((f) => (
                <a
                  key={text(f)}
                  className="block text-sm text-accent"
                  href={
                    "/api/download?" +
                    new URLSearchParams({
                      id: text(object(exportResult).export_id),
                      file: text(f),
                    })
                  }
                >
                  Download {text(f)}
                </a>
              ))}
            </div>
          ) : (
            <p role="alert">{state.message}</p>
          )}
        </Modal>
      )}
      <input
        ref={file}
        type="file"
        accept="application/json,.json"
        className="hidden"
        onChange={(e) => {
          const input = e.target.files?.[0];
          e.target.value = "";
          if (input)
            b.run(async () =>
              b.load(JSON.parse(await input.text()) as Project),
            );
        }}
      />
    </AppShell>
  );
}
function Panel({
  title,
  children,
  open = false,
}: {
  title: string;
  children: ReactNode;
  open?: boolean;
}) {
  return (
    <details open={open} className="rounded-lg border border-line p-3">
      <summary className="text-sm font-semibold">{title}</summary>
      <div className="mt-4 space-y-4">{children}</div>
    </details>
  );
}
createRoot(document.getElementById("root")!).render(<BuildingBuilder />);

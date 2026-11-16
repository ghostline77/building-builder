import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Button, Input, Select } from "@ghostline/ui-core";
import { DraftEditor, JsonFields } from "./forms";
import { array, human, nameOf, newEntry, object, text, uid } from "./model";
import type { Compiled, Json, JsonObject } from "./model";
import { api, response, building as b, useBuilding } from "./store";
export function Modal({
  title,
  children,
  close,
}: {
  title: string;
  children: ReactNode;
  close: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog
      ref={ref}
      onCancel={close}
      className="m-auto max-h-[90dvh] w-[min(1000px,96vw)] overflow-auto rounded-xl border border-line bg-panel p-6 text-foreground backdrop:bg-black/75"
    >
      <div className="mb-5 flex justify-between gap-4">
        <h2 className="text-xl font-semibold">{title}</h2>
        <Button onClick={close}>Close</Button>
      </div>
      {children}
    </dialog>
  );
}
const layers = [
  "structure",
  "furnishings",
  "devices",
  "collision",
  "occlusion",
  "navigation",
];
export function Retrofit({
  level,
  hidden,
  setHidden,
}: {
  level: number;
  hidden: string[];
  setHidden: (v: string[]) => void;
}) {
  const { compiled } = useBuilding(),
    project = compiled!.project,
    shells = array(project.shells),
    [active, setActive] = useState(""),
    [chosen, setChosen] = useState<string[]>([]),
    [importing, setImporting] = useState(false),
    [editingFloors, setEditingFloors] = useState(false),
    [operation, setOperation] = useState<JsonObject | null>(null);
  const shell = object(
      shells.find((v) => object(v).id === active) || shells[0],
    ),
    members = array(shell.members).map(object);
  return (
    <div className="space-y-4">
      <h2 className="text-xl font-semibold">Existing building</h2>
      <Button onClick={() => setImporting(true)}>Import another shell</Button>
      {shells.length > 0 && (
        <>
          <label className="grid gap-2 text-sm">
            Imported building
            <Select
              value={text(shell.id)}
              onChange={(e) => {
                setActive(e.target.value);
                setChosen([]);
              }}
            >
              {shells.map((s) => (
                <option key={text(object(s).id)} value={text(object(s).id)}>
                  {nameOf(s)}
                </option>
              ))}
            </Select>
          </label>
          <details open>
            <summary>Visible layers</summary>
            <div className="mt-3 space-y-2">
              {layers.map((layer) => (
                <label key={layer} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={!hidden.includes(layer)}
                    onChange={(e) =>
                      setHidden(
                        e.target.checked
                          ? hidden.filter((v) => v !== layer)
                          : [...hidden, layer],
                      )
                    }
                  />
                  {human(layer)}
                </label>
              ))}
            </div>
          </details>
          <p className="text-xs text-muted">
            {members.filter((m) => !m.removed).length} retained pieces ·{" "}
            {array(shell.operations).length} changes ·{" "}
            {array(shell.floors).length} floors
          </p>
          <Button onClick={() => setEditingFloors(true)}>
            Edit interior floor boundaries
          </Button>
          <div className="flex flex-wrap gap-2">
            {[
              ["entrance", "Create entrance"],
              ["replace_wall", "Replace wall"],
              ["remove", "Remove obstruction"],
              ["floor_connection", "Add floor connection"],
            ].map(([kind, label]) => (
              <Button
                key={kind}
                onClick={() =>
                  b.run(() => {
                    const selected = members.filter(
                      (m) => chosen.includes(text(m.id)) && !m.removed,
                    );
                    if (!selected.length)
                      throw Error(
                        "Select the existing pieces affected by the change first.",
                      );
                    const first =
                        selected.find((m) => m.layer === "structure") ||
                        selected[0]!,
                      p = array(first.position),
                      a = (-project.yaw * Math.PI) / 180,
                      dx = Number(p[0]) - Number(project.origin[0]),
                      dy = Number(p[1]) - Number(project.origin[1]),
                      position = [
                        dx * Math.cos(a) - dy * Math.sin(a),
                        dx * Math.sin(a) + dy * Math.cos(a),
                        Number(p[2]) - Number(project.origin[2]),
                      ],
                      id = uid("change");
                    setOperation({
                      id,
                      label: label!,
                      kind: kind!,
                      members: selected.map((m) => text(m.id)),
                      position,
                      size:
                        kind === "floor_connection"
                          ? [2.6, 3, 0.6]
                          : [1.5, 1, 2.5],
                      yaw: 0,
                      preset: "hinged_e",
                      diagnostics: [],
                      stair:
                        kind === "floor_connection"
                          ? {
                              id,
                              x: Number(position[0]) - 1.3,
                              y: Number(position[1]) - 1.5,
                              width: 2.6,
                              run: 3,
                              level,
                              landing: 1.2,
                              reverse: false,
                              railings: true,
                              axis: "y",
                              type: "return",
                              flight_width: 1.2,
                            }
                          : null,
                    });
                  })
                }
              >
                {label}
              </Button>
            ))}
          </div>
          <details open>
            <summary>Select existing pieces</summary>
            <div className="mt-4 space-y-2">
              {members.map((m) => (
                <label
                  key={text(m.id)}
                  className="flex items-start gap-2 text-xs"
                >
                  <input
                    type="checkbox"
                    checked={chosen.includes(text(m.id))}
                    disabled={!!m.removed && m.layer !== "navigation"}
                    onChange={(e) =>
                      setChosen(
                        e.target.checked
                          ? [...chosen, text(m.id)]
                          : chosen.filter((v) => v !== m.id),
                      )
                    }
                  />
                  <span>
                    {text(m.layer)}: {text(m.label)}
                    {m.removed ? " · changed" : ""}
                  </span>
                </label>
              ))}
            </div>
          </details>
          <section className="space-y-3">
            <h3 className="font-semibold">Planned changes</h3>
            {array(shell.operations).map((op) => (
              <div
                key={text(object(op).id)}
                className="space-y-2 rounded border border-line p-3"
              >
                <strong className="text-sm">{text(object(op).label)}</strong>
                <p className="text-xs text-muted">
                  {array(object(op).members).length} source pieces ·{" "}
                  {text(object(op).kind)}
                </p>
                <Button
                  size="sm"
                  onClick={() =>
                    b.run(async () =>
                      b.accept(
                        await api<Compiled>("/api/shell/operation", {
                          project: b.project,
                          shell: shell.id,
                          operation: op,
                          restore: true,
                        }),
                      ),
                    )
                  }
                >
                  Restore original
                </Button>
              </div>
            ))}
          </section>
          <details>
            <summary>Source pins and export review</summary>
            <p className="mt-3 text-xs text-muted">
              Source hashes and node/instance counts are validated on
              compilation and export. Retained references are preview-only.
              Additions and replacement geometry are packed with exact source
              removals.
            </p>
            <pre className="mt-3 overflow-auto whitespace-pre-wrap break-all text-xs">
              {JSON.stringify(project.retrofit, null, 2)}
            </pre>
          </details>
        </>
      )}
      {importing && <ImportWizard close={() => setImporting(false)} />}
      {editingFloors && (
        <Modal
          title="Interior floor boundaries"
          close={() => setEditingFloors(false)}
        >
          <FloorEditor
            floors={array(shell.floors)}
            apply={async (floors) => {
              await b.transact((p) => {
                const item = array(p.shells)
                  .map(object)
                  .find((s) => s.id === shell.id);
                if (!item) throw Error("Shell no longer exists.");
                item.floors = floors;
                p.shell_rooms = array(p.shell_rooms).filter(
                  (r) =>
                    !text(object(r).id).startsWith(text(shell.id) + "_floor_"),
                );
                for (const f of floors) {
                  const floor = object(f);
                  array(p.shell_rooms).push({
                    id: text(shell.id) + "_floor_" + text(floor.level),
                    position: floor.position!,
                    size: floor.size!,
                    yaw: floor.yaw!,
                    level: floor.level!,
                    retained_sides: [
                      "north",
                      "south",
                      "east",
                      "west",
                      "floor",
                      "ceiling",
                    ],
                    openings: {},
                  });
                }
              });
              setEditingFloors(false);
            }}
          />
        </Modal>
      )}
      {operation && (
        <Modal title={text(operation.label)} close={() => setOperation(null)}>
          <p className="mb-4 text-sm text-muted">
            Review collision and occlusion separately. Selected source pieces
            are replaced where the change volume intersects.
          </p>
          <JsonFields
            value={operation}
            onChange={(v) => setOperation(object(v))}
          />
          <details className="my-4">
            <summary>Related technical layers</summary>
            <div className="mt-3 space-y-2">
              {members
                .filter(
                  (m) =>
                    ["collision", "occlusion", "navigation"].includes(
                      text(m.layer),
                    ) && !m.removed,
                )
                .map((m) => (
                  <label
                    key={text(m.id)}
                    className="flex items-start gap-2 text-xs"
                  >
                    <input
                      type="checkbox"
                      checked={array(operation.members).includes(text(m.id))}
                      onChange={(e) =>
                        setOperation({
                          ...operation,
                          members: e.target.checked
                            ? [...array(operation.members), text(m.id)]
                            : array(operation.members).filter(
                                (v) => v !== m.id,
                              ),
                        })
                      }
                    />
                    {text(m.layer)}: {text(m.label)}
                  </label>
                ))}
            </div>
          </details>
          <Button
            variant="primary"
            onClick={() =>
              b.run(async () => {
                b.checkDrafts();
                b.accept(
                  await api<Compiled>("/api/shell/operation", {
                    project: b.project,
                    shell: shell.id,
                    operation,
                  }),
                );
                setOperation(null);
                setChosen([]);
              })
            }
          >
            Apply change
          </Button>
        </Modal>
      )}
    </div>
  );
}
function FloorEditor({
  floors,
  apply,
}: {
  floors: Json[];
  apply: (floors: Json[]) => Promise<unknown>;
}) {
  const [value, setValue] = useState<Json[]>(() => structuredClone(floors));
  return (
    <div className="space-y-4">
      <JsonFields value={value} onChange={(v) => setValue(array(v))} />
      <Button
        onClick={() =>
          setValue([
            ...value,
            {
              name: "Floor " + value.length,
              level: value.length,
              position: [0, 0, value.length * 4.2],
              size: [6, 6, 4.2],
              yaw: 0,
            },
          ])
        }
      >
        Add floor
      </Button>
      <Button variant="primary" onClick={() => b.run(() => apply(value))}>
        Apply floor boundaries
      </Button>
    </div>
  );
}
function ImportWizard({ close }: { close: () => void }) {
  const [sites, setSites] = useState<JsonObject[]>([]),
    [site, setSite] = useState(""),
    [anchor, setAnchor] = useState<JsonObject>({
      position: b.project.origin,
      yaw: b.project.yaw,
      radius: 15,
      name: "Existing building",
    }),
    [survey, setSurvey] = useState<JsonObject | null>(null),
    [chosen, setChosen] = useState<string[]>([]),
    [filter, setFilter] = useState("structure"),
    [query, setQuery] = useState(""),
    [floors, setFloors] = useState<Json[]>([
      {
        name: "Ground",
        level: 0,
        position: [0, 0, 0],
        size: [6, 6, 4.2],
        yaw: 0,
      },
    ]),
    [preview, setPreview] = useState(""),
    [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  useEffect(() => {
    api<Json[]>("/api/shell/sites")
      .then((v) => setSites(v.map(object)))
      .catch(b.report);
  }, []);
  useEffect(
    () => () => {
      if (preview) URL.revokeObjectURL(preview);
    },
    [preview],
  );
  const run = (fn: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    setError("");
    Promise.resolve()
      .then(fn)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const draft = structuredClone(b.project);
  draft.origin = array(anchor.position).map(Number);
  draft.yaw = Number(anchor.yaw);
  draft.name = text(anchor.name);
  const selectedSite = sites[Number(site)],
    members = array(survey?.members).map(object),
    visible = members.filter(
      (m) =>
        (!filter || m.layer === filter) &&
        text(m.label).toLowerCase().includes(query.toLowerCase()),
    );
  const payload = () => ({
    project: draft,
    name: anchor.name,
    sources: survey?.sources,
    members: members.filter((m) => chosen.includes(text(m.id))),
    floors,
  });
  return (
    <Modal title="Retrofit an existing building" close={close}>
      <div className="space-y-5">
        <p className="text-sm text-muted">
          1 · Locate → 2 · Select shell → 3 · Define floors. Native source
          pivots and materials remain pinned.
        </p>
        <label className="grid gap-2 text-sm">
          Surveyed location
          <Select
            value={site}
            onChange={(e) => {
              setSite(e.target.value);
              const s = sites[Number(e.target.value)];
              if (e.target.value !== "" && s) {
                setAnchor({
                  position: s.position!,
                  yaw: s.yaw!,
                  radius: s.radius!,
                  name: s.name!,
                });
                if (s.floors) setFloors(array(s.floors));
              }
            }}
          >
            <option value="">Custom location / current anchor</option>
            {sites.map((s, i) => (
              <option key={i} value={i}>
                {text(s.name)}
              </option>
            ))}
          </Select>
        </label>
        <JsonFields value={anchor} onChange={(v) => setAnchor(object(v))} />
        <Button
          disabled={busy}
          variant="primary"
          onClick={() =>
            run(async () => {
              let sectors = array(
                site !== "" ? selectedSite?.sectors : undefined,
              );
              if (site !== "") {
                const nearby = await api<JsonObject>("/api/world-sectors", {
                  position: anchor.position,
                  radius: anchor.radius,
                });
                sectors = [
                  ...new Set([
                    ...sectors,
                    ...array(nearby.sectors)
                      .filter((s) => object(s).category === "Navigation")
                      .map((s) => object(s).path!),
                  ]),
                ];
              }
              const found = await api<JsonObject>("/api/shell/survey", {
                position: anchor.position,
                radius: anchor.radius,
                sectors,
              });
              setSurvey(found);
              setChosen(
                array(found.members)
                  .map(object)
                  .filter(
                    (m) =>
                      ["collision", "occlusion", "navigation"].includes(
                        text(m.layer),
                      ) ||
                      (site !== ""
                        ? array(selectedSite?.selection).some((s) => {
                            const v = object(s);
                            return (
                              v.index === m.index &&
                              (v.instance ?? null) === (m.instance ?? null) &&
                              (v.actor ?? null) === (m.actor ?? null) &&
                              (v.shape ?? null) === (m.shape ?? null)
                            );
                          })
                        : m.layer === "structure"),
                  )
                  .map((m) => text(m.id)),
              );
            })
          }
        >
          Find building pieces
        </Button>
        {survey && (
          <>
            <div className="flex flex-wrap gap-3">
              <Select
                aria-label="Shell layer"
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
              >
                <option value="">All layers</option>
                {layers.map((v) => (
                  <option key={v}>{v}</option>
                ))}
              </Select>
              <Input
                type="search"
                aria-label="Filter shell pieces"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder="Filter piece names"
              />
              <Button
                onClick={() =>
                  setChosen([
                    ...new Set([...chosen, ...visible.map((m) => text(m.id))]),
                  ])
                }
              >
                Select visible
              </Button>
              <Button
                onClick={() =>
                  setChosen(
                    chosen.filter((id) => !visible.some((m) => m.id === id)),
                  )
                }
              >
                Clear visible
              </Button>
            </div>
            <p className="text-sm text-muted">
              {chosen.length} selected · {members.length} native pieces ·{" "}
              {text(survey.sectors_scanned)} sectors
            </p>
            <div className="max-h-72 space-y-2 overflow-auto rounded border border-line p-3">
              {visible.map((m) => (
                <label
                  key={text(m.id)}
                  className="flex items-start gap-2 text-sm"
                >
                  <input
                    type="checkbox"
                    checked={chosen.includes(text(m.id))}
                    onChange={(e) =>
                      setChosen(
                        e.target.checked
                          ? [...chosen, text(m.id)]
                          : chosen.filter((v) => v !== m.id),
                      )
                    }
                  />
                  {text(m.label)} · {text(m.layer)} ·{" "}
                  {array(m.diagnostics)
                    .map((v) => text(v))
                    .join("; ")}
                </label>
              ))}
            </div>
            <details>
              <summary>Source errors</summary>
              <pre className="overflow-auto whitespace-pre-wrap text-xs">
                {JSON.stringify(survey.failures, null, 2)}
              </pre>
            </details>
            <h3 className="font-semibold">Interior floors</h3>
            <JsonFields value={floors} onChange={(v) => setFloors(array(v))} />
            <Button
              onClick={() =>
                setFloors([
                  ...floors,
                  newEntry("floors", draft, floors.length, 4.2),
                ])
              }
            >
              Add floor
            </Button>
            {preview && (
              <img
                className="max-h-[60dvh] w-full object-contain"
                src={preview}
                alt="Native preview of the selected shell"
              />
            )}
            <div className="flex flex-wrap gap-3">
              <Button
                disabled={busy}
                onClick={() =>
                  run(async () => {
                    const value = await response("/api/shell/preview", {
                      import: payload(),
                      options: { size: 768, cutaway: false, level: null },
                    });
                    setPreview(URL.createObjectURL(await value.blob()));
                  })
                }
              >
                Preview selected shell
              </Button>
              <Button
                variant="primary"
                disabled={busy}
                onClick={() =>
                  run(async () => {
                    if (!chosen.length)
                      throw Error("Select the shell pieces first.");
                    if (!floors.length)
                      throw Error("Define at least one interior floor.");
                    b.checkDrafts();
                    b.accept(
                      await api<Compiled>("/api/shell/import", payload()),
                    );
                    close();
                  })
                }
              >
                Import shell and floors
              </Button>
            </div>
          </>
        )}
        {error && (
          <p role="alert" className="text-danger">
            {error}
          </p>
        )}
      </div>
    </Modal>
  );
}
export function WorldSurvey() {
  const { compiled } = useBuilding(),
    project = compiled!.project,
    [anchor, setAnchor] = useState<JsonObject>({
      position: project.origin,
      radius: 20,
      sector: "",
    }),
    [sectors, setSectors] = useState<Json[]>([]),
    [survey, setSurvey] = useState<JsonObject | null>(null),
    [filter, setFilter] = useState("");
  return (
    <div className="space-y-4">
      <p className="text-sm text-muted">
        Find sectors around the world anchor. Survey objects to import locked
        references or choose exact geometry to remove.
      </p>
      <JsonFields value={anchor} onChange={(v) => setAnchor(object(v))} />
      <Button
        onClick={() =>
          b.run(async () => {
            const value = await api<JsonObject>("/api/world-sectors", {
              position: anchor.position,
              radius: anchor.radius,
            });
            setSectors(array(value.sectors));
          })
        }
      >
        Find nearby sectors
      </Button>
      {sectors.length > 0 && (
        <>
          <Input
            type="search"
            aria-label="Filter nearby sectors"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          <Select
            aria-label="Nearby sectors"
            size={8}
            value={text(anchor.sector)}
            onChange={(e) => setAnchor({ ...anchor, sector: e.target.value })}
          >
            {sectors
              .filter((s) =>
                text(object(s).path)
                  .toLowerCase()
                  .includes(filter.toLowerCase()),
              )
              .slice(0, 100)
              .map((s) => (
                <option key={text(object(s).path)} value={text(object(s).path)}>
                  {text(object(s).path)}
                </option>
              ))}
          </Select>
        </>
      )}
      <Button
        onClick={() =>
          b.run(async () =>
            setSurvey(await api<JsonObject>("/api/survey", anchor)),
          )
        }
      >
        Survey sector objects
      </Button>
      {array(survey?.candidates).map((c, i) => (
        <Candidate
          key={i}
          value={object(c)}
          source={object(survey?.selection)}
        />
      ))}
      <details>
        <summary>Applied world changes</summary>
        <DraftEditor
          name="World changes"
          value={array(project.retrofit)}
          apply={(v) => b.set(["retrofit"], v)}
        />
      </details>
      {object(project.navigation).terrain_mask && (
        <DraftEditor
          name="Terrain navigation mask"
          value={object(project.navigation).terrain_mask!}
          apply={(v) => b.set(["navigation", "terrain_mask"], v)}
        />
      )}
    </div>
  );
}
function Candidate({
  value,
  source,
}: {
  value: JsonObject;
  source: JsonObject;
}) {
  const [shape, setShape] = useState("all"),
    [acknowledge, setAcknowledge] = useState(false);
  return (
    <article className="space-y-3 rounded border border-line p-3">
      <p className="break-all text-xs">
        {text(value.type)} · placement {text(value.index)}
        {value.instance !== undefined
          ? " · instance " + text(value.instance)
          : ""}
        {value.actor !== undefined ? " · actor " + text(value.actor) : ""}
      </p>
      {value.actor !== undefined && (
        <Select
          aria-label={"Collision selection for actor " + text(value.actor)}
          value={shape}
          onChange={(e) => setShape(e.target.value)}
        >
          <option value="all">Entire collision actor</option>
          {Array.from({ length: Number(value.shape_count || 0) }, (_, i) => (
            <option key={i} value={i}>
              Collision shape {i}
            </option>
          ))}
        </Select>
      )}
      {value.type === "worldInstancedOccluderNode" && (
        <label className="flex gap-2 text-xs">
          <input
            type="checkbox"
            checked={acknowledge}
            onChange={(e) => setAcknowledge(e.target.checked)}
          />
          Remove entire occluder group
        </label>
      )}
      <Button
        disabled={
          ![
            "worldMeshNode",
            "worldStaticMeshNode",
            "worldInstancedMeshNode",
          ].includes(text(value.type))
        }
        onClick={() =>
          b.run(async () =>
            b.accept(
              await api<Compiled>("/api/import-shell", {
                project: b.project,
                sector: source,
                selection: [
                  {
                    index: value.index,
                    instance: value.instance ?? null,
                    id: uid("reference"),
                    level: 0,
                  },
                ],
              }),
            ),
          )
        }
      >
        Import locked reference
      </Button>
      <Button
        onClick={() =>
          b.run(() =>
            b.transact((p) => {
              if (value.type === "worldInstancedOccluderNode" && !acknowledge)
                throw Error(
                  "Confirm removal of the entire occluder group first.",
                );
              const patches = array(p.retrofit);
              p.retrofit = patches;
              let patch = patches
                .map(object)
                .find((s) => s.path === source.path);
              if (!patch) {
                patch = structuredClone(source);
                patches.push(patch);
              }
              const deletions = array(patch.nodeDeletions);
              patch.nodeDeletions = deletions;
              let deletion = deletions
                .map(object)
                .find((d) => d.index === value.index);
              if (!deletion) {
                deletion = { index: value.index!, type: value.type! };
                deletions.push(deletion);
              }
              if (value.instance !== undefined) {
                deletion.expectedInstances = value.expectedInstances!;
                deletion.instanceDeletions = [
                  ...new Set([
                    ...array(deletion.instanceDeletions),
                    value.instance,
                  ]),
                ];
              }
              if (value.actor !== undefined) {
                deletion.expectedActors = value.expectedActors!;
                deletion.actorDeletions = [
                  ...array(deletion.actorDeletions),
                  shape === "all" ? value.actor : [value.actor, Number(shape)],
                ];
              }
              if (value.type === "worldInstancedOccluderNode") {
                deletion.expectedInstances = value.expectedInstances!;
                deletion.removeWholeGroup = true;
              }
              if (value.type === "worldNavigationNode") {
                if (value.resource_index === undefined)
                  throw Error(
                    "This node has no embedded Human navigation resource.",
                  );
                object(p.navigation).terrain_mask = {
                  sector: patch.path!,
                  sha256: patch.sha256!,
                  expected_nodes: patch.expectedNodes!,
                  placement_index: value.index!,
                  resource_index: value.resource_index,
                  outline: [
                    [0, 0],
                    [6, 0],
                    [6, 6],
                    [0, 6],
                  ],
                  height_range: [0, 4],
                };
              }
            }),
          )
        }
      >
        {value.type === "worldNavigationNode"
          ? "Create terrain navigation mask"
          : "Select for removal"}
      </Button>
    </article>
  );
}

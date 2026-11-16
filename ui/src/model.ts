export type Json = null | boolean | number | string | Json[] | JsonObject;
export interface JsonObject {
  [key: string]: Json | undefined;
}
export type Path = (string | number)[];
export const object = (v: Json | undefined): JsonObject =>
  v !== null && typeof v === "object" && !Array.isArray(v) ? v : {};
export const array = (v: Json | undefined): Json[] =>
  Array.isArray(v) ? v : [];
export const text = (v: Json | undefined, fallback = ""): string =>
  typeof v === "string" ? v : typeof v === "number" ? String(v) : fallback;
export const human = (v: string): string =>
  v.replaceAll("_", " ").replace(/^./, (c) => c.toUpperCase());
export const nameOf = (v: Json | undefined): string => {
  const o = object(v);
  return text(o.name || o.label || o.id || o.ref, "Entry");
};
export const at = (v: Json | undefined, path: Path): Json | undefined =>
  path.reduce<Json | undefined>(
    (v, k) => (Array.isArray(v) ? v[Number(k)] : object(v)[String(k)]),
    v,
  );
export function write(value: Json, path: Path, next: Json | undefined): void {
  const parent = at(value, path.slice(0, -1)),
    key = path.at(-1);
  if (!parent || typeof parent !== "object" || key === undefined)
    throw Error("The selected entry no longer exists.");
  if (Array.isArray(parent)) {
    if (next === undefined) parent.splice(Number(key), 1);
    else parent[Number(key)] = next;
  } else if (next === undefined) delete parent[String(key)];
  else
    Object.defineProperty(parent, String(key), {
      value: next,
      writable: true,
      enumerable: true,
      configurable: true,
    });
}
export const uid = (prefix: string): string =>
  prefix + "_" + crypto.randomUUID().slice(0, 8);
export interface Room extends JsonObject {
  id: string;
  x: number;
  y: number;
  width: number;
  depth: number;
  level: number;
}
export interface PlacedObject extends JsonObject {
  id: string;
  asset: string;
  appearance: string;
  position: number[];
  yaw: number;
  reference_only?: boolean;
  scale?: number[];
}
export interface Project extends JsonObject {
  id: string;
  name: string;
  revision: number;
  kit: string;
  origin: number[];
  yaw: number;
  wall_appearance: string;
  floor_appearance: string;
  rooms: Room[];
  objects: PlacedObject[];
  openings: Record<string, string>;
}
export interface Kit {
  id: string;
  name: string;
  storey_height: number;
  wall_height: number;
  wall_appearances: string[];
  floor_appearances: string[];
}
export interface Edge {
  id: string;
  axis: string;
  x: number;
  y: number;
  level: number;
  shared: boolean;
}
export interface Compiled {
  project: Project;
  placements: {
    id: string;
    role: string;
    position: number[];
    yaw: number;
    bounds: { min: number[]; max: number[]; dimensions: number[] };
    reference_only: boolean;
    level: number;
  }[];
  solids: { id: string; center: number[]; size: number[]; yaw: number }[];
  edges: Edge[];
  diagnostics: string[];
  reference_solids?: Json[];
  cooked_collision?: Json[];
}
export interface Bootstrap {
  starter: Project;
  kit: Kit;
  kits: Kit[];
  projects: string[];
  catalogue_count: number;
  library_source: string;
}
export interface LibraryModule {
  id: string;
  name: string;
  role: string;
  kind?: string;
  bounds?: { dimensions: number[] };
  appearances: string[];
  source_content_sha256?: string | null;
  previews?: Record<string, { path: string; view: string }[]>;
  states?: Record<
    string,
    { resolution: string; preview: string; diagnostics: string[] }
  >;
}
export interface PreviewOptions {
  browser_scene: boolean;
  size: number;
  cutaway: boolean;
  level: number | null;
  view: string;
  comparison: string;
  hidden_layers: string[];
  camera?: Json;
}
export function reconcileOpenings(project: Project): void {
  const cells = new Map<string, string>();
  for (const room of project.rooms)
    for (let x = room.x; x < room.x + room.width; x++)
      for (let y = room.y; y < room.y + room.depth; y++)
        cells.set(`${room.level}:${x}:${y}`, room.id);
  const edges = new Set<string>();
  for (const room of project.rooms)
    for (let x = room.x; x < room.x + room.width; x++)
      for (let y = room.y; y < room.y + room.depth; y++)
        for (const [axis, ex, ey, nx, ny] of [
          ["x", x, y, x, y - 1],
          ["y", x + 1, y, x + 1, y],
          ["x", x, y + 1, x, y + 1],
          ["y", x, y, x - 1, y],
        ] as const)
          if (cells.get(`${room.level}:${nx}:${ny}`) !== room.id)
            edges.add(`${room.level}:${axis}:${ex}:${ey}`);
  for (const id of Object.keys(project.openings))
    if (!edges.has(id)) {
      delete project.openings[id];
      delete object(project.door_states)[id];
    }
  for (const [id, device] of Object.entries(object(project.door_devices)))
    if (
      !object(device).position &&
      !["door_auto", "door_manual"].includes(project.openings[id] || "")
    )
      delete object(project.door_devices)[id];
}
export function newEntry(
  key: string,
  project: Project,
  level: number,
  height: number,
): Json {
  const id = uid(key.replace(/s$/, "")),
    z = level * height;
  const defaults: Record<string, Json> = {
    rooms: {
      id,
      x: project.rooms
        .filter((r) => r.level === level)
        .reduce((n, r) => Math.max(n, r.x + r.width), 0),
      y: 0,
      width: 2,
      depth: 2,
      level,
    },
    shell_rooms: {
      id,
      position: [0, 0, z],
      size: [4, 4, 3],
      yaw: 0,
      level,
      retained_sides: [],
      openings: {},
    },
    partitions: {
      id,
      from: [0, 1.5, z],
      to: [3, 1.5, z],
      height: 3,
      thickness: 0.12,
      level,
      openings: [],
    },
    stairs: {
      id,
      x: 0,
      y: 3,
      width: 2.6,
      run: 3,
      level,
      landing: 1.2,
      reverse: false,
      railings: true,
      axis: "y",
      type: "return",
      flight_width: 1.2,
    },
    lights: {
      id,
      position: [3, 3, z + 3],
      color: [255, 224, 190],
      radius: 4,
      intensity: 45,
    },
    audio_areas: {
      id,
      position: [3, 3, z],
      size: [6, 6, 3.8],
      preset: "residential-ground",
    },
    door_devices: {
      preset: "garage_w300_h250",
      position: [9, 0, z],
      yaw: 0,
      scale: [1, 1, 1],
      endpoints: [
        [9, -0.8, z],
        [9, 0.8, z],
      ],
      manual: false,
    },
    workstations: {
      id,
      desk: project.objects.filter((o) => !o.reference_only)[0]?.id || "",
      chair: project.objects.filter((o) => !o.reference_only)[1]?.id || "",
      actor: {
        record: "Character.WorkerSpecialistMale",
        appearance: "service__worker_ma_generic_worker_01",
        workspot:
          "base\\workspots\\common\\chair\\generic__sit_chair__sit_around__01.workspot",
        offset: [0, 0, 0],
        yaw_offset: 0,
        active_on_start: true,
      },
      computer: {
        offset: [0, 0, 0.8],
        yaw_offset: 0,
        title: "Office terminal",
        text: "",
      },
    },
    outline: [0, 0],
    connections: { id, from: [0, 0, 0], to: [0, 1, 0] },
    openings: { id, start: 0.5, width: 1, sill: 1, height: 1.2 },
    floors: {
      name: "Floor",
      level,
      position: [0, 0, z],
      size: [6, 6, height],
      yaw: 0,
    },
  };
  return defaults[key] || {};
}

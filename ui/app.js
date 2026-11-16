import { structuredEditor, pendingEditors, clearEditorDrafts } from "/forms.js";
import { installStudio } from "/studio.js";
import { installRetrofit } from "/retrofit.js";
import {mountViewport} from '@ghostline/ui-core/viewport';
const $ = (id) => document.getElementById(id);
const viewport=mountViewport(document.querySelector('.preview-wrap'),{label:'Building 3D preview',onError:error=>status(error.message,true),onLoad:()=>status('Interactive 3D ready')});
let studio;
let retrofit;
const extraControls = document.createElement("section");
extraControls.innerHTML = `<h2>Construction kit</h2><label>Measured kit<select id="kit"></select></label><h2>Stairs</h2><div id="stairs-list"></div><button id="add-stair">+ Add stair flight</button><h2>Technical layers</h2><label><input id="interior" type="checkbox"> Classify grid rooms as interiors</label><button id="add-light">+ Add practical light</button><button id="add-audio">+ Add ambience area</button><div id="technical-list"></div>`;
$("build-panel").append(extraControls);
$("opening").append(
  new Option("Automatic door", "door_auto"),
  new Option("Manual door", "door_manual"),
);
const stateLabel = document.createElement("label");
stateLabel.textContent = "Initial door state";
const doorState = document.createElement("select");
doorState.id = "door-state";
doorState.replaceChildren(
  ...["CLOSED", "OPENED", "LOCKED"].map((s) => new Option(s, s)),
);
stateLabel.append(doorState);
$("wall-controls").append(stateLabel);
let project,
  compiled,
  bootstrap,
  selection = null,
  wall = null,
  history = [],
  future = [],
  queue = Promise.resolve(),
  previewUrl = null,
  searchAbort;
let savedSnapshot = null,
  routeTriangles = [];
let metadataDraft = null;
let pendingObject = null;
$("plan").addEventListener(
  "click",
  (event) => {
    if (!pendingObject) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const point = $("plan").createSVGPoint();
    point.x = event.clientX;
    point.y = event.clientY;
    const screen = $("plan").getScreenCTM();
    if (!screen) return;
    const local = point.matrixTransform(screen.inverse()),
      placement = pendingObject;
    pendingObject = null;
    $("plan").style.cursor = "";
    edit((p) => {
      const id = "object_" + Date.now().toString(36);
      p.objects.push({
        id,
        asset: placement.asset,
        appearance: placement.appearance,
        position: [
          Math.round(local.x / Number($("snap-size").value)) *
            Number($("snap-size").value),
          Math.round(-local.y / Number($("snap-size").value)) *
            Number($("snap-size").value),
          Number($("level").value) * kitHeight(),
        ],
        level: Number($("level").value),
        anchor: "base_center",
        source_content_sha256: placement.pin,
        yaw: placement.yaw || 0,
      });
      selection = { kind: "object", id };
    });
  },
  true,
);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && pendingObject) {
    pendingObject = null;
    $("plan").style.cursor = "";
    status("Object placement cancelled");
    $("placement-banner").hidden = true;
    $("placement-ghost")?.remove();
  }
});
const navigationControls = document.createElement("section");
navigationControls.innerHTML = `<h2>Human navigation</h2><label><input id="native-navigation" type="checkbox"> Include native navigation in export</label><div id="navigation-fields" class="grid-fields"></div><details><summary>Check a route · local metres</summary><div id="route-fields" class="grid-fields"></div><button id="route-check">Check route</button><p id="route-result" class="muted"></p></details>`;
$("build-panel").append(navigationControls);
const routePoints = { start: [1.5, 1.5, 0], end: [1.5, 1.5, 4.2] };
const cameraControls = document.createElement("details");
cameraControls.innerHTML =
  '<summary>Interior camera</summary><label><input id="interior-camera" type="checkbox"> View from inside the building</label><div id="camera-fields" class="grid-fields"></div>';
document.querySelector(".inspector").append(cameraControls);
const cameraPoints = { eye: [1.5, 2, 1.7], target: [1.5, 14, 1.7] };
for (const kind of ["eye", "target"])
  for (let i = 0; i < 3; i++)
    $("camera-fields").append(
      field(
        `${kind} ${["X", "Y", "height"][i]}`,
        cameraPoints[kind][i],
        (v) => {
          cameraPoints[kind][i] = v;
        },
        { step: 0.1 },
      ),
    );
for (const kind of ["start", "end"])
  for (let i = 0; i < 3; i++)
    $("route-fields").append(
      field(
        `${kind} ${["X", "Y", "Z"][i]}`,
        routePoints[kind][i],
        (v) => {
          routePoints[kind][i] = v;
        },
        { step: 0.1 },
      ),
    );
$("native-navigation").onchange = () =>
  edit((p) => {
    p.navigation ??= {
      export: false,
      radius: 0.3,
      height: 1.8,
      climb: 0.6,
      layer: 200,
      connections: [],
    };
    p.navigation.export = $("native-navigation").checked;
  });
$("route-check").onclick = () =>
  action(async () => {
    const result = await (
      await api("/api/route", { project, ...routePoints })
    ).json();
    routeTriangles = result.triangles;
    $("route-result").textContent = result.connected
      ? `Connected · ${result.faces.length} polygons`
      : "No connected route";
    draw();
  });
const copy = (value) => structuredClone(value);
const escape = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
function status(text, error = false) {
  $("status").textContent = text;
  $("status").classList.toggle("error", error);
}
async function api(path, body, signal) {
  const response = await fetch(path, {
    method: body === undefined ? "GET" : "POST",
    headers: body === undefined ? {} : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal,
  });
  if (!response.ok) {
    let error = await response.text();
    try {
      error = JSON.parse(error).error || error;
    } catch {
      error ||= response.statusText;
    }
    throw Error(error);
  }
  return response;
}
function action(fn) {
  queue = queue.then(fn).catch((error) => {
    const activeEditor = document.activeElement?.closest(".structured-editor");
    if (project && compiled && !activeEditor) draw();
    status(error.message, true);
    if (activeEditor)
      activeEditor.querySelector(".form-error").textContent = error.message;
  });
  return queue;
}
async function edit(fn) {
  return action(async () => {
    const next = copy(project),
      oldSelection = copy(selection);
    try {
      fn(next);
      next.revision = project.revision + 1;
      const result = await (await api("/api/compile", next)).json();
      history.push(copy(project));
      future = [];
      project = result.project;
      compiled = result;
      routeTriangles = [];
      draw();
      status("Layout updated");
      studio?.schedulePreview();
    } catch (error) {
      selection = oldSelection;
      throw error;
    }
  });
}
function field(
  label,
  value,
  onchange,
  { step = 1, min, max, type = "number" } = {},
) {
  const node = document.createElement("label");
  node.textContent = label;
  const input = document.createElement("input");
  Object.assign(input, {
    type,
    value: type === "number" ? Number(Number(value).toFixed(4)) : value,
  });
  if (type === "number") {
    input.step = step;
    if (min !== undefined) input.min = min;
    if (max !== undefined) input.max = max;
  }
  input.onchange = () =>
    onchange(type === "number" ? Number(input.value) : input.value);
  node.append(input);
  return node;
}
function colliderCount() {
  return compiled.solids.length+(compiled.reference_solids||[]).filter(s=>s.id.startsWith('replacement_collision_')).length+(compiled.cooked_collision||[]).reduce((n,c)=>n+Math.ceil(c.geometry.triangles.length/15),0);
}
function draw() {
  drawFeatures();
  $("native-navigation").checked = !!project.navigation?.export;
  $("navigation-fields").replaceChildren(
    ...["radius", "height", "climb", "layer"].map((key) =>
      field(
        key,
        project.navigation?.[key] ??
          { radius: 0.3, height: 1.8, climb: 0.6, layer: 200 }[key],
        (value) =>
          edit((p) => {
            p.navigation ??= {
              export: false,
              radius: 0.3,
              height: 1.8,
              climb: 0.6,
              layer: 200,
              connections: [],
            };
            p.navigation[key] = value;
          }),
        { step: key === "layer" ? 1 : 0.05 },
      ),
    ),
  );
  $("project-title").textContent = project.name;
  $("name").value = metadataDraft?.name ?? project.name;
  $("identity").value = metadataDraft?.id ?? project.id;
  $("stats").textContent =
    `${compiled.placements.length} placements · ${colliderCount()} new colliders`;
  $("revision").textContent =
    `Revision ${project.revision}${savedSnapshot === JSON.stringify(project) ? " · saved" : " · unsaved"}`;
  $("undo").disabled = !history.length;
  $("redo").disabled = !future.length;
  $("wall-finish").value = project.wall_appearance;
  $("floor-finish").value = project.floor_appearance;
  $("rooms").replaceChildren(
    ...project.rooms.map((room) =>
      row(
        room.id,
        () => {
          selection = { kind: "room", id: room.id };
          draw();
        },
        () =>
          edit((p) => {
            p.rooms = p.rooms.filter((r) => r.id !== room.id);
            reconcileOpenings(p);
          }),
      ),
    ),
  );
  $("objects").replaceChildren(
    ...project.objects.map((object) =>
      row(
        object.id,
        () => {
          selection = { kind: "object", id: object.id };
          draw();
        },
        () =>
          edit((p) => {
            p.objects = p.objects.filter((o) => o.id !== object.id);
          }),
      ),
    ),
  );
  $("anchor").replaceChildren(
    ...["X", "Y", "Z"].map((axis, i) =>
      field(
        axis,
        project.origin[i],
        (v) =>
          edit((p) => {
            p.origin[i] = v;
          }),
        { step: 0.1 },
      ),
    ),
    field(
      "Yaw °",
      project.yaw,
      (v) =>
        edit((p) => {
          p.yaw = v;
        }),
      { step: 1 },
    ),
  );
  $("diagnostics").innerHTML = compiled.diagnostics
    .map((d) => `<p>${escape(d)}</p>`)
    .join("");
  $("layers").innerHTML = [
    ["Architecture", `${compiled.placements.length} placements`],
    ["Collision", `${colliderCount()} new shapes`],
    [
      "Door devices",
      Object.values(project.openings).filter((v) =>
        ["door_auto", "door_manual"].includes(v),
      ).length +
        Object.values(project.door_devices || {}).filter(
          (device) => device.position,
        ).length,
    ],
    ["Workstations", project.workstations?.length || 0],
    ["Interior areas", project.interior ? project.rooms.length : 0],
    ["Practical lights", project.lights?.length || 0],
    ["Ambience areas", project.audio_areas?.length || 0],
    [
      "Human navigation",
      project.navigation?.export
        ? `Layer ${project.navigation.layer}`
        : "Excluded from export",
    ],
  ]
    .map(
      ([a, b]) => `<div class="layer"><span>${a}</span><span>${b}</span></div>`,
    )
    .join("");
  const level = Number($("level").value),
    rooms = project.rooms.filter((r) => r.level === level),
    edges = compiled.edges.filter((e) => e.level === level);
  let lowX = 0,
    lowY = 0,
    highX = 9,
    highY = 9;
  for (const r of rooms) {
    lowX = Math.min(lowX, r.x * 3);
    lowY = Math.min(lowY, r.y * 3);
    highX = Math.max(highX, (r.x + r.width) * 3);
    highY = Math.max(highY, (r.y + r.depth) * 3);
  }
  const shellFootprints = (project.shell_rooms || [])
    .filter((r) => r.level === level)
    .map((room) => {
      const angle = (room.yaw * Math.PI) / 180,
        cos = Math.cos(angle),
        sin = Math.sin(angle);
      const points = [
        [0, 0],
        [room.size[0], 0],
        [room.size[0], room.size[1]],
        [0, room.size[1]],
      ].map(([x, y]) => [
        room.position[0] + x * cos - y * sin,
        room.position[1] + x * sin + y * cos,
      ]);
      return { room, points };
    });
  for (const point of [
    ...shellFootprints.flatMap((r) => r.points),
    ...project.objects
      .filter(
        (o) => (o.level ?? Math.floor(o.position[2] / kitHeight())) === level,
      )
      .map((o) => o.position),
  ]) {
    lowX = Math.min(lowX, point[0]);
    lowY = Math.min(lowY, point[1]);
    highX = Math.max(highX, point[0]);
    highY = Math.max(highY, point[1]);
  }
  const svg = $("plan");
  svg.setAttribute(
    "viewBox",
    `${lowX - 1} ${-highY - 1} ${highX - lowX + 2} ${highY - lowY + 2}`,
  );
  svg.innerHTML =
    `<defs><pattern id="grid" width="3" height="3" patternUnits="userSpaceOnUse"><path d="M 3 0 L 0 0 0 3" fill="none" stroke="#263a45" stroke-width=".025"/></pattern></defs><rect x="${lowX - 1}" y="${-highY - 1}" width="${highX - lowX + 2}" height="${highY - lowY + 2}" fill="url(#grid)"/>` +
    rooms
      .map(
        (r) =>
          `<g data-room="${escape(r.id)}"><rect class="room ${selection?.id === r.id ? "selected" : ""}" x="${r.x * 3}" y="${-(r.y + r.depth) * 3}" width="${r.width * 3}" height="${r.depth * 3}"/><text class="room-label" x="${(r.x + r.width / 2) * 3}" y="${-(r.y + r.depth / 2) * 3}">${escape(r.id)} · ${r.width * 3} × ${r.depth * 3} m</text></g>`,
      )
      .join("") +
    edges
      .map(
        (e) =>
          `<line data-wall="${escape(e.id)}" class="wall ${wall === e.id ? "selected" : ""} ${project.openings[e.id] ? "open" : ""}" x1="${e.x * 3}" y1="${-e.y * 3}" x2="${e.x * 3 + (e.axis === "x" ? 3 : 0)}" y2="${-e.y * 3 - (e.axis === "y" ? 3 : 0)}"/>`,
      )
      .join("") +
    project.objects
      .filter(
        (o) => (o.level ?? Math.floor(o.position[2] / kitHeight())) === level,
      )
      .map(
        (o) =>
          `<rect data-object="${escape(o.id)}" class="object" x="${o.position[0] - 0.2}" y="${-o.position[1] - 0.2}" width=".4" height=".4"/>`,
      )
      .join("");
  for (const { room, points } of shellFootprints) {
    const shape = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "polygon",
    );
    shape.setAttribute(
      "points",
      points.map(([x, y]) => `${x},${-y}`).join(" "),
    );
    shape.setAttribute("fill", "#253945");
    shape.setAttribute("stroke", "#89a6b5");
    shape.setAttribute("stroke-width", ".06");
    shape.onclick = () => {
      selection = { kind: "shell", id: room.id };
      draw();
      const detail = Array.from(
        $("construction-list").querySelectorAll("details"),
      ).find((d) => d.querySelector("summary")?.textContent === room.id);
      if (detail) {
        detail.open = true;
        detail.scrollIntoView({ block: "nearest" });
      }
    };
    shape.setAttribute("tabindex", "0");
    shape.setAttribute("role", "button");
    shape.setAttribute("aria-label", "Edit shell room " + room.id);
    shape.onkeydown = (e) => {
      if (e.key === "Enter") shape.onclick();
    };
    svg.prepend(shape);
    const label = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "text",
    );
    label.setAttribute("class", "room-label");
    label.setAttribute("x", points.reduce((n, p) => n + p[0], 0) / 4);
    label.setAttribute("y", -points.reduce((n, p) => n + p[1], 0) / 4);
    label.textContent = room.id.replaceAll("_", " ");
    svg.append(label);
  }
  svg.querySelectorAll("[data-room]").forEach(
    (e) =>
      (e.onclick = () => {
        selection = { kind: "room", id: e.dataset.room };
        draw();
      }),
  );
  svg.querySelectorAll("[data-wall]").forEach(
    (e) =>
      (e.onclick = () => {
        wall = e.dataset.wall;
        selection = { kind: "wall", id: wall };
        draw();
      }),
  );
  svg.querySelectorAll("[data-object]").forEach(
    (e) =>
      (e.onclick = () => {
        selection = { kind: "object", id: e.dataset.object };
        draw();
      }),
  );
  for (const triangle of routeTriangles) {
    if (Math.abs(triangle[0][2] - level * kitHeight()) > 0.1) continue;
    const polygon = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "polygon",
    );
    polygon.setAttribute(
      "points",
      triangle.map((p) => `${p[0]},${-p[1]}`).join(" "),
    );
    polygon.setAttribute("fill", "#c3f07070");
    polygon.setAttribute("stroke", "#c3f070");
    polygon.setAttribute("stroke-width", ".02");
    polygon.style.pointerEvents = "none";
    svg.append(polygon);
  }
  $("wall-controls").hidden = !wall;
  $("wall-hint").textContent = wall
    ? `Bay ${wall}`
    : "Select a wall in the plan to edit its opening.";
  $("opening").value = project.openings[wall] || "solid";
  const inspector = $("inspector");
  drawConstruction();
  inspector.replaceChildren();
  if (selection?.kind === "room") {
    const room = project.rooms.find((r) => r.id === selection.id);
    if (room) {
      inspector.append(
        field(
          "Room identity",
          room.id,
          (v) =>
            edit((p) => {
              p.rooms.find((r) => r.id === room.id).id = v;
              selection.id = v;
            }),
          { type: "text" },
        ),
      );
      for (const [key, label, min, max] of [
        ["x", "X · bays", -40, 40],
        ["y", "Y · bays", -40, 40],
        ["width", "Width · bays", 1, 20],
        ["depth", "Depth · bays", 1, 20],
        ["level", "Floor", 0, 3],
      ])
        inspector.append(
          field(
            label,
            room[key],
            (v) =>
              edit((p) => {
                p.rooms.find((r) => r.id === room.id)[key] = v;
              }),
            { min, max },
          ),
        );
    }
  } else if (selection?.kind === "object") {
    const object = project.objects.find((o) => o.id === selection.id);
    if (object) {
      inspector.append(
        field(
          "Object identity",
          object.id,
          (v) =>
            edit((p) => {
              p.objects.find((o) => o.id === object.id).id = v;
              selection.id = v;
            }),
          { type: "text" },
        ),
      );
      ["X", "Y", "Z"].forEach((axis, i) =>
        inspector.append(
          field(
            `${axis} · metres`,
            object.position[i],
            (v) =>
              edit((p) => {
                p.objects.find((o) => o.id === object.id).position[i] = v;
              }),
            { step: 0.05 },
          ),
        ),
      );
      inspector.append(
        field("Yaw °", object.yaw, (v) =>
          edit((p) => {
            p.objects.find((o) => o.id === object.id).yaw = v;
          }),
        ),
      );
      if (object.reference_only) {
        const note = document.createElement("p");
        note.className = "muted";
        note.textContent =
          "Locked surveyed shell reference · preview only · excluded from export";
        inspector.append(note);
        inspector.querySelectorAll("input").forEach((input) => {
          input.disabled = true;
        });
      } else {
        const settings = document.createElement("details"),
          heading = document.createElement("summary");
        heading.textContent = "Scale and native placement";
        settings.append(heading);
        ["X", "Y", "Z"].forEach((axis, i) =>
          settings.append(
            field(
              "Scale " + axis,
              (object.scale || [1, 1, 1])[i],
              (v) =>
                edit((p) => {
                  const o = p.objects.find((o) => o.id === object.id);
                  o.scale ||= [1, 1, 1];
                  o.scale[i] = v;
                }),
              { step: 0.05, min: 0.1, max: 10 },
            ),
          ),
        );
        const anchorLabel = document.createElement("label"),
          anchor = document.createElement("select");
        anchorLabel.textContent = "Placement anchor";
        anchor.replaceChildren(
          new Option("Native pivot", "pivot"),
          new Option("Base centre", "base_center"),
        );
        anchor.value = object.anchor || "pivot";
        anchor.onchange = () =>
          edit((p) => {
            p.objects.find((o) => o.id === object.id).anchor = anchor.value;
          });
        anchorLabel.append(anchor);
        settings.append(anchorLabel);
        const collisionLabel = document.createElement("label"),
          collision = document.createElement("input");
        collision.type = "checkbox";
        collision.checked = object.collision;
        collision.onchange = () =>
          edit((p) => {
            p.objects.find((o) => o.id === object.id).collision =
              collision.checked;
          });
        collisionLabel.append(
          collision,
          " Add solid box collision (structural meshes only)",
        );
        settings.append(collisionLabel);
        inspector.append(settings);
      }
    }
  } else
    inspector.innerHTML =
      '<p class="muted">Select a room or object to edit its dimensions and placement.</p>';
  studio?.draw();
  retrofit?.draw();
  if ($("world-changes")) drawWorldChanges();
}
function row(text, select, remove) {
  const row = document.createElement("div");
  row.className = "row";
  const button = document.createElement("button");
  button.textContent = text;
  button.onclick = select;
  const del = document.createElement("button");
  del.textContent = "×";
  del.className = "remove";
  del.setAttribute("aria-label", `Remove ${text}`);
  del.onclick = remove;
  row.append(button, del);
  return row;
}
document.querySelectorAll("[data-panel]").forEach(
  (button) =>
    (button.onclick = () => {
      document
        .querySelectorAll("[data-panel]")
        .forEach((b) => b.classList.toggle("active", b === button));
      for (const name of ["build", "library", "project","retrofit"])
        if($(name+"-panel"))$(name + "-panel").hidden = name !== button.dataset.panel;
    }),
);
$("add-room").onclick = () =>
  edit((p) => {
    const level = Number($("level").value);
    let x = p.rooms
      .filter((r) => r.level === level)
      .reduce((n, r) => Math.max(n, r.x + r.width), 0);
    const id = `room_${Date.now().toString(36)}`;
    p.rooms.push({ id, x, y: 0, width: 2, depth: 2, level });
    selection = { kind: "room", id };
  });
$("opening").onchange = () =>
  edit((p) => {
    const value = $("opening").value;
    if (value === "solid") delete p.openings[wall];
    else p.openings[wall] = value;
    if (!["door_auto", "door_manual"].includes(value) && p.door_states)
      delete p.door_states[wall];
    if (!["door_auto", "door_manual"].includes(value) && p.door_devices)
      delete p.door_devices[wall];
  });
$("door-state").onchange = () =>
  edit((p) => {
    p.door_states ??= {};
    p.door_states[wall] = $("door-state").value;
  });
$("kit").onchange = () =>
  edit((p) => {
    const kit = bootstrap.kits.find((k) => k.id === $("kit").value);
    p.kit = kit.id;
    p.wall_appearance = kit.wall_appearances[0];
    p.floor_appearance = kit.floor_appearances[0];
  });
$("interior").onchange = () =>
  edit((p) => {
    p.interior = $("interior").checked;
  });
$("add-stair").onclick = () =>
  edit((p) => {
    p.stairs ??= [];
    p.stairs.push({
      id: "stairs_" + Date.now().toString(36),
      x: 0,
      y: 3,
      width: 3,
      run: 9,
      level: Number($("level").value),
      landing: 3,
      reverse: false,
      railings: true,
      axis: "y",
      type: "straight",
      flight_width: 1.2,
    });
  });
$("add-light").onclick = () =>
  edit((p) => {
    p.lights ??= [];
    p.lights.push({
      id: "light_" + Date.now().toString(36),
      position: [3, 3, Number($("level").value) * kitHeight() + 3],
      color: [255, 224, 190],
      radius: 4,
      intensity: 45,
    });
  });
$("add-audio").onclick = () =>
  edit((p) => {
    p.audio_areas ??= [];
    p.audio_areas.push({
      id: "audio_" + Date.now().toString(36),
      position: [3, 3, Number($("level").value) * kitHeight()],
      size: [6, 6, 3.8],
      preset: "residential-ground",
    });
  });
function kitHeight() {
  return bootstrap.kits.find((k) => k.id === project.kit)?.storey_height || 4.2;
}
function drawFeatures() {
  const kit = bootstrap.kits.find((k) => k.id === project.kit);
  $("kit").replaceChildren(
    ...bootstrap.kits.map((k) => new Option(k.name, k.id)),
  );
  $("kit").value = project.kit;
  for (const [id, values] of [
    ["wall-finish", kit.wall_appearances],
    ["floor-finish", kit.floor_appearances],
  ])
    $(id).replaceChildren(...values.map((v) => new Option(v, v)));
  $("interior").checked = !!project.interior;
  stateLabel.hidden = !["door_auto", "door_manual"].includes(
    project.openings[wall],
  );
  $("door-state").value = project.door_states?.[wall] || "CLOSED";
  $("stairs-list").replaceChildren(
    ...(project.stairs || []).map((s) => {
      const card = document.createElement("div");
      card.className = "card";
      card.append(
        row(
          s.id,
          () => {},
          () =>
            edit((p) => {
              p.stairs = p.stairs.filter((v) => v.id !== s.id);
            }),
        ),
      );
      for (const key of ["x", "y", "width", "run", "level", "landing"])
        card.append(
          field(
            key,
            s[key],
            (v) =>
              edit((p) => {
                p.stairs.find((v) => v.id === s.id)[key] = v;
              }),
            { step: key === "level" ? 1 : 0.05 },
          ),
        );
      const type = document.createElement("select");
      type.setAttribute("aria-label", "Stair type");
      type.replaceChildren(
        new Option("Straight flight", "straight"),
        new Option("Return flight", "return"),
      );
      type.value = s.type;
      type.onchange = () =>
        edit((p) => {
          p.stairs.find((v) => v.id === s.id).type = type.value;
        });
      card.append(type);
      return card;
    }),
  );
  $("technical-list").replaceChildren(
    ...["lights", "audio_areas"].flatMap((kind) =>
      (project[kind] || []).map((item) => {
        const card = document.createElement("div");
        card.className = "card";
        card.append(
          row(
            item.id,
            () => {},
            () =>
              edit((p) => {
                p[kind] = p[kind].filter((v) => v.id !== item.id);
              }),
          ),
        );
        ["X", "Y", "Z"].forEach((axis, i) =>
          card.append(
            field(
              axis,
              item.position[i],
              (v) =>
                edit((p) => {
                  p[kind].find((v) => v.id === item.id).position[i] = v;
                }),
              { step: 0.1 },
            ),
          ),
        );
        if (kind === "lights")
          for (const key of ["radius", "intensity"])
            card.append(
              field(
                key,
                item[key],
                (v) =>
                  edit((p) => {
                    p[kind].find((v) => v.id === item.id)[key] = v;
                  }),
                { step: 0.1 },
              ),
            );
        return card;
      }),
    ),
  );
}
$("level").onchange = draw;
$("wall-finish").onchange = () =>
  edit((p) => {
    p.wall_appearance = $("wall-finish").value;
  });
$("floor-finish").onchange = () =>
  edit((p) => {
    p.floor_appearance = $("floor-finish").value;
  });
for (const id of ["name", "identity"])
  $(id).oninput = () => {
    metadataDraft = { name: $("name").value, id: $("identity").value };
    status("Project details changed · Save or Apply project details to commit");
  };
const applyProject = document.createElement("button");
applyProject.textContent = "Apply project details";
applyProject.onclick = () => action(applyMetadata);
$("identity").parentElement.after(applyProject);
async function applyMetadata() {
  if (!metadataDraft) return;
  if (metadataDraft.id === project.id && metadataDraft.name === project.name) {
    metadataDraft = null;
    return;
  }
  const next = {
    ...copy(project),
    ...metadataDraft,
    revision: project.revision + 1,
  };
  const result = await (await api("/api/compile", next)).json();
  history.push(copy(project));
  future = [];
  project = result.project;
  compiled = result;
  metadataDraft = null;
  draw();
  status("Project details applied");
}
async function restore(from, to) {
  if (!from.length) return;
  const next = from.at(-1);
  const result = await (await api("/api/compile", next)).json();
  to.push(project);
  from.pop();
  project = result.project;
  compiled = result;
  draw();
  status("History restored · preview needs refresh");
  studio?.schedulePreview();
}
$("undo").onclick = () => action(() => restore(history, future));
$("redo").onclick = () => action(() => restore(future, history));
$("save").onclick = () =>
  action(async () => {
    if (pendingEditors().length)
      throw Error(
        "Apply or discard pending settings first: " +
          pendingEditors().join(", "),
      );
    await applyMetadata();
    await api("/api/save", project);
    savedSnapshot = JSON.stringify(project);
    await saved();
    draw();
    status("Project saved");
  });
async function saved() {
  const data = await (await api("/api/bootstrap")).json();
  const examples = document.createElement("optgroup");
  examples.label = "Examples";
  examples.replaceChildren(
    ...[
      ["workshop", "Workshop"],
      ["stairs-and-devices", "Stairs, doors and navigation"],
      ["motel-modules", "Original motel module lab"],
    ].map(([id, name]) => new Option(name, "example:" + id)),
  );
  const projects = document.createElement("optgroup");
  projects.label = "Saved projects";
  projects.replaceChildren(...data.projects.map((id) => new Option(id, id)));
  $("saved").replaceChildren(
    new Option("Choose a saved project", ""),
    examples,
    projects,
  );
  if ([...$("saved").options].some((option) => option.value === project.id))
    $("saved").value = project.id;
}
async function load(next, isSaved = false) {
  const result = await (await api("/api/compile", next)).json();
  history.push(project);
  future = [];
  project = result.project;
  clearEditorDrafts();
  metadataDraft = null;
  compiled = result;
  selection = null;
  wall = null;
  savedSnapshot = isSaved ? JSON.stringify(project) : null;
  draw();
  status("Project opened · preview needs refresh");
  studio?.resetCamera();
  studio?.schedulePreview();
}
$("open").onclick = () =>
  action(async () => {
    const id = $("saved").value;
    if (!id) return;
    const main = document.querySelector("main");
    main.inert = true;
    main.setAttribute("aria-busy", "true");
    $("save").disabled = true;
    $("export").disabled = true;
    status("Opening project…");
    try {
      await load(
        await (
          await api(
            id.startsWith("example:")
              ? "/api/example?id=" + encodeURIComponent(id.slice(8))
              : "/api/open?id=" + encodeURIComponent(id),
          )
        ).json(),
        !id.startsWith("example:"),
      );
    } finally {
      main.inert = false;
      main.removeAttribute("aria-busy");
      $("save").disabled = false;
      $("export").disabled = false;
    }
  });
$("new").onclick = () =>
  action(async () => {
    if (
      !confirm(
        "Start a new building? Save or download the current project first.",
      )
    )
      return;
    const next = copy(bootstrap.starter);
    next.id = "building_" + Date.now().toString(36);
    next.name = "New building";
    next.rooms = [];
    next.openings = {};
    await load(next);
  });
$("import").onchange = () =>
  action(async () => {
    const file = $("import").files[0];
    if (file) await load(JSON.parse(await file.text()));
  });
$("download").onclick = () => {
  const url = URL.createObjectURL(
    new Blob([JSON.stringify(project, null, 2)], { type: "application/json" }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = project.id + ".building.json";
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
};
$("render").onclick = async () => {
  const button = $("render");
  if (button.disabled) return;
  const renderedProject = copy(project);
  button.disabled = true;
  status("Preparing interactive scene…");
  try {
    const response = await api("/api/preview", {
      project: renderedProject,
      options: {
        browser_scene:true,
        ...retrofit?.options(),
        size: 1024,
        cutaway: !$("interior-camera").checked && $("cutaway").checked,
        level: retrofit?.allFloors()?null:Number($("level").value),
        view: $("view").value,
        camera: $("interior-camera").checked
          ? {
              perspective: {
                eye: [
                  cameraPoints.eye[0],
                  cameraPoints.eye[2] + Number($("level").value) * kitHeight(),
                  -cameraPoints.eye[1],
                ],
                target: [
                  cameraPoints.target[0],
                  cameraPoints.target[2] +
                    Number($("level").value) * kitHeight(),
                  -cameraPoints.target[1],
                ],
                up: [0, 1, 0],
                vertical_fov: 1.15,
                near: 0.05,
                far: 200,
              },
            }
          : studio?.camera() || null,
      },
    });
    if (JSON.stringify(project) !== JSON.stringify(renderedProject)) {
      studio?.schedulePreview();
      return;
    }
    if (previewUrl) URL.revokeObjectURL(previewUrl);
    previewUrl = URL.createObjectURL(await response.blob());
    viewport.load(previewUrl);
    $("preview").hidden = true;
    $("preview-empty").hidden = true;
    status(`Interactive scene ready · revision ${project.revision}`);
    studio?.previewReady();
  } catch (error) {
    status(error.message, true);
  } finally {
    button.disabled = false;
  }
};
let libraryOffset = 0;
async function search(offset = 0) {
  libraryOffset = typeof offset === "number" ? offset : 0;
  searchAbort?.abort();
  searchAbort = new AbortController();
  try {
    const data = await (
      await api(
        "/api/library?q=" +
          encodeURIComponent($("search").value) +
          "&role=" +
          encodeURIComponent($("role").value) +
          "&offset=" +
          libraryOffset +
          "&kind=" +
          encodeURIComponent($("assembly-filter")?.value || ""),
        undefined,
        searchAbort.signal,
      )
    ).json();
    $("library-count").textContent =
      `${data.total.toLocaleString()} matches · ${Math.min(libraryOffset + 1, data.total)}–${Math.min(libraryOffset + 60, data.total)}`;
    $("library-previous").disabled = libraryOffset === 0;
    $("library-next").disabled = libraryOffset + 60 >= data.total;
    $("library-results").replaceChildren(
      ...data.entries.map((module) => {
        const card = document.createElement("div");
        card.className = "card";
        studio?.remember(module);
        const title = document.createElement("strong");
        title.textContent = module.name.replaceAll("_", " ");
        const info = document.createElement("small");
        info.textContent = `${module.bounds ? module.bounds.dimensions.map((v) => v.toFixed(2)).join(" × ") + " m · " : ""}${module.role}${module.kind ? " · " + ({ assembly: "Complete assembly", part: "Separate part", mesh: "Mesh" }[module.kind] || module.kind) : ""}`;
        const appearance = document.createElement("select");
        appearance.setAttribute("aria-label", `Appearance for ${module.name}`);
        appearance.replaceChildren(
          ...module.appearances.map((a) => new Option(a || "Default", a)),
        );
        const add = document.createElement("button");
        add.textContent = "+ Place on plan";
        add.onclick = () => {
          pendingObject = {
            asset: module.id,
            appearance: appearance.value,
            pin: module.source_content_sha256,
            name: module.name,
            yaw: 0,
            bounds: module.bounds,
          };
          studio?.placementStarted();
          $("plan").style.cursor = "crosshair";
          status(
            "Click the plan to place " +
              module.name.replaceAll("_", " ") +
              " · Escape cancels",
          );
        };
        const inspect = document.createElement("button");
        inspect.textContent = "Preview module";
        const moduleView = document.createElement("select");
        moduleView.setAttribute(
          "aria-label",
          "Preview view for " + module.name,
        );
        moduleView.replaceChildren(
          ...["hero", "front", "back", "left", "right"].map(
            (view) => new Option(view, view),
          ),
        );
        let moduleUrl = null;
        const image = document.createElement("img");
        image.alt = "Preview of " + module.name;
        image.style.width = "100%";
        image.loading = "lazy";
        image.onclick = () => studio?.enlarge(image.src, module.name);
        image.tabIndex = 0;
        image.setAttribute("role", "button");
        image.setAttribute("aria-label", "Enlarge preview of " + module.name);
        image.onkeydown = (e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            image.click();
          }
        };
        image.hidden = true;
        const availability = document.createElement("small");
        image.onerror = () => {
          image.hidden = true;
          availability.textContent =
            "Published image is missing; native preview available";
        };
        const updatePublishedPreview = () => {
          const media = module.previews?.[appearance.value] || [];
          const preview =
            media.find((m) => m.view === moduleView.value) ||
            media.find((m) => m.view === "hero") ||
            media[0];
          image.hidden = !preview;
          if (preview)
            image.src =
              "/api/catalogue-media?path=" + encodeURIComponent(preview.path);
          else image.removeAttribute("src");
          const state = module.states?.[appearance.value];
          const blocked =
            state &&
            (state.resolution !== "resolved" ||
              state.preview === "not_applicable");
          add.disabled = !!blocked || !module.appearances.length;
          inspect.disabled = add.disabled;
          availability.textContent = blocked
            ? state.diagnostics.join(" · ") || "No visible placeable geometry"
            : state?.preview === "failed"
              ? "Catalogue preview failed; native preview available"
              : module.previews && !preview
                ? "Catalogue preview not rendered yet"
                : "";
        };
        appearance.onchange = updatePublishedPreview;
        moduleView.onchange = updatePublishedPreview;
        updatePublishedPreview();
        inspect.onclick = async () => {
          inspect.disabled = true;
          try {
            const isolated = copy(project);
            isolated.rooms = [];
            isolated.openings = {};
            isolated.door_states = {};
            isolated.stairs = [];
            isolated.shell_rooms = [];
            isolated.partitions = [];
            isolated.door_devices = {};
            isolated.workstations = [];
            isolated.lights = [];
            isolated.audio_areas = [];
            isolated.retrofit = [];
            isolated.navigation.terrain_mask = null;
            isolated.navigation.connections = [];
            isolated.objects = [
              {
                id: "module_preview",
                asset: module.id,
                appearance: appearance.value,
                position: [0, 0, 0],
                yaw: 0,
                anchor: "base_center",
              },
            ];
            const measured = await (await api("/api/compile", isolated)).json();
            const bounds = measured.placements[0]?.bounds;
            if (bounds) {
              module.bounds = bounds;
              info.textContent =
                bounds.dimensions.map((v) => v.toFixed(2)).join(" × ") +
                " m · " +
                module.role +
                " · " +
                module.kind;
            }
            const response = await api("/api/preview", {
              project: isolated,
              options: {
                size: 512,
                cutaway: false,
                level: null,
                view: "hero",
                camera: { frame_projected: moduleView.value },
              },
            });
            if (moduleUrl) URL.revokeObjectURL(moduleUrl);
            moduleUrl = URL.createObjectURL(await response.blob());
            image.src = moduleUrl;
            image.hidden = false;
            status("Module preview ready");
          } catch (error) {
            status(error.message, true);
          } finally {
            inspect.disabled = false;
          }
        };
        card.append(
          title,
          info,
          appearance,
          moduleView,
          inspect,
          image,
          availability,
          add,
        );
        return card;
      }),
    );
  } catch (error) {
    if (error.name !== "AbortError") status(error.message, true);
  }
}
$("library-previous").onclick = () => search(Math.max(0, libraryOffset - 60));
$("library-next").onclick = () => search(libraryOffset + 60);
let searchTimer;
$("search").oninput = () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(search, 200);
};
$("role").onchange = search;
let exporting = false;
$("close-export").onclick = () => {
  if (!exporting) $("export-dialog").close();
};
$("export-dialog").addEventListener("cancel", (e) => {
  if (exporting) e.preventDefault();
});
$("export").onclick = () =>
  action(async () => {
    if (pendingEditors().length)
      throw Error(
        "Apply or discard pending settings first: " +
          pendingEditors().join(", "),
      );
    await applyMetadata();
    exporting = true;
    $("close-export").disabled = true;
    $("export-dialog").showModal();
    $("export-result").textContent =
      "Writing native resources, checking readback and packing archive…";
    try {
      const result = await (await api("/api/export", project)).json();
      $("export-result").replaceChildren();
      const receipt = document.createElement("pre");
      receipt.textContent = `${result.meshes} meshes · ${result.entities || 0} entities · ${result.colliders} colliders\n${result.verified_resources} native resources verified\nSHA-256 ${result.archive_sha256}\n\n${result.diagnostics.join("\n")}`;
      $("export-result").append(receipt);
      for (const file of result.files || [
        project.id + ".zip",
        project.id + ".archive",
        project.id + ".archive.xl",
        project.id + ".building.json",
        "export-report.json",
        "bundle-receipt.json",
      ]) {
        const a = document.createElement("a");
        a.href =
          "/api/download?id=" +
          encodeURIComponent(result.export_id) +
          "&file=" +
          encodeURIComponent(file);
        a.textContent = "Download " + file;
        $("export-result").append(a);
      }
      status("Native export verified");
      studio?.exportResult(result);
    } catch (error) {
      $("export-result").textContent = error.message;
      throw error;
    } finally {
      exporting = false;
      $("close-export").disabled = false;
    }
  });
document.addEventListener("keydown", (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key === "s") {
    e.preventDefault();
    $("save").click();
  }
  if (
    (e.ctrlKey || e.metaKey) &&
    e.key === "z" &&
    !["INPUT", "TEXTAREA"].includes(document.activeElement.tagName)
  ) {
    e.preventDefault();
    $(e.shiftKey ? "redo" : "undo").click();
  }
});
action(async () => {
  bootstrap = await (await api("/api/bootstrap")).json();
  project = copy(bootstrap.starter);
  compiled = await (await api("/api/compile", project)).json();
  project = compiled.project;
  for (const [id, values] of [
    ["wall-finish", bootstrap.kit.wall_appearances],
    ["floor-finish", bootstrap.kit.floor_appearances],
  ])
    $(id).replaceChildren(...values.map((v) => new Option(v, v)));
  $("coverage").textContent =
    `${(bootstrap.catalogue_count || 0).toLocaleString()} catalogue objects. ${bootstrap.library_source}.`;
  await saved();
  draw();
  await search();
  status("Ready · starter anchor is an isolated test position");
  studio?.schedulePreview();
});

function reconcileOpenings(p) {
  const cells = new Map();
  for (const room of p.rooms)
    for (let x = room.x; x < room.x + room.width; x++)
      for (let y = room.y; y < room.y + room.depth; y++)
        cells.set(`${room.level}:${x}:${y}`, room.id);
  const edges = new Set();
  for (const room of p.rooms)
    for (let x = room.x; x < room.x + room.width; x++)
      for (let y = room.y; y < room.y + room.depth; y++)
        for (const [axis, ex, ey, nx, ny] of [
          ["x", x, y, x, y - 1],
          ["y", x + 1, y, x + 1, y],
          ["x", x, y + 1, x, y + 1],
          ["y", x, y, x - 1, y],
        ])
          if (cells.get(`${room.level}:${nx}:${ny}`) !== room.id)
            edges.add(`${room.level}:${axis}:${ex}:${ey}`);
  for (const id of Object.keys(p.openings))
    if (!edges.has(id)) {
      delete p.openings[id];
      if (p.door_states) delete p.door_states[id];
    }
  for (const id of Object.keys(p.door_devices || {}))
    if (
      !p.door_devices[id].position &&
      !["door_auto", "door_manual"].includes(p.openings[id])
    )
      delete p.door_devices[id];
}

// Construction tools share the same validated transaction/history as numeric edits.
const construction = document.createElement("section");
construction.innerHTML = `<h2>Plan tools</h2><label>Tool<select id="plan-tool"><option value="select">Select</option><option value="draw">Draw room</option><option value="move">Move room</option><option value="resize">Resize room</option><option value="partition">Draw partition</option></select></label><p class="muted">Drag on the plan. Rooms snap to 3 m bays; partitions snap to 5 cm. Escape cancels.</p><h2>Partitions and shell rooms</h2><button id="add-partition">+ Partition / window wall</button><button id="add-shell-room">+ Measured shell room</button><div id="construction-list"></div><h2>Shell doors</h2><button id="add-shell-door">+ Door in measured aperture</button><div id="shell-door-list"></div><h2>Workstations</h2><button id="add-workstation">+ Bind desk and chair</button><div id="workstation-list"></div>`;
$("build-panel").append(construction);
const presets = [
  "sliding_entropy_c",
  "hinged_h",
  "hinged_e",
  "sliding_kitsch_double_f",
  "garage_w300_h250",
];
const presetLabel = document.createElement("label");
presetLabel.textContent = "Door preset";
const presetSelect = document.createElement("select");
presetSelect.id = "door-preset";
presetSelect.replaceChildren(
  ...presets.slice(0, 3).map((p) => new Option(p.replaceAll("_", " "), p)),
);
presetLabel.append(presetSelect);
$("wall-controls").append(presetLabel);
presetSelect.onchange = () =>
  edit((p) => {
    p.door_devices ||= {};
    p.door_devices[wall] = {
      ...p.door_devices[wall],
      preset: presetSelect.value,
    };
  });
const uniqueId = (prefix) => prefix + "_" + Date.now().toString(36);
function planPoint(event) {
  const point = $("plan").createSVGPoint();
  point.x = event.clientX;
  point.y = event.clientY;
  const matrix = $("plan").getScreenCTM();
  if (!matrix) return null;
  const p = point.matrixTransform(matrix.inverse());
  return [p.x, -p.y];
}
let planDrag = null,
  suppressPlanClick = false;
$("plan").style.touchAction = "none";
$("plan").addEventListener(
  "pointerdown",
  (event) => {
    const tool = $("plan-tool").value;
    if (tool === "select" || tool.startsWith("object-") || pendingObject)
      return;
    const point = planPoint(event);
    if (!point) return;
    const roomNode = event.target.closest("[data-room]");
    const room =
      roomNode && project.rooms.find((r) => r.id === roomNode.dataset.room);
    if (["move", "resize"].includes(tool) && !room) {
      status("Drag a room to " + tool);
      return;
    }
    planDrag = {
      tool,
      start: point,
      current: point,
      room: room && copy(room),
      pointer: event.pointerId,
    };
    $("plan").setPointerCapture(event.pointerId);
    event.preventDefault();
    event.stopPropagation();
  },
  true,
);
$("plan").addEventListener("pointermove", (event) => {
  if (!planDrag) return;
  planDrag.current = planPoint(event) || planDrag.current;
  let ghost = $("plan-ghost");
  if (!ghost) {
    ghost = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    ghost.id = "plan-ghost";
    ghost.setAttribute("fill", "#c3f07033");
    ghost.setAttribute("stroke", "#c3f070");
    ghost.setAttribute("stroke-width", ".06");
    ghost.style.pointerEvents = "none";
    $("plan").append(ghost);
  }
  const a = planDrag.start,
    b = planDrag.current;
  ghost.setAttribute("x", Math.min(a[0], b[0]));
  ghost.setAttribute("y", -Math.max(a[1], b[1]));
  ghost.setAttribute("width", Math.max(0.05, Math.abs(a[0] - b[0])));
  ghost.setAttribute("height", Math.max(0.05, Math.abs(a[1] - b[1])));
});
$("plan").addEventListener(
  "pointerup",
  (event) => {
    if (!planDrag) return;
    const drag = planDrag;
    planDrag = null;
    $("plan-ghost")?.remove();
    suppressPlanClick = true;
    if ($("plan").hasPointerCapture(event.pointerId))
      $("plan").releasePointerCapture(event.pointerId);
    const a = drag.start,
      b = planPoint(event) || drag.current,
      level = Number($("level").value);
    edit((p) => {
      if (drag.tool === "draw") {
        const x = Math.floor(Math.min(a[0], b[0]) / 3),
          y = Math.floor(Math.min(a[1], b[1]) / 3),
          right = Math.max(x + 1, Math.ceil(Math.max(a[0], b[0]) / 3)),
          top = Math.max(y + 1, Math.ceil(Math.max(a[1], b[1]) / 3));
        const id = uniqueId("room");
        p.rooms.push({ id, x, y, width: right - x, depth: top - y, level });
        selection = { kind: "room", id };
      } else if (drag.tool === "partition") {
        p.partitions ||= [];
        p.partitions.push({
          id: uniqueId("partition"),
          from: [...a.map((v) => Math.round(v * 20) / 20), level * kitHeight()],
          to: [...b.map((v) => Math.round(v * 20) / 20), level * kitHeight()],
          height: bootstrap.kits.find((k) => k.id === p.kit).wall_height,
          thickness: 0.12,
          openings: [],
          level,
        });
      } else {
        const room = p.rooms.find((r) => r.id === drag.room.id);
        if (drag.tool === "move") {
          room.x = drag.room.x + Math.round((b[0] - a[0]) / 3);
          room.y = drag.room.y + Math.round((b[1] - a[1]) / 3);
        } else {
          room.width = Math.max(1, Math.round(b[0] / 3) - room.x);
          room.depth = Math.max(1, Math.round(b[1] / 3) - room.y);
        }
        reconcileOpenings(p);
        selection = { kind: "room", id: room.id };
      }
    });
  },
  true,
);
$("plan").addEventListener(
  "click",
  (event) => {
    if (suppressPlanClick) {
      event.stopImmediatePropagation();
      event.preventDefault();
      suppressPlanClick = false;
    }
  },
  true,
);
$("plan").addEventListener("pointercancel", () => {
  planDrag = null;
  $("plan-ghost")?.remove();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    planDrag = null;
    $("plan-ghost")?.remove();
    $("plan-tool").value = "select";
  }
});
$("add-partition").onclick = () =>
  edit((p) => {
    const level = Number($("level").value);
    const room =
      p.rooms.find((r) => r.id === selection?.id && r.level === level) ||
      p.rooms.find((r) => r.level === level);
    const id = uniqueId("partition");
    const y = room
      ? ([0.5, 0.25, 0.75]
          .map((t) => (room.y + room.depth * t) * 3)
          .find(
            (y) =>
              !(p.partitions || []).some(
                (w) => w.level === level && Math.abs(w.from[1] - y) < 0.1,
              ),
          ) ?? (room.y + room.depth * 0.5) * 3)
      : 1.5;
    p.partitions ||= [];
    p.partitions.push({
      id,
      from: [room ? room.x * 3 : 0, y, level * kitHeight()],
      to: [room ? (room.x + room.width) * 3 : 3, y, level * kitHeight()],
      height: 3,
      thickness: 0.12,
      level,
      openings: [],
    });
    selection = { kind: "partition", id };
  });
$("add-shell-room").onclick = () =>
  edit((p) => {
    p.shell_rooms ||= [];
    const id = uniqueId("shell"),
      x =
        Math.max(
          0,
          ...p.rooms.map((r) => (r.x + r.width) * 3),
          ...p.shell_rooms.map((r) => r.position[0] + r.size[0]),
        ) + 3;
    p.shell_rooms.push({
      id,
      position: [x, 0, Number($("level").value) * kitHeight()],
      size: [4, 4, 3],
      yaw: 0,
      level: Number($("level").value),
      retained_sides: [],
      openings: {},
    });
    selection = { kind: "shell", id };
  });
$("add-shell-door").onclick = () =>
  edit((p) => {
    const id = uniqueId("door"),
      z = Number($("level").value) * kitHeight();
    p.door_devices ||= {};
    p.door_devices[id] = {
      preset: "garage_w300_h250",
      position: [9, 0, z],
      yaw: 0,
      scale: [1, 1, 1],
      endpoints: [
        [9, -0.8, z],
        [9, 0.8, z],
      ],
      manual: false,
    };
  });
$("add-workstation").onclick = () => {
  if (project.objects.filter((o) => !o.reference_only).length < 2) {
    status("Place a desk and chair from the catalogue first", true);
    return;
  }
  const objects = project.objects.filter((o) => !o.reference_only);
  return edit((p) => {
    p.workstations ||= [];
    p.workstations.push({
      id: uniqueId("office"),
      desk: objects[0].id,
      chair: objects[1].id,
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
    });
  });
};
function settingsEditor(title, value, onapply, onremove) {
  return structuredEditor(title, value, onapply, onremove, {
    actorAppearance: (record) => studio?.actorAppearance(record),
    choices: {
      desk: project.objects.filter((o) => !o.reference_only).map((o) => o.id),
      chair: project.objects.filter((o) => !o.reference_only).map((o) => o.id),
    },
    lists: {
      record: "actor-records",
      workspot: "seat-workspots",
      appearance: "actor-appearances",
    },
  });
}
function drawConstruction() {
  if (!$("construction-list")) return;
  $("door-preset").value =
    project.door_devices?.[wall]?.preset || "sliding_entropy_c";
  $("door-preset").disabled = !["door_auto", "door_manual"].includes(
    project.openings[wall],
  );
  $("construction-list").replaceChildren(
    ...["partitions", "shell_rooms"].flatMap((key) =>
      (project[key] || []).map((item, index) =>
        row(
          item.id,
          () => {
            selection = {
              kind: key === "shell_rooms" ? "shell" : "partition",
              id: item.id,
            };
            draw();
          },
          () =>
            edit((p) => {
              p[key].splice(index, 1);
            }),
        ),
      ),
    ),
  );
  $("shell-door-list").replaceChildren(
    ...Object.entries(project.door_devices || {})
      .filter(([, d]) => d.position)
      .map(([id, item]) =>
        settingsEditor(
          id,
          { ...item, initial_state: project.door_states?.[id] || "CLOSED" },
          (value) =>
            edit((p) => {
              const state = value.initial_state;
              delete value.initial_state;
              p.door_devices[id] = value;
              p.door_states ||= {};
              p.door_states[id] = state;
            }),
          () =>
            edit((p) => {
              delete p.door_devices[id];
              delete p.door_states[id];
            }),
        ),
      ),
  );
  $("workstation-list").replaceChildren(
    ...(project.workstations || []).map((item, index) =>
      workstationEditor(item, index),
    ),
  );
  for (const wall of project.partitions || []) {
    if (wall.level !== Number($("level").value)) continue;
    const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
    line.setAttribute("x1", wall.from[0]);
    line.setAttribute("y1", -wall.from[1]);
    line.setAttribute("x2", wall.to[0]);
    line.setAttribute("y2", -wall.to[1]);
    line.setAttribute("stroke", "#c3f070");
    line.setAttribute("stroke-width", wall.thickness);
    $("plan").append(line);
  }
}
function workstationEditor(item, index) {
  return settingsEditor(
    item.id,
    item,
    (value) =>
      edit((p) => {
        p.workstations[index] = value;
      }),
    () =>
      edit((p) => {
        p.workstations.splice(index, 1);
      }),
  );
}

const surveyPanel = document.createElement("section");
surveyPanel.innerHTML = `<h2>Existing world</h2><p class="muted">Find sectors around your world anchor, survey their objects, then import references or choose exact geometry to remove.</p><button id="find-sectors">Find nearby sectors</button><div id="sector-results"></div><label>Sector depot path<input id="survey-sector" placeholder="Choose a nearby sector or enter a path"></label><button id="survey-anchor">Use building world anchor</button><div class="grid-fields" id="survey-coordinates"></div><label>Radius · metres<input id="survey-radius" type="number" value="20" min=".1" max="1000"></label><button id="survey-world">Survey sector objects</button><div id="survey-results"></div><h3>Applied world changes</h3><div id="world-changes"></div><div id="terrain-editor"></div>`;
$("project-panel").append(surveyPanel);
let latestSurvey = null;
let surveyPosition = [100, 100, 0];
function surveyCoordinates() {
  $("survey-coordinates").replaceChildren(
    ...["X", "Y", "Z"].map((label, i) =>
      field(
        "World " + label,
        surveyPosition[i],
        (v) => (surveyPosition[i] = v),
        { step: 0.1 },
      ),
    ),
  );
}
surveyCoordinates();
$("survey-anchor").onclick = () => {
  surveyPosition = copy(project.origin);
  surveyCoordinates();
};
$("find-sectors").onclick = () =>
  action(async () => {
    const data = await (
      await api("/api/world-sectors", {
        position: project.origin,
        radius: Number($("survey-radius").value),
      })
    ).json();
    const filter = document.createElement("input"),
      label = document.createElement("label"),
      list = document.createElement("select"),
      count = document.createElement("p");
    label.textContent = "Filter nearby sectors";
    filter.type = "search";
    filter.placeholder = "Navigation, exterior, district…";
    label.append(filter);
    list.size = 8;
    list.setAttribute("aria-label", "Nearby sectors");
    const volume = (s) =>
      ["X", "Y", "Z"].reduce(
        (n, k) => n * Math.max(1, s.bounds.Max[k] - s.bounds.Min[k]),
        1,
      );
    data.sectors.sort((a, b) => volume(a) - volume(b));
    const fill = () => {
      const matches = data.sectors.filter((s) =>
        s.path.toLowerCase().includes(filter.value.toLowerCase()),
      );
      list.replaceChildren(
        ...matches
          .slice(0, 100)
          .map(
            (s) => new Option(s.path.split("\\").slice(-2).join(" / "), s.path),
          ),
      );
      list.selectedIndex = -1;
      count.textContent = `${matches.length} sectors · showing ${Math.min(matches.length, 100)} most local bounds. Filter to narrow results.`;
    };
    filter.oninput = fill;
    list.onchange = () => {
      $("survey-sector").value = list.value;
      surveyPosition = copy(project.origin);
      surveyCoordinates();
    };
    fill();
    $("sector-results").replaceChildren(label, list, count);
    status(data.sectors.length + " nearby sectors");
  });
$("survey-world").onclick = () =>
  action(async () => {
    const position = surveyPosition;
    latestSurvey = await (
      await api("/api/survey", {
        sector: $("survey-sector").value,
        position,
        radius: Number($("survey-radius").value),
      })
    ).json();
    const container = $("survey-results");
    container.replaceChildren();
    for (const [i, candidate] of latestSurvey.candidates.entries()) {
      const card = document.createElement("div"),
        label = document.createElement("small"),
        importButton = document.createElement("button"),
        patchButton = document.createElement("button");
      const shapeChoice = document.createElement("select");
      card.id = "survey-item-" + i;
      const resource =
        candidate.mesh?.DepotPath?.$value ||
        candidate.entityTemplate?.DepotPath?.$value ||
        candidate.debugName?.$value ||
        "";
      shapeChoice.setAttribute(
        "aria-label",
        "Collision selection for actor " + candidate.actor,
      );
      shapeChoice.add(new Option("Entire collision actor", "all"));
      for (let shape = 0; shape < (candidate.shape_count || 0); shape++)
        shapeChoice.add(new Option("Collision shape " + shape, String(shape)));
      const acknowledge = document.createElement("label");
      const acknowledged = document.createElement("input");
      acknowledged.type = "checkbox";
      acknowledge.append(acknowledged, " Remove entire occluder group");
      label.textContent = `${String(resource).split("\\").pop()} · ${candidate.type.replace(/^world/, "").replace(/Node$/, "")} · placement ${candidate.index}${candidate.instance !== undefined ? " · instance " + candidate.instance : ""}${candidate.actor !== undefined ? " · actor " + candidate.actor : ""}`;
      importButton.textContent = "Import locked reference";
      importButton.disabled = ![
        "worldMeshNode",
        "worldStaticMeshNode",
        "worldInstancedMeshNode",
      ].includes(candidate.type);
      importButton.onclick = () =>
        action(async () => {
          const result = await (
            await api("/api/import-shell", {
              project,
              sector: latestSurvey.selection,
              selection: [
                {
                  index: candidate.index,
                  instance: candidate.instance ?? null,
                  id: uniqueId("reference"),
                  level: Number($("level").value),
                },
              ],
            })
          ).json();
          history.push(copy(project));
          future = [];
          project = result.project;
          compiled = result;
          draw();
          status("Surveyed shell reference imported");
          studio?.schedulePreview();
        });
      patchButton.textContent = "Select for removal";
      patchButton.disabled = ![
        "worldMeshNode",
        "worldStaticMeshNode",
        "worldStaticOccluderMeshNode",
        "worldEntityProxyMeshNode",
        "worldGenericProxyMeshNode",
        "worldEntityNode",
        "worldInstancedMeshNode",
        "worldCollisionNode",
        "worldNavigationNode",
        "worldInstancedOccluderNode",
      ].includes(candidate.type);
      if (candidate.type === "worldNavigationNode")
        patchButton.textContent = "Create terrain navigation mask";
      patchButton.onclick = () =>
        edit((p) => {
          if (
            candidate.type === "worldInstancedOccluderNode" &&
            !acknowledged.checked
          )
            throw Error("Confirm removal of the entire occluder group first");
          p.retrofit ||= [];
          let patch = p.retrofit.find(
            (s) => s.path === latestSurvey.selection.path,
          );
          if (!patch) {
            patch = copy(latestSurvey.selection);
            p.retrofit.push(patch);
          }
          let deletion = patch.nodeDeletions.find(
            (d) => d.index === candidate.index,
          );
          if (!deletion) {
            deletion = { index: candidate.index, type: candidate.type };
            patch.nodeDeletions.push(deletion);
          }
          if (candidate.instance !== undefined) {
            deletion.expectedInstances = candidate.expectedInstances;
            deletion.instanceDeletions ||= [];
            if (!deletion.instanceDeletions.includes(candidate.instance))
              deletion.instanceDeletions.push(candidate.instance);
          }
          if (candidate.actor !== undefined) {
            deletion.expectedActors = candidate.expectedActors;
            deletion.actorDeletions ||= [];
            const value =
              shapeChoice.value === "all"
                ? candidate.actor
                : [candidate.actor, Number(shapeChoice.value)];
            if (
              !deletion.actorDeletions.some(
                (v) => JSON.stringify(v) === JSON.stringify(value),
              )
            )
              deletion.actorDeletions.push(value);
          }
          if (candidate.type === "worldInstancedOccluderNode") {
            deletion.expectedInstances = candidate.expectedInstances;
            deletion.removeWholeGroup = true;
          }
          if (candidate.type === "worldNavigationNode") {
            if (candidate.resource_index === undefined)
              throw Error(
                "This node has no embedded Human navigation resource",
              );
            p.navigation.terrain_mask = {
              sector: patch.path,
              sha256: patch.sha256,
              expected_nodes: patch.expectedNodes,
              placement_index: candidate.index,
              resource_index: candidate.resource_index,
              outline: [
                [0, 0],
                [6, 0],
                [6, 6],
                [0, 6],
              ],
              height_range: [0, 4],
            };
          }
        });
      card.append(label);
      if (candidate.actor !== undefined) card.append(shapeChoice);
      if (candidate.type === "worldInstancedOccluderNode")
        card.append(acknowledge);
      card.append(importButton, patchButton);
      container.append(card);
    }
    status(`${latestSurvey.candidates.length} surveyed candidates`);
    draw();
  });
function drawWorldChanges() {
  $("world-changes").replaceChildren(
    ...(project.retrofit || []).flatMap((patch, pi) =>
      (patch.nodeDeletions || []).map((d, di) =>
        row(
          `${d.type} · ${d.index}${d.actorDeletions ? " · actors " + d.actorDeletions.join(", ") : ""}${d.instanceDeletions ? " · instances " + d.instanceDeletions.join(", ") : ""}`,
          () => status(patch.path),
          () =>
            edit((p) => {
              p.retrofit[pi].nodeDeletions.splice(di, 1);
              if (d.type === "worldNavigationNode")
                p.navigation.terrain_mask = null;
            }),
        ),
      ),
    ),
  );
  const mask = project.navigation.terrain_mask;
  $("terrain-editor").replaceChildren(
    ...(mask
      ? [
          structuredEditor(
            "Terrain navigation mask",
            { outline: mask.outline, height_range: mask.height_range },
            (v) => edit((p) => Object.assign(p.navigation.terrain_mask, v)),
          ),
        ]
      : []),
  );
}
studio = installStudio({
  viewport,
  sceneKey:()=>JSON.stringify([project,$('level').value,$('cutaway').checked,retrofit?.options()]),
  survey: () => latestSurvey,
  state: () => ({ project, compiled, selection }),
  edit,
  redraw: draw,
  status,
  api,
  search,
  pending: () => pendingObject,
  select: (v) => (selection = v),
  height: kitHeight,
  reconcile: reconcileOpenings,
  cancelPlacement: () => {
    pendingObject = null;
    $("plan").style.cursor = "";
  },
  inside: (eye, target) => {
    Object.assign(cameraPoints, { eye, target });
  },
  route: (kind, value) => {
    routePoints[kind] = value;
    const inputs = $("route-fields").querySelectorAll("input");
    value.forEach((v, i) => (inputs[(kind === "start" ? 0 : 3) + i].value = v));
  },
});
retrofit=installRetrofit({state:()=>({project,compiled,selection}),api,edit,load,status,redraw:draw,starter:()=>copy(bootstrap.starter),resetCamera:()=>studio?.resetCamera(),preview:()=>studio?.schedulePreview(true),select:v=>{selection=v;},accept:result=>{history.push(copy(project));future=[];project=result.project;compiled=result;selection=null;draw();studio?.resetCamera();studio?.schedulePreview(true);}});

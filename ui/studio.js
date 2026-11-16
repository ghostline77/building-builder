import { structuredEditor, discardEditor } from "/forms.js";
const $ = (id) => document.getElementById(id);
const ns = "http://www.w3.org/2000/svg";
const clone = (v) => structuredClone(v);
const num = (v) => Number(Number(v).toFixed(3));
function el(tag, text, cls) {
  const e = document.createElement(tag);
  if (text) e.textContent = text;
  if (cls) e.className = cls;
  return e;
}
function btn(text, fn) {
  const b = el("button", text);
  b.type = "button";
  b.onclick = fn;
  return b;
}
function svg(tag, attrs) {
  const e = document.createElementNS(ns, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  return e;
}
export function installStudio(ctx) {
  const { edit, redraw, status, api } = ctx;
  const modules = new Map();
  const actorAppearances = new Map();
  let previewTimer,
    lastRendered = "",
    cameraState = null,
    planView = null,
    lastSelectionKey = null,
    drag = null,
    previewDrag = null;
  const get = () => ctx.state();
  for (const [id, key, listId] of [
    ["add-light", "lights", "technical-list"],
    ["add-audio", "audio_areas", "technical-list"],
    ["add-shell-door", "door_devices", "shell-door-list"],
    ["add-workstation", "workstations", "workstation-list"],
  ]) {
    const original = $(id).onclick;
    $(id).onclick = async () => {
      const previous = get().project[key] || [],
        before = new Set(
          Array.isArray(previous)
            ? previous.map((v) => v.id)
            : Object.keys(previous),
        );
      await original();
      const values = get().project[key] || [],
        name = (
          Array.isArray(values) ? values.map((v) => v.id) : Object.keys(values)
        ).find((id) => !before.has(id));
      const editor = [...$(listId).querySelectorAll("details")].find(
        (d) => d.querySelector("summary")?.textContent === name,
      );
      if (editor) {
        editor.open = true;
        editor.scrollIntoView({ block: "nearest" });
      }
    };
  }
  $("add-stair").onclick = () => {
    const { project, selection } = get(),
      level = Number($("level").value);
    const room =
      project.rooms.find((r) => r.id === selection?.id && r.level === level) ||
      project.rooms.find((r) => r.level === level);
    if (!room || level >= 3) {
      status(
        "Select a grid room below the top floor before adding stairs.",
        true,
      );
      return;
    }
    const dialog = el("dialog"),
      heading = el("h2", "Connect two floors"),
      help = el(
        "p",
        "Choose the flight dimensions and direction. The shaft and landings must fit grid rooms on both floors. The compiler checks the 35° slope limit and cuts matching floor openings.",
      );
    const createUpper = !project.rooms.some(
      (r) =>
        r.level === level + 1 &&
        r.x === room.x &&
        r.y === room.y &&
        r.width === room.width &&
        r.depth === room.depth,
    );
    const draft = {
      stair: {
        id: "stairs_" + Date.now().toString(36),
        x: room.x * 3 + 0.2,
        y: room.y * 3 + 1.2,
        width: 2.6,
        run: Math.max(
          3,
          Math.ceil((ctx.height() / 2 / Math.tan((35 * Math.PI) / 180)) * 20) /
            20,
        ),
        level,
        landing: 1.2,
        reverse: false,
        railings: true,
        axis: "y",
        type: "return",
        flight_width: 1.2,
      },
      create_matching_upper_room: createUpper,
    };
    const editor = structuredEditor(
      "New stairs",
      draft,
      async (value) => {
        const revision = get().project.revision;
        await edit((p) => {
          p.stairs ||= [];
          p.stairs.push(value.stair);
          if (value.create_matching_upper_room)
            p.rooms.push({
              ...clone(room),
              id: room.id + "_upper_" + Date.now().toString(36),
              level: level + 1,
            });
        });
        if (get().project.revision !== revision) {
          discardEditor("New stairs");
          dialog.close();
        }
      },
      null,
    );
    editor.open = true;
    dialog.append(
      heading,
      help,
      btn("Cancel stairs", () => dialog.close()),
      editor,
    );
    dialog.onclose = () => {
      discardEditor("New stairs");
      dialog.remove();
    };
    document.body.append(dialog);
    dialog.showModal();
  };
  for (const id of ["actor-records", "actor-appearances", "seat-workspots"]) {
    const list = el("datalist");
    list.id = id;
    document.body.append(list);
  }
  let actorSearchTimer;
  document.addEventListener("input", (event) => {
    if (event.target.name !== "actor.record") return;
    const input = event.target;
    clearTimeout(actorSearchTimer);
    actorSearchTimer = setTimeout(async () => {
      try {
        const result = await (
          await api("/api/actor-options?q=" + encodeURIComponent(input.value))
        ).json();
        for (const [record, appearance] of Object.entries(
          result.appearances || {},
        ))
          actorAppearances.set(record, appearance);
        $("actor-records").replaceChildren(
          ...result.records.map((record) => new Option(record, record)),
        );
      } catch (error) {
        status(error.message, true);
      }
    }, 250);
  });
  const point = (e) => {
    const p = $("plan").createSVGPoint();
    p.x = e.clientX;
    p.y = e.clientY;
    const m = $("plan").getScreenCTM();
    if (!m) return null;
    const v = p.matrixTransform(m.inverse());
    return [v.x, -v.y];
  };
  const snap = (v) =>
    Math.round(v / Number($("snap-size").value)) * Number($("snap-size").value);
  const tool = $("plan-tool");
  tool.add(new Option("Move object", "object-move"));
  tool.add(new Option("Rotate object", "object-rotate"));
  const tools = el("div", null, "plan-tools");
  const toolLabel = tool.parentElement;
  const oldHeading = toolLabel.previousElementSibling,
    oldHint = toolLabel.nextElementSibling;
  tools.append(toolLabel);
  oldHeading?.remove();
  oldHint?.remove();
  const snapLabel = el("label", "Snap");
  snapLabel.innerHTML +=
    ' <select id="snap-size"><option value="0.05">5 cm</option><option value="0.25">25 cm</option><option value="1">1 m</option></select>';
  tools.append(snapLabel);
  const collisionToggle = el("label");
  collisionToggle.innerHTML =
    '<input id="show-collision" type="checkbox"> Collision & door paths';
  tools.append(collisionToggle);
  collisionToggle.querySelector("input").onchange = redraw;
  tools.append(
    btn("Focus selection", () => focusSelection()),
    btn("Fit building", () => resetCamera()),
    btn("Duplicate", () => duplicate()),
    btn("Delete", () => remove()),
  );
  document.querySelector(".plan-wrap").before(tools);
  const hint = el(
    "p",
    "Select an object to drag it. R rotates 15° · Ctrl+D duplicates · Delete removes · Escape cancels.",
    "canvas-hint",
  );
  document.querySelector(".plan-wrap").append(hint);
  const mode = el("div", null, "placement-banner");
  mode.id = "placement-banner";
  mode.hidden = true;
  document.querySelector(".plan-wrap").prepend(mode);
  const previewBar = document.querySelector(".preview-toolbar");
  previewBar.querySelector("h2").textContent = "3D preview";
  const auto = el("label");
  auto.innerHTML =
    '<input type="checkbox" id="auto-preview" checked> Auto refresh';
  previewBar.append(auto);
  const viewButtons = el("div", null, "preview-navigation");
  viewButtons.append(
    btn("−", () => zoom(1.25)),
    btn("+", () => zoom(0.8)),
    btn("↶", () => orbit(-15, 0)),
    btn("↷", () => orbit(15, 0)),
    btn("↑", () => orbit(0, 10)),
    btn("↓", () => orbit(0, -10)),
    btn("Walk inside", () => walkInside()),
    btn("Maximize", (event) => {
      const maximized = document
        .querySelector(".workspace")
        .classList.toggle("maximized");
      event.currentTarget.textContent = maximized
        ? "Restore workspace"
        : "Maximize";
      event.currentTarget.setAttribute("aria-pressed", String(maximized));
      planLabels();
    }),
  );
  [
    "Zoom out",
    "Zoom in",
    "Orbit left",
    "Orbit right",
    "Orbit up",
    "Orbit down",
  ].forEach((name, i) =>
    viewButtons.children[i].setAttribute("aria-label", name),
  );
  previewBar.after(viewButtons);
  const previewHelp = el(
    "p",
    "Drag to orbit · Shift-drag to pan · Scroll to zoom",
    "preview-help",
  );
  document.querySelector(".preview-wrap").append(previewHelp);
  $("preview").draggable = false;
  const previewWrap = document.querySelector(".preview-wrap");
  previewWrap.tabIndex = 0;
  previewWrap.setAttribute(
    "aria-label",
    "3D preview. Arrow keys orbit, Shift and arrows pan, plus and minus zoom.",
  );
  if (!ctx.viewport) {
  previewWrap.onkeydown = (e) => {
    if (
      !["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "+", "-"].includes(
        e.key,
      )
    )
      return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "+" || e.key === "-") {
      zoom(e.key === "+" ? 0.8 : 1.25);
      return;
    }
    ensureCamera();
    if (e.shiftKey) {
      const step = cameraState.distance * 0.05;
      cameraState.target[0] +=
        e.key === "ArrowLeft" ? -step : e.key === "ArrowRight" ? step : 0;
      cameraState.target[2] +=
        e.key === "ArrowUp" ? step : e.key === "ArrowDown" ? -step : 0;
      schedulePreview(true);
    } else
      orbit(
        e.key === "ArrowLeft" ? -15 : e.key === "ArrowRight" ? 15 : 0,
        e.key === "ArrowUp" ? 10 : e.key === "ArrowDown" ? -10 : 0,
      );
  };
  previewWrap.onpointerdown = (e) => {
    if (e.target.closest("button")) return;
    ensureCamera();
    previewDrag = {
      x: e.clientX,
      y: e.clientY,
      state: clone(cameraState),
      pan: e.shiftKey,
    };
    previewWrap.setPointerCapture(e.pointerId);
  };
  previewWrap.onpointermove = (e) => {
    if (!previewDrag) return;
    const dx = e.clientX - previewDrag.x,
      dy = e.clientY - previewDrag.y;
    cameraState = clone(previewDrag.state);
    if (previewDrag.pan) {
      const scale =
          cameraState.distance / Math.max(previewWrap.clientHeight, 1),
        a = (cameraState.azimuth * Math.PI) / 180;
      cameraState.target[0] -=
        (dx * Math.cos(a) + dy * Math.sin(a) * 0.5) * scale;
      cameraState.target[1] +=
        (dx * Math.sin(a) - dy * Math.cos(a) * 0.5) * scale;
      cameraState.target[2] += dy * scale * 0.5;
    } else {
      cameraState.azimuth += dx * 0.5;
      cameraState.elevation = Math.max(
        -15,
        Math.min(85, cameraState.elevation + dy * 0.3),
      );
    }
    previewWrap.style.cursor = "grabbing";
  };
  previewWrap.onpointerup = (e) => {
    if (!previewDrag) return;
    previewDrag = null;
    previewWrap.releasePointerCapture(e.pointerId);
    previewWrap.style.cursor = "grab";
    schedulePreview(true);
  };
  previewWrap.onpointercancel = () => {
    previewDrag = null;
  };
  previewWrap.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      zoom(Math.exp(e.deltaY * 0.001));
    },
    { passive: false },
  );
  }
  $("view").addEventListener("change", () => {
    if(ctx.viewport){ctx.viewport.fit($('view').value==='hero'?'perspective':$('view').value);return;}
    ensureCamera();
    const angles = {
      hero: [45, 35],
      front: [0, 5],
      back: [180, 5],
      left: [-90, 5],
      right: [90, 5],
    };
    [cameraState.azimuth, cameraState.elevation] = angles[$("view").value];
    $("interior-camera").checked = false;
    schedulePreview(true);
  });
  $("cutaway").addEventListener("change", () => schedulePreview());
  $("level").addEventListener("change", () => {
    cameraState = null;
    planView = null;
    redraw();
    schedulePreview();
  });
  $("auto-preview").onchange = () => schedulePreview();
  // Less frequent project-wide settings no longer crowd the contextual inspector.
  const inspector = document.querySelector(".inspector");
  for (const heading of [...inspector.querySelectorAll(":scope > h2")].slice(
    1,
  )) {
    const details = el("details", null, "inspector-section"),
      summary = el("summary", heading.textContent);
    let n = heading.nextSibling;
    details.append(summary);
    heading.replaceWith(details);
    while (n && !(n.nodeType === 1 && n.tagName === "H2")) {
      const next = n.nextSibling;
      details.append(n);
      n = next;
    }
    details.open = ["Finishes", "Validation"].includes(summary.textContent);
  }
  const navDetails = el("details", null, "advanced-panel");
  navDetails.append(el("summary", "Navigation and route checks"));
  const nav = $("navigation-fields").closest("section");
  nav.before(navDetails);
  navDetails.append(nav);
  // Independent catalogue kind/category filters, grouping and image enlargement.
  const kindLabel = el("label", "Object completeness");
  for (const option of [...$("role").options])
    if (["assembly", "part"].includes(option.value)) option.remove();
  $("role").parentElement.firstChild.textContent = "Category";
  $("role").addEventListener("change", () => {
    if (!["", "furniture", "electronics", "props"].includes($("role").value))
      $("assembly-filter").value = "";
    ctx.search();
  });
  kindLabel.innerHTML +=
    '<select id="assembly-filter"><option value="assembly">Complete objects</option><option value="">All objects and parts</option><option value="part">Separate parts</option><option value="mesh">Individual meshes</option></select>';
  $("role").parentElement.after(kindLabel);
  $("assembly-filter").onchange = () => ctx.search();
  document
    .querySelectorAll("[data-panel]")
    .forEach((button) =>
      button.addEventListener("click", () =>
        document.body.classList.toggle(
          "catalogue-mode",
          button.dataset.panel === "library",
        ),
      ),
    );
  const grouping = el("label");
  grouping.innerHTML =
    '<input id="group-families" type="checkbox" checked> Group object families';
  kindLabel.after(grouping);
  $("group-families").onchange = groupLibrary;
  const libraryObserver = new MutationObserver(groupLibrary);
  libraryObserver.observe($("library-results"), { childList: true });
  let groupingNow = false;
  function groupLibrary() {
    if (groupingNow) return;
    groupingNow = true;
    libraryObserver.disconnect();
    const root = $("library-results");
    const cards = [...root.querySelectorAll(".card")];
    root.replaceChildren();
    const groups = new Map();
    for (const card of cards) {
      const name = card.querySelector("strong").textContent;
      const family = name
        .replace(/\b(?:[a-z]|\d+)\b/g, "")
        .replace(/\s+/g, " ")
        .trim();
      if (!$("group-families").checked) {
        root.append(card);
        continue;
      }
      if (!groups.has(family)) {
        const d = el("details", null, "library-family");
        d.open = groups.size === 0;
        d.append(el("summary", family), el("div", null, "family-cards"));
        root.append(d);
        groups.set(family, d);
      }
      groups.get(family).querySelector(".family-cards").append(card);
    }
    for (const d of groups.values())
      d.querySelector("summary").textContent +=
        ` · ${d.querySelectorAll(".card").length} styles`;
    libraryObserver.observe(root, { childList: true });
    groupingNow = false;
  }
  const imageDialog = el("dialog");
  imageDialog.className = "image-dialog";
  const imageTitle = el("h2"),
    largeImage = el("img");
  imageDialog.append(
    btn("Close preview", () => imageDialog.close()),
    imageTitle,
    largeImage,
  );
  document.body.append(imageDialog);
  function enlarge(src, name) {
    largeImage.src = src;
    largeImage.alt = name;
    imageTitle.textContent = name.replaceAll("_", " ");
    imageDialog.showModal();
  }
  // Direct plan manipulation with a live footprint and rotation direction.
  const plan = $("plan");
  let routePick = null;
  $("route-check").before(
    btn("Pick route start on plan", () => {
      routePick = "start";
      status("Click the plan for the route start");
    }),
    btn("Pick route end on plan", () => {
      routePick = "end";
      status("Click the plan for the route end");
    }),
  );
  plan.addEventListener(
    "click",
    (event) => {
      if (!routePick) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      const p = point(event);
      ctx.route(routePick, [
        snap(p[0]),
        snap(p[1]),
        Number($("level").value) * ctx.height(),
      ]);
      routePick = null;
      status("Route point selected");
    },
    true,
  );
  plan.tabIndex = 0;
  plan.addEventListener(
    "wheel",
    (event) => {
      event.preventDefault();
      const p = point(event),
        v = plan.viewBox.baseVal,
        factor = Math.exp(event.deltaY * 0.001);
      if (v.width * factor < 1 || v.width * factor > 10000) return;
      plan.setAttribute(
        "viewBox",
        `${p[0] - (p[0] - v.x) * factor} ${-p[1] - (-p[1] - v.y) * factor} ${v.width * factor} ${v.height * factor}`,
      );
      planView = plan.getAttribute("viewBox");
      planLabels();
    },
    { passive: false },
  );
  plan.addEventListener("keydown", (event) => {
    const pending = ctx.pending();
    if (event.key !== "Enter" || !pending) return;
    event.preventDefault();
    const { project, selection } = get();
    const room =
      project.rooms.find((r) => r.id === selection?.id) ||
      project.rooms.find((r) => r.level === Number($("level").value));
    edit((p) => {
      const id = "object_" + Date.now().toString(36);
      p.objects.push({
        id,
        asset: pending.asset,
        appearance: pending.appearance,
        source_content_sha256: pending.pin,
        position: room
          ? [
              (room.x + room.width / 2) * 3,
              (room.y + room.depth / 2) * 3,
              room.level * ctx.height(),
            ]
          : [0, 0, Number($("level").value) * ctx.height()],
        yaw: pending.yaw || 0,
        anchor: "base_center",
        level: Number($("level").value),
      });
      ctx.select({ kind: "object", id });
      ctx.cancelPlacement();
    });
  });
  function placementStarted() {
    mode.hidden = false;
    mode.replaceChildren(
      document.createTextNode(
        "Placing " +
          (ctx.pending()?.name || "object").replaceAll("_", " ") +
          " · Click plan, or Enter for room centre · R rotate · Esc cancel",
      ),
      btn("Cancel", () => {
        ctx.cancelPlacement();
        mode.hidden = true;
        $("placement-ghost")?.remove();
      }),
    );
    plan.focus();
  }
  plan.addEventListener(
    "pointerdown",
    (e) => {
      if (ctx.pending()) return;
      const endpoint = e.target.closest("[data-partition-end]");
      if (endpoint) {
        const partition = get().project.partitions.find(
          (p) => p.id === endpoint.dataset.partition,
        );
        drag = {
          partition: clone(partition),
          end: endpoint.dataset.partitionEnd,
          node: endpoint,
          moved: false,
        };
        plan.setPointerCapture(e.pointerId);
        e.preventDefault();
        e.stopImmediatePropagation();
        return;
      }
      const hit = e.target.closest("[data-object]");
      if (
        !hit ||
        !["select", "object-move", "object-rotate"].includes(tool.value)
      )
        return;
      const object = get().project.objects.find(
        (o) => o.id === hit.dataset.object,
      );
      if (!object || object.reference_only) return;
      const p = point(e);
      ctx.select({ kind: "object", id: object.id });
      drag = {
        object: clone(object),
        start: p,
        rotate:
          tool.value === "object-rotate" ||
          !!e.target.closest("[data-rotate-handle]"),
        node: hit,
        pointer: e.pointerId,
        moved: false,
      };
      plan.setPointerCapture(e.pointerId);
      e.preventDefault();
      e.stopImmediatePropagation();
    },
    true,
  );
  plan.addEventListener("pointermove", (e) => {
    const pending = ctx.pending(),
      p = point(e);
    if (!p) return;
    if (pending) {
      mode.hidden = false;
      let ghost = $("placement-ghost");
      if (!ghost) {
        ghost = svg("g", { id: "placement-ghost", "pointer-events": "none" });
        const dims = pending.bounds?.dimensions || [0.8, 0.8];
        ghost.append(
          svg("rect", {
            x: -dims[0] / 2,
            y: -dims[1] / 2,
            width: dims[0],
            height: dims[1],
            fill: "#c3f07055",
            stroke: "#c3f070",
            "stroke-width": 0.04,
          }),
        );
        plan.append(ghost);
      }
      ghost.setAttribute(
        "transform",
        `translate(${snap(p[0])},${-snap(p[1])}) rotate(${-pending.yaw})`,
      );
      return;
    }
    if (!drag) return;
    if (drag.partition) {
      drag.position = [snap(p[0]), snap(p[1]), drag.partition[drag.end][2]];
      drag.moved = true;
      drag.node.setAttribute("cx", drag.position[0]);
      drag.node.setAttribute("cy", -drag.position[1]);
      return;
    }
    drag.moved ||=
      Math.hypot(p[0] - drag.start[0], p[1] - drag.start[1]) > 0.05;
    const o = drag.object;
    if (drag.rotate) {
      const a = Math.atan2(p[1] - o.position[1], p[0] - o.position[0]);
      drag.yaw = Math.round((a * 180) / Math.PI / 15) * 15;
      drag.node.setAttribute(
        "transform",
        `translate(${o.position[0]},${-o.position[1]}) rotate(${-drag.yaw})`,
      );
    } else {
      drag.position = [
        snap(o.position[0] + p[0] - drag.start[0]),
        snap(o.position[1] + p[1] - drag.start[1]),
        o.position[2],
      ];
      drag.node.setAttribute(
        "transform",
        `translate(${drag.position[0]},${-drag.position[1]}) rotate(${-o.yaw})`,
      );
    }
  });
  plan.addEventListener(
    "pointerup",
    (e) => {
      if (!drag) return;
      const d = drag;
      drag = null;
      plan.releasePointerCapture(e.pointerId);
      e.stopImmediatePropagation();
      if (!d.moved) {
        redraw();
        return;
      }
      if (d.partition) {
        edit(
          (p) =>
            (p.partitions.find((w) => w.id === d.partition.id)[d.end] =
              d.position),
        );
        return;
      }
      edit((p) => {
        const o = p.objects.find((o) => o.id === d.object.id);
        if (d.rotate) o.yaw = d.yaw;
        else o.position = d.position;
      });
    },
    true,
  );
  plan.addEventListener("pointercancel", () => {
    drag = null;
    redraw();
  });
  plan.addEventListener(
    "click",
    (e) => {
      if (tool.value.startsWith("object-")) e.stopImmediatePropagation();
    },
    true,
  );
  document.addEventListener("keydown", (e) => {
    if (
      ["INPUT", "TEXTAREA", "SELECT"].includes(
        document.activeElement.tagName,
      ) ||
      document.querySelector("dialog[open]")
    )
      return;
    const pending = ctx.pending();
    if (e.key === "Escape") {
      routePick = null;
      ctx.cancelPlacement();
      mode.hidden = true;
      $("placement-ghost")?.remove();
      drag = null;
      document.querySelector(".workspace").classList.remove("maximized");
      viewButtons.lastElementChild.textContent = "Maximize";
      viewButtons.lastElementChild.setAttribute("aria-pressed", "false");
    }
    if (e.key.toLowerCase() === "r") {
      if (pending) pending.yaw = (pending.yaw + 15) % 360;
      else rotate(15);
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "d") {
      e.preventDefault();
      duplicate();
    }
    if (e.key === "Delete") remove();
    if (
      ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key) &&
      get().selection?.kind === "object"
    ) {
      e.preventDefault();
      const delta = Number($("snap-size").value);
      edit((p) => {
        const o = p.objects.find((o) => o.id === get().selection.id);
        if (!o || o.reference_only) return;
        o.position[0] +=
          e.key === "ArrowLeft" ? -delta : e.key === "ArrowRight" ? delta : 0;
        o.position[1] +=
          e.key === "ArrowDown" ? -delta : e.key === "ArrowUp" ? delta : 0;
      });
    }
  });
  function remove() {
    const { project, selection: s } = get();
    if (!s) return;
    if (
      s.kind === "object" &&
      project.objects.find((o) => o.id === s.id)?.reference_only
    ) {
      status("Surveyed references are locked");
      return;
    }
    edit((p) => {
      if (s.kind === "object") {
        p.objects = p.objects.filter((o) => o.id !== s.id);
        p.workstations = (p.workstations || []).filter(
          (w) => w.desk !== s.id && w.chair !== s.id,
        );
      } else if (s.kind === "room") {
        p.rooms = p.rooms.filter((o) => o.id !== s.id);
        ctx.reconcile(p);
      } else if (s.kind === "shell") {
        p.shell_rooms = p.shell_rooms.filter((o) => o.id !== s.id);
      } else if (s.kind === "partition") {
        p.partitions = p.partitions.filter((o) => o.id !== s.id);
      } else if (s.kind === "wall") {
        p.openings[s.id] = "passage";
        delete p.door_states?.[s.id];
        delete p.door_devices?.[s.id];
      }
      ctx.select(null);
    });
  }
  function duplicate() {
    const { selection: s } = get();
    if (!s) return;
    edit((p) => {
      const list = {
        object: p.objects,
        room: p.rooms,
        shell: p.shell_rooms,
        partition: p.partitions,
      }[s.kind];
      if (!list) return;
      const source = list.find((o) => o.id === s.id);
      if (!source || source.reference_only) return;
      const next = clone(source);
      next.id = source.id + "_copy_" + Date.now().toString(36);
      if (s.kind === "object") {
        next.position[0] += 0.5;
        next.position[1] += 0.5;
      } else if (s.kind === "room") next.x += next.width;
      else if (s.kind === "shell") next.position[0] += next.size[0] + 1;
      else {
        next.from[0] += 0.5;
        next.to[0] += 0.5;
      }
      list.push(next);
      ctx.select({ kind: s.kind, id: next.id });
    });
  }
  function rotate(degrees) {
    const s = get().selection;
    if (s?.kind !== "object") return;
    edit((p) => {
      const o = p.objects.find((o) => o.id === s.id);
      if (o && !o.reference_only) o.yaw = (o.yaw + degrees) % 360;
    });
  }
  function objectBounds(object) {
    const p =
      get().compiled.placements.find((p) => p.id === object.id) ||
      get().compiled.placements.find((p) => p.id.startsWith(object.id + ":"));
    const b = p?.bounds;
    const scale = object.scale || [1, 1, 1];
    const dims = b
      ? b.max.map((n, i) => (n - b.min[i]) * scale[i])
      : [0.6, 0.6, 1];
    return dims.map((n) => Math.max(0.05, n));
  }
  function geometryBounds(object) {
    const p = get().compiled.placements.find((p) => p.id === object.id);
    if (!p) {
      const d = objectBounds(object);
      return {
        low: object.position.map((v, i) => v - (i < 2 ? d[i] / 2 : 0)),
        high: object.position.map((v, i) => v + (i < 2 ? d[i] / 2 : d[i])),
        center: [
          object.position[0],
          object.position[1],
          object.position[2] + d[2] / 2,
        ],
      };
    }
    const points = [];
    for (const x of [p.bounds.min[0], p.bounds.max[0]])
      for (const y of [p.bounds.min[1], p.bounds.max[1]])
        for (const z of [p.bounds.min[2], p.bounds.max[2]]) {
          let [a, b, c] = [x * p.scale[0], y * p.scale[1], z * p.scale[2]];
          if (p.rotation) {
            const [qx, qy, qz, qw] = p.rotation,
              tx = 2 * (qy * c - qz * b),
              ty = 2 * (qz * a - qx * c),
              tz = 2 * (qx * b - qy * a);
            [a, b, c] = [
              a + qw * tx + qy * tz - qz * ty,
              b + qw * ty + qz * tx - qx * tz,
              c + qw * tz + qx * ty - qy * tx,
            ];
          } else {
            const angle = (p.yaw * Math.PI) / 180;
            [a, b] = [
              a * Math.cos(angle) - b * Math.sin(angle),
              a * Math.sin(angle) + b * Math.cos(angle),
            ];
          }
          points.push([
            a + p.position[0],
            b + p.position[1],
            c + p.position[2],
          ]);
        }
    const low = [0, 1, 2].map((i) => Math.min(...points.map((p) => p[i]))),
      high = [0, 1, 2].map((i) => Math.max(...points.map((p) => p[i])));
    return { low, high, center: low.map((v, i) => (v + high[i]) / 2) };
  }
  function draw() {
    const { project, selection, compiled } = get();
    const selectionKey = selection ? selection.kind + ":" + selection.id : "";
    if (selectionKey !== lastSelectionKey) {
      document.querySelector(".inspector").scrollTop = 0;
      lastSelectionKey = selectionKey;
    }
    if (planView) plan.setAttribute("viewBox", planView);
    if ($("show-collision").checked) {
      const floor = Number($("level").value) * ctx.height();
      for (const solid of compiled.solids.concat(compiled.reference_solids||[])) {
        if (
          solid.center[2] + solid.size[2] / 2 < floor ||
          solid.center[2] - solid.size[2] / 2 > floor + ctx.height()
        )
          continue;
        plan.append(
          svg("rect", {
            x: -solid.size[0] / 2,
            y: -solid.size[1] / 2,
            width: solid.size[0],
            height: solid.size[1],
            transform: `translate(${solid.center[0]},${-solid.center[1]}) rotate(${-solid.yaw})`,
            fill: "#ffac6720",
            stroke: "#ffac67",
            "stroke-width": 0.025,
            "pointer-events": "none",
          }),
        );
      }
      for (const collision of compiled.cooked_collision||[]) {
        const g=collision.geometry;
        for(const t of g.triangles){const p=t.map(i=>g.vertices[i]);if(p.every(v=>v[2]<floor)||p.every(v=>v[2]>floor+ctx.height()))continue;plan.append(svg('polygon',{points:p.map(v=>`${v[0]},${-v[1]}`).join(' '),fill:'#ffac6720',stroke:'#ffac67','stroke-width':.02,'pointer-events':'none'}));}
      }
      for (const door of Object.values(project.door_devices || {})) {
        if (!door.endpoints) continue;
        const [a, b] = door.endpoints;
        plan.append(
          svg("line", {
            x1: a[0],
            y1: -a[1],
            x2: b[0],
            y2: -b[1],
            stroke: "#f7ce7c",
            "stroke-width": 0.15,
            "stroke-dasharray": ".15 .1",
            "pointer-events": "none",
          }),
        );
      }
    }
    for (const stair of project.stairs || []) {
      if (![stair.level, stair.level + 1].includes(Number($("level").value)))
        continue;
      const w = stair.axis === "x" ? stair.run : stair.width,
        d = stair.axis === "x" ? stair.width : stair.run;
      const group = svg("g", {
        role: "button",
        tabindex: 0,
        "aria-label": "Edit stairs " + stair.id,
      });
      group.append(
        svg("rect", {
          x: stair.x,
          y: -stair.y - d,
          width: w,
          height: d,
          fill: "#f0bf7033",
          stroke: "#f0bf70",
          "stroke-width": 0.05,
        }),
      );
      const text = svg("text", {
        x: stair.x + w / 2,
        y: -stair.y - d / 2,
        class: "room-label",
      });
      text.textContent = "Stairs → floor " + (stair.level + 1);
      group.append(text);
      group.onclick = () => {
        document.querySelector('[data-panel="build"]').click();
        const editor = [...$("stairs-list").querySelectorAll("details")].find(
          (d) => d.querySelector("summary").textContent === stair.id,
        );
        if (editor) {
          editor.open = true;
          editor.scrollIntoView({ block: "nearest" });
        }
      };
      group.onkeydown = (e) => {
        if (e.key === "Enter") group.onclick();
      };
      plan.append(group);
    }
    planLabels();
    for (const [i, candidate] of (ctx.survey()?.candidates || []).entries()) {
      const native = Array.isArray(candidate.position)
        ? candidate.position
        : [candidate.position?.X, candidate.position?.Y, candidate.position?.Z];
      if (!native.every(Number.isFinite)) continue;
      const a = (-project.yaw * Math.PI) / 180,
        dx = native[0] - project.origin[0],
        dy = native[1] - project.origin[1],
        x = dx * Math.cos(a) - dy * Math.sin(a),
        y = dx * Math.sin(a) + dy * Math.cos(a);
      const marker = svg("circle", {
        cx: x,
        cy: -y,
        r: 0.15,
        fill: "#71cde0",
        stroke: "#142028",
        "stroke-width": 0.025,
        tabindex: 0,
        role: "button",
        "aria-label":
          "Select surveyed " + candidate.type + " " + candidate.index,
      });
      marker.onclick = () => {
        document.querySelector('[data-panel="project"]').click();
        const card = $("survey-item-" + i);
        if (card) {
          document
            .querySelectorAll(".survey-selected")
            .forEach((v) => v.classList.remove("survey-selected"));
          card.classList.add("survey-selected");
          card.scrollIntoView({ block: "nearest" });
        }
      };
      marker.onkeydown = (e) => {
        if (e.key === "Enter") marker.onclick();
      };
      plan.append(marker);
    }
    $("seat-workspots").replaceChildren(
      ...[
        ...new Set([
          "base\\workspots\\common\\chair\\generic__sit_chair__sit_around__01.workspot",
          ...(project.workstations || []).map((w) => w.actor.workspot),
        ]),
      ].map((v) => new Option(v, v)),
    );
    $("actor-appearances").replaceChildren(
      ...[
        ...new Set((project.workstations || []).map((w) => w.actor.appearance)),
      ].map((v) => new Option(v, v)),
    );
    mode.hidden = !ctx.pending();
    if (!ctx.pending()) $("placement-ghost")?.remove();
    for (const node of [...plan.querySelectorAll("[data-object]")]) {
      const o = project.objects.find((o) => o.id === node.dataset.object);
      if (!o) continue;
      const [w, d] = objectBounds(o);
      const g = svg("g", {
        "data-object": o.id,
        transform: `translate(${o.position[0]},${-o.position[1]}) rotate(${-o.yaw})`,
        tabindex: 0,
        role: "button",
        "aria-label": "Select " + o.id,
        class: "object-footprint" + (selection?.id === o.id ? " selected" : ""),
      });
      const pivot = o.anchor === "pivot";
      const native = get().compiled.placements.find((p) => p.id === o.id);
      g.append(
        svg("rect", {
          x: pivot && native ? native.bounds.min[0] * o.scale[0] : -w / 2,
          y: pivot && native ? -native.bounds.max[1] * o.scale[1] : -d / 2,
          width: w,
          height: d,
          rx: 0.02,
        }),
        svg("path", { d: "M 0 0 L 0 -.6 M -.12 -.42 L 0 -.6 L .12 -.42" }),
      );
      if (o.rotation) {
        const bounds = geometryBounds(o);
        g.removeAttribute("transform");
        const r = g.querySelector("rect");
        r.setAttribute("x", bounds.low[0]);
        r.setAttribute("y", -bounds.high[1]);
        r.setAttribute("width", bounds.high[0] - bounds.low[0]);
        r.setAttribute("height", bounds.high[1] - bounds.low[1]);
        g.querySelector("path")?.remove();
      }
      const title = svg("title", {});
      title.textContent = o.id + ` · ${num(w)} × ${num(d)} m`;
      g.append(title);
      if (selection?.id === o.id && !o.reference_only) {
        const handle = svg("circle", {
          cx: 0,
          cy: -d / 2 - 0.4,
          r: 0.13,
          fill: "#c3f070",
          "data-rotate-handle": "true",
        });
        const tip = svg("title", {});
        tip.textContent = "Drag to rotate";
        handle.append(tip);
        g.append(handle);
      }
      g.onclick = () => {
        ctx.select({ kind: "object", id: o.id });
        redraw();
      };
      g.onkeydown = (e) => {
        if (e.key === "Enter") {
          ctx.select({ kind: "object", id: o.id });
          redraw();
        }
      };
      node.replaceWith(g);
    }
    // Select shell rooms and partitions directly from the drawing.
    for (const partition of project.partitions || []) {
      if (partition.level !== Number($("level").value)) continue;
      const line = svg("line", {
        x1: partition.from[0],
        y1: -partition.from[1],
        x2: partition.to[0],
        y2: -partition.to[1],
        stroke: "transparent",
        "stroke-width": 0.4,
        class: "plan-hit",
        tabindex: 0,
        role: "button",
        "aria-label": "Edit " + partition.id,
      });
      line.onclick = () => {
        ctx.select({ kind: "partition", id: partition.id });
        redraw();
        openEditor(partition.id);
      };
      line.onkeydown = (e) => {
        if (e.key === "Enter") openEditor(partition.id);
      };
      plan.append(line);
      const length = Math.hypot(
        partition.to[0] - partition.from[0],
        partition.to[1] - partition.from[1],
      );
      for (const opening of partition.openings || []) {
        const a = opening.start / length,
          b = (opening.start + opening.width) / length;
        const indicator = svg("line", {
          x1: partition.from[0] + (partition.to[0] - partition.from[0]) * a,
          y1: -(partition.from[1] + (partition.to[1] - partition.from[1]) * a),
          x2: partition.from[0] + (partition.to[0] - partition.from[0]) * b,
          y2: -(partition.from[1] + (partition.to[1] - partition.from[1]) * b),
          stroke: opening.sill ? "#71cde0" : "#f0bf70",
          "stroke-width": 0.18,
          "pointer-events": "none",
        });
        plan.append(indicator);
      }
      if (selection?.kind === "partition" && selection.id === partition.id)
        for (const end of ["from", "to"])
          plan.append(
            svg("circle", {
              cx: partition[end][0],
              cy: -partition[end][1],
              r: 0.16,
              fill: "#c3f070",
              "data-partition": partition.id,
              "data-partition-end": end,
            }),
          );
    }
    // Furniture stays selectable where its footprint overlaps a partition.
    for (const footprint of plan.querySelectorAll(".object-footprint"))
      plan.append(footprint);
    const selected =
      selection?.kind === "object"
        ? project.objects.find((o) => o.id === selection.id)
        : null;
    if (selected) {
      const info = el("p", null, "selection-info");
      const dims = objectBounds(selected);
      info.textContent = `${dims.map(num).join(" × ")} m · ${selected.reference_only ? "Locked reference" : selected.collision ? "Solid box collision" : "Native components / visual geometry"}`;
      $("inspector").prepend(info);
      const actions = el("div", null, "selection-actions");
      actions.append(
        btn("Focus", focusSelection),
        btn("Rotate 15°", () => rotate(15)),
        btn("Duplicate", duplicate),
        btn("Delete", remove),
      );
      $("inspector").append(actions);
      checkClearance(selected);
    }
    if (["partition", "shell"].includes(selection?.kind)) {
      const key = selection.kind === "shell" ? "shell_rooms" : "partitions",
        index = project[key].findIndex((v) => v.id === selection.id);
      if (index >= 0) {
        $("inspector").replaceChildren(
          structuredEditor(
            project[key][index].id,
            project[key][index],
            (v) =>
              edit((p) => {
                const current = p[key].findIndex(
                  (item) => item.id === selection.id,
                );
                if (current < 0) throw Error("The selected item was removed");
                p[key][current] = v;
                ctx.select({ kind: selection.kind, id: v.id });
              }),
            () =>
              edit((p) => {
                p[key].splice(index, 1);
                ctx.select(null);
              }),
          ),
        );
        $("inspector").querySelector("details").open = true;
      }
    }
    if (selection?.kind === "wall") {
      const id = selection.id,
        device = project.door_devices?.[id] || {};
      $("inspector").replaceChildren(
        structuredEditor(
          "Wall opening",
          {
            treatment: project.openings[id] || "solid",
            initial_state: project.door_states?.[id] || "CLOSED",
            preset: device.preset || "sliding_entropy_c",
            offset: device.offset || [0, 0, 0],
            yaw_offset: device.yaw_offset || 0,
            scale: device.scale || [1, 1, 1],
          },
          (value) =>
            edit((p) => {
              if (value.treatment === "solid") delete p.openings[id];
              else p.openings[id] = value.treatment;
              p.door_devices ||= {};
              p.door_states ||= {};
              if (["door_auto", "door_manual"].includes(value.treatment)) {
                p.door_states[id] = value.initial_state;
                p.door_devices[id] = {
                  ...device,
                  preset: value.preset,
                  offset: value.offset,
                  yaw_offset: value.yaw_offset,
                  scale: value.scale,
                };
              } else {
                delete p.door_states[id];
                delete p.door_devices[id];
              }
            }),
          null,
          {
            choices: {
              treatment: [
                "solid",
                "doorway",
                "passage",
                "door_auto",
                "door_manual",
              ],
              preset: ["sliding_entropy_c", "hinged_h", "hinged_e"],
            },
          },
        ),
      );
      $("inspector").querySelector("details").open = true;
    }
    // Complete fields for technical items that previously had only partial controls.
    for (const [id, key] of [
      ["stairs-list", "stairs"],
      ["technical-list", null],
    ]) {
      const list = $(id);
      list.replaceChildren();
      for (const category of key ? [key] : ["lights", "audio_areas"])
        for (const [i, item] of (project[category] || []).entries())
          list.append(
            structuredEditor(
              item.id,
              item,
              (v) =>
                edit((p) => {
                  const current = p[category].findIndex(
                    (value) => value.id === item.id,
                  );
                  if (current < 0) throw Error("The selected item was removed");
                  p[category][current] = v;
                }),
              () =>
                edit((p) => {
                  p[category] = p[category].filter(
                    (value) => value.id !== item.id,
                  );
                }),
              {
                choices:
                  category === "audio_areas"
                    ? { preset: ["residential-ground", "residential-upper"] }
                    : undefined,
              },
            ),
          );
    }
    for (const w of project.workstations || [])
      for (const [kind, anchorId, data] of [
        ["Seat", w.chair, w.actor],
        ["Terminal", w.desk, w.computer],
      ]) {
        const o = project.objects.find((o) => o.id === anchorId);
        if (!o) continue;
        const a = (o.yaw * Math.PI) / 180,
          x =
            o.position[0] +
            data.offset[0] * Math.cos(a) -
            data.offset[1] * Math.sin(a),
          y =
            o.position[1] +
            data.offset[0] * Math.sin(a) +
            data.offset[1] * Math.cos(a);
        const marker = svg("g", {
          transform: `translate(${x},${-y}) rotate(${-(o.yaw + data.yaw_offset)})`,
          "pointer-events": "none",
        });
        marker.append(
          svg("circle", {
            r: kind === "Seat" ? 0.25 : 0.15,
            fill: kind === "Seat" ? "#f0bf70" : "#71cde0",
          }),
          svg("path", {
            d: "M 0 0 L 0 -.6",
            stroke: "#ffffff",
            "stroke-width": 0.04,
          }),
        );
        const t = svg("title", {});
        t.textContent =
          kind +
          " " +
          w.id +
          ` · height ${num(o.position[2] + data.offset[2])} m`;
        marker.append(t);
        plan.append(marker);
      }
    drawNavigation(project);
    if (lastRendered !== JSON.stringify(project))
      $("preview").classList.add("stale");
  }
  function openEditor(id) {
    document.querySelector('[data-panel="build"]').click();
    const d = [...$("construction-list").querySelectorAll("details")].find(
      (d) => d.querySelector("summary")?.textContent === id,
    );
    if (d) {
      d.open = true;
      d.scrollIntoView({ block: "nearest" });
    }
  }
  function checkClearance(o) {
    const note = el("p", null, "clearance-note");
    const [w, d] = objectBounds(o);
    const near = get()
      .project.objects.filter(
        (other) => other.id !== o.id && !other.reference_only,
      )
      .some((other) => {
        const [ow, od] = objectBounds(other);
        return (
          Math.abs(o.position[0] - other.position[0]) < (w + ow) / 2 &&
          Math.abs(o.position[1] - other.position[1]) < (d + od) / 2
        );
      });
    const center = geometryBounds(o).center;
    const h = objectBounds(o)[2],
      wall = get().compiled.solids.find((s) => {
        if (s.id === "object:" + o.id) return false;
        const a = ((o.yaw - s.yaw) * Math.PI) / 180,
          b = (-s.yaw * Math.PI) / 180,
          dx = center[0] - s.center[0],
          dy = center[1] - s.center[1];
        return (
          Math.abs(dx * Math.cos(b) - dy * Math.sin(b)) <
            (s.size[0] +
              w * Math.abs(Math.cos(a)) +
              d * Math.abs(Math.sin(a))) /
              2 -
              0.02 &&
          Math.abs(dx * Math.sin(b) + dy * Math.cos(b)) <
            (s.size[1] +
              w * Math.abs(Math.sin(a)) +
              d * Math.abs(Math.cos(a))) /
              2 -
              0.02 &&
          Math.abs(center[2] - s.center[2]) < (h + s.size[2]) / 2 - 0.02
        );
      });
    note.textContent = wall
      ? "Possible collision overlap: " +
        wall.id +
        ". Check the collision overlay and 3D placement."
      : near
        ? "Footprints overlap another object. Check 3D placement and collision."
        : "No overlapping object footprints at this position.";
    $("inspector").append(note);
  }
  function sceneBounds() {
    const { project } = get();
    const whole=project.shells?.length&&$("whole-building-preview")?.checked;
    let points = [];
    for (const r of project.rooms)
      if (whole||r.level === Number($("level").value))
        points.push(
          [r.x * 3, r.y * 3, r.level * ctx.height()],
          [
            (r.x + r.width) * 3,
            (r.y + r.depth) * 3,
            (r.level + 1) * ctx.height(),
          ],
        );
    for (const r of project.shell_rooms || [])
      if (whole||r.level === Number($("level").value)) {
        const a = (r.yaw * Math.PI) / 180;
        for (const [x, y] of [
          [0, 0],
          [r.size[0], 0],
          [0, r.size[1]],
          [r.size[0], r.size[1]],
        ])
          points.push(
            [
              r.position[0] + x * Math.cos(a) - y * Math.sin(a),
              r.position[1] + x * Math.sin(a) + y * Math.cos(a),
              r.position[2],
            ],
            [
              r.position[0] + x * Math.cos(a) - y * Math.sin(a),
              r.position[1] + x * Math.sin(a) + y * Math.cos(a),
              r.position[2] + r.size[2],
            ],
          );
      }
    for (const o of project.objects)
      if (whole||(o.level || 0) === Number($("level").value)) {
        const bounds = geometryBounds(o);
        points.push(bounds.low, bounds.high);
      }
    if (!points.length)
      points = [
        [0, 0, 0],
        [6, 6, 3],
      ];
    const low = [0, 1, 2].map((i) => Math.min(...points.map((p) => p[i]))),
      high = [0, 1, 2].map((i) => Math.max(...points.map((p) => p[i])));
    return {
      target: low.map((v, i) => (v + high[i]) / 2),
      distance: Math.max(
        2,
        Math.hypot(...low.map((v, i) => high[i] - v)) * 1.3,
      ),
    };
  }
  function ensureCamera() {
    cameraState ||= { ...sceneBounds(), azimuth: 45, elevation: 35 };
  }
  function resetCamera() {
    planView = null;
    redraw();
    cameraState = { ...sceneBounds(), azimuth: 45, elevation: 35 };
    $("interior-camera").checked = false;
    schedulePreview(true);
  }
  function focusSelection() {
    const { project, selection: s } = get();
    if (!s) {
      status("Select a room or object first");
      return;
    }
    if (s.kind === "wall") {
      const edge = get().compiled.edges.find((e) => e.id === s.id);
      if (!edge) return;
      ensureCamera();
      cameraState.target = [
        edge.x * 3 + (edge.axis === "x" ? 1.5 : 0),
        edge.y * 3 + (edge.axis === "y" ? 1.5 : 0),
        edge.level * ctx.height() + 1.4,
      ];
      cameraState.distance = 6;
      cameraState.azimuth = edge.axis === "x" ? 0 : 90;
      cameraState.elevation = 10;
      $("cutaway").checked = false;
      $("interior-camera").checked = false;
      schedulePreview(true);
      return;
    }
    const o = {
      object: project.objects,
      room: project.rooms,
      shell: project.shell_rooms,
      partition: project.partitions,
    }[s.kind]?.find((o) => o.id === s.id);
    if (!o) return;
    ensureCamera();
    if (s.kind === "object") {
      const dims = objectBounds(o);
      cameraState.target = geometryBounds(o).center;
      cameraState.distance = Math.max(1, Math.hypot(...dims) * 1.5);
    } else if (s.kind === "shell") {
      const angle = (o.yaw * Math.PI) / 180;
      cameraState.target = [
        o.position[0] +
          (o.size[0] / 2) * Math.cos(angle) -
          (o.size[1] / 2) * Math.sin(angle),
        o.position[1] +
          (o.size[0] / 2) * Math.sin(angle) +
          (o.size[1] / 2) * Math.cos(angle),
        o.position[2] + o.size[2] / 2,
      ];
      cameraState.distance = Math.hypot(...o.size) * 1.3;
    } else if (s.kind === "partition") {
      cameraState.target = o.from.map(
        (v, i) => (v + o.to[i]) / 2 + (i === 2 ? o.height / 2 : 0),
      );
      cameraState.distance = Math.max(
        2,
        Math.hypot(o.from[0] - o.to[0], o.from[1] - o.to[1], o.height) * 1.3,
      );
    } else {
      cameraState.target = [
        (o.x + o.width / 2) * 3,
        (o.y + o.depth / 2) * 3,
        o.level * ctx.height() + ctx.height() / 2,
      ];
      cameraState.distance =
        Math.hypot(o.width * 3, o.depth * 3, ctx.height()) * 1.3;
    }
    $("interior-camera").checked = false;
    schedulePreview(true);
    const r =
      s.kind === "object"
        ? geometryBounds(o).high.map((v, i) => v - geometryBounds(o).low[i])
        : s.kind === "shell"
          ? [
              Math.abs(o.size[0] * Math.cos((o.yaw * Math.PI) / 180)) +
                Math.abs(o.size[1] * Math.sin((o.yaw * Math.PI) / 180)),
              Math.abs(o.size[0] * Math.sin((o.yaw * Math.PI) / 180)) +
                Math.abs(o.size[1] * Math.cos((o.yaw * Math.PI) / 180)),
            ]
          : s.kind === "partition"
            ? [
                Math.abs(o.from[0] - o.to[0]) + 1,
                Math.abs(o.from[1] - o.to[1]) + 1,
              ]
            : [o.width * 3, o.depth * 3];
    const x = cameraState.target[0],
      y = cameraState.target[1];
    plan.setAttribute(
      "viewBox",
      `${x - r[0] / 2 - 1} ${-y - r[1] / 2 - 1} ${r[0] + 2} ${r[1] + 2}`,
    );
    planView = plan.getAttribute("viewBox");
    planLabels();
  }
  function planLabels() {
    const scale = plan.getScreenCTM()?.a || 1;
    for (const label of plan.querySelectorAll(".room-label"))
      label.style.fontSize = 12 / scale + "px";
  }
  function zoom(f) {
    if(ctx.viewport){ctx.viewport.zoom(f);return;}
    ensureCamera();
    cameraState.distance = Math.max(
      0.3,
      Math.min(1000, cameraState.distance * f),
    );
    schedulePreview(true);
  }
  function orbit(a, e) {
    if(ctx.viewport){ctx.viewport.orbit(a*Math.PI/180,-e*Math.PI/180);return;}
    ensureCamera();
    cameraState.azimuth += a;
    cameraState.elevation = Math.max(
      -15,
      Math.min(85, cameraState.elevation + e),
    );
    $("interior-camera").checked = false;
    schedulePreview(true);
  }
  function walkInside() {
    const { project, selection: s } = get();
    const shell = project.shell_rooms?.find((r) => r.id === s?.id);
    if (shell) {
      const a = (shell.yaw * Math.PI) / 180,
        convert = (x, y) => [
          shell.position[0] + x * Math.cos(a) - y * Math.sin(a),
          shell.position[1] + x * Math.sin(a) + y * Math.cos(a),
          shell.position[2] - Number($("level").value) * ctx.height() + 1.7,
        ];
      ctx.inside(
        convert(0.7, 0.7),
        convert(
          Math.max(0.8, shell.size[0] - 0.7),
          Math.max(0.8, shell.size[1] - 0.7),
        ),
      );
      $("interior-camera").checked = true;
      $("cutaway").checked = false;
      schedulePreview(true);
      return;
    }
    const r =
      project.rooms.find((r) => r.id === s?.id) ||
      project.rooms.find((r) => r.level === Number($("level").value));
    if (!r) {
      status(
        "Select a grid room to enter, or use the Interior camera coordinates",
      );
      return;
    }
    ctx.inside(
      [r.x * 3 + 0.7, r.y * 3 + 0.7, 1.7],
      [r.x * 3 + r.width * 3 - 0.7, r.y * 3 + r.depth * 3 - 0.7, 1.7],
    );
    $("interior-camera").checked = true;
    $("cutaway").checked = false;
    schedulePreview(true);
  }
  function camera() {
    ensureCamera();
    const a = (cameraState.azimuth * Math.PI) / 180,
      e = (cameraState.elevation * Math.PI) / 180,
      d = cameraState.distance,
      t = cameraState.target;
    return {
      perspective: {
        eye: [
          t[0] + Math.sin(a) * Math.cos(e) * d,
          t[2] + Math.sin(e) * d,
          -t[1] + Math.cos(a) * Math.cos(e) * d,
        ],
        target: [t[0], t[2], -t[1]],
        up: [0, 1, 0],
        vertical_fov: 0.85,
        near: 0.03,
        far: Math.max(500, d * 10),
      },
    };
  }
  function schedulePreview(force = false) {
    clearTimeout(previewTimer);
    if(ctx.viewport&&force){const view=camera().perspective;ctx.viewport.lookAt(view.eye,view.target);}
    if(ctx.viewport&&lastRendered===ctx.sceneKey()){
      return;
    }
    $("preview").classList.add("stale");
    if (!force && !$("auto-preview").checked) return;
    previewTimer = setTimeout(() => {
      if (!$("render").disabled && get().compiled.placements.length)
        $("render").click();
      else if ($("render").disabled) schedulePreview(force);
    }, 450);
  }
  function previewReady() {
    lastRendered = ctx.sceneKey?ctx.sceneKey():JSON.stringify(get().project);
    $("preview").classList.remove("stale");
  }
  function exportResult(result) {
    const root = $("export-result");
    const links = [...root.querySelectorAll("a")];
    const zip = links.find((a) => a.textContent.endsWith(".zip"));
    if (zip) {
      zip.className = "primary download-primary";
      zip.textContent = "Download complete building ZIP";
      root.prepend(zip);
    }
    const help = el("div", null, "export-guide");
    help.innerHTML =
      "<h3>Install and check in game</h3><ol><li>Extract the ZIP to a working folder. Keep the editable project for future changes.</li><li>Copy the exported <strong>.archive</strong> and <strong>.archive.xl</strong> files together into <strong>Cyberpunk 2077/archive/pc/mod</strong>.</li><li>Install ArchiveXL and its dependencies. Workstation quest fragments need integration in the owning quest.</li><li>Visit the world anchor. Check walking, stairs, doors, worker activation, streaming and save/load.</li></ol><p>Native readback passed. In-game behaviour has not been verified by this export.</p>";
    if (zip) zip.after(help);
    else root.prepend(help);
    const advanced = el("details");
    advanced.append(
      el(
        "summary",
        "Editable source, individual files and verification details",
      ),
    );
    for (const child of [...root.children])
      if (child !== zip && child !== help) advanced.append(child);
    root.append(advanced);
  }
  let navConnections;
  function drawNavigation(project) {
    if (!navConnections) {
      navConnections = el("div");
      $("navigation-fields").after(navConnections);
    }
    const data = { connections: project.navigation.connections || [] };
    navConnections.replaceChildren(
      structuredEditor("Doorway / navigation connections", data, (value) =>
        edit((p) => (p.navigation.connections = value.connections)),
      ),
    );
  }
  return {
    actorAppearance: (record) => actorAppearances.get(record),
    placementStarted,
    draw,
    schedulePreview,
    previewReady,
    camera,
    resetCamera,
    enlarge,
    remember: (m) => modules.set(m.id, m),
    exportResult,
  };
}

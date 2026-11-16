// Structured editors preserve native values while keeping serialization out of authoring.
const clone = (value) => structuredClone(value);
const title = (key) =>
  ({
    id: "Name",
    from: "Start · metres",
    to: "End · metres",
    position: "Position · metres",
    size: "Dimensions · metres",
    yaw: "Rotation · degrees",
    level: "Floor",
    start: "Distance along wall · metres",
    sill: "Sill height · metres",
    retained_sides: "Use existing shell surfaces",
    yaw_offset: "Rotation offset · degrees",
    offset: "Local offset · metres",
    active_on_start: "Activate worker on start",
    record: "Actor record",
    workspot: "Seat animation / workspot",
    resource_index: "Navigation resource index",
    height_range: "Height range · metres",
    outline: "Mask outline · local metres",
  })[key] || key.replaceAll("_", " ").replace(/^./, (s) => s.toUpperCase());
const choices = {
  preset: [
    "sliding_entropy_c",
    "hinged_h",
    "hinged_e",
    "sliding_kitsch_double_f",
    "garage_w300_h250",
  ],
  initial_state: ["CLOSED", "OPENED", "LOCKED"],
  axis: ["x", "y"],
  type: ["straight", "return"],
  anchor: ["base_center", "pivot"],
};
const templates = {
  openings: () => ({
    id: "opening_" + Date.now().toString(36),
    start: 0.5,
    width: 1,
    sill: 1,
    height: 1.2,
  }),
  outline: () => [0, 0],
  connections: () => ({
    id: "link_" + Date.now().toString(36),
    from: [0, 0, 0],
    to: [0, 1, 0],
  }),
};
const openEditors = new Set();
const drafts = new Map();
export function pendingEditors() {
  return [...drafts]
    .filter(([, draft]) => JSON.stringify(draft.value) !== draft.source)
    .map(([name]) => name);
}
export function clearEditorDrafts() {
  drafts.clear();
}
export function discardEditor(name) {
  drafts.delete(name);
}
function button(text, fn) {
  const b = document.createElement("button");
  b.type = "button";
  b.textContent = text;
  b.onclick = fn;
  return b;
}
export function structuredEditor(name, value, apply, remove, options = {}) {
  const box = document.createElement("details");
  box.className = "structured-editor";
  const summary = document.createElement("summary");
  summary.textContent = name;
  box.open = openEditors.has(name);
  box.ontoggle = () =>
    box.open ? openEditors.add(name) : openEditors.delete(name);
  const form = document.createElement("form");
  const source = JSON.stringify(value);
  let draft =
    drafts.get(name)?.source === source
      ? clone(drafts.get(name).value)
      : clone(value);
  const remember = () => drafts.set(name, { source, value: clone(draft) });
  form.addEventListener("input", remember);
  form.addEventListener("change", remember);
  const error = document.createElement("p");
  error.className = "form-error";
  error.setAttribute("role", "alert");
  function render() {
    form.replaceChildren();
    fields(form, draft, [], draft);
    const submit = document.createElement("button");
    submit.type = "submit";
    submit.className = "primary";
    submit.textContent = "Apply changes";
    form.append(error, submit);
    form.append(
      button("Discard changes", () => {
        drafts.delete(name);
        draft = clone(value);
        render();
      }),
    );
    if (remove)
      form.append(
        button("Remove " + name, () => {
          drafts.delete(name);
          remove();
        }),
      );
  }
  function fields(parent, obj, path, root) {
    for (const key of Object.keys(obj)) {
      const val = obj[key],
        here = [...path, key];
      if (key === "openings" && !Array.isArray(val)) {
        const group = document.createElement("fieldset");
        const legend = document.createElement("legend");
        legend.textContent = "Openings by wall";
        group.append(legend);
        obj[key].south ||= [];
        fields(group, obj[key], [...path, key], root);
        // Add other walls without requiring a map key to be typed.
        for (const side of ["south", "east", "north", "west"])
          if (!(side in val))
            group.append(
              button("Add " + side + " opening", () => {
                val[side] = [templates.openings()];
                remember();
                render();
              }),
            );
        parent.append(group);
        continue;
      }
      if (key === "retained_sides") {
        const group = document.createElement("fieldset");
        group.innerHTML = "<legend>Keep existing shell surfaces</legend>";
        for (const side of [
          "south",
          "east",
          "north",
          "west",
          "floor",
          "ceiling",
        ]) {
          const label = document.createElement("label"),
            input = document.createElement("input");
          input.type = "checkbox";
          input.checked = val.includes(side);
          input.onchange = () => {
            obj[key] = input.checked
              ? [...obj[key], side]
              : obj[key].filter((s) => s !== side);
          };
          label.append(input, document.createTextNode(title(side)));
          group.append(label);
        }
        parent.append(group);
        continue;
      }
      if (val && typeof val === "object") {
        const group = document.createElement("fieldset"),
          legend = document.createElement("legend");
        legend.textContent = title(key);
        group.append(legend);
        if (Array.isArray(val)) {
          const vector =
            val.length > 0 &&
            val.every((v) => typeof v === "number") &&
            !["actorDeletions", "instanceDeletions"].includes(key);
          if (vector) {
            group.className = "vector-fields";
            val.forEach((v, i) =>
              inputField(
                group,
                String(i),
                v,
                (n) => (val[i] = n),
                here,
                key === "color"
                  ? ["Red", "Green", "Blue"][i]
                  : key === "size"
                    ? ["Width", "Depth", "Height"][i]
                    : ["height_range"].includes(key)
                      ? ["Minimum", "Maximum"][i]
                      : ["X", "Y", "Z", "W"][i] || String(i + 1),
              ),
            );
          } else {
            val.forEach((v, i) => {
              const item = document.createElement("fieldset");
              item.innerHTML = "<legend>" + (i + 1) + "</legend>";
              if (Array.isArray(v) && v.every((n) => typeof n === "number")) {
                item.className = "vector-fields";
                v.forEach((n, j) =>
                  inputField(
                    item,
                    String(j),
                    n,
                    (number) => (v[j] = number),
                    [...here, String(i), String(j)],
                    ["X", "Y", "Z"][j],
                  ),
                );
              } else if (typeof v === "object") fields(item, v, here, root);
              else
                inputField(
                  item,
                  String(i),
                  v,
                  (n) => (val[i] = n),
                  here,
                  "Value",
                );
              item.append(
                button("Remove entry", () => {
                  val.splice(i, 1);
                  remember();
                  render();
                }),
              );
              group.append(item);
            });
            const template =
              templates[key] ||
              (here.includes("openings") ? templates.openings : null);
            if (template)
              group.append(
                button(
                  "Add " + (here.includes("openings") ? "opening" : "entry"),
                  () => {
                    val.push(template());
                    remember();
                    render();
                  },
                ),
              );
          }
        } else fields(group, val, here, root);
        parent.append(group);
      } else if (val !== null)
        inputField(parent, key, val, (n) => (obj[key] = n), here);
    }
  }
  function inputField(parent, key, value, set, path, labelText) {
    const label = document.createElement("label");
    label.append(document.createTextNode(labelText || title(key)));
    let input;
    const list = options.choices?.[key] || choices[key];
    if (list && list.includes(value)) {
      input = document.createElement("select");
      for (const v of list) input.add(new Option(title(v), v));
      input.value = value;
    } else if (key === "text") {
      input = document.createElement("textarea");
      input.rows = 4;
      input.value = value;
    } else {
      input = document.createElement("input");
      input.type =
        typeof value === "boolean"
          ? "checkbox"
          : typeof value === "number"
            ? "number"
            : "text";
      if (input.type === "checkbox") input.checked = value;
      else
        input.value =
          typeof value === "number" ? Number(value.toFixed(4)) : value;
      if (input.type === "number") {
        input.step = [
          "level",
          "resource_index",
          "placement_index",
          "expected_nodes",
        ].includes(key)
          ? "1"
          : "any";
        input.required = true;
        if (path.includes("color")) {
          input.min = "0";
          input.max = "255";
          input.step = "1";
        }
      }
    }
    input.name = path.join(".");
    if (key === "record") {
      input.placeholder = "Search native actors, e.g. Worker";
      input.onchange = () => {
        const appearance = options.actorAppearance?.(input.value);
        if (appearance && draft.actor) {
          draft.actor.appearance = appearance;
          remember();
          render();
        }
      };
    }
    input.oninput = () =>
      set(
        typeof value === "boolean"
          ? input.checked
          : typeof value === "number"
            ? Number(input.value)
            : input.value,
      );
    if (options.lists?.[key]) input.setAttribute("list", options.lists[key]);
    label.append(input);
    parent.append(label);
  }
  form.onsubmit = async (event) => {
    event.preventDefault();
    error.textContent = "";
    try {
      await apply(clone(draft));
      if (document.getElementById("status")?.classList.contains("error"))
        throw Error(document.getElementById("status").textContent);
      drafts.delete(name);
    } catch (e) {
      error.textContent = e.message;
      error.scrollIntoView({ block: "nearest" });
    }
  };
  render();
  box.append(summary, form);
  return box;
}

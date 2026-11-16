import { useEffect, useRef, useState } from "react";
import { Button, Input, Select } from "@ghostline/ui-core";
import { human, nameOf, object, write } from "./model";
import type { Json, Path } from "./model";
import { building as b } from "./store";
const choices: Record<string, string[]> = {
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
const drafts = new Map<string, { value: Json; source: string }>();
export function DraftEditor({
  name,
  value,
  apply,
  remove,
}: {
  name: string;
  value: Json;
  apply: (value: Json) => Promise<unknown> | void;
  remove?: () => Promise<unknown> | void;
}) {
  const [draft, setDraft] = useState(() =>
      structuredClone(drafts.get(name)?.value ?? value),
    ),
    [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    base = useRef(drafts.get(name)?.source ?? JSON.stringify(value));
  const source = JSON.stringify(value),
    dirty = JSON.stringify(draft) !== base.current;
  useEffect(() => {
    if (source !== base.current && !dirty) {
      base.current = source;
      setDraft(structuredClone(value));
    }
  }, [source, dirty]);
  useEffect(() => {
    if (dirty)
      drafts.set(name, { value: structuredClone(draft), source: base.current });
    else drafts.delete(name);
    b.draft(name, dirty);
  }, [name, dirty, draft]);
  return (
    <div className="space-y-4">
      <JsonFields value={draft} onChange={setDraft} />
      {dirty && (
        <p className="text-xs text-warning">
          Pending settings · apply or discard before saving or exporting.
        </p>
      )}
      {source !== base.current && dirty && (
        <p className="text-xs text-warning">
          The source changed while these settings were open. Discard to reload
          the current values.
        </p>
      )}
      <div className="flex flex-wrap gap-2">
        <Button
          variant="primary"
          disabled={!dirty || busy}
          onClick={() => {
            setBusy(true);
            setError("");
            Promise.resolve()
              .then(() => apply(draft))
              .then(() => {
                base.current = JSON.stringify(draft);
                drafts.delete(name);
                b.draft(name, false);
              })
              .catch((e) => setError(String(e)))
              .finally(() => setBusy(false));
          }}
        >
          Apply settings
        </Button>
        <Button
          disabled={busy || !dirty}
          onClick={() => {
            base.current = source;
            setDraft(structuredClone(value));
            drafts.delete(name);
            b.draft(name, false);
            setError("");
          }}
        >
          Discard settings
        </Button>
        {remove && (
          <Button
            variant="danger"
            disabled={busy}
            onClick={() =>
              b.run(async () => {
                await remove();
                drafts.delete(name);
                b.draft(name, false);
              })
            }
          >
            Remove
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="text-danger">
          {error}
        </p>
      )}
    </div>
  );
}
export function JsonFields({
  value,
  onChange,
  path = [],
}: {
  value: Json;
  onChange: (value: Json) => void;
  path?: Path;
}) {
  const change = (next: Json | undefined, key?: string | number): void => {
    if (key === undefined) {
      if (next !== undefined) onChange(next);
      return;
    }
    const copy = structuredClone(value);
    write(copy, [key], next);
    onChange(copy);
  };
  if (value !== null && typeof value === "object")
    return (
      <div className="space-y-3">
        {Object.entries(value).map(([key, entry]) =>
          entry === undefined ? null : (
            <div
              key={key}
              className="min-w-0 rounded border border-line/70 p-3"
            >
              {typeof entry === "object" && entry !== null ? (
                <details open={path.length < 1}>
                  <summary className="text-sm font-medium">
                    {Array.isArray(value)
                      ? `${Number(key) + 1} · ${nameOf(entry)}`
                      : human(key)}
                  </summary>
                  <div className="mt-3">
                    <JsonFields
                      value={entry}
                      path={[...path, key]}
                      onChange={(next) =>
                        change(next, Array.isArray(value) ? Number(key) : key)
                      }
                    />
                  </div>
                </details>
              ) : (
                <label className="grid gap-2 text-sm">
                  {Array.isArray(value) ? String(Number(key) + 1) : human(key)}
                  <JsonFields
                    value={entry}
                    path={[...path, key]}
                    onChange={(next) =>
                      change(next, Array.isArray(value) ? Number(key) : key)
                    }
                  />
                </label>
              )}
              {Array.isArray(value) && (
                <div className="mt-2 flex gap-2">
                  <Button
                    size="sm"
                    disabled={Number(key) === 0}
                    onClick={() => {
                      const next = [...value],
                        i = Number(key);
                      [next[i - 1], next[i]] = [next[i]!, next[i - 1]!];
                      onChange(next);
                    }}
                  >
                    ↑
                  </Button>
                  <Button
                    size="sm"
                    disabled={Number(key) === value.length - 1}
                    onClick={() => {
                      const next = [...value],
                        i = Number(key);
                      [next[i + 1], next[i]] = [next[i]!, next[i + 1]!];
                      onChange(next);
                    }}
                  >
                    ↓
                  </Button>
                  <Button
                    size="sm"
                    onClick={() => change(undefined, Number(key))}
                  >
                    Remove entry
                  </Button>
                </div>
              )}
            </div>
          ),
        )}
        {Array.isArray(value) ? (
          <Button
            size="sm"
            onClick={() => {
              const key = String(path.at(-1) || ""),
                defaults: Record<string, Json> = {
                  outline: [0, 0],
                  connections: {
                    id: crypto.randomUUID().slice(0, 8),
                    from: [0, 0, 0],
                    to: [0, 1, 0],
                  },
                  openings: {
                    id: crypto.randomUUID().slice(0, 8),
                    start: 0.5,
                    width: 1,
                    sill: 1,
                    height: 1.2,
                  },
                };
              onChange([
                ...value,
                defaults[key] ??
                  (typeof value[0] === "number"
                    ? 0
                    : typeof value[0] === "string"
                      ? ""
                      : {}),
              ]);
            }}
          >
            Add entry
          </Button>
        ) : (
          <AddProperty value={value} onChange={onChange} />
        )}
      </div>
    );
  if (typeof value === "boolean")
    return (
      <input
        type="checkbox"
        className="size-5 accent-accent"
        checked={value}
        onChange={(e) => change(e.target.checked)}
      />
    );
  const options = choices[String(path.at(-1))];
  if (options && typeof value === "string")
    return (
      <Select value={value} onChange={(e) => change(e.target.value)}>
        {!options.includes(value) && <option>{value}</option>}
        {options.map((o) => (
          <option key={o}>{o}</option>
        ))}
      </Select>
    );
  if (value === null)
    return (
      <Select
        value="null"
        onChange={(e) =>
          change(
            (
              {
                string: "",
                number: 0,
                object: {},
                array: [],
                boolean: false,
              } as Record<string, Json>
            )[e.target.value] ?? null,
          )
        }
      >
        {["null", "string", "number", "boolean", "object", "array"].map((o) => (
          <option key={o}>{o}</option>
        ))}
      </Select>
    );
  return typeof value === "number" ? (
    <NumberInput value={value} onChange={(next) => change(next)} />
  ) : (
    <Input value={value} onChange={(e) => change(e.target.value)} />
  );
}
function NumberInput({
  value,
  onChange,
}: {
  value: number;
  onChange: (v: number) => void;
}) {
  const [draft, setDraft] = useState(String(value)),
    [last, setLast] = useState(value);
  if (value !== last) {
    setLast(value);
    setDraft(String(value));
  }
  const valid = draft.trim() !== "" && Number.isFinite(Number(draft));
  return (
    <Input
      inputMode="decimal"
      value={draft}
      aria-invalid={!valid}
      onChange={(e) => {
        setDraft(e.target.value);
        if (
          e.target.value.trim() !== "" &&
          Number.isFinite(Number(e.target.value))
        )
          onChange(Number(e.target.value));
      }}
    />
  );
}
function AddProperty({
  value,
  onChange,
}: {
  value: Json;
  onChange: (v: Json) => void;
}) {
  const [name, setName] = useState(""),
    [type, setType] = useState("string");
  return (
    <details>
      <summary className="text-xs text-muted">Add optional setting</summary>
      <div className="mt-3 flex flex-wrap gap-2">
        <Input
          aria-label="Setting name"
          placeholder="Setting name"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <Select
          aria-label="Setting type"
          value={type}
          onChange={(e) => setType(e.target.value)}
        >
          {["string", "number", "boolean", "object", "array", "null"].map(
            (t) => (
              <option key={t}>{t}</option>
            ),
          )}
        </Select>
        <Button
          disabled={!name.trim() || Object.hasOwn(object(value), name.trim())}
          onClick={() => {
            const next = structuredClone(value);
            write(
              next,
              [name.trim()],
              (
                {
                  string: "",
                  number: 0,
                  boolean: false,
                  object: {},
                  array: [],
                  null: null,
                } as Record<string, Json>
              )[type]!,
            );
            onChange(next);
            setName("");
          }}
        >
          Add field
        </Button>
      </div>
    </details>
  );
}

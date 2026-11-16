import { useSyncExternalStore } from "react";
import { array, object, reconcileOpenings, write } from "./model";
import type {
  Bootstrap,
  Compiled,
  Json,
  Path,
  PreviewOptions,
  Project,
} from "./model";
export async function response(
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<Response> {
  const result = await fetch(path, {
    ...(body === undefined
      ? {}
      : {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify(body),
        }),
    ...(signal ? { signal } : {}),
  });
  if (!result.ok) {
    const value = await result.text();
    let message = value;
    try {
      message = (JSON.parse(value) as { error?: string }).error || value;
    } catch {}
    throw Error(message || result.statusText);
  }
  return result;
}
export async function api<T>(
  path: string,
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
  return (await (await response(path, body, signal)).json()) as T;
}
interface State {
  bootstrap: Bootstrap | null;
  compiled: Compiled | null;
  history: Project[];
  future: Project[];
  selection: Path;
  message: string;
  error: boolean;
  busy: boolean;
  pending: string[];
  saved: string | null;
  preview: string | null;
  previewKey: string;
  previewing: boolean;
}
class BuildingStore {
  private state: State = {
    bootstrap: null,
    compiled: null,
    history: [],
    future: [],
    selection: [],
    message: "Starting editor…",
    error: false,
    busy: false,
    pending: [],
    saved: null,
    preview: null,
    previewKey: "",
    previewing: false,
  };
  private listeners = new Set<() => void>();
  private queue: Promise<unknown> = Promise.resolve();
  private previewRequest = 0;
  getSnapshot = (): State => this.state;
  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  };
  update = (patch: Partial<State>): void => {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((fn) => fn());
  };
  notify = (message: string, error = false): void =>
    this.update({ message, error });
  report = (error: unknown): void =>
    this.notify(error instanceof Error ? error.message : String(error), true);
  run = (action: () => unknown | Promise<unknown>): void => {
    Promise.resolve().then(action).catch(this.report);
  };
  private enqueue<T>(action: () => Promise<T>): Promise<T> {
    const pending = this.queue
      .catch(() => undefined)
      .then(async () => {
        this.update({ busy: true });
        try {
          return await action();
        } finally {
          this.update({ busy: false });
        }
      });
    this.queue = pending;
    return pending;
  }
  get project(): Project {
    const project = this.state.compiled?.project;
    if (!project) throw Error("Wait for the project to load.");
    return project;
  }
  async boot(): Promise<void> {
    const bootstrap = await api<Bootstrap>("/api/bootstrap"),
      compiled = await api<Compiled>("/api/compile", bootstrap.starter);
    this.update({
      bootstrap,
      compiled,
      message: "Ready · starter anchor is an isolated test position",
    });
  }
  transact(change: (project: Project) => void): Promise<void> {
    return this.enqueue(async () => {
      const previous = this.project,
        next = structuredClone(previous);
      change(next);
      next.revision = previous.revision + 1;
      reconcileOpenings(next);
      const compiled = await api<Compiled>("/api/compile", next);
      this.update({
        compiled,
        history: [...this.state.history, previous].slice(-100),
        future: [],
        message: "Layout updated",
        error: false,
      });
    });
  }
  set(path: Path, value: Json): Promise<void> {
    return this.transact((project) => {
      write(project, path, value);
    });
  }
  accept(compiled: Compiled): void {
    this.update({
      compiled,
      history: this.state.compiled ? [...this.state.history, this.project] : [],
      future: [],
      selection: [],
      message: "Layout updated",
      error: false,
    });
  }
  async load(project: Project, saved = false): Promise<void> {
    this.checkDrafts();
    await this.enqueue(async () => {
      const compiled = await api<Compiled>("/api/compile", project);
      this.accept(compiled);
      this.update({
        saved: saved ? JSON.stringify(compiled.project) : null,
        pending: [],
      });
    });
  }
  undo(redo = false): Promise<void> {
    this.checkDrafts();
    return this.enqueue(async () => {
      const source = [...(redo ? this.state.future : this.state.history)],
        next = source.pop();
      if (!next) return;
      const compiled = await api<Compiled>("/api/compile", next),
        other = [
          ...(redo ? this.state.history : this.state.future),
          this.project,
        ];
      this.update({
        compiled,
        history: redo ? other : source,
        future: redo ? source : other,
        selection: [],
        message: "History restored",
      });
    });
  }
  draft(name: string, dirty: boolean): void {
    const next = this.state.pending.filter((v) => v !== name);
    if (dirty) next.push(name);
    if (next.join("\0") !== this.state.pending.join("\0"))
      this.update({ pending: next });
  }
  checkDrafts(): void {
    if (this.state.pending.length)
      throw Error(
        "Apply or discard pending settings first: " +
          this.state.pending.join(", "),
      );
  }
  async save(): Promise<void> {
    this.checkDrafts();
    await this.enqueue(async () => {
      await api("/api/save", this.project);
      const bootstrap = await api<Bootstrap>("/api/bootstrap");
      this.update({
        saved: JSON.stringify(this.project),
        bootstrap,
        message: "Project saved",
        error: false,
      });
    });
  }
  preview(options: PreviewOptions): Promise<void> {
    this.checkDrafts();
    const request = ++this.previewRequest,
      project = structuredClone(this.project),
      key = JSON.stringify([project, options]);
    this.update({ previewing: true });
    return response("/api/preview", { project, options })
      .then(async (result) => {
        const blob = await result.blob();
        if (
          request !== this.previewRequest ||
          JSON.stringify(this.project) !== JSON.stringify(project)
        )
          return;
        const preview = URL.createObjectURL(blob);
        if (this.state.preview) URL.revokeObjectURL(this.state.preview);
        this.update({
          preview,
          previewKey: key,
          message: "Interactive scene ready · revision " + project.revision,
          error: false,
        });
      })
      .finally(() => {
        if (request === this.previewRequest) this.update({ previewing: false });
      });
  }
  async export(): Promise<Json> {
    this.checkDrafts();
    return this.enqueue(() => api<Json>("/api/export", this.project));
  }
  add(key: string, value: Json): Promise<void> {
    return this.transact((p) => {
      if (key === "door_devices") {
        object(p.door_devices)[crypto.randomUUID().slice(0, 8)] = value;
      } else {
        const rows = array(p[key]);
        p[key] = rows;
        rows.push(value);
      }
    });
  }
}
export const building = new BuildingStore();
export const useBuilding = () =>
  useSyncExternalStore(building.subscribe, building.getSnapshot);

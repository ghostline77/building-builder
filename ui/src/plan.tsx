import { useEffect, useRef, useState } from "react";
import { Button } from "@ghostline/ui-core";
import { array, at, object, text, uid } from "./model";
import type { Json, LibraryModule, Path, Room } from "./model";
import { building as b, useBuilding } from "./store";
export interface Pending {
  module: LibraryModule;
  appearance: string;
  yaw: number;
}
interface Drag {
  start: [number, number];
  current: [number, number];
  path: Path;
  value: Json | undefined;
  tool: string;
}
export function Plan({
  level,
  tool,
  snap,
  pending,
  setPending,
  setWall,
  collision,
  route = [],
}: {
  level: number;
  tool: string;
  snap: number;
  pending: Pending | null;
  setPending: (v: Pending | null) => void;
  setWall: (v: string) => void;
  collision: boolean;
  route?: Json[];
}) {
  const { compiled, selection } = useBuilding(),
    project = compiled!.project,
    height =
      b.getSnapshot().bootstrap?.kits.find((k) => k.id === project.kit)
        ?.storey_height || 4.2;
  const [drag, setDrag] = useState<Drag | null>(null),
    [hover, setHover] = useState<[number, number]>([0, 0]),
    [zoom, setZoom] = useState(1),
    svg = useRef<SVGSVGElement>(null);
  const rooms = project.rooms.filter((r) => r.level === level),
    objects = project.objects.filter(
      (o) =>
        Number(o.level ?? Math.floor((o.position[2] || 0) / height)) === level,
    );
  const points = [
    ...rooms.flatMap((r) => [
      [r.x * 3, r.y * 3],
      [(r.x + r.width) * 3, (r.y + r.depth) * 3],
    ]),
    ...objects.map((o) => o.position),
    ...array(project.shell_rooms)
      .filter((r) => object(r).level === level)
      .flatMap((r) => {
        const v = object(r),
          p = array(v.position),
          s = array(v.size);
        return [
          [Number(p[0]), Number(p[1])],
          [Number(p[0]) + Number(s[0]), Number(p[1]) + Number(s[1])],
        ];
      }),
  ];
  const lowX = Math.min(-1, ...points.map((p) => Number(p[0]) - 1)),
    lowY = Math.min(-1, ...points.map((p) => Number(p[1]) - 1)),
    highX = Math.max(10, ...points.map((p) => Number(p[0]) + 2)),
    highY = Math.max(10, ...points.map((p) => Number(p[1]) + 2)),
    width = (highX - lowX) / zoom,
    heightView = (highY - lowY) / zoom,
    cx = (lowX + highX) / 2,
    cy = (lowY + highY) / 2;
  const point = (clientX: number, clientY: number): [number, number] => {
    const matrix = svg.current?.getScreenCTM();
    if (!matrix) return [0, 0];
    const p = new DOMPoint(clientX, clientY).matrixTransform(matrix.inverse());
    return [p.x, -p.y];
  };
  useEffect(() => {
    const host = svg.current;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      setZoom((z) =>
        Math.min(10, Math.max(0.1, z * (e.deltaY < 0 ? 1.15 : 1 / 1.15))),
      );
    };
    host?.addEventListener("wheel", wheel, { passive: false });
    return () => host?.removeEventListener("wheel", wheel);
  }, []);
  const place = (p: [number, number]): void => {
    if (!pending) return;
    const item = pending;
    setPending(null);
    b.run(() =>
      b.transact((next) => {
        const id = uid("object");
        next.objects.push({
          id,
          asset: item.module.id,
          appearance: item.appearance,
          position: [
            Math.round(p[0] / snap) * snap,
            Math.round(p[1] / snap) * snap,
            level * height,
          ],
          yaw: item.yaw,
          anchor: "base_center",
          collision: true,
          source_content_sha256: item.module.source_content_sha256 || null,
        });
        b.update({ selection: ["objects", next.objects.length - 1] });
      }),
    );
  };
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      )
        return;
      if (e.key === "Escape") {
        setPending(null);
        setDrag(null);
      }
      if (pending && e.key.toLowerCase() === "r")
        setPending({ ...pending, yaw: (pending.yaw + 15) % 360 });
      if (pending && e.key === "Enter") {
        e.preventDefault();
        const room = rooms[0];
        place(
          room
            ? [(room.x + room.width / 2) * 3, (room.y + room.depth / 2) * 3]
            : [1.5, 1.5],
        );
      }
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, [pending, level]);
  const selected = (path: Path) =>
    JSON.stringify(path) === JSON.stringify(selection);
  return (
    <div className="space-y-3">
      <div className="flex flex-wrap gap-2">
        <Button size="sm" onClick={() => setZoom(1)}>
          Fit plan
        </Button>
        <Button
          size="sm"
          aria-label="Zoom plan in"
          onClick={() => setZoom((z) => Math.min(10, z * 1.2))}
        >
          +
        </Button>
        <Button
          size="sm"
          aria-label="Zoom plan out"
          onClick={() => setZoom((z) => Math.max(0.1, z / 1.2))}
        >
          −
        </Button>
        {pending && (
          <>
            <p className="flex-1 text-sm text-accent">
              Placing {pending.module.name} · click plan · R rotate · Enter room
              centre
            </p>
            <Button size="sm" onClick={() => setPending(null)}>
              Cancel placement
            </Button>
          </>
        )}
      </div>
      <svg
        ref={svg}
        viewBox={`${cx - width / 2} ${-cy - heightView / 2} ${width} ${heightView}`}
        role="img"
        aria-label="Building plan. Select rooms and wall bays."
        className="h-[420px] w-full touch-none rounded-xl border border-line bg-background"
        onPointerDown={(e) => {
          if (pending) {
            setHover(point(e.clientX, e.clientY));
            return;
          }
          const pathText =
              e.target instanceof Element
                ? e.target.closest("[data-path]")?.getAttribute("data-path")
                : null,
            path: Path = pathText ? (JSON.parse(pathText) as Path) : [],
            value = at(project, path);
          if (path.length) b.update({ selection: path });
          if (tool === "select") return;
          if (["move", "resize"].includes(tool) && path[0] !== "rooms") return;
          if (
            tool.startsWith("object-") &&
            (path[0] !== "objects" || object(value).reference_only)
          )
            return;
          e.currentTarget.setPointerCapture(e.pointerId);
          const p = point(e.clientX, e.clientY);
          setDrag({
            start: p,
            current: p,
            path,
            value: structuredClone(value),
            tool,
          });
        }}
        onPointerMove={(e) => {
          const p = point(e.clientX, e.clientY);
          setHover(p);
          if (drag) setDrag({ ...drag, current: p });
        }}
        onPointerCancel={() => setDrag(null)}
        onPointerUp={(e) => {
          if (pending) {
            place(point(e.clientX, e.clientY));
            return;
          }
          const d = drag;
          setDrag(null);
          if (!d) return;
          const end = point(e.clientX, e.clientY),
            dx = end[0] - d.start[0],
            dy = end[1] - d.start[1];
          if (Math.abs(dx) + Math.abs(dy) < 0.03) return;
          b.run(() =>
            b.transact((next) => {
              if (d.tool === "draw") {
                const x = Math.floor(Math.min(d.start[0], end[0]) / 3),
                  y = Math.floor(Math.min(d.start[1], end[1]) / 3);
                next.rooms.push({
                  id: uid("room"),
                  x,
                  y,
                  width: Math.max(
                    1,
                    Math.ceil(Math.max(d.start[0], end[0]) / 3) - x,
                  ),
                  depth: Math.max(
                    1,
                    Math.ceil(Math.max(d.start[1], end[1]) / 3) - y,
                  ),
                  level,
                });
              } else if (d.tool === "partition") {
                const walls = array(next.partitions);
                next.partitions = walls;
                walls.push({
                  id: uid("partition"),
                  from: [
                    Math.round(d.start[0] / snap) * snap,
                    Math.round(d.start[1] / snap) * snap,
                    level * height,
                  ],
                  to: [
                    Math.round(end[0] / snap) * snap,
                    Math.round(end[1] / snap) * snap,
                    level * height,
                  ],
                  height: 3,
                  thickness: 0.12,
                  level,
                  openings: [],
                });
              } else if (d.path[0] === "rooms") {
                const room = next.rooms[Number(d.path[1])]!,
                  old = d.value as Room;
                if (d.tool === "move") {
                  room.x = old.x + Math.round(dx / 3);
                  room.y = old.y + Math.round(dy / 3);
                } else {
                  room.width = Math.max(1, Math.round(end[0] / 3) - room.x);
                  room.depth = Math.max(1, Math.round(end[1] / 3) - room.y);
                }
              } else if (d.path[0] === "objects") {
                const item = next.objects[Number(d.path[1])]!,
                  old = object(d.value),
                  p = array(old.position);
                if (d.tool === "object-move")
                  item.position = [
                    Math.round((Number(p[0]) + dx) / snap) * snap,
                    Math.round((Number(p[1]) + dy) / snap) * snap,
                    Number(p[2]),
                  ];
                else
                  item.yaw =
                    Number(old.yaw) +
                    Math.round(
                      ((Math.atan2(
                        end[1] - Number(p[1]),
                        end[0] - Number(p[0]),
                      ) -
                        Math.atan2(
                          d.start[1] - Number(p[1]),
                          d.start[0] - Number(p[0]),
                        )) *
                        180) /
                        Math.PI /
                        15,
                    ) *
                      15;
              }
            }),
          );
        }}
      >
        <defs>
          <pattern
            id="building-grid"
            width="3"
            height="3"
            patternUnits="userSpaceOnUse"
          >
            <path
              d="M3 0 H0 V3"
              fill="none"
              stroke="#343a36"
              strokeWidth=".025"
            />
          </pattern>
        </defs>
        <rect
          x={cx - width / 2}
          y={-cy - heightView / 2}
          width={width}
          height={heightView}
          fill="url(#building-grid)"
        />
        {rooms.map((room) => {
          const index = project.rooms.indexOf(room),
            path: Path = ["rooms", index];
          return (
            <g
              key={room.id}
              data-path={JSON.stringify(path)}
              role="button"
              tabIndex={0}
              aria-label={room.id}
              onKeyDown={(e) => {
                if (e.key === "Enter") b.update({ selection: path });
              }}
            >
              <rect
                x={room.x * 3}
                y={-(room.y + room.depth) * 3}
                width={room.width * 3}
                height={room.depth * 3}
                fill={selected(path) ? "#c6ee8335" : "#70908020"}
                stroke={selected(path) ? "#c6ee83" : "#657169"}
                strokeWidth=".06"
              />
              <text
                x={(room.x + room.width / 2) * 3}
                y={-(room.y + room.depth / 2) * 3}
                textAnchor="middle"
                fill="#c8d0c8"
                fontSize=".28"
                pointerEvents="none"
              >
                {room.id} · {room.width * 3} × {room.depth * 3} m
              </text>
            </g>
          );
        })}
        {compiled!.edges
          .filter((edge) => edge.level === level)
          .map((edge) => (
            <g
              key={edge.id}
              tabIndex={0}
              role="button"
              aria-label={`Wall ${edge.id}`}
              onClick={(e) => {
                e.stopPropagation();
                setWall(edge.id);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") setWall(edge.id);
              }}
              className="cursor-pointer"
            >
              <rect
                x={edge.x * 3 - 0.15}
                y={-edge.y * 3 - (edge.axis === "y" ? 3 : 0) - 0.15}
                width={edge.axis === "x" ? 3.3 : 0.3}
                height={edge.axis === "y" ? 3.3 : 0.3}
                fill="transparent"
              />
              <line
                x1={edge.x * 3}
                y1={-edge.y * 3}
                x2={edge.x * 3 + (edge.axis === "x" ? 3 : 0)}
                y2={-edge.y * 3 - (edge.axis === "y" ? 3 : 0)}
                stroke={project.openings[edge.id] ? "#d4ae78" : "#94a88a"}
                strokeWidth=".12"
                pointerEvents="none"
              />
            </g>
          ))}
        {array(project.shell_rooms)
          .filter((v) => object(v).level === level)
          .map((v) => {
            const r = object(v),
              p = array(r.position),
              s = array(r.size),
              index = array(project.shell_rooms).indexOf(v);
            return (
              <rect
                key={text(r.id)}
                data-path={JSON.stringify(["shell_rooms", index])}
                x={Number(p[0])}
                y={-Number(p[1]) - Number(s[1])}
                width={Number(s[0])}
                height={Number(s[1])}
                transform={`rotate(${-Number(r.yaw)} ${Number(p[0])} ${-Number(p[1])})`}
                fill="#8abfd515"
                stroke="#8abfd5"
                strokeWidth=".05"
              />
            );
          })}
        {array(project.partitions)
          .filter((v) => object(v).level === level)
          .map((v) => {
            const r = object(v),
              a = array(r.from),
              z = array(r.to),
              index = array(project.partitions).indexOf(v);
            return (
              <line
                key={text(r.id)}
                data-path={JSON.stringify(["partitions", index])}
                x1={Number(a[0])}
                y1={-Number(a[1])}
                x2={Number(z[0])}
                y2={-Number(z[1])}
                stroke="#c6ee83"
                strokeWidth={Number(r.thickness || 0.12)}
              />
            );
          })}
        {objects.map((o) => {
          const index = project.objects.indexOf(o),
            path: Path = ["objects", index],
            p = compiled!.placements.find(
              (p) => p.id === o.id || p.id.startsWith(o.id + ":"),
            ),
            dims = p?.bounds.dimensions || [0.6, 0.6, 1],
            w = Math.max(0.1, dims[0] || 0.6),
            d = Math.max(0.1, dims[1] || 0.6);
          return (
            <g
              key={o.id}
              data-path={JSON.stringify(path)}
              transform={`translate(${o.position[0]},${-Number(o.position[1])}) rotate(${-o.yaw})`}
              role="button"
              tabIndex={0}
              aria-label={o.id}
              onKeyDown={(e) => {
                if (e.key === "Enter") b.update({ selection: path });
              }}
            >
              <rect
                x={-w / 2}
                y={-d / 2}
                width={w}
                height={d}
                fill={
                  o.reference_only
                    ? "#83b9e215"
                    : selected(path)
                      ? "#c6ee8380"
                      : "#c6ee8330"
                }
                stroke={
                  selected(path)
                    ? "#fff"
                    : o.reference_only
                      ? "#83b9e2"
                      : "#c6ee83"
                }
                strokeWidth=".05"
              />
              <title>
                {o.id} · {o.asset}
              </title>
            </g>
          );
        })}
        {collision &&
          compiled!.solids.map((s) => (
            <rect
              key={s.id}
              x={Number(s.center[0]) - Number(s.size[0]) / 2}
              y={-Number(s.center[1]) - Number(s.size[1]) / 2}
              width={Number(s.size[0])}
              height={Number(s.size[1])}
              transform={`rotate(${-s.yaw} ${s.center[0]} ${-Number(s.center[1])})`}
              fill="#ed8d8d20"
              stroke="#ed8d8d"
              strokeWidth=".025"
              pointerEvents="none"
            />
          ))}
        {route.map((t, i) => (
          <polygon
            key={i}
            points={array(t)
              .map((p) => `${Number(array(p)[0])},${-Number(array(p)[1])}`)
              .join(" ")}
            fill="#80ccf555"
            stroke="#80ccf5"
            strokeWidth=".02"
            pointerEvents="none"
          />
        ))}
        {pending && (
          <rect
            x={hover[0] - 0.4}
            y={-hover[1] - 0.4}
            width=".8"
            height=".8"
            fill="#c6ee8360"
            stroke="#c6ee83"
            strokeWidth=".04"
            pointerEvents="none"
          />
        )}
        {drag && (
          <rect
            x={Math.min(drag.start[0], drag.current[0])}
            y={-Math.max(drag.start[1], drag.current[1])}
            width={Math.max(0.05, Math.abs(drag.start[0] - drag.current[0]))}
            height={Math.max(0.05, Math.abs(drag.start[1] - drag.current[1]))}
            fill="#c6ee8333"
            stroke="#c6ee83"
            strokeWidth=".04"
            pointerEvents="none"
          />
        )}
      </svg>
      <p className="text-xs text-muted">
        PLAN / metres · RED X–Y · drag to draw, move or resize · scroll to zoom
      </p>
    </div>
  );
}

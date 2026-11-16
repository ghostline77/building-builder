# Workflow verification

New placements use the published items catalogue, including kit pieces.
Imported world references are read directly from their source-pinned sectors.
`data/kits.json` contains construction rules only.

| Workflow | Implementation and checks |
| --- | --- |
| Projects, save/open, JSON import/download, undo/redo | Shared validated editor transactions |
| Structured authoring | Forms for partitions, apertures, shell rooms, doors, stairs, lights, ambience, navigation links, terrain masks and workstations; Apply/Discard and draft validation |
| Plan and preview controls | Native footprints, snapped object dragging, rotation, duplicate/delete, partition endpoints, collision overlays, orbit/pan/zoom, focus and interior views |
| Catalogue browsing | Independent category/completeness filters, family grouping, enlarged images and native measurement |
| Guided world editing | Nearby native sector lookup, selectable survey markers, instance/actor/shape selections and explicit occluder-group acknowledgement |
| Construction kits | All 15 compile against catalogue assets and native geometry |
| Object library | Complete published props collection, appearances, previews and paged search |
| Complete furniture assemblies | Native entities retain rigs, attachments and component bindings |
| Draw, move and resize rooms on the plan | Three-metre snapping; checked in the in-app browser |
| Multiple floors, shared partitions, openings and lowered ceilings | Shared visual/collision placement compiler |
| Independent partitions and window apertures | Metre-based panels; collision-volume and open-aperture checks |
| Measured rooms and shell lining | Rotated arbitrary-size rooms, retained walls, floor/ceiling panels and openings |
| Straight/return stairs and enclosure joins | Shafts, treads, guards and landings across adjoining rooms; route/topology checks |
| Door presets | Entropy sliding, hinged H/E, double kitsch and garage devices |
| Scaled garage rigs | Native root-scale edit in `ghostline-red`, frame/collider instance overrides, native readback |
| Door state and traversal links | Closed/opened/locked state and explicit approach points; locked-route checks |
| Practical lighting, interiors and ambience | Native nodes and corresponding GPU lighting |
| Generated Human navigation | Clearance surfaces, rotated shell rooms, stairs, entrance links and native VAND tiles |
| Terrain navigation replacement | Source-pinned native resource; only selected polygon flags change |
| Vanilla shell survey/import | Static and instanced transforms, normalized quaternions and scale; locked preview-only references |
| Grouped retrofit workflow | Location picker, layered native inventory and plan selection, retained multi-storey floor boundaries, whole-building preview, Existing/Proposed/Changes and reversible source changes |
| Structural retrofit operations | Native rigid mesh cuts across LODs, box fragments, cooked triangle/convex and angled-box cuts, preserved physics bindings, native door pivots, partitions and measured floor connections |
| Multiple retained navigation tiles | Operation-owned regions, combined edits on shared tiles, independent restoration, per-tile native readback and packed-resource checks |
| Facade, collision and occlusion patches | Source-hash/placement-count checks, render-instance and actor/shape selections, explicit occluder groups |
| Workstation actors and terminals | Streamed communities, persistent seat workspots, native computers and document-read facts |
| Quest integration | Exported actor/terminal bindings, area trigger and quest-building fragment |
| Native preview | Shared assets and retained GPU scenes; representative furniture and scaled-garage images inspected |
| Native archive and editable ZIP | Sector/block/device/rig/navigation readback and packed-resource byte verification |

The combined export fixture exercises partitions, shell rooms, hinged/double/garage
doors, a scaled rig, a workstation, its terminal, and navigation in one package.
Separate native checks cover surveyed instance import, source-hash rejection,
visual/collision patch registration, and retained terrain buffers. Compiler tests
check route connectivity and geometry effects rather than snapshots of generated
implementation details.

## Runtime acceptance

The AKPP acceptance fixture retains twelve measured visual pieces and two
interior floor boundaries without emitting duplicate shell geometry. Its native
GPU preview uses the installed material bindings, including native
first-appearance fallback. A separate AKPP facade cut passes mesh/resource
readback and archive byte checks.

The Rancho acceptance export cuts one render instance and one box shape in a
128-actor source node, writes one replacement mesh and four full-quaternion
collision fragments, and verifies all four packed resources. Its ArchiveXL
patch selects instance 0 of four and actor/shape [119, 0]; neighbouring instances
and actors remain selected by neither patch. Existing/proposed GPU images were
inspected. Door preview preparation omits occlusion-only mesh components.

The browser acceptance run used the complete guided import, created an AKPP
entrance from a facade piece, its two matching collision shapes and its occluder
group, then saved and exported the project. That export contains one replacement
mesh, eight collision fragments and four verified resources across two source
sector patches.

Installed triangle and convex collision resources were decoded, cut and exported
together as mod-owned native entities. The acceptance package verifies five
resources, including both collider entities, with original physics material and
filter-preset identities. Geometry checks cover retained volume, an open aperture,
closed convex collision reveals and cooked topology/materials.

A separate native acceptance package replaces two Human navigation resources,
verifies all five packed resources, and records only polygon-flag changes in the
retained buffers. A second operation on a shared tile remains active after the
first is restored; both retained resources still export. Generated navigation
checks a blocked wall and the traversable cut aperture.

Rigid visual surface cuts retain source faces; use authored lining or native door
frames where wall thickness is exposed.

Native readback and package checks do not establish in-game behaviour. Each
export reports `runtime_verified: false` until its specific archive has been
playtested. Walking, door interaction, worker spawning, terminal facts, streaming
and save/load are the acceptance checks in [exporting](exporting.md). The current
implementation checks above are offline/GPU checks; no in-game result is claimed.

For usage, start with [getting started](getting-started.md),
[authoring](authoring.md), or [world editing](world-editing.md).

# Architecture and data ownership

The builder owns building rules, authored placements, editor interactions, world
patch selections, and export composition. `ghostline-red` owns native archive and
CR2W access; `renderer` owns shared mesh/material preparation and GPU rendering.
There is no Python or WolvenKit runtime dependency.

`data/kits.json` is the single construction-kit contract. It contains dimensions,
native piece paths, chosen finishes, and pivot offsets. It is not an inventory,
and carries no extraction provenance. There is no separate common-kit file.

The published items package owns asset identity, membership, native appearances,
availability diagnostics, preview recipes, and media. The builder keeps no embedded
module or prop catalogue. It reads the current published package at startup and
measures selected native geometry on demand. Restart after republishing items.
Kit pieces resolve through the same catalogue as placed furniture.

World surveys resolve 2.5 cooked mesh and collision records by the placement's
native `InstanceIndex`. Instanced mesh groups use their own mesh, appearance and
transform range. The shared native reader owns these layouts; source selections
retain cooked class names and placement indices for ArchiveXL patch validation.

`Library` is a session cache of the geometry metadata needed by the current
projects, not a separately generated dataset. Pure compiler tests use an analytic
fixture rather than installing a hidden fallback inventory. Missing catalogue
entries and native resource failures are explicit errors.

Preview and export consume the same compiled transforms. Native entity assemblies
retain their rig and attachment hierarchy. Repeated meshes and prepared materials
are shared, and transform-only edits update the retained GPU scene. Preview uses
native materials with studio/practical lighting; it does not reproduce the game's
full shadowing, reflections, indirect lighting, or post-processing.

The API and renderer share one archive source through `Arc`; compilation, surveys
and export do not take the GPU session lock. Preview requests run separately from
the editor transaction queue and coalesce updates while a frame is being prepared.
`ui/forms.js` owns transactional settings forms and draft recovery;
`ui/studio.js` owns camera, plan manipulation and catalogue presentation.
Developer servers can set `GHOSTLINE_BUILDING_UI` to this package's `ui` directory
to serve edits directly; release binaries otherwise use their bundled interface.

| Source | Responsibility |
| --- | --- |
| `model.rs`, `interiors.rs`, `stairs.rs` | Authored schema and geometry/collision composition |
| `catalogue.rs` | Items package adapter and selected-asset metadata cache |
| `technical.rs`, `encounters.rs` | Doors, areas, lights, audio, workers and terminals |
| `retrofit.rs`, `terrain.rs` | Native surveys, shell references and retained-navigation patches |
| `navigation.rs` | Generated Human surfaces, route queries and tiled VAND data |
| `preview.rs` | Retained renderer scene |
| `export.rs`, `bundle.rs` | Native resources, readback and portable packaging |
| `server.rs`, `ui/` | Local same-origin API and browser editor |

Game resources are read-only. Work, projects, renders and exports belong under
ignored `work/`. Tests, clippy and targeted formatting operate on this package;
do not format sibling repositories as part of builder maintenance.

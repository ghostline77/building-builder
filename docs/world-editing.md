# Survey and edit an existing shell

Choose **Start project → Retrofit existing building**. Select a measured site or
enter world XYZ, yaw and survey radius. A pasted viewpoint can supply XYZ.
The survey inventories structure, furnishings, entities, collision shapes,
occluders and navigation across nearby sectors.

Select the building's pieces on the plan or in the layer lists. Select only the
intended shell; proximity alone does not establish building ownership. The AKPP
example supplies twelve measured shell selections and keeps its Ru Lai boundary.
Use **Preview selected shell** to inspect native materials and placement.

Define each interior floor's local corner, dimensions, elevation and rotation,
then **Import shell and floors**. These boundaries add authoring space without
duplicating retained walls or floors. Use the normal Library, partitions,
lighting, stairs and workstations to build the interior. Floor boundaries remain
editable in the Retrofit panel.

Select affected shell members and choose **Create entrance**, **Replace wall**,
**Remove obstruction** or **Add floor connection**. Position and size the change
volume, align it to the source surface and include the relevant technical layers.
Rigid native mesh cuts preserve outside geometry, LODs, UVs, vertex colours and
material bindings. Aligned box collision is split into retained fragments. Angled
box, triangle and convex collision use native cooked replacement surfaces with
the original material and filter bindings; neighbouring actors and instances in
the same source node are preserved. Convex cuts include closed collision reveals. Select an occluder
group only after reviewing the entire group.

Use **Existing / Proposed / Changes** to compare the GPU preview. The Retrofit
panel lists changes, source pins and export counts. **Restore original** reverses
a change together with its replacement door, wall or stair. Source hashes and
native counts are checked again on export. The export report includes replacement
mesh cut counts and collision fragments.

Cuts require rigid mesh surfaces. Surface cuts retain original faces; use lining
or a native door frame where a visual cut exposes wall thickness. Enable
**Collision & door paths** to inspect retained boxes and cooked replacement faces.
Generated Human navigation uses the retained and replacement collision surfaces
with agent clearance.

Select all affected Human navigation tiles when a change crosses a tile boundary.
Each change owns its navigation regions. Regions on the same tile are combined;
different tiles produce separate retained resources. **Restore original** removes
that change's regions while preserving other changes on a shared tile. The
Retrofit panel reports region and tile counts; export includes a report per tile.

## Individual source controls

Set **World anchor** to the intended location. In Project → **Existing world**,
press **Find nearby sectors** and choose a sector. You can also enter a known
native sector depot path. Press **Use building world anchor**, adjust world XYZ
and radius if needed, then **Survey sector objects**.

Results distinguish static placements, individual render instances and collision
actors. **Import locked reference** adds supported geometry at its native
transform. References are locked, excluded from export and pinned to source hashes.

Use **Select for removal** on an exact result. Applied world changes list those
selections; remove a selection there or use Undo to restore it. An entrance can
require separate visual, collision and occlusion changes. Original game sectors
are never overwritten: the builder exports source-checked ArchiveXL declarations.

Choose **Create terrain navigation mask** on a Human navigation result. Its
embedded resource and matching source deletion are assigned automatically. Edit
outline points and height range in the form, then apply. Coordinates are local
metres. Only fully covered ordinary Human polygons lose walkable flags; other
native bytes are retained. Export requires at least one covered polygon and emits
a before/after hash report.

Build → **Door in measured aperture** provides forms for sliding, hinged H/E,
double kitsch and garage devices. Set position, rotation, scale, initial state and
two approach endpoints. Endpoints must span at most four metres and 60 cm vertically.
Garage scaling includes a mod-owned rig so opening translation scales as well.

See [authoring](authoring.md) for shell rooms, windows and furniture, and
[exporting](exporting.md) for installation and in-game checks.

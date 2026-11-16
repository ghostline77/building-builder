# Edit a building

All positions use native RED coordinates: X and Y lie on the ground, and Z is
height, in metres. Grid-room coordinates and dimensions are in three-metre bays.
The plan shows local coordinates; **World anchor** places the building in-game.

## Rooms and floors

Choose a floor, then a plan tool:

| Tool | Action |
| --- | --- |
| Select | Click a room, wall bay, or placed object |
| Draw room | Drag a rectangle; its edges snap to three-metre bays |
| Move room | Drag a room to a new bay |
| Resize room | Drag from inside a room to its new upper-right corner |
| Draw partition | Drag between two points on a five-centimetre grid |

Escape cancels a gesture. Invalid edits leave the previous layout intact.
Use the selection inspector for exact values. Undo and redo include plan edits,
placed objects, and technical settings. Changing an opening or moving rooms can
remove wall settings that no longer belong to an existing wall bay.

Rooms can have a lower ceiling than the kit's storey height. Adjacent rooms share
a collision partition and have a decorative facing on each side. Passage removes
the wall; doorway keeps the frame. Automatic and manual doors add a native device
and a traversal link. The initial state can be closed, opened, or locked.
Grid bays support the entropy sliding and H/E hinged presets. Wider double doors
and garages use explicit measured shell apertures.

## Furniture and other objects

The library searches the published items catalogue. Filter by furniture,
electronics, complete assemblies, separate parts, or architectural type.
Previous/Next pages expose all matches. The appearance menu changes both the
published preview and the appearance placed on the plan.

**Complete assembly** keeps the native entity: a filing cabinet includes its
drawers, rig, attachments, and native component bindings. **Separate part** places
only that mesh. A bare cabinet shell does not acquire drawers automatically.

Select an object to set its position, yaw, scale, or native pivot/base-centre
anchor. Structural meshes can add explicit box collision. That box fills their
whole bounds, so use panel geometry for walls with openings. Complete entities
keep their original native components; separate meshes do not acquire gameplay
behaviour from their catalogue category.

## Partitions, windows and measured shell rooms

**Partition / window wall** creates an independently positioned wall. Open its
form to set wall start/end, height, thickness and floor. Under Openings, press
**Add opening** and set distance along the wall, width, sill and height.
Press **Apply changes** to validate and commit the complete form.

Distance measures along the wall from its start. A zero sill makes a doorway; a raised
sill makes a window opening. Visual panels and collision are cut together.
Place a frame/glass assembly from the catalogue if the opening needs one.

**Measured shell room** uses arbitrary metre dimensions instead of grid bays.
Set position, dimensions and rotation in its form. Check existing shell surfaces
to retain, then add openings under the appropriate wall. No JSON editing is needed.

Sides are south/east/north/west in the room's local frame. A retained side keeps
the surveyed vanilla wall rather than adding a duplicate. The room adds floors,
ceilings, collision and Human navigation surfaces. Its dimensions and wall
openings are editable under Build. Surveyed references remain preview-only.

## Stairs, lighting and navigation

Straight and return stairs cut their shafts through floors and ceilings. Their
landings can span adjoining rooms; guards already enclosed by a wall are omitted.
The compiler checks slope, riser height, clear width, and room coverage.

**Add stair flight** opens a setup dialog before changing the plan. It starts
with a compact return flight and can create a matching upper room. Review the
dimensions and the rooms on both floors, then apply. Existing stairs have forms
for axis, direction, railings, landing size and flight width.

Practical lights appear in the preview and export. Ambience areas and interior
classification create native world nodes. Additive Human navigation uses its own
layer and supports stairs, door states, and short entrance links. Check a route
with local XYZ endpoints before exporting. World navigation replacement is covered
in [world editing](world-editing.md).

## Workstations

Place a desk and chair, then choose **Bind desk and chair**. Choose placed objects
from the Desk and Chair menus, search actor-record suggestions, and set the actor's
appearance and seat workspot. Offsets
are relative to each placed object's position and yaw. The seated example uses
`Character.WorkerSpecialistMale`, appearance `service__worker_ma_generic_worker_01`, and
`base\workspots\common\chair\generic__sit_chair__sit_around__01.workspot`.

The computer's offset places the terminal relative to the desk. Set its document
title and text. Export includes a native actor community, persistent workspot,
terminal, document-read fact, `workstation-bindings.json`, and `quest-fragment.json`.
The owning quest supplies its journal, lifecycle, and character-spawned gate.

## Preview and direct editing

Select an object on the plan and drag to move using the chosen snap size.
Choose **Rotate object** and drag, or press **R** for 15° steps. Arrow keys move
the object by one snap increment. **Ctrl+D** duplicates; **Delete** removes.
Surveyed references remain locked. Footprints use native bounds and a direction
arrow. The inspector shows dimensions and possible footprint overlap. Seat and
terminal markers show workstation alignment, not animated worker poses.

Turn on **Collision & door paths** to inspect collision boxes and explicit
approach links. Overlap hints are approximate bounds checks; inspect the native
preview and perform the in-game acceptance checks before shipping a layout.

The 3D preview refreshes after validated edits. Drag to orbit, Shift-drag to pan,
and scroll to zoom; adjacent buttons provide keyboard-accessible alternatives.
Use **Focus selection**, **Fit building**, **Walk inside** and **Maximize** to
inspect the layout. Disable Auto refresh for manual updates.

Library category and completeness filters are independent. Complete objects are
the default. Expand a family to choose a style and appearance; click its image
to enlarge it. **Preview module** generates a native preview and measures the
object's dimensions before placement.

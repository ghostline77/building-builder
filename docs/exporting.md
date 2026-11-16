# Export and test

Press **Export native archive**. Export uses the complete building regardless of
the preview's cutaway or selected floor. The destination is a fresh directory
under `work/exports`; an existing destination is never overwritten.

Choose **Download complete building ZIP** for the complete package. Extract it,
then copy the `.archive` and `.archive.xl` from its root together into
`Cyberpunk 2077/archive/pc/mod`. ArchiveXL and its dependencies must be installed.
The dialog includes installation and playtest steps; individual resources and
verification details are under its expandable developer section.

| File | Purpose |
| --- | --- |
| `<id>.archive` | Native game resources |
| `<id>.archive.xl` | Streaming, source-sector patches, and device registration |
| `<id>.building.json` | Authored building project |
| `<id>.zip` | Editable source project and distributable resources |
| `export-report.json`, `bundle-receipt.json` | Readback results and package hashes |
| `workstation-bindings.json`, `quest-fragment.json` | Actor/terminal quest integration, when authored |
| `terrain-mask-report-<index>.json` | Source pins, regions and retained polygon-flag changes for each navigation tile |

The ZIP contains `source/archive` binaries, `source/raw` editable resource JSON,
`source/resources` registration, project metadata, and a conventional mod-project
file. Keep the archive and `.archive.xl` together in the game's mod archive folder
when installing a reviewed package. No command installs it automatically.

The exporter checks native readback, resource bindings, placements, collision
buffers, device instance data, embedded navigation, and archive extraction bytes.
Cooked collider entities retain native physics names resolved from the installed
material and collision-preset libraries. Navigation handle references are checked
against their declaration targets after native renumbering.
An export error publishes no incomplete archive. Runtime evidence is separate:
walk both directions through entrances and stairs, test open/closed/locked doors,
observe worker activation and terminal-read facts, leave and return to the streaming
area, then test save/load. Confirm vanilla shell edits remove the intended render,
collision, and occlusion geometry only. Remove the exported archive and XL to
uninstall the building package.

## Command line

Run from the builder directory. Native compilation requires `--game` (reflection is automatic; `--schema` is an optional override);
the catalogue comes from the configured Items package unless `--catalogue PATH` is supplied.

```powershell
./target/release/ghostline-building-builder.exe new --out work/my-building.json
./target/release/ghostline-building-builder.exe validate work/my-building.json --game 'path/to/Cyberpunk 2077'
./target/release/ghostline-building-builder.exe preview work/my-building.json --game 'path/to/Cyberpunk 2077' --out work/my-building.png
./target/release/ghostline-building-builder.exe export work/my-building.json --game 'path/to/Cyberpunk 2077' --out work/exports/my-building
./target/release/ghostline-building-builder.exe route work/my-building.json --game 'path/to/Cyberpunk 2077' --start 1.5 1.5 0 --end 1.5 1.5 4.2
./target/release/ghostline-building-builder.exe navigation work/my-building.json --game 'path/to/Cyberpunk 2077' --out work/navigation
```

`preview --closed` shows all walls/ceilings. `--camera FILE` accepts a renderer
camera; the examples include motel interior viewpoints. `inspect` writes native
documents, material bindings, UV streams, prepared texture maps, and dependencies
for rendering diagnosis. Keep these generated game-asset outputs under `work`.

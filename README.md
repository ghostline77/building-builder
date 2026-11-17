# Ghostline building builder

Create Cyberpunk 2077 buildings, place catalogue furniture, edit vanilla shells,
and export native mod archives from a local browser editor.

With the Windows distribution, use the executable in `windows-x64` and the separate `items-catalogue` folder. Pass the folder containing `manifest.json` to `--catalogue`, or set `GHOSTLINE_ITEMS_CATALOGUE` to its full path. Keep its data and media directories together.

To build from source, install Rust 1.92+, Node.js 20+ and GitHub access to the Ghostline dependencies. Run from this repository.

```powershell
cargo build --release --locked
./target/release/ghostline-building-builder.exe serve --game 'path/to/Cyberpunk 2077' --port 8793
```

Open <http://127.0.0.1:8793/>. Supply the Items package directory using `--catalogue PATH`. The game installation is read-only.

An existing Items catalogue can be reused after a game update. Its game-content
fingerprint records provenance, rather than requiring an identical installation.
Selected assets are resolved and measured from the installed game; missing
resources or unavailable appearances still report errors.

Saved imported shells also revalidate changed sector sources automatically. When
their selected native pieces still match, the opened project receives refreshed
source pins while retaining authored edits. Save the project to retain the new
pins. When the world layout changed, the editor opens the saved shell for editing
and preview and reports that its sector edits need a new survey before export.
Export still validates every sector pin and patch target against the installed
game. Opening a project does not certify its older world patches for export.

Choose **Start project** for a new building or a guided retrofit of an existing
shell. Retrofit imports keep native world geometry in place; normal rooms,
partitions and catalogue objects provide the editable interior.

The preview loads native geometry into ui-core's interactive browser viewport.
Drag to orbit, right-drag to pan and scroll to zoom. View, focus and camera
navigation controls operate locally. Editing geometry or changing floor/cutaway
visibility refreshes the scene; camera movement does not request a native render.

- [Getting started](docs/getting-started.md)
- [Rooms, furniture, stairs and workstations](docs/authoring.md)
- [Vanilla shells, world patches and terrain navigation](docs/world-editing.md)
- [Export, installation and command-line usage](docs/exporting.md)
- [Architecture and data ownership](docs/architecture.md)
- [Workflow verification](docs/coverage.md)

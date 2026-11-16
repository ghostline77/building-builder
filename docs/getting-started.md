# Start the building builder

Run these commands from the building-builder repository after installing the
parent workspace's JavaScript dependencies:

```powershell
cargo build --release --locked
./target/release/ghostline-building-builder.exe serve --game 'path/to/Cyberpunk 2077' --port 8793
```

Open <http://127.0.0.1:8793/> in a browser. The editor runs locally;
it reads the game but does not install anything into it.

The sibling `items/dist/catalogue` package supplies the object library and
previews. Use `--catalogue PATH`, or set `GHOSTLINE_ITEMS_CATALOGUE`, to choose
another published package. Keep its `data` and `media` directories together.
The catalogue records its source version. Selected resources are resolved against
the installed game; missing resources and appearances are reported during preparation
or export.

For a first building:

1. Open **Project**, choose **Workshop**, and press **Open project**.
2. Open **Build** and choose a construction kit.
3. Choose **Draw room** and drag on the plan. Each bay is three metres.
4. Select a wall and choose a doorway or an automatic/manual door.
5. Open **Library**, search for furniture, choose an appearance, and press
   **Place on plan**. Click where it should sit.
6. Inspect the automatically refreshed preview; drag to orbit or focus a selection.
   Press **Save project**.
7. Press **Export native archive** when the layout is ready.

Local projects are saved under `work/projects`; exports go under
`work/exports`. `--work PATH` changes that location. Project JSON can also be
downloaded and imported using the Project panel.

Continue with [editing buildings](authoring.md), [working in vanilla shells](world-editing.md),
or [exporting and testing](exporting.md).

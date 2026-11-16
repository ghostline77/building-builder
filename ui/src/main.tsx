import { AppShell, Button, Input, Select, mountApp } from "@ghostline/ui-core";
import "./style.css";

function BuildingBuilder() {
  return (
    <AppShell
      product="Building builder"
      project={<strong id="project-title">Workshop</strong>}
      actions={
        <>
          <Button id="undo" disabled>
            Undo
          </Button>
          <Button id="redo" disabled>
            Redo
          </Button>
          <Button id="save">Save project</Button>
          <Button id="export" variant="primary">
            Export native archive
          </Button>
        </>
      }
    >
      <main>
        <aside>
          <nav aria-label="Editor panels">
            <Button data-panel="build" className="active">
              Build
            </Button>
            <Button data-panel="library">Library</Button>
            <Button data-panel="project">Project</Button>
          </nav>
          <section id="build-panel">
            <h2>Rooms</h2>
            <div id="rooms" />
            <Button id="add-room">+ Add room</Button>
            <h2>Selected wall</h2>
            <p id="wall-hint" className="muted">
              Select a wall in the plan to edit its opening.
            </p>
            <div id="wall-controls" hidden>
              <label>
                Wall treatment
                <Select id="opening">
                  <option value="solid">Solid wall</option>
                  <option value="doorway">Doorway</option>
                  <option value="passage">Open bay</option>
                </Select>
              </label>
            </div>
            <h2>Placed objects</h2>
            <div id="objects" />
          </section>
          <section id="library-panel" hidden>
            <h2>Objects and architecture</h2>
            <label>
              Search library
              <Input
                id="search"
                type="search"
                placeholder="Wall, staircase, apartment…"
              />
            </label>
            <label>
              Piece type
              <Select id="role">
                <option value="">All types</option>
                <optgroup label="Catalogue objects">
                  {[
                    ["furniture", "Furniture"],
                    ["electronics", "Electronics"],
                    ["props", "Other props"],
                    ["assembly", "Complete assemblies"],
                    ["part", "Separate parts"],
                  ].map(([v, l]) => (
                    <option key={v} value={v}>
                      {l}
                    </option>
                  ))}
                </optgroup>
                <optgroup label="Measured architecture">
                  {[
                    "wall",
                    "floor",
                    "ceiling",
                    "doorway",
                    "stairs",
                    "roof",
                    "pillar",
                    "railing",
                  ].map((v) => (
                    <option key={v}>{v}</option>
                  ))}
                </optgroup>
              </Select>
            </label>
            <p id="library-count" className="muted" />
            <div className="row">
              <Button id="library-previous" disabled>
                Previous
              </Button>
              <Button id="library-next" disabled>
                Next
              </Button>
            </div>
            <div id="library-results" />
          </section>
          <section id="project-panel" hidden>
            <h2>Project</h2>
            <label>
              Building name
              <Input id="name" maxLength={200} />
            </label>
            <label>
              Project identity
              <Input id="identity" maxLength={64} />
            </label>
            <label>
              Saved project
              <Select id="saved">
                <option value="">Choose a saved project</option>
              </Select>
            </label>
            <Button id="open">Open project</Button>
            <hr />
            <Button id="new">New empty project</Button>
            <Button id="download">Download project JSON</Button>
            <label className="file">
              Import project JSON
              <Input id="import" type="file" accept="application/json,.json" />
            </label>
            <h2>Library coverage</h2>
            <p id="coverage" className="muted" />
          </section>
        </aside>
        <section className="workspace">
          <div className="workspace-toolbar">
            <span id="stats" className="muted" />
            <label>
              Floor
              <Select id="level" defaultValue="0">
                {["Ground", "First", "Second", "Third"].map((v, i) => (
                  <option key={v} value={i}>
                    {v}
                  </option>
                ))}
              </Select>
            </label>
          </div>
          <div className="plan-wrap">
            <svg
              id="plan"
              viewBox="-1 -1 10 10"
              role="img"
              aria-label="Building plan. Select rooms and wall bays."
            />
            <span className="plan-caption">PLAN / metres · RED X–Y</span>
          </div>
          <div className="preview-toolbar">
            <h2>Material preview</h2>
            <label>
              <Input type="checkbox" id="cutaway" defaultChecked /> Cutaway
            </label>
            <label>
              View
              <Select id="view">
                <option value="hero">Perspective</option>
                <option value="front">Front</option>
                <option value="back">Rear</option>
                <option value="left">Left</option>
                <option value="right">Right</option>
              </Select>
            </label>
            <Button id="render">Refresh preview</Button>
          </div>
          <div className="preview-wrap">
            <img id="preview" alt="Native building preview" hidden />
            <p id="preview-empty" className="muted">
              Load the current layout to inspect its geometry and materials.
            </p>
          </div>
        </section>
        <aside className="inspector">
          <h2>Selection</h2>
          <div id="inspector">
            <p className="muted">
              Select a room or object to edit its dimensions and placement.
            </p>
          </div>
          <h2>Finishes</h2>
          <label>
            Wall finish
            <Select id="wall-finish" />
          </label>
          <label>
            Floor finish
            <Select id="floor-finish" />
          </label>
          <h2>World anchor</h2>
          <div className="grid-fields" id="anchor" />
          <h2>Technical layers</h2>
          <div id="layers" />
          <h2>Validation</h2>
          <div id="diagnostics" />
        </aside>
      </main>
      <footer>
        <span id="status" role="status" aria-live="polite">
          Starting editor…
        </span>
        <span id="revision" />
      </footer>
      <dialog id="export-dialog">
        <div className="dialog-heading">
          <h2>Native export</h2>
          <Button id="close-export" aria-label="Close export">
            ×
          </Button>
        </div>
        <p>
          Exports the complete building. Cutaway and floor filters affect only
          the preview.
        </p>
        <div id="export-result" role="status" />
      </dialog>
    </AppShell>
  );
}
mountApp(<BuildingBuilder />, () => import("../app.js"));

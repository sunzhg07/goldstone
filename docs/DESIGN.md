# Goldstone — Design Document

Status: **M0, M1 and M2 implemented** (file format, geometry, CLI renderer, editor, multi-selection editing) · 2026-11-17

Goldstone is a desktop app and command-line tool for drawing many-body
diagrams (coupled cluster, IMSRG, MBPT: Goldstone and Hugenholtz diagrams)
for papers and talks. It is a new program written in Rust. It replaces
JaxoDraw for this use case but shares no code with it; JaxoDraw served only
as a checklist of features.

---

## 1. Goals and non-goals

**Goals**

1. Draw publication-quality many-body diagrams quickly, with consistent style
   across a whole paper.
2. Store diagrams as **readable, hand-editable, script-generatable text**
   (TOML) that diffs cleanly in git.
3. Store **topology** (vertices and lines between them), not loose pixel
   coordinates, so moving a vertex moves everything attached to it.
4. Export to **TikZ** (for LaTeX), **SVG** and **PDF**, with the source
   embedded in SVG/PDF so any exported figure can be reopened and edited.
5. Allow batch generation: a script writes TOML, and `goldstone render`
   produces figures without opening the GUI.
6. A single native binary with no JVM, for macOS, Linux and Windows.

**Non-goals for v1**

- Generating diagrams automatically from Wick contractions (possible later,
  as an external script that emits TOML).
- Feature parity with JaxoDraw for particle physics (gluon Béziers, the six
  JaxoDraw vertex types, axodraw output).
- Collaborative or online editing.

---

## 2. Concepts

A **document** (one `.gold` file) contains one diagram, made of:

| Concept | What it is |
|---|---|
| **Vertex** | A named point. It can be drawn (dot, cross ✕, circled cross ⊗, …) or invisible (`none`, used for free line ends). |
| **Edge** | A line between two vertices (`from`, `to`). The shape can be straight, an arc (`bend`), a Bézier curve, or a loop (`from == to`). The stroke can be plain, dashed, dotted, wavy, curly, zigzag or double, with optional arrows and a label. |
| **Text** | A free LaTeX label placed at a point. |
| **Axes** | An x/y axis pair with optional arrowheads, labels and ticks. |
| **Shape** | A box, ellipse or polygon, filled or outlined (blobs, operator boxes). |
| **Group** | A named set of objects that are selected and moved together. |
| **Style** | A named property preset (like a CSS class). Objects reference it with `class = "hole"`. Editing the preset restyles every object that uses it. |

The **one-body operator `---X`** needs no special object: it is a dashed
edge ending at a vertex of kind `cross`.

Every line endpoint is a vertex. A "free" end is simply a vertex with
`kind = "none"`. This keeps the model uniform: attaching, detaching and
dragging all work the same way everywhere.

---

## 3. Coordinate system

- The unit is the **TeX point (pt)**, so sizes in the file mean the same in
  the paper. Line widths, arrow sizes and the grid are all in pt.
- **y points up** (mathematical convention, same as TikZ). The GUI flips y
  for the screen.
- Coordinates are floats. The grid snaps them while editing, but the file
  can hold any value.
- The origin is arbitrary. Exports crop to the bounding box of the drawing
  plus a margin.

---

## 4. File format (`.gold`, TOML)

### 4.1 Complete example

A second-order MBPT energy diagram, with an external field attached:

```toml
format = 1                       # schema version; older files are migrated on load

[canvas]
grid = 10                        # pt
margin = 4                       # pt, added around the bounding box on export

# ---- style presets ---------------------------------------------------------
[style.particle]
stroke = "plain"
arrow = "forward"
width = 0.8

[style.hole]
stroke = "plain"
arrow = "backward"
width = 0.8

[style.interaction]
stroke = "dashed"
width = 0.8
dash = [3, 2]                    # on, off (pt)

# ---- vertices --------------------------------------------------------------
[[vertex]]
id = "a"
at = [0, 0]
kind = "dot"
size = 2.5

[[vertex]]
id = "b"
at = [40, 0]
kind = "dot"

[[vertex]]
id = "c"
at = [0, 60]
kind = "dot"

[[vertex]]
id = "d"
at = [40, 60]
kind = "dot"

[[vertex]]
id = "f"
at = [70, 30]
kind = "cross"                   # the X of a one-body operator
size = 5

# ---- edges -----------------------------------------------------------------
[[edge]]
from = "a"
to = "b"
class = "interaction"

[[edge]]
from = "c"
to = "d"
class = "interaction"

[[edge]]
from = "a"
to = "c"
class = "particle"
bend = 30                        # degrees; positive bends to the left of from→to
label = "$a$"
label_side = "left"

[[edge]]
from = "a"
to = "c"
class = "hole"
bend = -30
label = "$i$"

[[edge]]
from = "b"
to = "d"
class = "particle"
label = "$b$"

[[edge]]
from = "b"
to = "f"
class = "interaction"            # ---X

# ---- annotations -----------------------------------------------------------
[[text]]
at = [-30, 30]
content = "$E^{(2)} =$"
anchor = "east"

[[axes]]
origin = [-50, -10]
x = { length = 0, show = false }
y = { length = 80, label = "$t$", arrow = true, ticks = false }
```

### 4.2 Field reference

**Top level:** `format` (required int), `[canvas]`, `[style.<name>]`,
`[[vertex]]`, `[[edge]]`, `[[text]]`, `[[axes]]`, `[[shape]]`, `[[group]]`.

**Properties shared by all drawable objects** (all optional; resolved in the
order *object field → class preset → built-in default*):

| Field | Type | Default |
|---|---|---|
| `id` | string, unique | auto (`v1`, `e3`, …) |
| `class` | name of a `[style.*]` | none |
| `color` | `"#rrggbb"` or a name (`"black"`, `"red"`, …) | `"black"` |
| `width` | pt | `0.8` |
| `layer` | int; higher draws on top | 0 (vertices: 1) |

**Vertex:** `at = [x, y]`; `kind = "none" | "dot" | "cross" | "ocross" |
"square" | "circle"`; `size` (pt); `rotation` (deg, for `cross`); `fill`;
`label` (LaTeX) and `label_anchor` (default `south-west`, i.e. the label sits
above right of the vertex).

**Edge:**

| Field | Values |
|---|---|
| `from`, `to` | vertex ids (equal ids give a loop) |
| `stroke` | `plain`, `dashed`, `dotted`, `wavy`, `curly`, `zigzag` |
| `double` | bool, for any stroke and line shape; `double_sep` (pt): distance between the two lines |
| `ticks` | bool: short bars across the line, evenly spaced (skipping the arrowhead); `tick_spacing` (pt, fitted to the length), `tick_length` (pt) |
| `marks` | 0–3 bars across the line at `mark_pos` (0–1; default the middle, or a quarter when the arrow is in the middle); length `tick_length` |
| `bend` | degrees (an arc through both endpoints); mutually exclusive with `controls` |
| `controls` | `[[x, y], [x, y]]`, cubic Bézier control points |
| `loop` | `{ radius, angle }`, used when `from == to` |
| `dash` | `[on, off]` pt |
| `amplitude`, `wavelength` | pt, for wavy/curly/zigzag; the number of wiggles is rounded so the line ends cleanly |
| `arrow` | `none`, `forward`, `backward` |
| `arrow_pos` | 0–1 along the line, default 0.5 (the arrowhead is centred there) |
| `arrow_length`, `arrow_width` | pt (defaults 7 and 5.6, the proportions of the user's JaxoDraw figures) |
| `label` | LaTeX string |
| `label_side` | `left`, `right` (relative to from→to) |
| `label_pos`, `label_offset` | 0–1, pt |

**Text:** `at`, `content` (LaTeX), `anchor` (`center`, `north`,
`south-west`, …), `size` (pt), `rotation`.

**Axes:** `origin`, then `x` and `y` tables, each with
`length`, `show`, `arrow`, `label`, `ticks` (bool), `tick_spacing`,
`tick_labels` (bool or a list of strings), `tick_length`.

**Shape:** `kind = "box" | "ellipse" | "polygon" | "path"`; `at` + `size`
(box, ellipse), `points` (polygon) or `d` (path: SVG path data in
document coordinates, y up, e.g. `"M 0 0 L 10 0 C 12 2 12 8 10 10 Z"`);
`fill` color or `"none"`; `rotation`. Boolean operations produce paths.

**Group:** `id`, `members = ["a", "e3", …]`. Groups do not nest in v1.

### 4.3 Rules

- Unknown fields are an error, which catches typos (`colour = …`).
- Unknown or missing vertex ids are an error that names the offending edge.
- The GUI writes files in a **canonical order** (canvas, styles, vertices,
  edges, text, axes, shapes, groups) and leaves out fields that equal their
  defaults, so saving an unchanged file produces no diff.
- The draw order is `layer`, then file order.

---

## 5. Geometry and rendering

All drawing goes through one pipeline, so the canvas, SVG, PDF and TikZ
always agree:

```
Document ──resolve styles──▶ Resolved objects ──geometry──▶ Display list ──▶ backends
                                                             (paths, glyph runs)   ├─ egui canvas
                                                                                   ├─ SVG
                                                                                   ├─ PDF (via SVG)
                                                                                   └─ TikZ
```

- **Base paths** (straight line, arc, Bézier, loop) are built with the
  `kurbo` crate.
- **Wavy, curly and zigzag** strokes are generated by sampling the base path
  at equal arc length and offsetting along the normal. The algorithms are
  written from scratch. The wiggle count is rounded so every line starts and
  ends at a node.
- **Dashes** are computed by us, not left to each backend, so dashed lines
  look identical everywhere.
- **Arrows** are placed by arc length at `arrow_pos` and oriented along the
  tangent there.
- **Edge-to-vertex trimming:** lines stop at the outline of `cross`, `ocross`
  and `circle` vertices instead of running into their center.

### 5.1 Export backends

| Format | How | Notes |
|---|---|---|
| **TikZ** | All geometry is emitted as explicit computed paths (lines, Bézier curves), so it matches the canvas exactly; dashes use `dash pattern`. Labels become `\node{...}` with the **raw LaTeX** string. | `.tikz`: a bare `tikzpicture` to `\input` into a paper. `.tex`: a complete document cropped to the picture with a few lines of plain TeX, so it needs only TikZ and amsmath (the `standalone` class is not part of BasicTeX). |
| **SVG** | Written directly from the display list. Labels are typeset (see 5.2). The `.gold` source is embedded in `<metadata>`. | Reopen with `goldstone open fig.svg`. |
| **PDF** | Produced from the SVG by the `svg2pdf` crate. | Vector output; glyphs are outlines. Embedding the source in PDFs is planned for M3. |
| **PNG** | Rasterized with `resvg` at a chosen DPI. | For slides. |

### 5.2 LaTeX labels

This is the main technical risk, so there are two paths:

1. **Built-in (default, no TeX needed):** LaTeX math is converted to Typst
   math (the `mitex` crate) and typeset with Typst's math engine, which is
   pure Rust, fast and good quality. This drives the on-canvas preview and
   SVG/PDF/PNG export.
2. **Exact (optional):** if `latex` and `dvisvgm` are installed, labels can
   be rendered by real LaTeX with your document's preamble, and cached by
   content hash. Use this when SVG/PDF figures must match the paper's fonts
   exactly.

TikZ export always passes the raw LaTeX through, so in a paper the labels
are typeset by your own LaTeX run.

---

## 6. Editing

### 6.1 Window layout (Inkscape-style)

The interface follows Inkscape's layout and conventions, so it feels
familiar to anyone who uses Inkscape.

```
┌───────────────────────────────────────────────────────────────────────────┐
│ File  Edit  View  Object  Diagram  Export  Help                 (menu bar)│
├───────────────────────────────────────────────────────────────────────────┤
│ Tool controls: options of the active tool (stroke, class, arrow, width…)  │
├────┬────────────────────────────────────────────────────┬─────────────────┤
│ ▶  │  ruler                                             │ Docked dialogs  │
│ ✎  │ ┌────────────────────────────────────────────────┐ │ (tabs):         │
│ ╱  │ │                                                │ │ • Fill & Stroke │
│ •  │ │                                                │ │ • Objects       │
│ ✕  │ │                 canvas + grid                  │ │ • Align         │
│ T  │ │                                                │ │ • Styles        │
│ ▭  │ │                                                │ │ • Source (TOML) │
│ ◯  │ │                                                │ │                 │
│ ⊥  │ └────────────────────────────────────────────────┘ │                 │
├────┴────────────────────────────────────────────────────┴─────────────────┤
│ color palette  │  stroke palette: ─  - -  ···  ∿∿  ℓℓℓ  ⋀⋀  ═            │
├───────────────────────────────────────────────────────────────────────────┤
│ status bar: selection summary · hints for the active tool · x, y · zoom   │
└───────────────────────────────────────────────────────────────────────────┘
```

- **Toolbox** (left): one tool active at a time (§6.2).
- **Tool controls bar** (top): context-sensitive options for the active tool.
  With the Select tool it shows x, y, width and height of the selection;
  with the Edge tool it shows stroke, class, arrow and width for the next line.
- **Docked dialogs** (right), as tabs that can be rearranged, detached or closed:
  - **Fill & Stroke:** the multi-object property editor of §6.5.
  - **Objects:** a tree of all vertices, edges, text, groups; click to select,
    toggle visibility and lock.
  - **Align & Distribute:** align or evenly space selected vertices.
  - **Styles:** edit class presets (§4); shows how many objects use each.
  - **Source:** the live TOML of the document, the counterpart of Inkscape's
    XML editor. Selecting an object on the canvas highlights its TOML block;
    editing the text updates the canvas when the text is valid.
- **Palettes** (bottom): click a color to set the stroke color of the
  selection (Shift + click sets the fill). Next to it, a **stroke palette**
  sets plain, dashed, dotted, wavy, curly, zigzag or double with one click.
- **Status bar:** what is selected ("3 edges, 2 vertices"), hints for the
  current tool and modifiers, pointer position in pt, zoom.
- **Rulers and guides:** drag from a ruler to create a guide; vertices snap
  to guides.
- **Page:** the export area. By default it follows the bounding box of the
  drawing plus the margin; it can also be fixed in Document Properties.

### 6.2 Tools

| Tool | Key | Inkscape counterpart | Behavior |
|---|---|---|---|
| Select | `S` | Selector | Select, move; click a selected object again to switch between scale and rotate handles |
| Edit | `N` | Node tool | Edit the geometry of one object: bend of an arc, Bézier control points, loop size, arrow position, label position, wiggle amplitude |
| Edge | `B` | Pen (Bézier) | Click a start, click an end: straight line. Drag at the end: curve. Click the same vertex twice: loop. Ends snap to existing vertices; otherwise an invisible vertex is created |
| Vertex | `P` | (Pencil, unused here) | Click to place a dot vertex; the vertex kind is chosen in the tool controls |
| Cross | `X` | — | Click to place a cross vertex; click-drag from a vertex to create the `---X` dashed line and cross in one step |
| Text | `T` | Text | Click to place a LaTeX label |
| Rectangle | `R` | Rectangle | Box shape |
| Ellipse | `E` | Ellipse | Ellipse or blob shape |
| Axes | `A` | (Spray, unused here) | Drag to set origin and lengths |
| Zoom | `Z` | Zoom | Click to zoom in, Shift + click to zoom out, drag to zoom a region |

Goldstone-only tools reuse the letters of Inkscape tools that Goldstone does
not have (pencil `P`, spray `A`), so no Inkscape key changes meaning.

### 6.3 Snapping

Snap targets, in priority order: existing vertices, then the grid, then
alignment guides (same x or y as another vertex). Hold the snap-override
key while dragging to place freely.

### 6.4 Selection model

- The selection is a set of object ids.
- **Region selection (rubber band):** dragging a box on empty canvas selects
  every object whose bounding box lies **fully inside** the region, as in
  Inkscape.
- **Touch selection:** Alt + drag draws a freehand path and selects every
  object the path crosses, as in Inkscape. Useful for picking lines out of a
  dense diagram.
- Modifier keys add to or remove from the selection (key table in §7).
- **Select similar:** selects every object with the same class, or the same
  kind, as the current object.
- Selecting an edge does not select its vertices. Moving a selection moves
  the selected vertices plus the endpoints of every selected edge, and edges
  attached to unselected vertices stretch.

### 6.5 Fill & Stroke panel (multi-edit)

- The panel shows the union of the properties of all selected objects,
  grouped by section (Common, Vertex, Edge, Text, …). A section is shown
  only if at least one selected object has those properties.
- A field whose value differs between the selected objects shows **"—"
  (mixed)**. Editing it sets the value on every selected object that has
  that property.
- Fields set by the object's class preset are shown as inherited. Editing
  one overrides it on the object, and a reset button removes the override.
- "Apply to class" writes the change into the preset instead, restyling
  every object of that class.
- One panel edit is one undo step, whatever the number of objects.

### 6.6 Undo and redo

The document is small (kilobytes), so each committed change stores a full
snapshot (cheap with shared, copy-on-write data). A continuous drag
counts as one step. The history depth is unlimited for the session.

### 6.7 Clipboard

Copying puts the selected objects on the clipboard **as TOML text**, so
diagram fragments can be pasted between documents, into a text editor or
into a script. Pasting renames ids to avoid collisions. Edges whose vertices
were not copied bring their vertices along.

---

## 7. Key bindings

Bindings follow **Inkscape's defaults** wherever Inkscape has an equivalent
action, so existing Inkscape habits carry over. Goldstone-specific actions
use keys Inkscape leaves free (or uses for tools Goldstone does not have).
The manual (`docs/MANUAL.md`) and Help → Keyboard Shortcuts are generated
from the same table in the code, so they cannot drift apart. Ctrl means Cmd
on macOS. Bindings will be checked against Inkscape's default keymap while
implementing M1; any mismatch found there is fixed in favor of Inkscape.

**Selection (Select tool)**

| Input | Action | Source |
|---|---|---|
| Click | Select only this object | Inkscape |
| Shift + click | Add or remove this object (toggle) | Inkscape |
| Click a selected object again | Switch between scale and rotate handles | Inkscape |
| Alt + click | Select the object below the current one | Inkscape |
| Ctrl + click | Select an object inside a group | Inkscape |
| Drag on empty canvas | Rubber-band select (replace selection) | Inkscape |
| Shift + drag | Rubber band: add to the selection | Inkscape |
| Ctrl + Shift + drag | Rubber band: **remove** from the selection | Goldstone |
| Alt + drag | Touch select (objects crossed by the path) | Inkscape |
| Shift + Alt + drag | Touch select: add to the selection | Inkscape |
| Tab / Shift + Tab | Select next / previous object | Inkscape |
| Ctrl + A | Select all | Inkscape |
| Edit → Select Same → class / stroke / color | Select similar objects | Inkscape menu |
| Esc | Deselect, or cancel the current drawing | Inkscape |

**Tools:** see §6.2 (`S N B P X T R E A Z`). Space + drag pans with any tool.

**Editing**

| Input | Action | Source |
|---|---|---|
| Arrow keys | Move selection by 2 pt (preference) | Inkscape |
| Shift + arrow | Move by 10× that step | Inkscape |
| Alt + arrow | Move by one screen pixel | Inkscape |
| Delete / Backspace | Delete selection (edges attached to deleted vertices are deleted too) | Inkscape |
| Ctrl + Z / Ctrl + Shift + Z | Undo / redo | Inkscape |
| Ctrl + C / X / V | Copy / cut / paste | Inkscape |
| Ctrl + Alt + V | Paste in place | Inkscape |
| Ctrl + D | Duplicate | Inkscape |
| Ctrl + G / Ctrl + Shift + G | Group / ungroup | Inkscape |
| H / V | Flip horizontally / vertically | Inkscape |
| [ / ] | Rotate selection by 15° | Inkscape |
| Page Up / Page Down | Raise / lower one layer | Inkscape |
| Home / End | Raise to top / lower to bottom | Inkscape |
| Ctrl while dragging | Constrain movement to horizontal or vertical | Inkscape |
| Shift while dragging | Disable snapping | Inkscape |

**Edge tool**

| Input | Action |
|---|---|
| Click, click | Straight edge |
| Click, drag | Curved edge (Bézier) |
| Click the same vertex twice | Loop |
| Ctrl while placing | Constrain the angle to 15° steps (Inkscape) |
| Enter / right click | Finish; Esc cancels |

**View**

| Input | Action | Source |
|---|---|---|
| Scroll / Shift + scroll | Pan vertically / horizontally | Inkscape |
| Ctrl + scroll, or + / − | Zoom | Inkscape |
| Middle-button drag, or Space + drag | Pan | Inkscape |
| 1 | Zoom 1:1 | Inkscape |
| 3 | Zoom to selection | Inkscape |
| 4 | Zoom to drawing | Inkscape |
| 5 | Zoom to page | Inkscape |
| # | Toggle grid | Inkscape |
| \| | Toggle guides | Inkscape |

**Dialogs and file**

| Input | Action | Source |
|---|---|---|
| Ctrl + Shift + F | Fill & Stroke | Inkscape |
| Ctrl + Shift + L | Objects | Inkscape |
| Ctrl + Shift + A | Align & Distribute | Inkscape |
| Ctrl + Shift + X | Source (TOML), counterpart of the XML editor | Inkscape |
| Ctrl + Shift + D | Document Properties | Inkscape |
| Ctrl + Shift + P | Preferences | Inkscape |
| Ctrl + N / O / S | New / open / save | Inkscape |
| Ctrl + Shift + S | Save as | Inkscape |
| Ctrl + Shift + E | Export dialog | Inkscape |

---

## 8. Architecture

A Cargo workspace with separate crates, so the core can be used without the
GUI:

```
goldstone/
├─ crates/
│  ├─ goldstone-core     data model, TOML (serde), validation, migration,
│  │                     style resolution, geometry → display list
│  ├─ goldstone-export   SVG, PDF, PNG, TikZ backends; label typesetting
│  ├─ goldstone-app      egui/eframe desktop GUI (tools, selection, panels, undo)
│  ├─ goldstone-cli      `goldstone render|check|fmt|import`
│  └─ goldstone-jd3      read-only importer for JaxoDraw .jd3 files
└─ docs/
   ├─ DESIGN.md          this document
   └─ MANUAL.md          user manual (key table generated from code)
```

**Main dependencies:** `serde` + `toml`, `kurbo` (geometry), `egui`/`eframe`
(GUI), `egui_dock` (dockable dialog tabs), `svg2pdf` + `resvg` (PDF/PNG), `typst` + `mitex` (labels),
`quick-xml` (jd3 import).

**CLI**

```
goldstone render fig.gold -o fig.pdf        # or .svg, .png, .tex
goldstone render diagrams/*.gold --format tikz --out-dir figs/
goldstone check fig.gold                    # validate, report errors with line numbers
goldstone fmt fig.gold                      # rewrite in canonical order
goldstone import old.jd3 -o new.gold        # convert JaxoDraw files
```

**The JaxoDraw importer** reads the subset of `XMLEncoder` output that
JaxoDraw writes, without running any Java. It merges line endpoints that
coincide (within a tolerance) into shared vertices, so old diagrams gain
topology. JaxoDraw's own code is not used or translated (JaxoDraw is GPL;
Goldstone is a clean reimplementation and can use a permissive license).

---

## 9. Milestones

| # | Deliverable | You can… |
|---|---|---|
| **M0** | `core` + `export` + CLI: TOML → SVG/PDF/TikZ, all strokes, arrows, cross, axes, labels | write or generate diagrams as text and render them for a paper |
| **M1** | GUI basics: Inkscape-style window (toolbox, tool controls, docked dialogs, palettes, status bar), Select/Edit/Edge/Vertex/Cross/Text tools, vertex snapping, single selection, Fill & Stroke, undo, save/open | draw diagrams by hand |
| **M2** | Multi-selection: rubber band and touch select, modifier keys, multi-edit with mixed values, Select Same, groups, clipboard, Objects and Align dialogs | restyle many objects at once |
| **M3** | Styles and Source (TOML) dialogs, rulers and guides, exact-LaTeX label mode, export dialog, source embedding and reopen | keep a whole paper's figures consistent |
| **M4** | `.jd3` importer, user manual, release builds for macOS/Linux/Windows | migrate old diagrams and hand the tool to colleagues |

M0 comes first on purpose: it settles the file format and the geometry
before any GUI work, and it is already useful for scripted diagrams.

---

## 9a. Implementation notes (M1)

- The canvas is rasterized by the export pipeline (SVG → resvg) for the
  visible area at screen resolution, so the editor shows exactly what is
  exported. Selection highlights, handles and previews are drawn on top.
- The right-hand dialogs are tabs in a fixed side panel. Detachable,
  rearrangeable docking (`egui_dock`) is postponed to M3.
- Inkscape conventions applied: `F1` is also the Select tool; the shortcut
  window is under Help (and `?`, a Goldstone addition). The table in
  `crates/goldstone-app/src/keys.rs` marks each binding as "same as
  Inkscape" or "Goldstone only", and the manual is generated from it.
- Saving keeps comments and layout (`goldstone_core::io::save`).
- The UI follows the system light/dark theme; DejaVu Sans Mono (bundled
  with Typst) is a fallback font for symbols.
- Interaction is covered by tests that drive the editor with simulated
  mouse and keyboard input (`crates/goldstone-app/tests/interaction.rs`);
  `tests/screenshots.rs` renders the window offscreen for visual checks.

Deferred from M1 to M2 (all done in M2): touch selection, multi-object
editing with mixed values, groups, clipboard, rotation, Tab cycling,
Align & Distribute.

## 9b. Implementation notes (M2)

- **Multi-edit**: Fill & Stroke shows one section per kind of selected
  object; fields where the objects' own values differ say *mixed*.
  (Values inherited from different classes are not marked mixed.)
- **Groups** are flat (`[[group]]` with member ids). Grouping writes the
  automatic id of members that have none (`e3`), so the group keeps
  pointing at them when other objects are added or deleted.
- **Generated groups** remember their source: `latex = "…"` (a diagram
  from LaTeX) or `plot = { functions, x, y, size, labels, x_label,
  y_label }` (a function plot; ranges are strings such as `"-2pi"`). The
  LaTeX tab redraws the group from it, keeping its lower-left corner and
  id. A path shape without `Z` is drawn open (plot curves).
- **Clipboard** content is `.gold` text: the selected objects, the
  vertices their edges need, their classes and contained groups.
- **Boolean operations** use `linesweeper` (exact on Bézier curves);
  results are `kind = "path"` shapes. Rotation and flips are applied to
  coordinates, so polygons and paths have their rotation baked in.
- **Platform keys**: bindings are written once with the command modifier
  (Cmd on macOS, Ctrl elsewhere, chosen at compile time). Labels use the
  platform's notation (`⇧⌘Z` / `Ctrl+Shift+Z`); a few bindings exist on
  one platform only (Ctrl+Y redo is not a macOS convention). On macOS the
  Control key is distinct from Command.
- **Icons** are `.gold` files rendered by the export pipeline and tinted
  for the theme; the boolean icons' outlines were computed by Goldstone's
  own boolean code (`cargo run -p goldstone-core --example bool_icons`).
- **Line and Arc tools** are separate (B, A). The Arc tool takes three
  clicks (start, end, bulge); the same vertex twice makes a loop. Fill &
  Stroke shows an arc's bend, height (sagitta) and radius (+ *deep*),
  kept in sync; the file stores only `bend`.
- **Diagrams from LaTeX** (`goldstone_core::expr`): parses a tensor
  expression (⟨pq|X|rs⟩ index order, a–g particles, h–n holes, numbered as a_1, h_2, …), checks the
  contractions, orders operators vertically by arrow direction (particles
  up, holes down), aligns vertices over their partners (trying vertex
  permutations), spreads parallel lines into arcs and leans external lines
  away from internal ones. Output is a plain `.gold` document.
  The parser multiplies out parentheses, drops permutation operators,
  reads `\langle pq||rs\rangle`, and chooses the reading of each
  `X_{..}^{..}` (by search over swaps) that makes contractions consistent
  and external lines agree across terms.
- **Coordinates**: rulers, an id/coordinate overlay (I), hover info in the
  status bar, and `goldstone ls`.
- egui moves keyboard focus with Tab and the arrow keys; while no widget
  has focus, the editor cancels that so the keys reach the canvas.

## 10. Open questions

Resolved: license **MIT**; sample figure `ss.jd3` (line width, arrow
proportions); Rust toolchain installed.

New:

- **Comments on save.** The TOML serializer drops comments, so
  `goldstone fmt` (and a naive GUI save) loses them. Plan for M1: save
  through `toml_edit`, which edits the document in place and keeps comments
  and formatting.
- **Label checking.** `mitex` accepts some malformed LaTeX (for example an
  unclosed brace) that real LaTeX rejects, so `goldstone check` may pass a
  label that later fails in the paper.

Previously open:

1. **License:** MIT, Apache-2.0, or both (the Rust convention)?
2. **Default look:** line widths, dot size, arrow shape and dash pattern
   should match what you use in papers now. Please share 2–3 figures you
   consider "right".
3. **Typical diagrams:** a few `.jd3` files you actually use would let the
   importer and the defaults be tested against real work.
4. **Toolchain:** Rust is not installed on this machine yet
   (`rustup` from https://rustup.rs is the standard installer).

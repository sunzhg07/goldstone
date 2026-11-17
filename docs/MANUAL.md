# Goldstone User Manual

Goldstone draws many-body diagrams (coupled cluster, IMSRG, MBPT) for
papers and talks. A diagram is a `.gold` text file; the editor and the
command line both read and write it.

- Start the editor: `goldstone` or `goldstone edit fig.gold`
- Render from the command line: `goldstone render fig.gold -o fig.pdf`

Ctrl means Cmd on macOS throughout.

## The window

The layout follows Inkscape:

| Area | What it does |
|---|---|
| Menu bar (top) | File, Edit, View, Dialogs, Help |
| Tool controls (below the menu) | Options of the active tool: class, stroke and arrow for new lines, vertex kind; grid on/off and spacing |
| Toolbox (left) | Select, Edit, Edge, Vertex, Cross, Text |
| Canvas (center) | The diagram on a white page (the export area) with an optional grid |
| Dialogs (right) | **Fill & Stroke**: properties of the selection. **Objects**: every object in the diagram; click to select. **Source**: the `.gold` text, editable live |
| Palettes (bottom) | Click a color to set the stroke color of the selection (Shift+click: fill). Click a stroke to set it on selected lines and on new lines; *double* toggles double lines |
| Status bar | What is selected, hints for the current tool, errors, pointer position and zoom |

The canvas is drawn by the same renderer as the PDF/SVG export, so what
you see is exactly what you get.

## Drawing a diagram

1. Press **B** (Edge tool). Click where a line starts and click where it
   ends. Ends snap to existing vertices (a ring shows the snap) or to the
   grid; a free end becomes an invisible vertex. Choose the class, stroke
   and arrow for new lines in the tool controls bar first.
2. To draw an **arc**, click the start, then *press at the end point and
   drag*: the arc passes through the point where you release.
   Clicking the **same vertex twice** draws a loop.
3. Press **P** (Vertex tool) and click to place dots or other vertex kinds.
4. Press **X** (Cross tool) and *drag from a vertex*: this draws the dashed
   line and the cross of a one-body operator (`---X`) in one step.
5. Press **T** (Text tool) and click to place a label; type it in Fill &
   Stroke. Math goes between `$…$` and is written in LaTeX.

Labels on lines are set in Fill & Stroke (*Label*, *Label side*,
*Label position*). Vertices can carry labels too.

## Selecting and editing

- **S** (Select tool): click to select, Shift+click to add or remove,
  drag on empty canvas to select everything inside a rectangle
  (Shift+drag adds, Ctrl+Shift+drag removes).
- Drag a selected object to move the selection. Lines attached to vertices
  that are not selected stretch to follow. Hold Ctrl to move only
  horizontally or vertically, Shift to place freely without snapping.
- **N** (Edit tool): select one line to show its handles. Drag the round
  handle to bend it into an arc (Ctrl: 15° steps), resize a loop, or move
  a Bézier control point. Drag the diamond to move the arrow along the
  line. *Make Bézier curve* in Fill & Stroke turns an arc into a curve
  with two control points.
- **Fill & Stroke** shows every property of the selected object. Values in
  grey come from the object's class or the defaults; editing one sets it
  on the object, and ↺ removes the override again.
- Classes (`[style.<name>]` in the file) are presets such as `particle`
  and `hole`. Changing a class in the Source dialog restyles every object
  that uses it.

Every change can be undone (Ctrl+Z); a whole drag or an edited field is
one step.

## Files

- **Save** (Ctrl+S) writes the `.gold` file. Comments and the layout of a
  hand-written file are kept; only values that changed are rewritten.
- **Export** (Ctrl+Shift+E) writes PDF, SVG, PNG, `.tikz` (a
  `tikzpicture` to `\input` into a paper, labels typeset by your LaTeX) or
  `.tex` (a complete document cropped to the picture).
- SVG exports contain their `.gold` source: open them in Goldstone to
  keep editing, or run `goldstone extract fig.svg`.
- You can drop a `.gold` or Goldstone SVG file onto the window to open it.

The file format is described in [DESIGN.md §4](DESIGN.md#4-file-format-gold-toml).

## Command line

```sh
goldstone                                  # open the editor
goldstone edit fig.gold                    # open a file in the editor
goldstone render fig.gold -o fig.pdf       # .pdf .svg .png .tikz .tex
goldstone render figs/*.gold -f tikz --out-dir paper/figs
goldstone check figs/*.gold                # validate, typeset all labels
goldstone extract fig.svg > fig.gold       # source embedded in an SVG
goldstone fmt fig.gold                     # canonical form (drops comments)
goldstone keys                             # the reference below
```

## Keyboard and mouse reference

Generated from the editor's key table (`goldstone keys`); the editor
dispatches keys from the same table, so this list is always accurate.
"Inkscape: same" means Inkscape uses the same input for the same action.

<!-- keys:start -->

### Tools

| Keys | Action | Inkscape |
|---|---|---|
| `S` | Select tool | same |
| `F1` | Select tool | same |
| `N` | Edit tool (bend, control points, loop size, arrow position) | same |
| `B` | Edge tool (draw lines between vertices) | same |
| `P` | Vertex tool | Goldstone only |
| `X` | Cross tool (one-body operator ---X) | Goldstone only |
| `T` | Text tool (LaTeX labels) | same |

### Editing

| Keys | Action | Inkscape |
|---|---|---|
| `Ctrl+Shift+Z` | Redo | same |
| `Ctrl+Y` | Redo | same |
| `Ctrl+Z` | Undo | same |
| `Delete` | Delete selection (and lines attached to deleted vertices) | same |
| `Backspace` | Delete selection | same |
| `Ctrl+A` | Select all | same |
| `Esc` | Deselect, or cancel the line being drawn | same |
| `H` | Flip selection horizontally | same |
| `V` | Flip selection vertically | same |
| `Shift+Arrow` | Move selection by 20 pt | same |
| `Alt+Arrow` | Move selection by one screen pixel | same |
| `Arrow keys` | Move selection by 2 pt | same |

### View

| Keys | Action | Inkscape |
|---|---|---|
| `+` | Zoom in | same |
| `=` | Zoom in | same |
| `-` | Zoom out | same |
| `1` | Zoom 1:1 | same |
| `3` | Zoom to selection | same |
| `4` | Zoom to drawing | same |
| `5` | Zoom to page (the export area) | same |
| `#` | Toggle grid (snapping to the grid follows its visibility) | same |

### Dialogs and file

| Keys | Action | Inkscape |
|---|---|---|
| `Ctrl+Shift+F` | Fill & Stroke (object properties) | same |
| `Ctrl+Shift+L` | Objects | same |
| `Ctrl+Shift+X` | Source (the .gold text; counterpart of Inkscape's XML editor) | same |
| `Ctrl+Shift+S` | Save as | same |
| `Ctrl+Shift+E` | Export (PDF, SVG, PNG, TikZ) | same |
| `Ctrl+N` | New | same |
| `Ctrl+O` | Open (.gold, or an SVG made by Goldstone) | same |
| `Ctrl+S` | Save | same |
| `?` | Show keyboard shortcuts | Goldstone only |

### Mouse

| Where | Input | Action | Inkscape |
|---|---|---|---|
| Select tool | Click | Select only this object | same |
| Select tool | Shift+click | Add or remove this object (toggle) | same |
| Select tool | Drag an object | Move the selection (lines attached to unselected vertices stretch) | same |
| Select tool | Drag on empty canvas | Rubber-band select (objects fully inside) | same |
| Select tool | Shift+drag on empty canvas | Rubber band: add to the selection | same |
| Select tool | Ctrl+Shift+drag on empty canvas | Rubber band: remove from the selection | Goldstone only |
| Any tool | Ctrl while dragging | Move only horizontally or vertically | same |
| Any tool | Shift while dragging or placing | Do not snap | same |
| Edge tool | Click, click | Straight line; ends snap to vertices, or create invisible vertices | same |
| Edge tool | Click, then drag at the second point | Arc through the point where the mouse is released | Goldstone only |
| Edge tool | Click the same vertex twice | Loop | Goldstone only |
| Edge tool | Esc or right click | Cancel the line being drawn | same |
| Edit tool | Drag the round handle | Bend a line into an arc / move a Bézier control point / resize a loop | same |
| Edit tool | Drag the diamond handle | Move the arrow along the line | Goldstone only |
| Cross tool | Click | Place a cross vertex | Goldstone only |
| Cross tool | Drag from a vertex | Create a dashed line ending in a cross (---X) | Goldstone only |
| View | Scroll / Shift+scroll | Pan vertically / horizontally | same |
| View | Ctrl+scroll, or pinch | Zoom around the pointer | same |
| View | Middle-button drag, or Space+drag | Pan | same |
<!-- keys:end -->

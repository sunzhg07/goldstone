# Goldstone User Manual

Goldstone draws many-body diagrams (coupled cluster, IMSRG, MBPT) for
papers and talks. A diagram is a `.gold` text file; the editor and the
command line both read and write it.

- Start the editor: `goldstone` or `goldstone edit fig.gold`
- Render from the command line: `goldstone render fig.gold -o fig.pdf`

Shortcuts follow the platform the editor runs on: **Cmd** (⌘) on macOS,
**Ctrl** on Linux and Windows. The editor's menus and shortcut window show
the keys of your platform; the reference at the end lists both. In the
prose below, "Ctrl" means ⌘ on macOS and "Alt" means ⌥.

## The window

The layout follows Inkscape:

| Area | What it does |
|---|---|
| Menu bar (top) | File, Edit, View, Object, Path, Dialogs, Help |
| Commands bar | Group, Ungroup; Union, Difference, Intersection, Exclusion, Object to Path; Flip, Rotate 90°; Align & Distribute |
| Tool controls | Options of the active tool: class, stroke and arrow for new lines, vertex kind; for the Select tool the selection's X, Y, W and H (all editable; *lock* keeps W:H, *widths & text* also scales line widths and text); grid on/off and spacing; ids on/off |
| Toolbox (left) | Select, Edit, Line, Arc, Vertex, Cross, Text, Rectangle, Ellipse. Right-click a tool for its options |
| Rulers | Along the top and left of the canvas, in pt (y up); the pointer is marked on both. Ctrl+R hides them |
| Tabs | One tab per open diagram, above the canvas; each keeps its own undo history, selection and zoom. `*` marks unsaved changes. Click to switch (Ctrl+Tab, Ctrl+PageDown/PageUp), middle-click or × to close (Ctrl+W), + for a new diagram |
| Canvas (center) | The diagram on a white page (the export area) with an optional grid. Right-click for a menu: commands for the clicked object or the selection, all tools, the active tool's options, view |
| Dialogs (right) | **Fill & Stroke**: properties of the selection. **Objects**: every object with its id and coordinates, and the groups; click to select. **Align**: Align & Distribute. **Source**: the `.gold` text, editable live |
| Palettes (bottom) | Click a color to set the stroke color of the selection (Shift+click: fill). Click a stroke to set it on selected lines and on new lines; *double* toggles double lines |
| Status bar | What is selected; the object under the pointer (or the one selected object) with its kind, id and coordinates; hints; errors; pointer position and zoom |

The canvas is drawn by the same renderer as the PDF/SVG export, so what
you see is exactly what you get. All icons are Goldstone diagrams too
(`crates/goldstone-app/icons/*.gold`), drawn by the same renderer.

### Finding objects by id and coordinates

Every object has an id: vertices always (`a`, `v3`), other objects
either the id written in the file or an automatic one (`e1`, `t1`,
`ax1`, `s1`) in file order. Coordinates are absolute, in pt, with y up,
exactly as in the `.gold` file.

- Press **I** (View ▸ Object Ids and Coordinates) to label every object
  on the canvas with its id, and vertices and texts with their position.
- Hover an object: the status bar shows e.g. `edge e3: a (0, 0) → c (0, 60), plain, class particle, bend 40°`.
- In a terminal, `goldstone ls fig.gold` prints the same list for the
  whole file, so you can say "move vertex `c`" or find `e3` in the file.

## Drawing a diagram

1. Press **B** (Line tool). Click where a line starts and click where it
   ends. Ends snap to existing vertices (a ring shows the snap) or to the
   grid; a free end becomes an invisible vertex. Choose the class, stroke
   and arrow for new lines in the tool controls bar first.
2. Press **A** (Arc tool) for an **arc**: click the start, click the end,
   then move the mouse: the arc follows it; click to fix it (Ctrl: bend in
   15° steps). Pressing at the end point and dragging does the same in one
   gesture. Clicking the **same vertex twice** starts a **loop**, which
   points toward the mouse and grows with its distance.
3. Press **P** (Vertex tool) and click to place dots or other vertex kinds.
4. Press **X** (Cross tool) and *drag from a vertex*: this draws the dashed
   line and the cross of a one-body operator (`---X`) in one step.
5. Press **T** (Text tool) and click to place a label; type it in Fill &
   Stroke. Math goes between `$…$` and is written in LaTeX.

Labels on lines are set in Fill & Stroke (*Label*, *Label side*,
*Label position*). Vertices can carry labels too.

6. Press **R** (Rectangle) or **E** (Ellipse) and drag to draw a shape,
   e.g. a blob or an operator box. Hold Ctrl for a square or circle.

## Diagrams from LaTeX

Insert ▸ **Diagram from LaTeX…** (Ctrl+Shift+T) draws the diagram of a
tensor expression; so does `goldstone latex "…" -o fig.pdf` in a terminal.

The diagram is one group that remembers its equation. While it is
selected, the **LaTeX** tab of the right panel shows the equation: edit
it and the diagram is redrawn as you type (it stays where it is; an
error leaves the last good drawing). Ungroup it to edit the objects one by
one; it then forgets the equation.

```text
E2 = \sum_{abij} V_{ijab} T_{abij}            closed (energy) diagram
Z_{abic} = V_{abef} S_{efic}                  open diagram: a, b, i, c external
Z_{abij} = \frac{1}{2} V_{abef} T_{efij} + \frac{1}{4} V_{klef} T_{efij} T_{abkl}
```

- `X_{pqrs}` is ⟨pq|X|rs⟩: the first half of the indices are outgoing
  (bra), the second half incoming (ket); `X^{pq}_{rs}` means the same.
  Write indices as LaTeX: `V_{ijab}` (or `V_ijab`). Something that is not
  LaTeX, such as `T{aj}`, is an error that says what to write instead.
- **a–h are particles** (arrow up), **i–n holes** (arrow down); digits and
  primes are allowed (`a1`, `i'`).
- Each vertex of an operator joins one outgoing and one incoming index
  (p with r, q with s). An index used twice is an internal line, from where
  it is outgoing to where it is incoming; that decides which operator is
  drawn above which. An index used once is an external line and must stand
  on the left-hand side.
- `V W v w g H F f U` are interactions (dashed line; with two indices a
  one-body `---X`); other names (`T`, `S`, `R`, …) are amplitudes (thick
  bar). Several lines between two vertices become arcs.
- Equations as written in papers work too, e.g. the CCSD doubles
  equation with `\begin{aligned} … \\ … \end{aligned}`, `&`, `\quad`:
  - `\langle ij || ab \rangle` is an antisymmetrized integral (drawn as
    `v`), `\mathcal{W}`, `\tau`, `\bar{H}` are operator names;
  - `\left( … \right)` around a sum is multiplied out
    (`t (F - \frac{1}{2} t F)` gives two diagrams);
  - `P(ab)`, `P(ij)` permutation operators are left out;
  - in `X_{..}^{..}` notations differ on which group is outgoing
    (`t_{ij}^{ab}` vs `\mathcal{W}_{mn}^{ij}`), so Goldstone uses the reading
    that makes every summed index outgoing on one operator and incoming on
    the other, and that points the external lines the same way in all
    terms (an integral may be read as ⟨ab||ij⟩ for ⟨ij||ab⟩);
  - `0 = …` takes the external lines from what is left open; each `\\`
    starts a new row of diagrams.
- **IMSRG / VS-IMSRG notation** (occupation factors, or the operators η,
  Γ, Ω, χ): every index is a general orbital. `n_a` makes line *a* a hole,
  `\bar n_a` or `(1-n_a)` a particle; factors such as `(n_a - n_b)` are
  multiplied out, giving one diagram per particle–hole assignment. Lines
  without a factor, and all external lines, are drawn without arrows.
  Orbitals may be digits (`f_{12}`).
- **Layout**: vertices joined by internal lines share a column, so those
  lines run straight up and down and a ladder keeps its width. External
  lines run to the top or bottom of the whole diagram and lean just enough
  to clear vertices and other lines.
- `+` and `-` separate terms, drawn side by side, each as a group.
  Prefactors (`\frac{1}{4}`, `0.5`) are written before the term; `\sum`,
  `*`, `\cdot` are ignored.
- The dialog checks the expression as you type and explains mistakes
  (an index used twice as outgoing, an external index missing on the left,
  `p` instead of a particle or hole letter). **Insert** adds the diagram
  next to the drawing, **Replace drawing** starts over; everything can be
  undone, and the result is an ordinary diagram to edit further.

### A whole document

`goldstone paper.tex` draws every equation in the document (`equation`,
`align`, `eqnarray`, `multline`, `gather`, `dmath`, `\[ \]`, `$$ $$`; an
`align` with several equations gives one diagram per equation) into
`paper-diagrams/`:

- `eq0003-L37.pdf`: equation 3, which starts on line 37 (`-f svg`, `png`,
  `tikz` or `gold` for other formats);
- `overview-01.pdf`, …: 20 equations per page, each labelled with its
  number and line, for checking;
- `all.gold`: all diagrams in one file to open and edit;
- `report.txt`: what was drawn, which terms were left out and why, and
  which equations could not be drawn (definitions, operator algebra, …).

Factors that are not part of a diagram (coupling factors `\hat J`,
`(-1)^{…}`, 6j symbols, Kronecker deltas, labels such as `^{J}`) are left
out. Coverage of CC, IMSRG and VS-IMSRG notation is tracked in
[PARSER_PLAN.md](PARSER_PLAN.md).

## Function plots

Insert ▸ **Function Plot…** draws functions of x with axes, ticks and
numbered labels:

```text
sin(x); cos(x)            several functions: separate them with ; or new lines
x^3 - 2x                  plain notation: + - * / ^, implicit products
\frac{1}{1+x^2}           LaTeX: \frac \sqrt \sqrt[3] e^{…} \sin^2 x |x| \log_2
exp(-x^2/2); tanh(x)      sin cos tan sec csc cot, arcsin…, sinh cosh tanh,
                          exp ln log (= ln) log10 log2 sqrt abs sign floor ceil
```

The x range may use `pi` (`-2pi` to `2pi`); the y range is found from the
values (poles such as those of tan x are cut off) or set by hand. Gaps
(no real value) and poles break the curve. With several functions each is
written next to its curve, where there is room.

The plot is a group of ordinary objects (paths and labels): restyle
curves, move labels, or ungroup it. Like a LaTeX diagram it remembers its
settings, which the **LaTeX** tab shows for editing; the plot is redrawn as
you type. In a terminal:

```sh
goldstone plot 'sin(x); cos(x)' --x=-2pi:2pi -o fig.pdf
goldstone plot 'tan(x)' --x=-pi:pi --y=-5:5 --size 240x160 -o tan.svg
```

## Selecting and editing

- **S** (Select tool): click to select, Shift+click to add or remove,
  drag on empty canvas to select everything inside a rectangle
  (Shift+drag adds, Ctrl+Shift+drag removes). **Alt+drag** draws a red
  path and selects every object it touches (Shift+Alt+drag adds).
  **Tab** / Shift+Tab select the next / previous object.
- **Edit ▸ Select Same** selects every object with the same fill and
  stroke, stroke color, stroke style, class, or object type as the
  selection.
- Drag a selected object to move the selection. Lines attached to vertices
  that are not selected stretch to follow. Hold Ctrl to move only
  horizontally or vertically, Shift to place freely without snapping.
- **N** (Edit tool): select one line to show its handles. Drag the round
  handle to bend it into an arc (Ctrl: 15° steps), resize a loop, or move
  a Bézier control point. Drag the diamond to move the arrow along the
  line. *Make Bézier curve* in Fill & Stroke turns an arc into a curve
  with two control points.
- **Ticks and marks**: *Ticks* puts short bars across a line, evenly
  spaced along all of it (*Tick spacing*, *Tick length*); *Marks* puts
  1, 2 or 3 bars at one place (*Mark position*). Both work on every stroke
  and line shape, including double lines; ticks leave the arrowhead free.
  The *ticks* button in the bottom palette toggles ticks on the selected
  edges; the rest is in Fill & Stroke.
- **Double lines** work with every stroke (plain, dashed, dotted, wavy,
  curly, zigzag) and every line shape (straight, arc, Bézier, loop). Turn
  them on with *double* in the bottom palette, in the Line/Arc tool
  controls for new lines, or in Fill & Stroke; the *gap* next to it (and
  *Double gap* in Fill & Stroke) is the distance between the two lines in
  pt. A double wiggly line stays in phase, also on arcs.
- An arc's shape can be given three ways in Fill & Stroke, kept in sync:
  **Bend** (the angle stored in the file), **Arc height** (how far it
  bulges from the straight line between its vertices; with the chord
  length this fixes the arc) and **Arc radius** (at least half the chord;
  one radius fits a shallow and a deep arc, so tick *deep* for the one
  that is more than half a circle).
- **Fill & Stroke** shows every property of the selected object. Values in
  grey come from the object's class or the defaults; editing one sets it
  on the object, and ↺ removes the override again.
- With **several objects** selected, Fill & Stroke has a section per kind
  (vertices, edges, texts, shapes). A field shows *mixed* when the
  objects differ; setting it applies the value to all of them, and ↺
  resets it on all of them.
- **Arrange**: H / V flip, `[` / `]` rotate by 15°, Ctrl+`[` / Ctrl+`]`
  by 90° (labels stay upright). The **Align** dialog aligns edges or
  centers, or spaces objects evenly; groups move as one.
- **Groups**: Ctrl+G groups the selection, Ctrl+Shift+G ungroups.
  Clicking any member selects the whole group; Ctrl+click selects one
  object inside it.
- **Clipboard**: Ctrl+C / Ctrl+X copy and cut, Ctrl+V pastes at the
  pointer, Ctrl+Alt+V in place, Ctrl+D duplicates. The clipboard holds
  the objects as `.gold` text: paste it into a file, a message or a
  terminal, and paste `.gold` text from elsewhere into the diagram. Ids
  that already exist are renamed (`a` → `a1`).
- **Paths** (Path menu, commands bar): Union, Difference (bottom shape
  minus the others), Intersection and Exclusion combine the selected
  shapes into one path that keeps the style and id of the bottom shape.
  Curves stay curves. Object to Path turns a box, ellipse or polygon into
  a path.
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
- **New** (Ctrl+N) and **Open** (Ctrl+O, several files at once) open
  new tabs; a file that is already open is switched to. Dropping files
  onto the window opens them too, and `goldstone a.gold b.gold` opens
  both. **Close** (Ctrl+W) asks before discarding changes; Quit asks for
  each tab with unsaved changes.

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
goldstone ls fig.gold                      # every object: id, kind, coordinates
goldstone latex 'E2 = V_ijab T_abij' -o e2.pdf   # diagram of an expression
goldstone paper.tex                        # draw every equation of a document
goldstone tex paper.tex -f png --out-dir figs
goldstone latex 'Z_abic = V_abef S_efic' > z.gold
goldstone keys                             # the reference below
```

## Keyboard and mouse reference

Generated from the editor's key table (`goldstone keys`); the editor
dispatches keys from the same table, so this list is always accurate.
"Inkscape: same" means Inkscape uses the same input for the same action.

<!-- keys:start -->
### Tools

| Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|
| `S` | `S` | Select tool | same |
| `F1` | `F1` | Select tool | same |
| `N` | `N` | Edit tool (bend, control points, loop size, arrow position) | same |
| `B` | `B` | Line tool (straight lines between vertices) | same |
| `A` | `A` | Arc tool (click start, click end, click to set the bulge; same vertex twice: loop) | Goldstone only |
| `P` | `P` | Vertex tool | Goldstone only |
| `X` | `X` | Cross tool (one-body operator ---X) | Goldstone only |
| `T` | `T` | Text tool (LaTeX labels) | same |
| `R` | `R` | Rectangle tool | same |
| `E` | `E` | Ellipse tool | same |

### Editing

| Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|
| `Ctrl+Shift+Z` | `⇧⌘Z` | Redo | same |
| `Ctrl+Y` | — | Redo | same |
| `Ctrl+Z` | `⌘Z` | Undo | same |
| `Ctrl+C` | `⌘C` | Copy (also puts the objects' .gold text on the clipboard) | same |
| `Ctrl+X` | `⌘X` | Cut | same |
| `Ctrl+V` | `⌘V` | Paste (Goldstone objects or .gold text) next to the original | same |
| `Ctrl+Alt+V` | `⌥⌘V` | Paste in place | same |
| `Ctrl+D` | `⌘D` | Duplicate | same |
| `Delete` | `⌦` | Delete selection (and lines attached to deleted vertices) | same |
| `Backspace` | `⌫` | Delete selection | same |
| `Ctrl+A` | `⌘A` | Select all | same |
| `Esc` | `Esc` | Deselect, or cancel the line being drawn | same |
| `Shift+Tab` | `⇧⇥` | Select previous object | same |
| `Tab` | `⇥` | Select next object | same |

### Objects and paths

| Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|
| `Ctrl+Shift+G` | `⇧⌘G` | Ungroup | same |
| `Ctrl+G` | `⌘G` | Group | same |
| `Ctrl++` | `⌘+` | Union of shapes | same |
| `Ctrl+-` | `⌘-` | Difference (bottom shape minus the others) | same |
| `Ctrl+Shift+8` | `⇧⌘8` | Intersection of shapes (Inkscape's Ctrl+*) | same |
| `Ctrl+Shift+6` | `⇧⌘6` | Exclusion of shapes (Inkscape's Ctrl+^) | same |
| `Ctrl+Shift+C` | `⇧⌘C` | Object to path | same |

### Transform

| Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|
| `H` | `H` | Flip selection horizontally | same |
| `V` | `V` | Flip selection vertically | same |
| `Ctrl+[` | `⌘[` | Rotate selection 90° counterclockwise | same |
| `Ctrl+]` | `⌘]` | Rotate selection 90° clockwise | same |
| `[` | `[` | Rotate selection 15° counterclockwise | same |
| `]` | `]` | Rotate selection 15° clockwise | same |
| `Shift+Arrow` | `⇧ Arrow` | Move selection by 20 pt | same |
| `Alt+Arrow` | `⌥ Arrow` | Move selection by one screen pixel | same |
| `Arrow` | `Arrow` | Move selection by 2 pt | same |

### View

| Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|
| `+` | `+` | Zoom in | same |
| `=` | `=` | Zoom in | same |
| `-` | `-` | Zoom out | same |
| `1` | `1` | Zoom 1:1 | same |
| `3` | `3` | Zoom to selection | same |
| `4` | `4` | Zoom to drawing | same |
| `5` | `5` | Zoom to page (the export area) | same |
| `#` | `#` | Toggle grid (snapping to the grid follows its visibility) | same |
| `Ctrl+R` | `⌘R` | Toggle rulers | same |
| `I` | `I` | Show object ids and coordinates on the canvas | Goldstone only |

### Dialogs and file

| Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|
| `Ctrl+Shift+F` | `⇧⌘F` | Fill & Stroke (object properties) | same |
| `Ctrl+Shift+L` | `⇧⌘L` | Objects | same |
| `Ctrl+Shift+A` | `⇧⌘A` | Align & Distribute | same |
| `Ctrl+Shift+X` | `⇧⌘X` | Source (the .gold text; counterpart of Inkscape's XML editor) | same |
| `Ctrl+Shift+S` | `⇧⌘S` | Save as | same |
| `Ctrl+Shift+E` | `⇧⌘E` | Export (PDF, SVG, PNG, TikZ) | same |
| `Ctrl+N` | `⌘N` | New | same |
| `Ctrl+O` | `⌘O` | Open (.gold, or an SVG made by Goldstone) | same |
| `Ctrl+S` | `⌘S` | Save | same |
| `Ctrl+W` | `⌘W` | Close the diagram (tab) | Goldstone only |
| — | `⌃⇧⇥` | Previous tab | Goldstone only |
| — | `⌃⇥` | Next tab | Goldstone only |
| `Ctrl+Shift+Tab` | — | Previous tab | Goldstone only |
| `Ctrl+Tab` | — | Next tab | Goldstone only |
| `Ctrl+PageDown` | `⌘ PageDown` | Next tab | Goldstone only |
| `Ctrl+PageUp` | `⌘ PageUp` | Previous tab | Goldstone only |
| `Ctrl+Shift+T` | `⇧⌘T` | Diagram from LaTeX: draw a tensor expression such as E2 = V_ijab T_abij | Goldstone only |
| `Ctrl+Q` | `⌘Q` | Quit | same |
| `?` | `?` | Show keyboard shortcuts | Goldstone only |

### Mouse

| Where | Linux / Windows | macOS | Action | Inkscape |
|---|---|---|---|---|
| Select tool | Drag a handle of the selection box | Drag a handle of the selection box | Resize: corners scale, sides stretch; Ctrl+ keeps the proportions, Shift+ scales about the center. Line widths and text keep their size unless "widths & text" is on | same |
| Any tool | Right-click on the canvas | Right-click on the canvas | Menu: commands for the selection (the clicked object is selected first), tools, tool options, view | same |
| Toolbox | Right-click a tool | Right-click a tool | That tool's options (class, stroke, arrow, double line, vertex kind, grid) | Goldstone only |
| Tabs | Click / middle-click / right-click a tab | Click / middle-click / right-click a tab | Switch to it / close it / menu | Goldstone only |
| Select tool | Click | Click | Select only this object (a whole group if it is in one) | same |
| Select tool | Shift+click | ⇧click | Add or remove this object (toggle) | same |
| Select tool | Ctrl+click | ⌘click | Select one object inside a group | same |
| Select tool | Drag an object | Drag an object | Move the selection (lines attached to unselected vertices stretch) | same |
| Select tool | Drag on empty canvas | Drag on empty canvas | Rubber-band select (objects fully inside) | same |
| Select tool | Shift+drag on empty canvas | ⇧drag on empty canvas | Rubber band: add to the selection | same |
| Select tool | Ctrl+Shift+drag on empty canvas | ⌘⇧drag on empty canvas | Rubber band: remove from the selection | Goldstone only |
| Select tool | Alt+drag | ⌥drag | Touch select: select every object the red path crosses | same |
| Select tool | Shift+Alt+drag | ⇧⌥drag | Touch select: add to the selection | same |
| Any tool | Ctrl+ while dragging | ⌘ while dragging | Move only horizontally or vertically | same |
| Any tool | Shift+ while dragging or placing | ⇧ while dragging or placing | Do not snap | same |
| Line tool | Click, click (or drag) | Click, click (or drag) | Straight line; ends snap to vertices, or create invisible vertices | same |
| Line, Arc tool | Esc or right click | Esc or right click | Cancel the line being drawn | same |
| Arc tool | Click start, click end, click | Click start, click end, click | Arc; the last click sets the bulge (Ctrl+: bend in 15° steps) | Goldstone only |
| Arc tool | Click, then drag at the end point | Click, then drag at the end point | Arc through the point where the mouse is released | Goldstone only |
| Arc tool | Click the same vertex twice, then click | Click the same vertex twice, then click | Loop toward the pointer, sized by its distance | Goldstone only |
| Edit tool | Drag the round handle | Drag the round handle | Bend a line into an arc / move a Bézier control point / resize a loop | same |
| Edit tool | Drag the diamond handle | Drag the diamond handle | Move the arrow along the line | Goldstone only |
| Cross tool | Click | Click | Place a cross vertex | Goldstone only |
| Cross tool | Drag from a vertex | Drag from a vertex | Create a dashed line ending in a cross (---X) | Goldstone only |
| Rectangle, Ellipse | Drag | Drag | Draw the shape; Ctrl+ makes it a square / circle | same |
| View | Scroll / Shift+scroll | Scroll / ⇧scroll | Pan vertically / horizontally | same |
| View | Ctrl+scroll, or pinch | ⌘scroll, or pinch | Zoom around the pointer | same |
| View | Middle-button drag, or Space+drag | Middle-button drag, or Space+drag | Pan | same |
<!-- keys:end -->

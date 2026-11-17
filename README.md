# Goldstone

Draw many-body diagrams (coupled cluster, IMSRG, MBPT: Goldstone and
Hugenholtz diagrams) for papers and talks.

Diagrams are plain-text `.gold` files (TOML) that you can write by hand,
generate from scripts, and keep in git. Goldstone renders them to **PDF,
SVG, PNG and TikZ**, with LaTeX labels typeset in Computer Modern without
needing a TeX installation.

Status: **M2**: file format, renderer, and an Inkscape-style editor with multi-selection editing, groups, clipboard, boolean path operations and Align & Distribute
(drawing, snapping, selection, Fill & Stroke, undo, save/export). See the
[user manual](docs/MANUAL.md) and the [design](docs/DESIGN.md).

## Build

```sh
cargo build --release        # binary: target/release/goldstone
```

## Usage

```sh
goldstone                                    # open the editor
goldstone edit fig.gold                      # open a file in the editor
goldstone render fig.gold                    # → fig.pdf
goldstone render fig.gold -o fig.svg         # format from the extension
goldstone render figs/*.gold -f tikz --out-dir paper/figs
goldstone render fig.svg -o fig.pdf          # SVGs carry their .gold source
goldstone check figs/*.gold                  # validate, typeset all labels
goldstone extract fig.svg > fig.gold         # recover the source from an SVG
goldstone fmt fig.gold                       # canonical form (drops comments)
```

Output formats:

| Extension | What you get |
|---|---|
| `.pdf` | Vector PDF, page cropped to the drawing |
| `.svg` | Vector SVG with the `.gold` source embedded |
| `.png` | Raster image (`--dpi`, default 300) |
| `.tikz` | A bare `tikzpicture` to `\input` into a paper; labels are typeset by your document |
| `.tex` | A complete LaTeX document cropped to the picture (needs only TikZ and amsmath) |

## A first diagram

```toml
format = 1

[style.particle]
arrow = "forward"

[style.hole]
arrow = "backward"

[[vertex]]
id = "v"
at = [0, 40]
kind = "dot"

[[vertex]]
id = "x"
at = [40, 40]
kind = "cross"          # one-body operator: ---X
label = "$f$"

[[vertex]]
id = "in"
at = [0, 0]

[[vertex]]
id = "out"
at = [0, 80]

[[edge]]
from = "in"
to = "v"
class = "hole"
label = "$i$"

[[edge]]
from = "v"
to = "out"
class = "particle"
label = "$a$"

[[edge]]
from = "v"
to = "x"
stroke = "dashed"
```

Coordinates are in TeX points with y pointing up. Lines refer to vertices
by name, so moving a vertex moves every line attached to it. See
[`examples/`](examples) and the field reference in
[docs/DESIGN.md §4](docs/DESIGN.md#4-file-format-gold-toml).

## License

MIT

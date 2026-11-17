//! TikZ backend. Geometry is emitted as explicit paths (so the output matches
//! the canvas exactly); labels are emitted as `\node`s with the raw LaTeX, so
//! they are typeset by the paper's own LaTeX run.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use goldstone_core::color::Color;
use goldstone_core::kurbo::{BezPath, PathEl, Point};
use goldstone_core::scene::{Cap, Join, Mark, Scene};

use crate::num;

fn coord(p: Point) -> String {
    format!("({},{})", num(p.x), num(p.y))
}

fn path_ops(path: &BezPath) -> String {
    let mut out = String::new();
    let mut last = Point::ZERO;
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&coord(p));
                last = p;
            }
            PathEl::LineTo(p) => {
                write!(out, " -- {}", coord(p)).unwrap();
                last = p;
            }
            PathEl::QuadTo(a, p) => {
                // Elevate to cubic.
                let c1 = last + (a - last) * (2.0 / 3.0);
                let c2 = p + (a - p) * (2.0 / 3.0);
                write!(
                    out,
                    " .. controls {} and {} .. {}",
                    coord(c1),
                    coord(c2),
                    coord(p)
                )
                .unwrap();
                last = p;
            }
            PathEl::CurveTo(a, b, p) => {
                write!(
                    out,
                    " .. controls {} and {} .. {}",
                    coord(a),
                    coord(b),
                    coord(p)
                )
                .unwrap();
                last = p;
            }
            PathEl::ClosePath => out.push_str(" -- cycle"),
        }
    }
    out
}

struct Colors(BTreeMap<(u8, u8, u8), String>);

impl Colors {
    fn name(&mut self, c: Color) -> String {
        if c == Color::BLACK {
            return "black".into();
        }
        let n = self.0.len() + 1;
        self.0
            .entry((c.r, c.g, c.b))
            .or_insert_with(|| format!("gs{n}"))
            .clone()
    }
}

/// Renders a `tikzpicture` environment. `source_name` is mentioned in the
/// header comment.
pub fn render_picture(scene: &Scene, source_name: Option<&str>) -> String {
    let mut colors = Colors(BTreeMap::new());
    let mut body = String::new();
    for item in &scene.items {
        match &item.mark {
            Mark::Path(p) => {
                let mut opts: Vec<String> = Vec::new();
                let cmd = match (&p.stroke, p.fill) {
                    (Some(_), Some(_)) => "\\filldraw",
                    (Some(_), None) => "\\draw",
                    (None, Some(_)) => "\\fill",
                    (None, None) => continue,
                };
                if let Some(s) = &p.stroke {
                    let c = colors.name(s.color);
                    opts.push(if p.fill.is_some() {
                        format!("draw={c}")
                    } else {
                        c
                    });
                    opts.push(format!("line width={}pt", num(s.width)));
                    if s.cap == Cap::Round {
                        opts.push("line cap=round".into());
                    }
                    if s.join == Join::Round {
                        opts.push("line join=round".into());
                    }
                    if let Some([on, off]) = s.dash {
                        opts.push(format!("dash pattern=on {}pt off {}pt", num(on), num(off)));
                    }
                }
                if let Some(f) = p.fill {
                    let c = colors.name(f);
                    opts.push(if p.stroke.is_some() {
                        format!("fill={c}")
                    } else {
                        c
                    });
                }
                writeln!(
                    body,
                    "  {cmd}[{}] {};  % {}",
                    opts.join(", "),
                    path_ops(&p.path),
                    item.owner
                )
                .unwrap();
            }
            Mark::Text(t) => {
                let mut opts = vec![
                    format!("anchor={}", t.anchor.tikz()),
                    "inner sep=1pt".to_string(),
                ];
                if t.color != Color::BLACK {
                    opts.push(format!("text={}", colors.name(t.color)));
                }
                if t.size_explicit {
                    opts.push(format!(
                        "font=\\fontsize{{{}}}{{{}}}\\selectfont",
                        num(t.size),
                        num(t.size * 1.2)
                    ));
                }
                if t.rotation != 0.0 {
                    opts.push(format!("rotate={}", num(t.rotation)));
                }
                writeln!(
                    body,
                    "  \\node[{}] at {} {{{}}};  % {}",
                    opts.join(", "),
                    coord(t.at),
                    t.content,
                    item.owner
                )
                .unwrap();
            }
        }
    }

    let mut s = String::new();
    match source_name {
        Some(n) => writeln!(
            s,
            "% Generated by Goldstone {} from {n}. Edit the .gold file, not this one.",
            env!("CARGO_PKG_VERSION")
        ),
        None => writeln!(s, "% Generated by Goldstone {}.", env!("CARGO_PKG_VERSION")),
    }
    .unwrap();
    s.push_str("\\begin{tikzpicture}[x=1pt, y=1pt]\n");
    for ((r, g, b), name) in &colors.0 {
        writeln!(s, "  \\definecolor{{{name}}}{{RGB}}{{{r},{g},{b}}}").unwrap();
    }
    s.push_str(&body);
    s.push_str("\\end{tikzpicture}\n");
    s
}

/// Renders a complete LaTeX document whose page is cropped to the picture
/// plus the margin. Needs only TikZ and amsmath (no `standalone` class), so it
/// compiles on minimal TeX installations such as BasicTeX.
pub fn render_standalone(scene: &Scene, source_name: Option<&str>) -> String {
    format!(
        "\\documentclass{{article}}\n\
         \\usepackage{{tikz, amsmath, amssymb}}\n\
         \\begin{{document}}\n\
         \\setbox0=\\hbox{{%\n{picture}}}%\n\
         \\dimen0={margin}pt\n\
         \\pdfpagewidth=\\dimexpr\\wd0+2\\dimen0\\relax\n\
         \\pdfpageheight=\\dimexpr\\ht0+\\dp0+2\\dimen0\\relax\n\
         \\hoffset=-1in \\voffset=-1in\n\
         \\shipout\\vbox{{\\kern\\dimen0\\hbox{{\\kern\\dimen0\\box0}}}}\n\
         \\end{{document}}\n",
        picture = render_picture(scene, source_name),
        margin = num(scene.margin),
    )
}

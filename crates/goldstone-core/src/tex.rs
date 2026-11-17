//! Finding equations in LaTeX documents, and putting many drawn equations
//! on one sheet.

use crate::model::*;

/// An equation found in a `.tex` file.
#[derive(Debug, Clone, PartialEq)]
pub struct TexEquation {
    /// 1-based line where the equation starts.
    pub line: usize,
    pub source: String,
}

const ENVS: &[&str] = &[
    "equation",
    "equation*",
    "align",
    "align*",
    "eqnarray",
    "eqnarray*",
    "multline",
    "multline*",
    "gather",
    "gather*",
    "dmath",
    "dmath*",
    "flalign",
    "flalign*",
];

/// Removes `%` comments, keeping line breaks (so line numbers stay right).
fn strip_comments(tex: &str) -> String {
    tex.lines()
        .map(|l| {
            let mut out = String::new();
            let mut prev = ' ';
            for c in l.chars() {
                if c == '%' && prev != '\\' {
                    break;
                }
                out.push(c);
                prev = c;
            }
            out
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Index of the first `=` outside braces, if any.
fn top_level_equals(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (k, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            '=' if depth == 0 => return Some(k),
            _ => {}
        }
    }
    None
}

/// Splits an environment body into equations: lines (`\\`) that start a new
/// left-hand side (`x &= …`, `y &+=& …`) begin a new equation; lines that
/// continue (`&\quad + …`) stay with the previous one.
fn split_lines(body: &str) -> Vec<(usize, String)> {
    let pieces: Vec<&str> = body.split("\\\\").collect();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut offset = 0;
    for piece in pieces {
        let lead = &piece[..piece.len() - piece.trim_start().len()];
        let line_off = body[..offset].matches('\n').count() + lead.matches('\n').count();
        offset += piece.len() + 2;
        let t = piece.trim();
        if t.is_empty() {
            continue;
        }
        // A new equation has an `=` whose left side holds more than `&`.
        let starts_new =
            top_level_equals(t).is_some_and(|k| !t[..k].replace(['&', '+', ' '], "").is_empty());
        match out.last_mut() {
            Some(last) if !starts_new => {
                last.1.push_str(" \\\\ ");
                last.1.push_str(t);
            }
            _ => out.push((line_off, t.to_string())),
        }
    }
    out
}

/// All equations in a LaTeX document, in order.
pub fn equations(tex: &str) -> Vec<TexEquation> {
    let tex = strip_comments(tex);
    let line_at = |pos: usize| tex[..pos].matches('\n').count() + 1;
    let mut found: Vec<(usize, String)> = Vec::new(); // (byte position, body)
    for env in ENVS {
        let (open, close) = (format!("\\begin{{{env}}}"), format!("\\end{{{env}}}"));
        let mut from = 0;
        while let Some(a) = tex[from..].find(&open) {
            let start = from + a + open.len();
            let Some(b) = tex[start..].find(&close) else {
                break;
            };
            found.push((start, tex[start..start + b].to_string()));
            from = start + b + close.len();
        }
    }
    for (open, close) in [("\\[", "\\]"), ("$$", "$$")] {
        let mut from = 0;
        while let Some(a) = tex[from..].find(open) {
            let start = from + a + open.len();
            let Some(b) = tex[start..].find(close) else {
                break;
            };
            found.push((start, tex[start..start + b].to_string()));
            from = start + b + close.len();
        }
    }
    found.sort_by_key(|f| f.0);
    let mut out = Vec::new();
    for (pos, body) in found {
        let mut body = body;
        for cmd in ["\\label{", "\\tag{"] {
            while let Some(a) = body.find(cmd) {
                let end = body[a..].find('}').map_or(body.len(), |b| a + b + 1);
                body.replace_range(a..end, "");
            }
        }
        // A whole `aligned` block is one equation; otherwise split lines.
        let parts = if body.contains("\\begin{aligned}") || body.contains("\\begin{split}") {
            vec![(0, body.trim().to_string())]
        } else {
            split_lines(&body)
        };
        let line = line_at(pos);
        for (off, src) in parts {
            if !src.trim().is_empty() {
                out.push(TexEquation {
                    line: line + off,
                    source: src.trim().to_string(),
                });
            }
        }
    }
    out
}

/// Stacks drawn equations top to bottom, each with a caption on its left
/// (`(3) line 37`); ids get a prefix so they stay unique.
pub fn stack(items: Vec<(String, Document)>) -> Document {
    let mut out = Document::default();
    out.canvas.grid = Some(10.0);
    let mut y = 0.0;
    for (n, (caption, doc)) in items.into_iter().enumerate() {
        let prefix = format!("q{}_", n + 1);
        let ren = |id: &str| format!("{prefix}{id}");
        let (lo, hi) = doc
            .vertices
            .iter()
            .map(|v| v.at[1])
            .chain(doc.texts.iter().map(|t| t.at[1]))
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(v), b.max(v))
            });
        if !lo.is_finite() {
            continue;
        }
        // Top of this equation goes 40 pt below the previous one.
        let dy = y - hi - 40.0;
        for (k, v) in doc.styles {
            out.styles.entry(k).or_insert(v);
        }
        // Captions stand at x = 0; the diagrams start 70 pt to the right.
        for mut v in doc.vertices {
            v.id = ren(&v.id);
            v.at[0] += 70.0;
            v.at[1] += dy;
            out.vertices.push(v);
        }
        for mut e in doc.edges {
            e.id = e.id.map(|i| ren(&i));
            e.from = ren(&e.from);
            e.to = ren(&e.to);
            out.edges.push(e);
        }
        for mut t in doc.texts {
            t.id = t.id.map(|i| ren(&i));
            t.at[1] += dy;
            t.at[0] += 70.0;
            out.texts.push(t);
        }
        for mut g in doc.groups {
            g.id = ren(&g.id);
            g.members = g.members.iter().map(|m| ren(m)).collect();
            out.groups.push(g);
        }
        out.texts.push(Text {
            id: Some(format!("{prefix}caption")),
            at: [0.0, ((lo + hi) / 2.0 + dy).round()],
            content: caption,
            class: None,
            anchor: Some(Anchor::East),
            size: Some(9.0),
            rotation: None,
            color: Some("#555555".into()),
            layer: None,
        });
        y = lo + dy;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_splits_equations() {
        let tex = "intro\n% \\begin{equation} x \\end{equation}\n\\begin{equation}\nE = V_{ijab} T_{abij}\n\\label{e}\\end{equation}\n\\begin{align}\nz_0 &+=& f_{ia} t_{ai} \\\\\nz_0 &+=& \\frac{1}{4} V_{ijab} T_{abij}\n\\end{align}\n";
        let eqs = equations(tex);
        assert_eq!(eqs.len(), 3, "{eqs:?}");
        assert_eq!(eqs[0].line, 4);
        assert!(eqs[0].source.starts_with("E = V"));
        assert_eq!((eqs[1].line, eqs[2].line), (7, 8));
    }

    #[test]
    fn continuation_lines_stay_together() {
        let eqs = equations("\\begin{align} Z &= A_{ai} \\\\ &\\quad + B_{ai} \\end{align}");
        assert_eq!(eqs.len(), 1);
    }

    #[test]
    fn stacked_ids_are_unique() {
        let a = crate::expr::from_latex("E = V_{ijab} T_{abij}")
            .unwrap()
            .doc;
        let doc = stack(vec![("(1)".into(), a.clone()), ("(2)".into(), a)]);
        assert!(crate::scene::build(&doc).is_ok());
    }
}

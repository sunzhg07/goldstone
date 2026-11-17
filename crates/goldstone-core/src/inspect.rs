//! Human-readable descriptions of objects with their ids and absolute
//! coordinates (pt, y up), so an object seen in the editor can be found in
//! the `.gold` file or named in a terminal (`goldstone ls`).

use kurbo::Shape as _;

use crate::model::*;
use crate::scene::{Ids, ObjRef, ids};
use crate::shapes;

pub fn num(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn point(p: [f64; 2]) -> String {
    format!("({}, {})", num(p[0]), num(p[1]))
}

/// The object's kind as written in the file (`vertex`, `edge`, …).
pub fn kind(o: ObjRef) -> &'static str {
    match o {
        ObjRef::Vertex(_) => "vertex",
        ObjRef::Edge(_) => "edge",
        ObjRef::Text(_) => "text",
        ObjRef::Axes(_) => "axes",
        ObjRef::Shape(_) => "shape",
    }
}

fn vertex_kind(doc: &Document, v: &Vertex) -> &'static str {
    let st = v.class.as_ref().and_then(|c| doc.styles.get(c));
    match v
        .kind
        .or(st.and_then(|s| s.kind))
        .unwrap_or(VertexKind::None)
    {
        VertexKind::None => "point",
        VertexKind::Dot => "dot",
        VertexKind::Cross => "cross",
        VertexKind::Ocross => "ocross",
        VertexKind::Circle => "circle",
        VertexKind::Square => "square",
    }
}

fn stroke_name(s: Stroke) -> &'static str {
    match s {
        Stroke::Plain => "plain",
        Stroke::Dashed => "dashed",
        Stroke::Dotted => "dotted",
        Stroke::Wavy => "wavy",
        Stroke::Curly => "curly",
        Stroke::Zigzag => "zigzag",
    }
}

/// What the object is and where, without its id, e.g.
/// `dot at (70, 0)` or `a → b, wavy, bend 30°`.
pub fn summary(doc: &Document, o: ObjRef) -> String {
    match o {
        ObjRef::Vertex(i) => {
            let v = &doc.vertices[i];
            let mut s = format!("{} at {}", vertex_kind(doc, v), point(v.at));
            if let Some(l) = &v.label {
                s.push_str(&format!(", label {l}"));
            }
            s
        }
        ObjRef::Edge(i) => {
            let e = &doc.edges[i];
            let st = e.class.as_ref().and_then(|c| doc.styles.get(c));
            let at = |id: &str| {
                doc.vertices
                    .iter()
                    .find(|v| v.id == id)
                    .map_or(String::new(), |v| format!(" {}", point(v.at)))
            };
            let mut s = if e.from == e.to {
                format!("loop at {}{}", e.from, at(&e.from))
            } else {
                format!("{}{} → {}{}", e.from, at(&e.from), e.to, at(&e.to))
            };
            let stroke = e
                .stroke
                .or(st.and_then(|s| s.stroke))
                .unwrap_or(Stroke::Plain);
            s.push_str(&format!(", {}", stroke_name(stroke)));
            if let Some(c) = &e.class {
                s.push_str(&format!(", class {c}"));
            }
            if let Some(b) = e.bend {
                s.push_str(&format!(", bend {}°", num(b)));
            }
            if e.controls.is_some() {
                s.push_str(", curve");
            }
            if e.double.or(st.and_then(|s| s.double)).unwrap_or(false) {
                s.push_str(", double");
            }
            if e.ticks.or(st.and_then(|s| s.ticks)).unwrap_or(false) {
                s.push_str(", ticks");
            }
            match e.marks.or(st.and_then(|s| s.marks)).unwrap_or(0) {
                0 => {}
                1 => s.push_str(", 1 mark"),
                n => s.push_str(&format!(", {n} marks")),
            }
            if let Some(l) = &e.label {
                s.push_str(&format!(", label {l}"));
            }
            s
        }
        ObjRef::Text(i) => {
            let t = &doc.texts[i];
            format!("{:?} at {}", t.content, point(t.at))
        }
        ObjRef::Axes(i) => format!("origin {}", point(doc.axes[i].origin)),
        ObjRef::Shape(i) => {
            let s = &doc.shapes[i];
            let name = shapes::kind_name(s.kind);
            match (s.at, s.size) {
                (Some(at), Some([w, h])) => {
                    format!("{name} at {}, size {} × {}", point(at), num(w), num(h))
                }
                _ => match shapes::outline(s) {
                    Ok(p) => {
                        let b = p.bounding_box();
                        format!(
                            "{name}, bounds {} – {}",
                            point([b.x0, b.y0]),
                            point([b.x1, b.y1])
                        )
                    }
                    Err(_) => name.to_string(),
                },
            }
        }
    }
}

/// One line per object: kind, id, summary; then groups.
pub fn list(doc: &Document) -> String {
    let ids: Ids = ids(doc);
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let all = (0..doc.vertices.len())
        .map(ObjRef::Vertex)
        .chain((0..doc.edges.len()).map(ObjRef::Edge))
        .chain((0..doc.texts.len()).map(ObjRef::Text))
        .chain((0..doc.axes.len()).map(ObjRef::Axes))
        .chain((0..doc.shapes.len()).map(ObjRef::Shape));
    for o in all {
        rows.push((kind(o).into(), ids.of(doc, o), summary(doc, o)));
    }
    for g in &doc.groups {
        rows.push(("group".into(), g.id.clone(), g.members.join(", ")));
    }
    let w = rows.iter().map(|r| r.1.chars().count()).max().unwrap_or(0);
    rows.iter()
        .map(|(k, id, s)| format!("{k:<7} {id:<w$}  {s}\n"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_ids_and_coordinates() {
        let doc = crate::io::parse(
            r#"
format = 1
[[vertex]]
id = "a"
at = [0, 0]
kind = "dot"
[[vertex]]
id = "b"
at = [70, 0.5]
[[edge]]
from = "a"
to = "b"
stroke = "wavy"
bend = 30
"#,
        )
        .unwrap();
        let out = list(&doc);
        assert!(out.contains("vertex  a   dot at (0, 0)"), "{out}");
        assert!(out.contains("vertex  b   point at (70, 0.5)"), "{out}");
        assert!(
            out.contains("edge    e1  a (0, 0) → b (70, 0.5), wavy, bend 30°"),
            "{out}"
        );
    }
}

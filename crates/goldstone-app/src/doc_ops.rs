//! Editing operations on a [`Document`]. Objects are addressed by
//! [`ObjRef`] (kind + index).

use std::collections::{BTreeSet, HashSet};

use goldstone_core::kurbo::{Point, Rect, Vec2};
use goldstone_core::model::*;
use goldstone_core::scene::ObjRef;

pub type Selection = BTreeSet<ObjRef>;

pub fn new_vertex(id: String, at: Point, kind: Option<VertexKind>) -> Vertex {
    Vertex {
        id,
        at: [round(at.x), round(at.y)],
        class: None,
        kind,
        size: None,
        rotation: None,
        color: None,
        width: None,
        fill: None,
        layer: None,
        label: None,
        label_anchor: None,
    }
}

pub fn new_edge(from: String, to: String) -> Edge {
    Edge {
        id: None,
        from,
        to,
        class: None,
        bend: None,
        controls: None,
        loop_: None,
        stroke: None,
        color: None,
        width: None,
        layer: None,
        double: None,
        double_sep: None,
        dash: None,
        amplitude: None,
        wavelength: None,
        arrow: None,
        arrow_pos: None,
        arrow_length: None,
        arrow_width: None,
        label: None,
        label_side: None,
        label_pos: None,
        label_offset: None,
    }
}

pub fn new_text(at: Point, content: String) -> Text {
    Text {
        id: None,
        at: [round(at.x), round(at.y)],
        content,
        class: None,
        anchor: None,
        size: None,
        rotation: None,
        color: None,
        layer: None,
    }
}

/// Rounds coordinates to 0.01 pt so files stay readable.
pub fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

pub fn all_ids(doc: &Document) -> HashSet<String> {
    let mut ids: HashSet<String> = doc.vertices.iter().map(|v| v.id.clone()).collect();
    ids.extend(doc.edges.iter().filter_map(|e| e.id.clone()));
    ids.extend(doc.texts.iter().filter_map(|t| t.id.clone()));
    ids.extend(doc.axes.iter().filter_map(|a| a.id.clone()));
    ids.extend(doc.shapes.iter().filter_map(|s| s.id.clone()));
    ids.extend(doc.groups.iter().map(|g| g.id.clone()));
    ids
}

/// A fresh id `prefix<n>`.
pub fn fresh_id(doc: &Document, prefix: &str) -> String {
    let ids = all_ids(doc);
    (1..)
        .map(|n| format!("{prefix}{n}"))
        .find(|id| !ids.contains(id))
        .expect("infinite")
}

pub fn vertex_index(doc: &Document, id: &str) -> Option<usize> {
    doc.vertices.iter().position(|v| v.id == id)
}

/// Whether `r` still points at an existing object.
pub fn exists(doc: &Document, r: ObjRef) -> bool {
    match r {
        ObjRef::Vertex(i) => i < doc.vertices.len(),
        ObjRef::Edge(i) => i < doc.edges.len(),
        ObjRef::Text(i) => i < doc.texts.len(),
        ObjRef::Axes(i) => i < doc.axes.len(),
        ObjRef::Shape(i) => i < doc.shapes.len(),
    }
}

pub fn all_objects(doc: &Document) -> Selection {
    let mut s = Selection::new();
    s.extend((0..doc.vertices.len()).map(ObjRef::Vertex));
    s.extend((0..doc.edges.len()).map(ObjRef::Edge));
    s.extend((0..doc.texts.len()).map(ObjRef::Text));
    s.extend((0..doc.axes.len()).map(ObjRef::Axes));
    s.extend((0..doc.shapes.len()).map(ObjRef::Shape));
    s
}

/// Vertices that move with the selection: selected vertices plus the
/// endpoints of selected edges.
pub fn moved_vertices(doc: &Document, sel: &Selection) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for r in sel {
        match *r {
            ObjRef::Vertex(i) => {
                out.insert(i);
            }
            ObjRef::Edge(i) => {
                if let Some(e) = doc.edges.get(i) {
                    out.extend(vertex_index(doc, &e.from));
                    out.extend(vertex_index(doc, &e.to));
                }
            }
            _ => {}
        }
    }
    out
}

/// Edges whose geometry moves rigidly with the selection (both ends move).
fn rigid_edges(doc: &Document, moved: &BTreeSet<usize>) -> Vec<usize> {
    doc.edges
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            vertex_index(doc, &e.from).is_some_and(|i| moved.contains(&i))
                && vertex_index(doc, &e.to).is_some_and(|i| moved.contains(&i))
        })
        .map(|(i, _)| i)
        .collect()
}

fn shift(p: &mut [f64; 2], d: Vec2) {
    p[0] = round(p[0] + d.x);
    p[1] = round(p[1] + d.y);
}

/// Moves the selection by `d`.
pub fn translate(doc: &mut Document, sel: &Selection, d: Vec2) {
    let moved = moved_vertices(doc, sel);
    for &i in &moved {
        shift(&mut doc.vertices[i].at, d);
    }
    for i in rigid_edges(doc, &moved) {
        if let Some(c) = doc.edges[i].controls.as_mut() {
            shift(&mut c[0], d);
            shift(&mut c[1], d);
        }
    }
    for r in sel {
        match *r {
            ObjRef::Text(i) => shift(&mut doc.texts[i].at, d),
            ObjRef::Axes(i) => shift(&mut doc.axes[i].origin, d),
            ObjRef::Shape(i) => {
                let s = &mut doc.shapes[i];
                if let Some(at) = s.at.as_mut() {
                    shift(at, d);
                }
                if let Some(pts) = s.points.as_mut() {
                    pts.iter_mut().for_each(|p| shift(p, d));
                }
            }
            _ => {}
        }
    }
}

/// Reference points of the selection, for flipping and zooming.
pub fn selection_points(doc: &Document, sel: &Selection) -> Vec<Point> {
    let mut pts: Vec<Point> = moved_vertices(doc, sel)
        .iter()
        .map(|&i| pt(doc.vertices[i].at))
        .collect();
    for r in sel {
        match *r {
            ObjRef::Text(i) => pts.push(pt(doc.texts[i].at)),
            ObjRef::Axes(i) => pts.push(pt(doc.axes[i].origin)),
            ObjRef::Shape(i) => {
                let s = &doc.shapes[i];
                pts.extend(s.at.map(pt));
                pts.extend(s.points.iter().flatten().map(|p| pt(*p)));
            }
            _ => {}
        }
    }
    pts
}

pub fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

pub fn bounds(points: &[Point]) -> Option<Rect> {
    let first = *points.first()?;
    Some(
        points
            .iter()
            .fold(Rect::from_points(first, first), |r, p| r.union_pt(*p)),
    )
}

/// Mirrors the selection about the vertical (`horizontal = true`) or
/// horizontal axis through its center.
pub fn flip(doc: &mut Document, sel: &Selection, horizontal: bool) {
    let Some(b) = bounds(&selection_points(doc, sel)) else {
        return;
    };
    let c = b.center();
    let m = |p: &mut [f64; 2]| {
        if horizontal {
            p[0] = round(2.0 * c.x - p[0]);
        } else {
            p[1] = round(2.0 * c.y - p[1]);
        }
    };
    let moved = moved_vertices(doc, sel);
    for &i in &moved {
        m(&mut doc.vertices[i].at);
    }
    for i in rigid_edges(doc, &moved) {
        let e = &mut doc.edges[i];
        if let Some(b) = e.bend.as_mut() {
            *b = -*b;
        }
        if let Some(cs) = e.controls.as_mut() {
            m(&mut cs[0]);
            m(&mut cs[1]);
        }
        if let Some(l) = e.loop_.as_mut() {
            let a = l.angle.unwrap_or(90.0);
            l.angle = Some(if horizontal { 180.0 - a } else { -a }.rem_euclid(360.0));
        }
    }
    for r in sel {
        match *r {
            ObjRef::Text(i) => m(&mut doc.texts[i].at),
            ObjRef::Axes(i) => m(&mut doc.axes[i].origin),
            ObjRef::Shape(i) => {
                let s = &mut doc.shapes[i];
                if let Some(at) = s.at.as_mut() {
                    m(at);
                }
                if let Some(pts) = s.points.as_mut() {
                    pts.iter_mut().for_each(m);
                }
            }
            _ => {}
        }
    }
}

/// Deletes the selection. Edges attached to deleted vertices are deleted
/// too; group members that no longer exist are dropped.
pub fn delete(doc: &mut Document, sel: &Selection) {
    let dead_vertices: HashSet<String> = sel
        .iter()
        .filter_map(|r| {
            if let ObjRef::Vertex(i) = r {
                Some(doc.vertices[*i].id.clone())
            } else {
                None
            }
        })
        .collect();
    let mut dead_ids: HashSet<String> = dead_vertices.clone();
    let idx = |kind: fn(&ObjRef) -> Option<usize>| -> Vec<usize> {
        let mut v: Vec<usize> = sel.iter().filter_map(kind).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    let edges = idx(|r| {
        if let ObjRef::Edge(i) = r {
            Some(*i)
        } else {
            None
        }
    });
    let texts = idx(|r| {
        if let ObjRef::Text(i) = r {
            Some(*i)
        } else {
            None
        }
    });
    let axes = idx(|r| {
        if let ObjRef::Axes(i) = r {
            Some(*i)
        } else {
            None
        }
    });
    let shapes = idx(|r| {
        if let ObjRef::Shape(i) = r {
            Some(*i)
        } else {
            None
        }
    });
    for &i in edges.iter().rev() {
        dead_ids.extend(doc.edges.remove(i).id);
    }
    doc.edges.retain(|e| {
        let dead = dead_vertices.contains(&e.from) || dead_vertices.contains(&e.to);
        if dead {
            dead_ids.extend(e.id.clone());
        }
        !dead
    });
    doc.vertices.retain(|v| !dead_vertices.contains(&v.id));
    for &i in texts.iter().rev() {
        dead_ids.extend(doc.texts.remove(i).id);
    }
    for &i in axes.iter().rev() {
        dead_ids.extend(doc.axes.remove(i).id);
    }
    for &i in shapes.iter().rev() {
        dead_ids.extend(doc.shapes.remove(i).id);
    }
    for g in &mut doc.groups {
        g.members.retain(|m| !dead_ids.contains(m));
    }
    doc.groups.retain(|g| !g.members.is_empty());
}

/// Renames a vertex and updates every reference to it.
pub fn rename_vertex(doc: &mut Document, i: usize, new_id: &str) -> Result<(), String> {
    let new_id = new_id.trim();
    if new_id.is_empty() {
        return Err("an id must not be empty".into());
    }
    let old = doc.vertices[i].id.clone();
    if old == new_id {
        return Ok(());
    }
    if all_ids(doc).contains(new_id) {
        return Err(format!("id {new_id:?} is already used"));
    }
    doc.vertices[i].id = new_id.to_string();
    for e in &mut doc.edges {
        if e.from == old {
            e.from = new_id.to_string();
        }
        if e.to == old {
            e.to = new_id.to_string();
        }
    }
    for g in &mut doc.groups {
        for m in &mut g.members {
            if *m == old {
                *m = new_id.to_string();
            }
        }
    }
    Ok(())
}

/// The bend (degrees) of the circular arc from `a` to `b` through `p`.
pub fn bend_through(a: Point, b: Point, p: Point) -> f64 {
    let chord = b - a;
    let len = chord.hypot();
    if len < 1e-9 {
        return 0.0;
    }
    let side = chord.cross(p - a) / len; // signed distance from the chord, + = left
    if side.abs() < 0.5 {
        return 0.0;
    }
    let (u, v) = (a - p, b - p);
    let phi = (u.dot(v) / (u.hypot() * v.hypot())).clamp(-1.0, 1.0).acos();
    let theta = (std::f64::consts::PI - phi).to_degrees().min(179.0);
    theta.copysign(side).round()
}

#[cfg(test)]
mod tests {
    use super::*;
    use goldstone_core::io::parse;

    fn doc() -> Document {
        parse(include_str!("../../../examples/mbpt2.gold")).unwrap()
    }

    #[test]
    fn bend_through_semicircle() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(40.0, 0.0);
        assert_eq!(bend_through(a, b, Point::new(20.0, 20.0)), 90.0);
        assert_eq!(bend_through(a, b, Point::new(20.0, -20.0)), -90.0);
        assert_eq!(bend_through(a, b, Point::new(20.0, 0.1)), 0.0);
    }

    #[test]
    fn deleting_a_vertex_deletes_its_edges() {
        let mut d = doc();
        let n_edges = d.edges.len();
        let a = vertex_index(&d, "a").unwrap();
        delete(&mut d, &[ObjRef::Vertex(a)].into());
        assert!(vertex_index(&d, "a").is_none());
        assert_eq!(d.edges.len(), n_edges - 3); // a–b interaction, a–c particle, a–c hole
        assert!(goldstone_core::scene::build(&d).is_ok());
    }

    #[test]
    fn rename_updates_edges() {
        let mut d = doc();
        let a = vertex_index(&d, "a").unwrap();
        rename_vertex(&mut d, a, "v_bottom").unwrap();
        assert!(d.edges.iter().any(|e| e.from == "v_bottom"));
        assert!(rename_vertex(&mut d, a, "b").is_err());
        assert!(goldstone_core::scene::build(&d).is_ok());
    }

    #[test]
    fn moving_an_edge_moves_its_vertices() {
        let mut d = doc();
        translate(&mut d, &[ObjRef::Edge(0)].into(), Vec2::new(5.0, 0.0));
        assert_eq!(d.vertices[vertex_index(&d, "a").unwrap()].at, [5.0, 0.0]);
        assert_eq!(d.vertices[vertex_index(&d, "c").unwrap()].at, [0.0, 60.0]);
    }

    #[test]
    fn flip_negates_bends() {
        let mut d = doc();
        let all = all_objects(&d);
        let before: Vec<_> = d.edges.iter().map(|e| e.bend).collect();
        flip(&mut d, &all, true);
        let after: Vec<_> = d.edges.iter().map(|e| e.bend.map(|b| -b)).collect();
        assert_eq!(before, after);
    }
}

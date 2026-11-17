//! Editing operations on a [`Document`]. Objects are addressed by
//! [`ObjRef`] (kind + index).

use std::collections::{BTreeSet, HashMap, HashSet};

use goldstone_core::color::Color;
use goldstone_core::kurbo::{Affine, Point, Rect, Vec2};
use goldstone_core::model::*;
use goldstone_core::scene::{self, ObjRef, defaults as d};
use goldstone_core::shapes;

use crate::keys::{AlignOp, Same};

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
    translate_units(doc, &[(sel.clone(), d)]);
}

/// Moves several parts of the selection by different amounts. A vertex
/// shared by two parts moves with the first one only.
pub fn translate_units(doc: &mut Document, units: &[(Selection, Vec2)]) {
    let mut done: BTreeSet<usize> = BTreeSet::new();
    for (sel, d) in units {
        let d = *d;
        let moved: BTreeSet<usize> = moved_vertices(doc, sel)
            .difference(&done)
            .copied()
            .collect();
        for &i in &moved {
            shift(&mut doc.vertices[i].at, d);
        }
        for i in rigid_edges(doc, &moved) {
            if let Some(c) = doc.edges[i].controls.as_mut() {
                shift(&mut c[0], d);
                shift(&mut c[1], d);
            }
        }
        done.extend(moved);
        for r in sel {
            match *r {
                ObjRef::Text(i) => shift(&mut doc.texts[i].at, d),
                ObjRef::Axes(i) => shift(&mut doc.axes[i].origin, d),
                ObjRef::Shape(i) => shapes::transform(&mut doc.shapes[i], Affine::translate(d)),
                _ => {}
            }
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

/// Center of the selection's reference points.
fn center(doc: &Document, sel: &Selection) -> Option<Point> {
    bounds(&selection_points(doc, sel)).map(|b| b.center())
}

/// Mirrors the selection about the vertical (`horizontal = true`) or
/// horizontal axis through its center.
pub fn flip(doc: &mut Document, sel: &Selection, horizontal: bool) {
    let Some(c) = center(doc, sel) else { return };
    let (sx, sy) = if horizontal { (-1.0, 1.0) } else { (1.0, -1.0) };
    let a = Affine::translate(c.to_vec2())
        * Affine::scale_non_uniform(sx, sy)
        * Affine::translate(-c.to_vec2());
    transform(doc, sel, a);
}

/// Rotates the selection counterclockwise by `deg` about its center.
/// Labels and text stay upright.
pub fn rotate(doc: &mut Document, sel: &Selection, deg: f64) {
    let Some(c) = center(doc, sel) else { return };
    transform(doc, sel, Affine::rotate_about(deg.to_radians(), c));
}

/// Applies a rigid transform to the selection.
pub fn transform(doc: &mut Document, sel: &Selection, a: Affine) {
    let reflect = a.determinant() < 0.0;
    let ap = |p: &mut [f64; 2]| {
        let q = a * pt(*p);
        *p = [round(q.x), round(q.y)];
    };
    let dir = |deg: f64| {
        let r = deg.to_radians();
        let v = a * Point::new(r.cos(), r.sin()) - a * Point::ZERO;
        (v.y.atan2(v.x).to_degrees().round()).rem_euclid(360.0)
    };
    let moved = moved_vertices(doc, sel);
    for &i in &moved {
        ap(&mut doc.vertices[i].at);
    }
    for i in rigid_edges(doc, &moved) {
        let side = doc.edges[i]
            .label_side
            .or_else(|| {
                let c = doc.edges[i].class.as_ref()?;
                doc.styles.get(c)?.label_side
            })
            .unwrap_or(Side::Left);
        let e = &mut doc.edges[i];
        if reflect {
            if let Some(b) = e.bend.as_mut() {
                *b = -*b;
            }
            if e.label.is_some() {
                e.label_side = Some(match side {
                    Side::Left => Side::Right,
                    Side::Right => Side::Left,
                });
            }
        }
        if let Some(cs) = e.controls.as_mut() {
            ap(&mut cs[0]);
            ap(&mut cs[1]);
        }
        if e.from == e.to {
            let l = e.loop_.get_or_insert(LoopSpec {
                radius: None,
                angle: None,
            });
            let new = dir(l.angle.unwrap_or(d::LOOP_ANGLE));
            l.angle = (new != d::LOOP_ANGLE || l.angle.is_some()).then_some(new);
            if l.angle.is_none() && l.radius.is_none() {
                e.loop_ = None;
            }
        }
    }
    for r in sel {
        match *r {
            ObjRef::Text(i) => ap(&mut doc.texts[i].at),
            ObjRef::Axes(i) => ap(&mut doc.axes[i].origin),
            ObjRef::Shape(i) => shapes::transform(&mut doc.shapes[i], a),
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

// ---- groups -------------------------------------------------------------------

/// Map from id (explicit or automatic) to object.
pub fn by_id(doc: &Document) -> HashMap<String, ObjRef> {
    let ids = scene::ids(doc);
    all_objects(doc)
        .into_iter()
        .map(|o| (ids.of(doc, o), o))
        .collect()
}

/// Index of the group that `o` belongs to.
pub fn group_of(doc: &Document, o: ObjRef) -> Option<usize> {
    if doc.groups.is_empty() {
        return None;
    }
    let id = scene::ids(doc).of(doc, o);
    doc.groups.iter().position(|g| g.members.contains(&id))
}

pub fn group_members(doc: &Document, g: usize) -> Selection {
    let map = by_id(doc);
    doc.groups[g]
        .members
        .iter()
        .filter_map(|m| map.get(m).copied())
        .collect()
}

/// `o`, or all members of its group.
pub fn with_group(doc: &Document, o: ObjRef) -> Selection {
    match group_of(doc, o) {
        Some(g) => group_members(doc, g),
        None => [o].into(),
    }
}

fn set_explicit_id(doc: &mut Document, o: ObjRef, id: String) {
    let slot = match o {
        ObjRef::Vertex(_) => return,
        ObjRef::Edge(i) => &mut doc.edges[i].id,
        ObjRef::Text(i) => &mut doc.texts[i].id,
        ObjRef::Axes(i) => &mut doc.axes[i].id,
        ObjRef::Shape(i) => &mut doc.shapes[i].id,
    };
    if slot.is_none() {
        *slot = Some(id);
    }
}

/// Groups the selection (taking members out of their old groups). Members
/// without an id get their automatic id written out, so the group keeps
/// pointing at them. Returns the new group's id.
pub fn group(doc: &mut Document, sel: &Selection) -> Option<String> {
    if sel.len() < 2 {
        return None;
    }
    let ids = scene::ids(doc);
    let members: Vec<String> = sel.iter().map(|o| ids.of(doc, *o)).collect();
    for (o, id) in sel.iter().zip(&members) {
        set_explicit_id(doc, *o, id.clone());
    }
    for g in &mut doc.groups {
        g.members.retain(|m| !members.contains(m));
    }
    doc.groups.retain(|g| !g.members.is_empty());
    let id = fresh_id(doc, "g");
    doc.groups.push(Group {
        id: id.clone(),
        members,
    });
    Some(id)
}

/// Dissolves every group with a member in the selection. Returns how many.
pub fn ungroup(doc: &mut Document, sel: &Selection) -> usize {
    let ids = scene::ids(doc);
    let selected: HashSet<String> = sel.iter().map(|o| ids.of(doc, *o)).collect();
    let before = doc.groups.len();
    doc.groups
        .retain(|g| !g.members.iter().any(|m| selected.contains(m)));
    before - doc.groups.len()
}

// ---- clipboard ------------------------------------------------------------------

/// The selection as a document of its own: selected objects, the vertices
/// their edges need, the classes they use and the groups they contain.
pub fn extract(doc: &Document, sel: &Selection) -> Document {
    let mut out = Document {
        canvas: Canvas::default(),
        ..Document::default()
    };
    let verts = moved_vertices(doc, sel);
    out.vertices = verts.iter().map(|&i| doc.vertices[i].clone()).collect();
    let pick =
        |f: fn(&ObjRef) -> Option<usize>| -> Vec<usize> { sel.iter().filter_map(f).collect() };
    out.edges = pick(|o| {
        if let ObjRef::Edge(i) = o {
            Some(*i)
        } else {
            None
        }
    })
    .into_iter()
    .map(|i| doc.edges[i].clone())
    .collect();
    out.texts = pick(|o| {
        if let ObjRef::Text(i) = o {
            Some(*i)
        } else {
            None
        }
    })
    .into_iter()
    .map(|i| doc.texts[i].clone())
    .collect();
    out.axes = pick(|o| {
        if let ObjRef::Axes(i) = o {
            Some(*i)
        } else {
            None
        }
    })
    .into_iter()
    .map(|i| doc.axes[i].clone())
    .collect();
    out.shapes = pick(|o| {
        if let ObjRef::Shape(i) = o {
            Some(*i)
        } else {
            None
        }
    })
    .into_iter()
    .map(|i| doc.shapes[i].clone())
    .collect();
    let present = all_ids(&out);
    out.groups = doc
        .groups
        .iter()
        .filter(|g| g.members.iter().all(|m| present.contains(m)))
        .cloned()
        .collect();
    let classes: BTreeSet<String> = out
        .vertices
        .iter()
        .filter_map(|v| v.class.clone())
        .chain(out.edges.iter().filter_map(|e| e.class.clone()))
        .chain(out.texts.iter().filter_map(|t| t.class.clone()))
        .chain(out.axes.iter().filter_map(|a| a.class.clone()))
        .chain(out.shapes.iter().filter_map(|s| s.class.clone()))
        .collect();
    out.styles = doc
        .styles
        .iter()
        .filter(|(k, _)| classes.contains(*k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    out
}

/// Adds the objects of `part` to `doc`, renaming ids that are already
/// taken (`a` → `a1`, `v3` → `v4`). Classes missing from `doc` are added;
/// existing classes are kept. Returns the new objects.
pub fn merge(doc: &mut Document, mut part: Document) -> Selection {
    let mut taken = all_ids(doc);
    let mut rename: HashMap<String, String> = HashMap::new();
    let mut fresh = |id: &mut String, taken: &mut HashSet<String>| {
        if taken.contains(id.as_str()) {
            let base = id.trim_end_matches(|c: char| c.is_ascii_digit());
            let base = if base.is_empty() { "v" } else { base };
            let new = (1..)
                .map(|n| format!("{base}{n}"))
                .find(|c| !taken.contains(c))
                .expect("infinite");
            rename.insert(id.clone(), new.clone());
            *id = new;
        }
        taken.insert(id.clone());
    };
    for v in &mut part.vertices {
        fresh(&mut v.id, &mut taken);
    }
    for id in part
        .edges
        .iter_mut()
        .map(|e| &mut e.id)
        .chain(part.texts.iter_mut().map(|t| &mut t.id))
        .chain(part.axes.iter_mut().map(|a| &mut a.id))
        .chain(part.shapes.iter_mut().map(|s| &mut s.id))
        .flatten()
    {
        fresh(id, &mut taken);
    }
    for g in &mut part.groups {
        fresh(&mut g.id, &mut taken);
    }
    let map = |s: &mut String| {
        if let Some(n) = rename.get(s.as_str()) {
            *s = n.clone();
        }
    };
    for e in &mut part.edges {
        map(&mut e.from);
        map(&mut e.to);
    }
    for g in &mut part.groups {
        g.members.iter_mut().for_each(map);
    }
    for (k, v) in part.styles {
        doc.styles.entry(k).or_insert(v);
    }
    let mut new = Selection::new();
    let n = doc.vertices.len();
    new.extend((n..n + part.vertices.len()).map(ObjRef::Vertex));
    doc.vertices.extend(part.vertices);
    let n = doc.edges.len();
    new.extend((n..n + part.edges.len()).map(ObjRef::Edge));
    doc.edges.extend(part.edges);
    let n = doc.texts.len();
    new.extend((n..n + part.texts.len()).map(ObjRef::Text));
    doc.texts.extend(part.texts);
    let n = doc.axes.len();
    new.extend((n..n + part.axes.len()).map(ObjRef::Axes));
    doc.axes.extend(part.axes);
    let n = doc.shapes.len();
    new.extend((n..n + part.shapes.len()).map(ObjRef::Shape));
    doc.shapes.extend(part.shapes);
    doc.groups.extend(part.groups);
    new
}

// ---- align and distribute -----------------------------------------------------

/// Moves units (each a set of objects with its bounding box) so that they
/// line up, or are evenly spaced, within the bounding box of all of them.
pub fn align(doc: &mut Document, units: &[(Selection, Rect)], op: AlignOp) {
    let Some(all) = units.iter().map(|u| u.1).reduce(|a, b| a.union(b)) else {
        return;
    };
    let mut moves: Vec<(Selection, Vec2)> = Vec::new();
    match op {
        AlignOp::DistributeX | AlignOp::DistributeY => {
            if units.len() < 3 {
                return;
            }
            let c = |r: &Rect| {
                if op == AlignOp::DistributeX {
                    r.center().x
                } else {
                    r.center().y
                }
            };
            let mut order: Vec<&(Selection, Rect)> = units.iter().collect();
            order.sort_by(|a, b| c(&a.1).total_cmp(&c(&b.1)));
            let (lo, hi) = (c(&order[0].1), c(&order[order.len() - 1].1));
            let step = (hi - lo) / (order.len() - 1) as f64;
            for (k, (sel, r)) in order.into_iter().enumerate() {
                let delta = lo + step * k as f64 - c(r);
                let v = if op == AlignOp::DistributeX {
                    Vec2::new(delta, 0.0)
                } else {
                    Vec2::new(0.0, delta)
                };
                moves.push((sel.clone(), v));
            }
        }
        _ => {
            for (sel, r) in units {
                let v = match op {
                    AlignOp::Left => Vec2::new(all.x0 - r.x0, 0.0),
                    AlignOp::CenterX => Vec2::new(all.center().x - r.center().x, 0.0),
                    AlignOp::Right => Vec2::new(all.x1 - r.x1, 0.0),
                    AlignOp::Top => Vec2::new(0.0, all.y1 - r.y1),
                    AlignOp::CenterY => Vec2::new(0.0, all.center().y - r.center().y),
                    AlignOp::Bottom => Vec2::new(0.0, all.y0 - r.y0),
                    _ => unreachable!(),
                };
                moves.push((sel.clone(), v));
            }
        }
    }
    translate_units(doc, &moves);
}

// ---- paths -------------------------------------------------------------------------

fn selected_shapes(doc: &Document, sel: &Selection) -> Vec<usize> {
    let mut v: Vec<usize> = sel
        .iter()
        .filter_map(|o| {
            if let ObjRef::Shape(i) = o {
                Some(*i)
            } else {
                None
            }
        })
        .collect();
    // Bottom first: by layer, then file order (the drawing order).
    v.sort_by_key(|&i| (doc.shapes[i].layer.unwrap_or(d::LAYER_SHAPE), i));
    v
}

/// Combines the selected shapes into one `path` shape, which takes the
/// place, style and id of the bottom-most one. Returns its index.
pub fn boolean(doc: &mut Document, sel: &Selection, op: shapes::BoolOp) -> Result<usize, String> {
    let idx = selected_shapes(doc, sel);
    if idx.len() < 2 {
        return Err(format!("{}: select at least two shapes", op.name()));
    }
    let paths = idx
        .iter()
        .map(|&i| shapes::outline(&doc.shapes[i]))
        .collect::<Result<Vec<_>, _>>()?;
    let result = shapes::boolean(op, &paths).map_err(|e| format!("{}: {e}", op.name()))?;
    let bottom = idx[0];
    let mut s = doc.shapes[bottom].clone();
    s.kind = ShapeKind::Path;
    s.d = Some(shapes::format_d(&result));
    s.at = None;
    s.size = None;
    s.points = None;
    s.rotation = None;
    doc.shapes[bottom] = s;
    let rest: Selection = idx[1..].iter().map(|&i| ObjRef::Shape(i)).collect();
    delete(doc, &rest);
    Ok(bottom - idx[1..].iter().filter(|&&i| i < bottom).count())
}

/// Turns the selected shapes into `path` shapes. Returns how many.
pub fn object_to_path(doc: &mut Document, sel: &Selection) -> usize {
    let mut n = 0;
    for i in selected_shapes(doc, sel) {
        if doc.shapes[i].kind != ShapeKind::Path && shapes::to_path(&mut doc.shapes[i]).is_ok() {
            n += 1;
        }
    }
    n
}

pub fn new_shape(kind: ShapeKind, a: Point, b: Point) -> Shape {
    let r = Rect::from_points(a, b);
    Shape {
        id: None,
        kind,
        at: Some([round(r.center().x), round(r.center().y)]),
        size: Some([round(r.width()), round(r.height())]),
        points: None,
        d: None,
        rotation: None,
        class: None,
        color: None,
        fill: None,
        width: None,
        layer: None,
    }
}

// ---- select same ------------------------------------------------------------------

fn style_of<'a>(doc: &'a Document, class: &Option<String>) -> Option<&'a Style> {
    class.as_ref().and_then(|c| doc.styles.get(c))
}

fn norm_color(c: Option<&String>, default: &str) -> String {
    let s = c.map_or(default, |s| s.as_str());
    Color::parse(s).map_or(s.to_ascii_lowercase(), |c| c.hex())
}

/// The key that Select Same compares, or `None` if it does not apply.
fn same_key(doc: &Document, o: ObjRef, same: Same) -> Option<String> {
    let (color, fill, class, style, kind): (String, String, Option<String>, String, String) =
        match o {
            ObjRef::Vertex(i) => {
                let v = &doc.vertices[i];
                let st = style_of(doc, &v.class);
                let kind = v
                    .kind
                    .or(st.and_then(|s| s.kind))
                    .unwrap_or(VertexKind::None);
                (
                    norm_color(
                        v.color.as_ref().or(st.and_then(|s| s.color.as_ref())),
                        "black",
                    ),
                    norm_color(v.fill.as_ref().or(st.and_then(|s| s.fill.as_ref())), "none"),
                    v.class.clone(),
                    format!(
                        "vertex {}",
                        v.width.or(st.and_then(|s| s.width)).unwrap_or(d::WIDTH)
                    ),
                    format!("vertex {kind:?}"),
                )
            }
            ObjRef::Edge(i) => {
                let e = &doc.edges[i];
                let st = style_of(doc, &e.class);
                let stroke = e
                    .stroke
                    .or(st.and_then(|s| s.stroke))
                    .unwrap_or(Stroke::Plain);
                let double = e.double.or(st.and_then(|s| s.double)).unwrap_or(false);
                let width = e.width.or(st.and_then(|s| s.width)).unwrap_or(d::WIDTH);
                (
                    norm_color(
                        e.color.as_ref().or(st.and_then(|s| s.color.as_ref())),
                        "black",
                    ),
                    "none".into(),
                    e.class.clone(),
                    format!("edge {stroke:?} {double} {width}"),
                    "edge".into(),
                )
            }
            ObjRef::Text(i) => {
                let t = &doc.texts[i];
                let st = style_of(doc, &t.class);
                (
                    norm_color(
                        t.color.as_ref().or(st.and_then(|s| s.color.as_ref())),
                        "black",
                    ),
                    "none".into(),
                    t.class.clone(),
                    "text".into(),
                    "text".into(),
                )
            }
            ObjRef::Axes(i) => {
                let a = &doc.axes[i];
                (
                    norm_color(a.color.as_ref(), "black"),
                    "none".into(),
                    a.class.clone(),
                    format!("axes {}", a.width.unwrap_or(d::WIDTH)),
                    "axes".into(),
                )
            }
            ObjRef::Shape(i) => {
                let s = &doc.shapes[i];
                let st = style_of(doc, &s.class);
                (
                    norm_color(
                        s.color.as_ref().or(st.and_then(|s| s.color.as_ref())),
                        "black",
                    ),
                    norm_color(s.fill.as_ref().or(st.and_then(|s| s.fill.as_ref())), "none"),
                    s.class.clone(),
                    format!(
                        "shape {}",
                        s.width.or(st.and_then(|s| s.width)).unwrap_or(d::WIDTH)
                    ),
                    format!("shape {}", shapes::kind_name(s.kind)),
                )
            }
        };
    match same {
        Same::FillAndStroke => Some(format!("{color} {fill}")),
        Same::StrokeColor => Some(color),
        Same::StrokeStyle => Some(style),
        Same::Class => class,
        Same::Kind => Some(kind),
    }
}

/// All objects that match a selected object in the given respect.
pub fn select_same(doc: &Document, sel: &Selection, same: Same) -> Selection {
    let keys: HashSet<String> = sel.iter().filter_map(|o| same_key(doc, *o, same)).collect();
    all_objects(doc)
        .into_iter()
        .filter(|o| same_key(doc, *o, same).is_some_and(|k| keys.contains(&k)))
        .collect()
}

/// Signed height (sagitta) of an arc with chord `l` and bend `bend` (°):
/// how far it bulges from the chord, + to the left.
pub fn arc_height(l: f64, bend: f64) -> f64 {
    l / 2.0 * (bend.to_radians() / 2.0).tan()
}

/// The bend (°) of the arc with chord `l` and signed height `h`.
pub fn bend_from_height(l: f64, h: f64) -> f64 {
    (2.0 * (2.0 * h / l).atan())
        .to_degrees()
        .clamp(-179.0, 179.0)
}

/// Radius of an arc with chord `l` and bend `bend` (°); `None` if straight.
pub fn arc_radius(l: f64, bend: f64) -> Option<f64> {
    (bend != 0.0).then(|| l / (2.0 * bend.to_radians().sin().abs()))
}

/// The bend (°) of the arc with chord `l` and radius `r` (at least `l/2`),
/// bulging to the side of `sign` (+ left); `deep` picks the arc that is
/// more than half a circle.
pub fn bend_from_radius(l: f64, r: f64, deep: bool, sign: f64) -> f64 {
    let half = (l / (2.0 * r.max(l / 2.0)))
        .clamp(-1.0, 1.0)
        .asin()
        .to_degrees();
    let b = if deep { 180.0 - half } else { half };
    let sign = if sign < 0.0 { -1.0 } else { 1.0 };
    (sign * b).clamp(-179.0, 179.0)
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
    fn arc_height_radius_and_bend_agree() {
        let l = 40.0;
        for bend in [30.0, -60.0, 90.0, 135.0] {
            let h = arc_height(l, bend);
            assert!((bend_from_height(l, h) - bend).abs() < 1e-9);
            let r = arc_radius(l, bend).unwrap();
            let back = bend_from_radius(l, r, bend.abs() > 90.0, bend);
            assert!((back - bend).abs() < 1e-9, "{bend} -> {back}");
            // r = L²/(8h) + h/2
            assert!((r - (l * l / (8.0 * h.abs()) + h.abs() / 2.0)).abs() < 1e-9);
        }
        assert_eq!(arc_height(l, 90.0).round(), 20.0, "semicircle");
        assert_eq!(arc_radius(l, 0.0), None);
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

    #[test]
    fn rotate_quarter_turns() {
        let mut d = doc();
        let all = all_objects(&d);
        let orig = d.clone();
        for _ in 0..4 {
            rotate(&mut d, &all, 90.0);
        }
        for (a, b) in d.vertices.iter().zip(&orig.vertices) {
            assert!((a.at[0] - b.at[0]).abs() < 0.05 && (a.at[1] - b.at[1]).abs() < 0.05);
        }
        assert!(goldstone_core::scene::build(&d).is_ok());
    }

    #[test]
    fn group_and_ungroup() {
        let mut d = doc();
        let sel: Selection = [ObjRef::Vertex(0), ObjRef::Edge(0)].into();
        let g = group(&mut d, &sel).unwrap();
        assert!(d.edges[0].id.is_some(), "members get explicit ids");
        assert_eq!(group_of(&d, ObjRef::Edge(0)), Some(0));
        assert_eq!(with_group(&d, ObjRef::Vertex(0)), sel);
        assert!(goldstone_core::scene::build(&d).is_ok(), "{g}");
        assert_eq!(ungroup(&mut d, &[ObjRef::Vertex(0)].into()), 1);
        assert!(d.groups.is_empty());
    }

    #[test]
    fn copy_paste_renames() {
        let mut d = doc();
        let n = d.vertices.len();
        let part = extract(&d, &[ObjRef::Edge(0)].into());
        assert_eq!(part.vertices.len(), 2);
        let text = goldstone_core::io::to_toml(&part);
        let new = merge(&mut d, goldstone_core::io::parse(&text).unwrap());
        assert_eq!(d.vertices.len(), n + 2);
        assert_eq!(new.len(), 3);
        assert!(goldstone_core::scene::build(&d).is_ok());
        let e = d.edges.last().unwrap();
        assert!(e.from != d.edges[0].from && e.to != d.edges[0].to);
    }

    #[test]
    fn align_left_and_distribute() {
        let mut d = doc();
        let units: Vec<(Selection, Rect)> = (0..d.vertices.len())
            .map(|i| {
                let p = pt(d.vertices[i].at);
                ([ObjRef::Vertex(i)].into(), Rect::from_points(p, p))
            })
            .collect();
        align(&mut d, &units, AlignOp::Left);
        assert!(d.vertices.iter().all(|v| v.at[0] == 0.0));
    }

    #[test]
    fn union_of_two_boxes() {
        let mut d = Document::default();
        d.shapes.push(new_shape(
            ShapeKind::Box,
            Point::new(0.0, 0.0),
            Point::new(10.0, 10.0),
        ));
        d.shapes.push(new_shape(
            ShapeKind::Ellipse,
            Point::new(5.0, 0.0),
            Point::new(15.0, 10.0),
        ));
        d.shapes[0].fill = Some("red".into());
        let all = all_objects(&d);
        let i = boolean(&mut d, &all, shapes::BoolOp::Union).unwrap();
        assert_eq!((i, d.shapes.len()), (0, 1));
        assert_eq!(d.shapes[0].kind, ShapeKind::Path);
        assert_eq!(
            d.shapes[0].fill.as_deref(),
            Some("red"),
            "style of the bottom shape"
        );
        assert!(goldstone_core::scene::build(&d).is_ok());
    }

    #[test]
    fn select_same_stroke_color() {
        let d = doc();
        let all = select_same(&d, &[ObjRef::Edge(0)].into(), Same::StrokeColor);
        assert!(all.len() >= d.edges.len());
    }
}

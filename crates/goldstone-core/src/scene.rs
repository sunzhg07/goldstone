//! Resolves a [`Document`] (styles, defaults, validation) into a [`Scene`]:
//! a flat display list of paths and text that every backend draws the same way.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;

use kurbo::{Affine, BezPath, Circle, Point, Rect, Shape as _, Vec2};

use crate::color::Color;
use crate::geom::{self, ArcLen, left_normal, pt};
use crate::model::*;

/// Built-in defaults (pt). Proportions follow the user's existing JaxoDraw
/// figures: arrow length ≈ 7× and arrow width ≈ 5.6× the line width.
pub mod defaults {
    pub const WIDTH: f64 = 1.0;
    pub const MARGIN: f64 = 4.0;
    pub const DASH: [f64; 2] = [4.0, 3.0];
    pub const DOT_GAP: f64 = 3.0;
    pub const DOUBLE_SEP: f64 = 2.5;
    pub const EDGE_TICK_SPACING: f64 = 6.0;
    pub const EDGE_TICK_LENGTH: f64 = 5.0;
    /// Distance between the bars of a double or triple mark.
    pub const MARK_GAP: f64 = 2.5;
    pub const ARROW_POS: f64 = 0.5;
    pub const ARROW_LENGTH: f64 = 7.0;
    pub const ARROW_WIDTH: f64 = 5.6;
    pub const WAVY: (f64, f64) = (3.0, 8.0);
    pub const CURLY: (f64, f64) = (4.0, 7.0);
    pub const ZIGZAG: (f64, f64) = (3.0, 6.0);
    pub const LABEL_POS: f64 = 0.5;
    pub const LABEL_OFFSET: f64 = 3.0;
    pub const TEXT_SIZE: f64 = 10.0;
    pub const LOOP_RADIUS: f64 = 12.0;
    pub const LOOP_ANGLE: f64 = 90.0;
    pub const AXIS_LENGTH: f64 = 50.0;
    pub const TICK_SPACING: f64 = 10.0;
    pub const TICK_LENGTH: f64 = 3.0;
    pub const LAYER_SHAPE: i32 = 0;
    pub const LAYER_EDGE: i32 = 0;
    pub const LAYER_VERTEX: i32 = 1;
    pub const LAYER_TEXT: i32 = 2;

    use crate::model::VertexKind;
    pub fn vertex_size(kind: VertexKind) -> f64 {
        match kind {
            VertexKind::None => 0.0,
            VertexKind::Dot => 4.0,
            VertexKind::Cross => 7.0,
            VertexKind::Ocross => 8.0,
            VertexKind::Circle => 6.0,
            VertexKind::Square => 4.0,
        }
    }
}
use defaults as d;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    Butt,
    Round,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Join {
    Miter,
    Round,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrokeSpec {
    pub color: Color,
    pub width: f64,
    /// `[on, off]` in pt, already fitted to the path length.
    pub dash: Option<[f64; 2]>,
    pub cap: Cap,
    pub join: Join,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathItem {
    pub path: BezPath,
    pub stroke: Option<StrokeSpec>,
    pub fill: Option<Color>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    /// The anchor point.
    pub at: Point,
    pub anchor: Anchor,
    /// Text with LaTeX math in `$…$`.
    pub content: String,
    pub size: f64,
    /// Whether the size was set explicitly (TikZ then sets the font size).
    pub size_explicit: bool,
    pub color: Color,
    /// Degrees, counter-clockwise.
    pub rotation: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Mark {
    Path(PathItem),
    Text(TextItem),
}

/// Reference to a document object by kind and index into its list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ObjRef {
    Vertex(usize),
    Edge(usize),
    Text(usize),
    Axes(usize),
    Shape(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// Id of the document object this item was generated from.
    pub owner: String,
    /// The document object this item was generated from.
    pub obj: ObjRef,
    pub layer: i32,
    pub mark: Mark,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    /// Items in drawing order.
    pub items: Vec<Item>,
    pub margin: f64,
    pub background: Option<Color>,
    /// Per edge (same index as the document): the base path from vertex to
    /// vertex, before trimming and stroke decoration. Used for editing handles.
    pub edge_paths: Vec<Option<BezPath>>,
}

impl Scene {
    /// Bounding box of all path items, including half the stroke width.
    /// Text is not included; backends add it once text is typeset.
    pub fn path_bounds(&self) -> Option<Rect> {
        let mut acc: Option<Rect> = None;
        for item in &self.items {
            if let Mark::Path(p) = &item.mark {
                if p.path.elements().is_empty() {
                    continue;
                }
                let w = p.stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
                let r = p.path.bounding_box().inflate(w, w);
                acc = Some(acc.map_or(r, |a| a.union(r)));
            }
        }
        acc
    }
}

/// One or more validation errors, each a complete sentence.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostics(pub Vec<String>);

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "error: {e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostics {}

struct Ctx<'a> {
    styles: &'a BTreeMap<String, Style>,
    errors: Vec<String>,
    items: Vec<Item>,
    /// Object whose items are currently being generated.
    current: ObjRef,
}

impl Ctx<'_> {
    fn style(&mut self, class: &Option<String>, what: &str) -> Style {
        match class {
            None => Style::default(),
            Some(name) => match self.styles.get(name) {
                Some(s) => s.clone(),
                None => {
                    self.errors.push(format!(
                        "{what}: unknown class {name:?} (no [style.{name}] table)"
                    ));
                    Style::default()
                }
            },
        }
    }

    fn color(
        &mut self,
        a: &Option<String>,
        b: &Option<String>,
        default: Color,
        what: &str,
    ) -> Color {
        match a.as_ref().or(b.as_ref()) {
            None => default,
            Some(s) => Color::parse(s).unwrap_or_else(|e| {
                self.errors.push(format!("{what}: {e}"));
                default
            }),
        }
    }

    /// `None` = no fill.
    fn fill(&mut self, a: &Option<String>, b: &Option<String>, what: &str) -> Option<Color> {
        match a.as_ref().or(b.as_ref()) {
            None => None,
            Some(s) if s.eq_ignore_ascii_case("none") => None,
            Some(s) => match Color::parse(s) {
                Ok(c) => Some(c),
                Err(e) => {
                    self.errors.push(format!("{what}: {e}"));
                    None
                }
            },
        }
    }

    fn positive(&mut self, v: f64, name: &str, what: &str) -> f64 {
        if !(v.is_finite() && v > 0.0) {
            self.errors
                .push(format!("{what}: {name} must be a positive number, got {v}"));
            1.0
        } else {
            v
        }
    }

    fn non_negative(&mut self, v: f64, name: &str, what: &str) -> f64 {
        if !(v.is_finite() && v >= 0.0) {
            self.errors
                .push(format!("{what}: {name} must be zero or positive, got {v}"));
            0.0
        } else {
            v
        }
    }

    fn fraction(&mut self, v: f64, name: &str, what: &str) -> f64 {
        if !(0.0..=1.0).contains(&v) {
            self.errors
                .push(format!("{what}: {name} must be between 0 and 1, got {v}"));
            v.clamp(0.0, 1.0)
        } else {
            v
        }
    }

    fn push_path(
        &mut self,
        owner: &str,
        layer: i32,
        path: BezPath,
        stroke: Option<StrokeSpec>,
        fill: Option<Color>,
    ) {
        self.items.push(Item {
            owner: owner.to_string(),
            obj: self.current,
            layer,
            mark: Mark::Path(PathItem { path, stroke, fill }),
        });
    }

    fn push_text(&mut self, owner: &str, layer: i32, text: TextItem) {
        self.items.push(Item {
            owner: owner.to_string(),
            obj: self.current,
            layer,
            mark: Mark::Text(text),
        });
    }
}

fn plain_stroke(color: Color, width: f64) -> StrokeSpec {
    StrokeSpec {
        color,
        width,
        dash: None,
        cap: Cap::Butt,
        join: Join::Miter,
    }
}

/// The anchor of a label box that faces back towards a point the label was
/// pushed away from in direction `v`.
pub fn anchor_facing(v: Vec2) -> Anchor {
    let sector = ((v.y.atan2(v.x).to_degrees() / 45.0).round() as i32).rem_euclid(8);
    match sector {
        0 => Anchor::West,
        1 => Anchor::SouthWest,
        2 => Anchor::South,
        3 => Anchor::SouthEast,
        4 => Anchor::East,
        5 => Anchor::NorthEast,
        6 => Anchor::North,
        _ => Anchor::NorthWest,
    }
}

#[derive(Clone, Copy)]
struct VertexInfo {
    at: Point,
    kind: VertexKind,
    size: f64,
}

impl VertexInfo {
    /// Distance by which edges stop short of this vertex's center.
    fn trim(&self) -> f64 {
        match self.kind {
            VertexKind::Circle | VertexKind::Ocross => self.size / 2.0,
            _ => 0.0,
        }
    }
}

/// Assigns ids to objects that have none (`e1`, `t1`, …), avoiding collisions.
fn auto_id(
    explicit: &Option<String>,
    prefix: &str,
    index: usize,
    taken: &HashSet<String>,
) -> String {
    if let Some(id) = explicit {
        return id.clone();
    }
    let mut n = index + 1;
    loop {
        let id = format!("{prefix}{n}");
        if !taken.contains(&id) {
            return id;
        }
        n += 1;
    }
}

/// Puts every object of `doc` into one group `id` (replacing its groups);
/// objects without an id get their automatic one.
pub fn group_everything<'a>(doc: &'a mut Document, id: &str) -> &'a mut crate::model::Group {
    let ids = ids(doc);
    for (e, i) in doc.edges.iter_mut().zip(&ids.edges) {
        e.id.get_or_insert_with(|| i.clone());
    }
    for (t, i) in doc.texts.iter_mut().zip(&ids.texts) {
        t.id.get_or_insert_with(|| i.clone());
    }
    for (a, i) in doc.axes.iter_mut().zip(&ids.axes) {
        a.id.get_or_insert_with(|| i.clone());
    }
    for (s, i) in doc.shapes.iter_mut().zip(&ids.shapes) {
        s.id.get_or_insert_with(|| i.clone());
    }
    let members: Vec<String> = doc
        .vertices
        .iter()
        .map(|v| v.id.clone())
        .chain(ids.edges)
        .chain(ids.texts)
        .chain(ids.axes)
        .chain(ids.shapes)
        .collect();
    // The group's id must differ from every object's.
    let id = std::iter::once(id.to_string())
        .chain((2..).map(|n| format!("{id}{n}")))
        .find(|c| !members.contains(c))
        .expect("infinite");
    doc.groups = vec![crate::model::Group {
        id,
        members,
        latex: None,
        latex_rule: None,
        plot: None,
    }];
    &mut doc.groups[0]
}

/// The id of every object; objects without one get an automatic id
/// (`e1`, `t1`, `ax1`, `s1`), as in error messages and exports.
#[derive(Debug, Clone, PartialEq)]
pub struct Ids {
    pub edges: Vec<String>,
    pub texts: Vec<String>,
    pub axes: Vec<String>,
    pub shapes: Vec<String>,
}

impl Ids {
    pub fn of(&self, doc: &Document, r: ObjRef) -> String {
        let get = |v: &[String], i: usize| v.get(i).cloned().unwrap_or_default();
        match r {
            ObjRef::Vertex(i) => doc.vertices.get(i).map_or(String::new(), |v| v.id.clone()),
            ObjRef::Edge(i) => get(&self.edges, i),
            ObjRef::Text(i) => get(&self.texts, i),
            ObjRef::Axes(i) => get(&self.axes, i),
            ObjRef::Shape(i) => get(&self.shapes, i),
        }
    }
}

pub fn ids(doc: &Document) -> Ids {
    let mut taken: HashSet<String> = doc
        .vertices
        .iter()
        .map(|v| Some(v.id.clone()))
        .chain(doc.edges.iter().map(|e| e.id.clone()))
        .chain(doc.texts.iter().map(|t| t.id.clone()))
        .chain(doc.axes.iter().map(|a| a.id.clone()))
        .chain(doc.shapes.iter().map(|s| s.id.clone()))
        .chain(doc.groups.iter().map(|g| Some(g.id.clone())))
        .flatten()
        .collect();
    let mut assign = |explicit: Vec<&Option<String>>, prefix: &str| -> Vec<String> {
        let out: Vec<String> = explicit
            .iter()
            .enumerate()
            .map(|(i, e)| auto_id(e, prefix, i, &taken))
            .collect();
        taken.extend(out.iter().cloned());
        out
    };
    Ids {
        edges: assign(doc.edges.iter().map(|e| &e.id).collect(), "e"),
        texts: assign(doc.texts.iter().map(|t| &t.id).collect(), "t"),
        axes: assign(doc.axes.iter().map(|a| &a.id).collect(), "ax"),
        shapes: assign(doc.shapes.iter().map(|s| &s.id).collect(), "s"),
    }
}

/// Validates a document and builds its scene.
pub fn build(doc: &Document) -> Result<Scene, Diagnostics> {
    let mut ctx = Ctx {
        styles: &doc.styles,
        errors: Vec::new(),
        items: Vec::new(),
        current: ObjRef::Vertex(0),
    };
    let mut edge_paths: Vec<Option<BezPath>> = vec![None; doc.edges.len()];

    if doc.format == 0 || doc.format > FORMAT_VERSION {
        ctx.errors.push(format!(
            "format = {} is not supported by this version of Goldstone (supports 1..={FORMAT_VERSION})",
            doc.format
        ));
    }

    // ---- ids -------------------------------------------------------------
    let mut taken: HashSet<String> = HashSet::new();
    let explicit = doc
        .vertices
        .iter()
        .map(|v| Some(v.id.clone()))
        .chain(doc.edges.iter().map(|e| e.id.clone()))
        .chain(doc.texts.iter().map(|t| t.id.clone()))
        .chain(doc.axes.iter().map(|a| a.id.clone()))
        .chain(doc.shapes.iter().map(|s| s.id.clone()))
        .chain(doc.groups.iter().map(|g| Some(g.id.clone())));
    for id in explicit.flatten() {
        if id.is_empty() {
            ctx.errors.push("an id must not be empty".into());
        } else if !taken.insert(id.clone()) {
            ctx.errors.push(format!("id {id:?} is used more than once"));
        }
    }
    let Ids {
        edges: edge_ids,
        texts: text_ids,
        axes: axes_ids,
        shapes: shape_ids,
    } = ids(doc);
    taken.extend(edge_ids.iter().cloned());
    taken.extend(text_ids.iter().cloned());
    taken.extend(axes_ids.iter().cloned());
    taken.extend(shape_ids.iter().cloned());

    for g in &doc.groups {
        for m in &g.members {
            if !taken.contains(m) {
                ctx.errors
                    .push(format!("group {:?}: unknown member {m:?}", g.id));
            }
        }
    }

    // ---- shapes ----------------------------------------------------------
    for (idx, (s, id)) in doc.shapes.iter().zip(&shape_ids).enumerate() {
        ctx.current = ObjRef::Shape(idx);
        let what = format!("shape {id:?}");
        let st = ctx.style(&s.class, &what);
        let color = ctx.color(&s.color, &st.color, Color::BLACK, &what);
        let fill = ctx.fill(&s.fill, &st.fill, &what);
        let width = s.width.or(st.width).unwrap_or(d::WIDTH);
        let width = ctx.non_negative(width, "width", &what);
        let layer = s.layer.or(st.layer).unwrap_or(d::LAYER_SHAPE);
        let path = match crate::shapes::outline_open(s) {
            Ok(p) => p,
            Err(e) => {
                ctx.errors.push(format!("{what}: {e}"));
                continue;
            }
        };
        let stroke = (width > 0.0).then(|| plain_stroke(color, width));
        ctx.push_path(id, layer, path, stroke, fill);
    }

    // ---- axes ------------------------------------------------------------
    for (idx, (a, id)) in doc.axes.iter().zip(&axes_ids).enumerate() {
        ctx.current = ObjRef::Axes(idx);
        let what = format!("axes {id:?}");
        let st = ctx.style(&a.class, &what);
        let color = ctx.color(&a.color, &st.color, Color::BLACK, &what);
        let width = a.width.or(st.width).unwrap_or(d::WIDTH);
        let width = ctx.positive(width, "width", &what);
        let layer = a.layer.or(st.layer).unwrap_or(d::LAYER_EDGE);
        let alen = st.arrow_length.unwrap_or(d::ARROW_LENGTH);
        let awid = st.arrow_width.unwrap_or(d::ARROW_WIDTH);
        let size = st.text_size.unwrap_or(d::TEXT_SIZE);
        let o = pt(a.origin);
        for (axis, dir) in [(&a.x, Vec2::new(1.0, 0.0)), (&a.y, Vec2::new(0.0, 1.0))] {
            let axis = axis.clone().unwrap_or_default();
            if !axis.show.unwrap_or(true) {
                continue;
            }
            let len = ctx.positive(axis.length.unwrap_or(d::AXIS_LENGTH), "axis length", &what);
            let end = o + dir * len;
            let arrow = axis.arrow.unwrap_or(true);
            let line_end = if arrow { end - dir * (alen * 0.5) } else { end };
            ctx.push_path(
                id,
                layer,
                geom::line(o, line_end),
                Some(plain_stroke(color, width)),
                None,
            );
            if arrow {
                let head = geom::arrowhead(end - dir * (alen / 2.0), dir, alen, awid);
                ctx.push_path(id, layer, head, None, Some(color));
            }
            let normal = left_normal(dir); // x axis: up; y axis: left
            let outward = if dir.x > 0.0 { -normal } else { normal }; // below / left
            if axis.ticks.unwrap_or(false) {
                let spacing = ctx.positive(
                    axis.tick_spacing.unwrap_or(d::TICK_SPACING),
                    "tick_spacing",
                    &what,
                );
                let tl = axis.tick_length.unwrap_or(d::TICK_LENGTH);
                let usable = len - if arrow { alen } else { 0.0 };
                let labels = axis.tick_labels.clone().unwrap_or_default();
                let mut i = 1usize;
                while spacing * i as f64 <= usable + 1e-9 && i < 10_000 {
                    let p = o + dir * (spacing * i as f64);
                    ctx.push_path(
                        id,
                        layer,
                        geom::line(p + normal * (tl / 2.0), p - normal * (tl / 2.0)),
                        Some(plain_stroke(color, width)),
                        None,
                    );
                    if let Some(lbl) = labels.get(i - 1).filter(|l| !l.is_empty()) {
                        ctx.push_text(
                            id,
                            layer + d::LAYER_TEXT,
                            TextItem {
                                at: p + outward * (tl / 2.0 + 2.0),
                                anchor: anchor_facing(outward),
                                content: lbl.clone(),
                                size,
                                size_explicit: st.text_size.is_some(),
                                color,
                                rotation: 0.0,
                            },
                        );
                    }
                    i += 1;
                }
                if labels.len() >= i {
                    ctx.errors.push(format!(
                        "{what}: more tick_labels ({}) than ticks ({})",
                        labels.len(),
                        i - 1
                    ));
                }
            }
            if let Some(lbl) = axis.label.as_ref().filter(|l| !l.is_empty()) {
                ctx.push_text(
                    id,
                    layer + d::LAYER_TEXT,
                    TextItem {
                        at: end + dir * 3.0,
                        anchor: anchor_facing(dir),
                        content: lbl.clone(),
                        size,
                        size_explicit: st.text_size.is_some(),
                        color,
                        rotation: 0.0,
                    },
                );
            }
        }
    }

    // ---- vertices (resolve first; drawn after edges) ---------------------
    let mut vinfo: HashMap<&str, VertexInfo> = HashMap::new();
    let mut vertex_marks = Vec::new();
    for v in &doc.vertices {
        let what = format!("vertex {:?}", v.id);
        let st = ctx.style(&v.class, &what);
        let kind = v.kind.or(st.kind).unwrap_or(VertexKind::None);
        let size = v.size.or(st.size).unwrap_or_else(|| d::vertex_size(kind));
        let size = ctx.non_negative(size, "size", &what);
        let color = ctx.color(&v.color, &st.color, Color::BLACK, &what);
        let fill = ctx.fill(&v.fill, &st.fill, &what);
        let width = v.width.or(st.width).unwrap_or(d::WIDTH);
        let width = ctx.positive(width, "width", &what);
        let layer = v.layer.or(st.layer).unwrap_or(d::LAYER_VERTEX);
        let at = pt(v.at);
        vinfo.insert(v.id.as_str(), VertexInfo { at, kind, size });
        vertex_marks.push((v, kind, size, color, fill, width, layer, at, st));
    }

    // ---- edges -----------------------------------------------------------
    for (idx, (e, id)) in doc.edges.iter().zip(&edge_ids).enumerate() {
        ctx.current = ObjRef::Edge(idx);
        let what = format!("edge {id:?} ({} → {})", e.from, e.to);
        let (Some(va), Some(vb)) = (
            vinfo.get(e.from.as_str()).copied(),
            vinfo.get(e.to.as_str()).copied(),
        ) else {
            // Still resolve the style so its errors are reported too.
            let st = ctx.style(&e.class, &what);
            ctx.color(&e.color, &st.color, Color::BLACK, &what);
            for end in [&e.from, &e.to] {
                if !vinfo.contains_key(end.as_str()) {
                    ctx.errors.push(format!("{what}: unknown vertex {end:?}"));
                }
            }
            continue;
        };
        let st = ctx.style(&e.class, &what);
        let stroke = e.stroke.or(st.stroke).unwrap_or(Stroke::Plain);
        let color = ctx.color(&e.color, &st.color, Color::BLACK, &what);
        let width = e.width.or(st.width).unwrap_or(d::WIDTH);
        let width = ctx.positive(width, "width", &what);
        let layer = e.layer.or(st.layer).unwrap_or(d::LAYER_EDGE);
        let double = e.double.or(st.double).unwrap_or(false);
        let double_sep = e.double_sep.or(st.double_sep).unwrap_or(d::DOUBLE_SEP);
        let arrow = e.arrow.or(st.arrow).unwrap_or(ArrowDir::None);
        let arrow_pos = e.arrow_pos.or(st.arrow_pos).unwrap_or(d::ARROW_POS);
        let arrow_pos = ctx.fraction(arrow_pos, "arrow_pos", &what);
        let alen = e
            .arrow_length
            .or(st.arrow_length)
            .unwrap_or(d::ARROW_LENGTH);
        let alen = ctx.positive(alen, "arrow_length", &what);
        let awid = e.arrow_width.or(st.arrow_width).unwrap_or(d::ARROW_WIDTH);
        let awid = ctx.positive(awid, "arrow_width", &what);

        // Base geometry.
        let base = if e.from == e.to {
            if e.bend.is_some() || e.controls.is_some() {
                ctx.errors.push(format!(
                    "{what}: a loop (from == to) cannot have `bend` or `controls`"
                ));
            }
            let lp = e.loop_.clone().unwrap_or(LoopSpec {
                radius: None,
                angle: None,
            });
            let r = ctx.positive(lp.radius.unwrap_or(d::LOOP_RADIUS), "loop radius", &what);
            geom::loop_at(va.at, r, lp.angle.unwrap_or(d::LOOP_ANGLE))
        } else {
            if e.loop_.is_some() {
                ctx.errors
                    .push(format!("{what}: `loop` is only valid when from == to"));
            }
            match (e.bend, e.controls) {
                (Some(_), Some(_)) => {
                    ctx.errors
                        .push(format!("{what}: use either `bend` or `controls`, not both"));
                    continue;
                }
                (Some(b), None) => {
                    if b.is_nan() || b.abs() >= 180.0 {
                        ctx.errors.push(format!(
                            "{what}: bend must be between -180 and 180 degrees, got {b}"
                        ));
                        continue;
                    }
                    geom::bent(va.at, vb.at, b)
                }
                (None, Some([c1, c2])) => geom::cubic(va.at, pt(c1), pt(c2), vb.at),
                (None, None) => geom::line(va.at, vb.at),
            }
        };
        let full = ArcLen::new(&base);
        if full.length() < 1e-6 {
            ctx.errors.push(format!("{what}: the edge has zero length"));
            continue;
        }
        edge_paths[idx] = Some(base.clone());
        let (ta, tb) = (va.trim(), vb.trim());
        let base = if (ta > 0.0 || tb > 0.0) && full.length() > ta + tb + 1e-6 {
            full.sub_path(ta, full.length() - tb)
        } else {
            base
        };
        let al = ArcLen::new(&base);
        let len = al.length();

        let (amp_wl, extra_offset) = match stroke {
            Stroke::Wavy | Stroke::Curly | Stroke::Zigzag => {
                let (da, dw) = match stroke {
                    Stroke::Wavy => d::WAVY,
                    Stroke::Curly => d::CURLY,
                    _ => d::ZIGZAG,
                };
                let amp = ctx.positive(
                    e.amplitude.or(st.amplitude).unwrap_or(da),
                    "amplitude",
                    &what,
                );
                let wl = ctx.positive(
                    e.wavelength.or(st.wavelength).unwrap_or(dw),
                    "wavelength",
                    &what,
                );
                (Some((amp, wl)), amp)
            }
            _ => (None, 0.0),
        };
        let double_sep = if double {
            ctx.positive(double_sep, "double_sep", &what)
        } else {
            0.0
        };
        let extra_offset = extra_offset + double_sep / 2.0;
        let decorated =
            e.ticks.or(st.ticks).unwrap_or(false) || e.marks.or(st.marks).unwrap_or(0) > 0;
        let extra_offset = if decorated {
            extra_offset.max(
                double_sep / 2.0
                    + e.tick_length
                        .or(st.tick_length)
                        .unwrap_or(d::EDGE_TICK_LENGTH)
                        / 2.0,
            )
        } else {
            extra_offset
        };
        // The center line(s): the base path, or two copies offset by half
        // the separation on either side. Every stroke style is drawn along
        // each of them.
        let centers: Vec<BezPath> = if double {
            let n = ((len / 3.0).ceil() as usize).max(4);
            let h = double_sep / 2.0;
            vec![
                geom::displaced(&al, n, |_| (0.0, h)),
                geom::displaced(&al, n, |_| (0.0, -h)),
            ]
        } else {
            vec![base.clone()]
        };

        let mut spec = plain_stroke(color, width);
        let offsets: Vec<f64> = if double {
            vec![double_sep / 2.0, -double_sep / 2.0]
        } else {
            vec![0.0]
        };
        let wiggle = |f: fn(&ArcLen, f64, f64, f64) -> BezPath, a: f64, w: f64| -> Vec<BezPath> {
            offsets.iter().map(|&o| f(&al, a, w, o)).collect()
        };
        let paths: Vec<BezPath> = match (stroke, amp_wl) {
            (Stroke::Wavy, Some((a, w))) => {
                spec.cap = Cap::Round;
                spec.join = Join::Round;
                wiggle(geom::wavy, a, w)
            }
            (Stroke::Curly, Some((a, w))) => {
                spec.cap = Cap::Round;
                spec.join = Join::Round;
                wiggle(geom::curly, a, w)
            }
            (Stroke::Zigzag, Some((a, w))) => wiggle(geom::zigzag, a, w),
            _ => {
                match stroke {
                    Stroke::Dashed => {
                        let [on, off] = e.dash.or(st.dash).unwrap_or(d::DASH);
                        let on = ctx.positive(on, "dash length", &what);
                        let off = ctx.positive(off, "dash gap", &what);
                        spec.dash = Some(geom::fit_dash(len, on, off));
                    }
                    Stroke::Dotted => {
                        let gap = e
                            .dash
                            .or(st.dash)
                            .map_or(d::DOT_GAP.max(2.5 * width), |[_, off]| off);
                        let gap = ctx.positive(gap, "dot gap", &what);
                        spec.dash = Some([0.0, geom::fit_dots(len, gap)]);
                        spec.cap = Cap::Round;
                    }
                    _ => {}
                }
                centers
            }
        };
        for p in paths {
            ctx.push_path(id, layer, p, Some(spec.clone()), None);
        }

        // Ticks along the line and marks at one position: short bars across
        // the line, as long as the line is wide (also across double lines).
        let ticks = e.ticks.or(st.ticks).unwrap_or(false);
        let marks = e.marks.or(st.marks).unwrap_or(0);
        if ticks || marks > 0 {
            if marks > 3 {
                ctx.errors
                    .push(format!("{what}: `marks` must be 0, 1, 2 or 3, got {marks}"));
            }
            let tl = ctx.positive(
                e.tick_length
                    .or(st.tick_length)
                    .unwrap_or(d::EDGE_TICK_LENGTH),
                "tick_length",
                &what,
            );
            let half = tl / 2.0 + double_sep / 2.0;
            let mut bars = BezPath::new();
            let mut bar = |s: f64| {
                let (q, t) = al.eval(s.clamp(0.0, len));
                let n = left_normal(t);
                bars.move_to(q + n * half);
                bars.line_to(q - n * half);
            };
            if ticks {
                let sp = ctx.positive(
                    e.tick_spacing
                        .or(st.tick_spacing)
                        .unwrap_or(d::EDGE_TICK_SPACING),
                    "tick_spacing",
                    &what,
                );
                let n = (len / sp).round().max(1.0) as usize;
                for k in 0..n {
                    let at = len * (k as f64 + 0.5) / n as f64;
                    // Leave the arrowhead free.
                    if arrow == ArrowDir::None || (at - arrow_pos * len).abs() > alen / 2.0 + 1.0 {
                        bar(at);
                    }
                }
            }
            if marks > 0 {
                // Default: the middle, or a quarter when the arrow is there.
                let default_pos = if arrow != ArrowDir::None && (arrow_pos - 0.5).abs() < 0.15 {
                    0.25
                } else {
                    0.5
                };
                let pos = ctx.fraction(
                    e.mark_pos.or(st.mark_pos).unwrap_or(default_pos),
                    "mark_pos",
                    &what,
                );
                let m = marks.min(3) as f64;
                for k in 0..marks.min(3) {
                    bar(pos * len + (k as f64 - (m - 1.0) / 2.0) * d::MARK_GAP);
                }
            }
            let mut tick_spec = plain_stroke(color, width);
            tick_spec.cap = Cap::Butt;
            ctx.push_path(id, layer, bars, Some(tick_spec), None);
        }

        if arrow != ArrowDir::None {
            let (p, t) = al.eval(arrow_pos * len);
            let dir = if arrow == ArrowDir::Forward { t } else { -t };
            ctx.push_path(
                id,
                layer,
                geom::arrowhead(p, dir, alen, awid),
                None,
                Some(color),
            );
        }

        if let Some(label) = e.label.as_ref().filter(|l| !l.is_empty()) {
            let pos = ctx.fraction(
                e.label_pos.or(st.label_pos).unwrap_or(d::LABEL_POS),
                "label_pos",
                &what,
            );
            let side = e.label_side.or(st.label_side).unwrap_or(Side::Left);
            let mut offset = e
                .label_offset
                .or(st.label_offset)
                .unwrap_or(d::LABEL_OFFSET)
                + extra_offset;
            if arrow != ArrowDir::None && (pos - arrow_pos).abs() * len < alen {
                offset = offset.max(d::LABEL_OFFSET + awid / 2.0);
            }
            let (p, t) = al.eval(pos * len);
            let n = if side == Side::Left {
                left_normal(t)
            } else {
                -left_normal(t)
            };
            ctx.push_text(
                id,
                layer + d::LAYER_TEXT,
                TextItem {
                    at: p + n * offset,
                    anchor: anchor_facing(n),
                    content: label.clone(),
                    size: st.text_size.unwrap_or(d::TEXT_SIZE),
                    size_explicit: st.text_size.is_some(),
                    color,
                    rotation: 0.0,
                },
            );
        }
    }

    // ---- vertex marks ----------------------------------------------------
    for (idx, (v, kind, size, color, fill, width, layer, at, st)) in
        vertex_marks.into_iter().enumerate()
    {
        ctx.current = ObjRef::Vertex(idx);
        let id = v.id.as_str();
        let r = size / 2.0;
        let rot = v.rotation.unwrap_or(0.0).to_radians();
        let cross = |half: f64| {
            let mut p = BezPath::new();
            for a in [45f64, 135.0] {
                let dir = Vec2::from_angle(a.to_radians() + rot);
                p.move_to(at - dir * half);
                p.line_to(at + dir * half);
            }
            p
        };
        match kind {
            VertexKind::None => {}
            VertexKind::Dot => {
                ctx.push_path(
                    id,
                    layer,
                    Circle::new(at, r).to_path(geom::CURVE_TOL),
                    None,
                    Some(fill.unwrap_or(color)),
                );
            }
            VertexKind::Square => {
                let sq = Affine::rotate_about(rot, at)
                    * Rect::from_center_size(at, (size, size)).to_path(geom::CURVE_TOL);
                ctx.push_path(id, layer, sq, None, Some(fill.unwrap_or(color)));
            }
            VertexKind::Circle => {
                ctx.push_path(
                    id,
                    layer,
                    Circle::new(at, r).to_path(geom::CURVE_TOL),
                    Some(plain_stroke(color, width)),
                    fill,
                );
            }
            VertexKind::Cross => {
                ctx.push_path(id, layer, cross(r), Some(plain_stroke(color, width)), None);
            }
            VertexKind::Ocross => {
                ctx.push_path(
                    id,
                    layer,
                    Circle::new(at, r).to_path(geom::CURVE_TOL),
                    Some(plain_stroke(color, width)),
                    fill,
                );
                ctx.push_path(id, layer, cross(r), Some(plain_stroke(color, width)), None);
            }
        }
        if let Some(label) = v.label.as_ref().filter(|l| !l.is_empty()) {
            let anchor = v.label_anchor.unwrap_or(Anchor::SouthWest);
            let (ux, uy) = anchor.unit_offset();
            let gap = r + 1.5;
            ctx.push_text(
                id,
                layer + d::LAYER_TEXT,
                TextItem {
                    at: at - Vec2::new(ux, uy) * gap,
                    anchor,
                    content: label.clone(),
                    size: st.text_size.unwrap_or(d::TEXT_SIZE),
                    size_explicit: st.text_size.is_some(),
                    color,
                    rotation: 0.0,
                },
            );
        }
    }

    // ---- free text -------------------------------------------------------
    for (idx, (t, id)) in doc.texts.iter().zip(&text_ids).enumerate() {
        ctx.current = ObjRef::Text(idx);
        let what = format!("text {id:?}");
        let st = ctx.style(&t.class, &what);
        let color = ctx.color(&t.color, &st.color, Color::BLACK, &what);
        let size = ctx.positive(
            t.size.or(st.text_size).unwrap_or(d::TEXT_SIZE),
            "size",
            &what,
        );
        ctx.push_text(
            id,
            t.layer.or(st.layer).unwrap_or(d::LAYER_TEXT),
            TextItem {
                at: pt(t.at),
                anchor: t.anchor.unwrap_or(Anchor::Center),
                content: t.content.clone(),
                size,
                size_explicit: t.size.or(st.text_size).is_some(),
                color,
                rotation: t.rotation.unwrap_or(0.0),
            },
        );
    }

    let margin = doc.canvas.margin.unwrap_or(d::MARGIN);
    let margin = ctx.non_negative(margin, "margin", "[canvas]");
    let background = match &doc.canvas.background {
        None => None,
        Some(s) if s.eq_ignore_ascii_case("none") => None,
        Some(s) => match Color::parse(s) {
            Ok(c) => Some(c),
            Err(e) => {
                ctx.errors.push(format!("[canvas] background: {e}"));
                None
            }
        },
    };

    if !ctx.errors.is_empty() {
        return Err(Diagnostics(ctx.errors));
    }
    let mut items = ctx.items;
    items.sort_by_key(|i| i.layer); // stable: keeps document order within a layer
    Ok(Scene {
        items,
        margin,
        background,
        edge_paths,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn ticks_and_marks() {
        let src = "format = 1\n[[vertex]]\nid = \"a\"\nat = [0, 0]\n[[vertex]]\nid = \"b\"\nat = [60, 0]\n\
                   [[edge]]\nfrom = \"a\"\nto = \"b\"\nticks = true\ntick_spacing = 6\nmarks = 2\n";
        let scene = build(&crate::io::parse(src).unwrap()).unwrap();
        let bars = scene
            .items
            .iter()
            .filter_map(|i| match &i.mark {
                Mark::Path(p) => Some(
                    p.path
                        .elements()
                        .iter()
                        .filter(|e| matches!(e, kurbo::PathEl::MoveTo(_)))
                        .count(),
                ),
                _ => None,
            })
            .max()
            .unwrap();
        assert_eq!(bars, 10 + 2, "ten ticks, two marks");
        let bad = src.replace("marks = 2", "marks = 7");
        assert!(build(&crate::io::parse(&bad).unwrap()).is_err());
    }

    #[test]
    fn every_stroke_can_be_double() {
        for stroke in ["plain", "dashed", "dotted", "wavy", "curly", "zigzag"] {
            let src = format!(
                "format = 1\n[[vertex]]\nid = \"a\"\nat = [0, 0]\n[[vertex]]\nid = \"b\"\nat = [60, 0]\n\
                 [[edge]]\nfrom = \"a\"\nto = \"b\"\nbend = 60\nstroke = \"{stroke}\"\ndouble = true\ndouble_sep = 4\n"
            );
            let doc = crate::io::parse(&src).unwrap();
            let scene = build(&doc).unwrap_or_else(|e| panic!("{stroke}: {e}"));
            let paths = scene
                .items
                .iter()
                .filter(|i| matches!(i.mark, Mark::Path(_)))
                .count();
            assert_eq!(paths, 2, "{stroke}: two lines");
        }
    }

    use super::*;
    use crate::io::parse;
    use kurbo::ParamCurve;

    fn scene(src: &str) -> Result<Scene, Diagnostics> {
        build(&parse(src).expect("parse"))
    }

    #[test]
    fn minimal_edge() {
        let s = scene(
            r#"
            format = 1
            [[vertex]]
            id = "a"
            at = [0, 0]
            kind = "dot"
            [[vertex]]
            id = "b"
            at = [0, 40]
            [[edge]]
            from = "a"
            to = "b"
            arrow = "forward"
            label = "$p$"
            "#,
        )
        .unwrap();
        // line, arrow, label, dot
        assert_eq!(s.items.len(), 4);
        assert!(matches!(s.items.last().unwrap().mark, Mark::Text(_)));
        let Mark::Text(t) = &s.items.last().unwrap().mark else {
            unreachable!()
        };
        // Upward edge: left is −x, so the label sits to the left, anchored east.
        assert_eq!(t.anchor, Anchor::East);
        assert!(t.at.x < 0.0);
    }

    #[test]
    fn reports_all_errors() {
        let err = scene(
            r#"
            format = 1
            [[vertex]]
            id = "a"
            at = [0, 0]
            [[vertex]]
            id = "a"
            at = [1, 0]
            [[edge]]
            from = "a"
            to = "zz"
            [[edge]]
            from = "a"
            to = "a"
            class = "nope"
            color = "chartreuse"
            "#,
        )
        .unwrap_err();
        let all = err.to_string();
        assert!(all.contains("used more than once"), "{all}");
        assert!(all.contains("unknown vertex \"zz\""), "{all}");
        assert!(all.contains("unknown class \"nope\""), "{all}");
        assert!(all.contains("chartreuse"), "{all}");
    }

    #[test]
    fn style_presets_apply_and_objects_override() {
        let s = scene(
            r##"
            format = 1
            [style.hole]
            stroke = "dashed"
            color = "#ff0000"
            width = 2
            [[vertex]]
            id = "a"
            at = [0, 0]
            [[vertex]]
            id = "b"
            at = [50, 0]
            [[edge]]
            from = "a"
            to = "b"
            class = "hole"
            width = 0.5
            "##,
        )
        .unwrap();
        let Mark::Path(p) = &s.items[0].mark else {
            panic!()
        };
        let st = p.stroke.as_ref().unwrap();
        assert_eq!(st.color, Color::rgb(255, 0, 0));
        assert_eq!(st.width, 0.5);
        assert!(st.dash.is_some());
    }

    #[test]
    fn edges_stop_at_circle_vertices() {
        let s = scene(
            r#"
            format = 1
            [[vertex]]
            id = "a"
            at = [0, 0]
            kind = "ocross"
            size = 10
            [[vertex]]
            id = "b"
            at = [50, 0]
            [[edge]]
            from = "a"
            to = "b"
            "#,
        )
        .unwrap();
        let Mark::Path(p) = &s.items[0].mark else {
            panic!()
        };
        let start = p.path.segments().next().unwrap().start();
        assert!((start.x - 5.0).abs() < 1e-6);
    }

    #[test]
    fn anchors() {
        assert_eq!(anchor_facing(Vec2::new(0.0, 1.0)), Anchor::South);
        assert_eq!(anchor_facing(Vec2::new(-1.0, 0.0)), Anchor::East);
        assert_eq!(anchor_facing(Vec2::new(1.0, -1.0)), Anchor::NorthWest);
    }
}

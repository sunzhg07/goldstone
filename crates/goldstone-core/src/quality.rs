//! Measures how readable a drawn diagram is, so that automatic layouts can
//! be compared and improved: crossings, lines running close to vertices,
//! overlapping labels, slanted lines, size.

use kurbo::{BezPath, Point, Rect};

use crate::model::Document;
use crate::scene::{self, Mark, ObjRef};

/// Layout problems found in one diagram. Lower is better.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Quality {
    /// Fermion lines crossing each other.
    pub crossings: usize,
    /// Fermion lines crossing an amplitude bar, or internal lines crossing
    /// an interaction line.
    pub through_operators: usize,
    /// External lines crossing an interaction line (common and acceptable
    /// in Goldstone diagrams, counted separately).
    pub external_over_interaction: usize,
    /// Lines passing within 5 pt of a vertex they do not end at.
    pub near_vertices: usize,
    /// Label boxes overlapping each other.
    pub label_overlaps: usize,
    /// Label boxes overlapping a line (other than their own).
    pub label_on_lines: usize,
    /// Internal fermion lines that are neither vertical nor arcs.
    pub slanted: usize,
    /// Bounding box of the drawing (pt).
    pub width: f64,
    pub height: f64,
}

impl Quality {
    /// One number to rank diagrams by.
    pub fn score(&self) -> f64 {
        10.0 * self.crossings as f64
            + 6.0 * self.through_operators as f64
            + 0.5 * self.external_over_interaction as f64
            + 12.0 * self.near_vertices as f64
            + 8.0 * self.label_overlaps as f64
            + 5.0 * self.label_on_lines as f64
            + 1.0 * self.slanted as f64
    }
}

fn polyline(path: &BezPath) -> Vec<Point> {
    let mut pts = Vec::new();
    kurbo::flatten(path, 0.2, |el| match el {
        kurbo::PathEl::MoveTo(p) | kurbo::PathEl::LineTo(p) => pts.push(p),
        _ => {}
    });
    pts
}

fn seg_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let o = |p: Point, q: Point, r: Point| (q - p).cross(r - p);
    let (d1, d2, d3, d4) = (o(c, d, a), o(c, d, b), o(a, b, c), o(a, b, d));
    d1 * d2 < -1e-9 && d3 * d4 < -1e-9
}

fn polys_cross(a: &[Point], b: &[Point]) -> usize {
    let mut n = 0;
    for x in a.windows(2) {
        for y in b.windows(2) {
            if seg_cross(x[0], x[1], y[0], y[1]) {
                n += 1;
            }
        }
    }
    n
}

fn dist_to_poly(p: Point, poly: &[Point]) -> f64 {
    poly.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let ab = b - a;
            let t = if ab.hypot2() > 0.0 {
                ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (p - (a + ab * t)).hypot()
        })
        .fold(f64::INFINITY, f64::min)
}

fn poly_hits_rect(poly: &[Point], r: Rect) -> bool {
    if poly.iter().any(|p| r.contains(*p)) {
        return true;
    }
    let corners = [
        Point::new(r.x0, r.y0),
        Point::new(r.x1, r.y0),
        Point::new(r.x1, r.y1),
        Point::new(r.x0, r.y1),
    ];
    poly.windows(2)
        .any(|w| (0..4).any(|k| seg_cross(w[0], w[1], corners[k], corners[(k + 1) % 4])))
}

/// Estimated box of a label (no typesetting needed): about 5.5 pt per
/// visible character at 10 pt, sub/superscripts smaller.
fn label_box(at: Point, anchor: crate::model::Anchor, content: &str, size: f64) -> Rect {
    let mut w = 0.0;
    let mut small = false;
    let mut chars = content.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                while chars.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    chars.next();
                }
                w += 5.5; // a symbol
            }
            '_' | '^' => small = true,
            '$' | '{' | '}' | ' ' => {}
            _ => {
                w += if small { 3.8 } else { 5.5 };
                small = false;
            }
        }
    }
    let (w, h) = (w * size / 10.0, 8.0 * size / 10.0);
    let off = anchor.unit_offset();
    // The anchor point sits at that side of the box.
    let cx = at.x - off.0 * w / 2.0;
    let cy = at.y - off.1 * h / 2.0;
    Rect::from_center_size((cx, cy), (w, h))
}

/// Measures a diagram drawn from LaTeX (classes `particle`, `hole`, `line`
/// for fermion lines, `interaction` and `amplitude` for operators).
pub fn measure(doc: &Document) -> Quality {
    let Ok(sc) = scene::build(doc) else {
        return Quality::default();
    };
    let class = |o: ObjRef| match o {
        ObjRef::Edge(i) => doc.edges[i].class.clone().unwrap_or_default(),
        _ => String::new(),
    };
    let fermion = |o: ObjRef| matches!(class(o).as_str(), "particle" | "hole" | "line");
    let operator = |o: ObjRef| matches!(class(o).as_str(), "interaction" | "amplitude");
    // One polyline per edge (its first stroked path: the line itself).
    let mut lines: Vec<(ObjRef, Vec<Point>)> = Vec::new();
    let mut labels: Vec<(ObjRef, Rect)> = Vec::new();
    for it in &sc.items {
        match &it.mark {
            Mark::Path(p) if p.stroke.is_some() && !lines.iter().any(|l| l.0 == it.obj) => {
                lines.push((it.obj, polyline(&p.path)));
            }
            Mark::Text(t) if matches!(it.obj, ObjRef::Edge(_)) => {
                labels.push((it.obj, label_box(t.at, t.anchor, &t.content, t.size)));
            }
            _ => {}
        }
    }
    let mut q = Quality::default();
    let fl: Vec<&(ObjRef, Vec<Point>)> = lines.iter().filter(|l| fermion(l.0)).collect();
    let ops: Vec<&(ObjRef, Vec<Point>)> = lines.iter().filter(|l| operator(l.0)).collect();
    for (k, a) in fl.iter().enumerate() {
        for b in &fl[k + 1..] {
            q.crossings += polys_cross(&a.1, &b.1).min(1);
        }
        let external = match a.0 {
            ObjRef::Edge(i) => {
                doc.edges[i].from.ends_with("_end") || doc.edges[i].to.ends_with("_end")
            }
            _ => false,
        };
        for o in &ops {
            let hit = polys_cross(&a.1, &o.1).min(1);
            if external && class(o.0) == "interaction" {
                q.external_over_interaction += hit;
            } else {
                q.through_operators += hit;
            }
        }
    }
    // Lines near vertices they do not end at.
    for (o, poly) in &fl {
        let ObjRef::Edge(i) = *o else { continue };
        let e = &doc.edges[i];
        for v in &doc.vertices {
            if v.id == e.from || v.id == e.to || v.id.ends_with("_end") {
                continue;
            }
            if dist_to_poly(Point::new(v.at[0], v.at[1]), poly) < 5.0 {
                q.near_vertices += 1;
            }
        }
        // Slanted: an internal straight line that is not vertical.
        let ends_free = e.from.ends_with("_end") || e.to.ends_with("_end");
        if !ends_free && e.bend.is_none() && e.from != e.to {
            let (a, b) = (poly[0], poly[poly.len() - 1]);
            if (a.x - b.x).abs() > 1.0 {
                q.slanted += 1;
            }
        }
    }
    for (k, (oa, a)) in labels.iter().enumerate() {
        for (_, b) in &labels[k + 1..] {
            if a.intersect(*b).area() > 1.0 {
                q.label_overlaps += 1;
            }
        }
        for (ol, poly) in &lines {
            if ol != oa && poly_hits_rect(poly, *a) {
                q.label_on_lines += 1;
            }
        }
    }
    if let Some(b) = sc.path_bounds() {
        q.width = b.width();
        q.height = b.height();
    }
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mbpt2_is_clean() {
        let d = crate::expr::from_latex(r"E = \frac{1}{4} V_{ijab} T_{abij}")
            .unwrap()
            .doc;
        let q = measure(&d);
        assert_eq!(
            (q.crossings, q.near_vertices, q.label_overlaps),
            (0, 0, 0),
            "{q:?}"
        );
    }
}

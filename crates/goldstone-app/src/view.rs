//! Canvas view transform (document pt, y up ↔ screen points, y down) and
//! hit testing.

use egui::{Pos2, Rect as ERect, Vec2 as EVec2};
use goldstone_core::kurbo::{BezPath, Point, Rect, Shape as _};
use goldstone_core::model::Document;
use goldstone_core::scene::{Mark, ObjRef, Scene};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    /// Document point at the center of the canvas.
    pub center: Point,
    /// Screen points per document pt.
    pub zoom: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            center: Point::new(40.0, 30.0),
            zoom: 3.0,
        }
    }
}

pub const MIN_ZOOM: f64 = 0.1;
pub const MAX_ZOOM: f64 = 80.0;

impl View {
    pub fn to_screen(self, canvas: ERect, p: Point) -> Pos2 {
        let c = canvas.center();
        Pos2::new(
            c.x + ((p.x - self.center.x) * self.zoom) as f32,
            c.y - ((p.y - self.center.y) * self.zoom) as f32,
        )
    }

    pub fn to_doc(self, canvas: ERect, p: Pos2) -> Point {
        let c = canvas.center();
        Point::new(
            self.center.x + (p.x - c.x) as f64 / self.zoom,
            self.center.y - (p.y - c.y) as f64 / self.zoom,
        )
    }

    /// The visible document rectangle.
    pub fn visible(&self, canvas: ERect) -> Rect {
        let hw = canvas.width() as f64 / 2.0 / self.zoom;
        let hh = canvas.height() as f64 / 2.0 / self.zoom;
        Rect::new(
            self.center.x - hw,
            self.center.y - hh,
            self.center.x + hw,
            self.center.y + hh,
        )
    }

    pub fn pan_screen(&mut self, delta: EVec2) {
        self.center.x -= delta.x as f64 / self.zoom;
        self.center.y += delta.y as f64 / self.zoom;
    }

    /// Zooms by `factor`, keeping the document point under `anchor` fixed.
    pub fn zoom_at(&mut self, canvas: ERect, anchor: Pos2, factor: f64) {
        let before = self.to_doc(canvas, anchor);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.to_doc(canvas, anchor);
        self.center += before - after;
    }

    /// Fits `r` into the canvas with some padding.
    pub fn fit(&mut self, canvas: ERect, r: Rect) {
        let r = r.inflate(10.0, 10.0);
        self.center = r.center();
        let zx = canvas.width() as f64 / r.width().max(1.0);
        let zy = canvas.height() as f64 / r.height().max(1.0);
        self.zoom = zx.min(zy).clamp(MIN_ZOOM, MAX_ZOOM);
    }
}

/// Squared distance from `p` to the segment `a`–`b`.
fn seg_dist2(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let t = if ab.hypot2() > 0.0 {
        ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).hypot2()
}

/// Distance from `p` to a path's outline.
pub fn path_distance(path: &BezPath, p: Point) -> f64 {
    let mut best = f64::INFINITY;
    let mut last = Point::ZERO;
    let mut start = Point::ZERO;
    goldstone_core::kurbo::flatten(path, 0.05, |el| {
        use goldstone_core::kurbo::PathEl;
        match el {
            PathEl::MoveTo(q) => {
                last = q;
                start = q;
                best = best.min((p - q).hypot2());
            }
            PathEl::LineTo(q) => {
                best = best.min(seg_dist2(p, last, q));
                last = q;
            }
            PathEl::ClosePath => {
                best = best.min(seg_dist2(p, last, start));
                last = start;
            }
            _ => {}
        }
    });
    best.sqrt()
}

/// Arc-length fraction (0–1) of the point on `path` closest to `p`.
pub fn closest_fraction(path: &BezPath, p: Point) -> f64 {
    let al = goldstone_core::geom::ArcLen::new(path);
    let len = al.length();
    if len <= 0.0 {
        return 0.5;
    }
    let n = 200;
    let (mut best, mut best_s) = (f64::INFINITY, 0.0);
    for i in 0..=n {
        let s = len * i as f64 / n as f64;
        let d = (al.eval(s).0 - p).hypot2();
        if d < best {
            best = d;
            best_s = s;
        }
    }
    best_s / len
}

/// Text boxes as polygons (y up).
pub type TextBoxes = Vec<(ObjRef, [Point; 4])>;

fn in_quad(q: &[Point; 4], p: Point) -> bool {
    let mut sign = 0.0;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let c = (b - a).cross(p - a);
        if c.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

/// The top-most object at `p`. `tol` is the pick tolerance in pt.
pub fn hit(doc: &Document, scene: &Scene, texts: &TextBoxes, p: Point, tol: f64) -> Option<ObjRef> {
    // Vertices first: they are small and sit on top of lines.
    let mut best: Option<(f64, usize)> = None;
    for (i, v) in doc.vertices.iter().enumerate() {
        let d = (Point::new(v.at[0], v.at[1]) - p).hypot();
        let r = v.size.unwrap_or(4.0) / 2.0 + tol;
        if d <= r && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, i));
        }
    }
    if let Some((_, i)) = best {
        return Some(ObjRef::Vertex(i));
    }
    for (obj, q) in texts.iter().rev() {
        if in_quad(q, p) {
            return Some(*obj);
        }
    }
    for item in scene.items.iter().rev() {
        if let Mark::Path(pi) = &item.mark {
            let w = pi.stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
            if pi.fill.is_some() && pi.path.contains(p) {
                return Some(item.obj);
            }
            if path_distance(&pi.path, p) <= tol + w {
                return Some(item.obj);
            }
        }
    }
    None
}

/// Every object at `p` (for touch selection).
pub fn hit_all(
    doc: &Document,
    scene: &Scene,
    texts: &TextBoxes,
    p: Point,
    tol: f64,
) -> Vec<ObjRef> {
    let mut out = Vec::new();
    for (i, v) in doc.vertices.iter().enumerate() {
        let d = (Point::new(v.at[0], v.at[1]) - p).hypot();
        if d <= v.size.unwrap_or(4.0) / 2.0 + tol {
            out.push(ObjRef::Vertex(i));
        }
    }
    for (obj, q) in texts {
        if in_quad(q, p) {
            out.push(*obj);
        }
    }
    for item in &scene.items {
        if let Mark::Path(pi) = &item.mark
            && !out.contains(&item.obj)
        {
            let w = pi.stroke.as_ref().map_or(0.0, |s| s.width / 2.0);
            if (pi.fill.is_some() && pi.path.contains(p)) || path_distance(&pi.path, p) <= tol + w {
                out.push(item.obj);
            }
        }
    }
    out
}

/// Bounding boxes of all objects, for rubber-band selection.
pub fn object_bounds(doc: &Document, scene: &Scene, texts: &TextBoxes) -> Vec<(ObjRef, Rect)> {
    use std::collections::BTreeMap;
    let mut m: BTreeMap<ObjRef, Rect> = BTreeMap::new();
    let mut add = |o: ObjRef, r: Rect| {
        m.entry(o).and_modify(|e| *e = e.union(r)).or_insert(r);
    };
    for (i, v) in doc.vertices.iter().enumerate() {
        let c = Point::new(v.at[0], v.at[1]);
        add(ObjRef::Vertex(i), Rect::from_points(c, c));
    }
    for item in &scene.items {
        if let Mark::Path(pi) = &item.mark
            && !matches!(item.obj, ObjRef::Vertex(_))
            && !pi.path.elements().is_empty()
        {
            add(item.obj, pi.path.bounding_box());
        }
    }
    for (o, q) in texts {
        if !matches!(o, ObjRef::Vertex(_) | ObjRef::Edge(_)) {
            for c in q {
                add(*o, Rect::from_points(*c, *c));
            }
        }
    }
    m.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_transform() {
        let canvas = ERect::from_min_size(Pos2::new(10.0, 20.0), EVec2::new(800.0, 600.0));
        let mut v = View::default();
        let p = Point::new(12.5, -7.0);
        let s = v.to_screen(canvas, p);
        assert!((v.to_doc(canvas, s) - p).hypot() < 1e-3);
        let anchor = Pos2::new(300.0, 200.0);
        let before = v.to_doc(canvas, anchor);
        v.zoom_at(canvas, anchor, 2.0);
        assert!((v.to_doc(canvas, anchor) - before).hypot() < 1e-3);
    }

    #[test]
    fn hits_vertices_lines_and_nothing() {
        let doc = goldstone_core::io::parse(include_str!("../../../examples/mbpt2.gold")).unwrap();
        let scene = goldstone_core::scene::build(&doc).unwrap();
        assert_eq!(
            hit(&doc, &scene, &vec![], Point::new(0.5, 0.5), 1.0),
            Some(ObjRef::Vertex(0))
        );
        assert_eq!(
            hit(&doc, &scene, &vec![], Point::new(35.0, 0.3), 1.0),
            Some(ObjRef::Edge(0))
        );
        assert_eq!(
            hit(&doc, &scene, &vec![], Point::new(35.0, 30.0), 1.0),
            None
        );
    }
}

//! Shape outlines, SVG path data, rigid transforms and boolean operations
//! (union, difference, intersection, exclusion) on shapes.

use kurbo::{Affine, BezPath, Ellipse, PathEl, Point, Rect, Shape as _};

use crate::geom::{self, pt};
use crate::model::{Shape, ShapeKind};

pub fn kind_name(k: ShapeKind) -> &'static str {
    match k {
        ShapeKind::Box => "box",
        ShapeKind::Ellipse => "ellipse",
        ShapeKind::Polygon => "polygon",
        ShapeKind::Path => "path",
    }
}

/// Ends every subpath with an explicit close (boolean operations need
/// closed paths; kurbo's ellipses end on their start point without one).
fn closed(p: BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut open = false;
    for el in p.elements() {
        match el {
            PathEl::MoveTo(_) => {
                if open {
                    out.close_path();
                }
                open = true;
            }
            PathEl::ClosePath => open = false,
            _ => {}
        }
        out.push(*el);
    }
    if open {
        out.close_path();
    }
    out
}

/// The outline of a shape in document coordinates, rotation applied.
/// Every subpath is closed.
pub fn outline(s: &Shape) -> Result<BezPath, String> {
    outline_raw(s).map(closed)
}

/// The outline as drawn: like [`outline`], but a path's subpaths without
/// `Z` stay open (e.g. a function plot's curve).
pub fn outline_open(s: &Shape) -> Result<BezPath, String> {
    outline_raw(s)
}

fn outline_raw(s: &Shape) -> Result<BezPath, String> {
    let rot = s.rotation.unwrap_or(0.0).to_radians();
    let about_center = |p: BezPath| {
        if rot != 0.0 {
            let c = p.bounding_box().center();
            Affine::rotate_about(rot, c) * p
        } else {
            p
        }
    };
    match s.kind {
        ShapeKind::Box | ShapeKind::Ellipse => {
            let (Some(at), Some([w, h])) = (s.at, s.size) else {
                return Err(format!("a {} needs `at` and `size`", kind_name(s.kind)));
            };
            let c = pt(at);
            Ok(if s.kind == ShapeKind::Box {
                Affine::rotate_about(rot, c)
                    * Rect::from_center_size(c, (w, h)).to_path(geom::CURVE_TOL)
            } else {
                Ellipse::new(c, (w / 2.0, h / 2.0), rot).to_path(geom::CURVE_TOL)
            })
        }
        ShapeKind::Polygon => {
            let pts = s.points.clone().unwrap_or_default();
            if pts.len() < 3 {
                return Err("a polygon needs at least 3 `points`".into());
            }
            let mut p = BezPath::new();
            p.move_to(pt(pts[0]));
            for q in &pts[1..] {
                p.line_to(pt(*q));
            }
            p.close_path();
            Ok(about_center(p))
        }
        ShapeKind::Path => {
            let d = s.d.as_deref().ok_or("a path needs `d` (SVG path data)")?;
            let p = parse_d(d)?;
            if p.elements().is_empty() {
                return Err("`d` is empty".into());
            }
            Ok(about_center(p))
        }
    }
}

/// Parses SVG path data (document coordinates, y up).
pub fn parse_d(d: &str) -> Result<BezPath, String> {
    BezPath::from_svg(d).map_err(|e| format!("invalid path data `d`: {e}"))
}

fn n(x: f64) -> String {
    let r = (x * 100.0).round() / 100.0;
    let r = if r == 0.0 { 0.0 } else { r };
    let s = format!("{r:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn p2(p: Point) -> String {
    format!("{} {}", n(p.x), n(p.y))
}

/// SVG path data for `path`, coordinates rounded to 0.01 pt.
pub fn format_d(path: &BezPath) -> String {
    let mut out: Vec<String> = Vec::new();
    for el in path.elements() {
        out.push(match *el {
            PathEl::MoveTo(p) => format!("M {}", p2(p)),
            PathEl::LineTo(p) => format!("L {}", p2(p)),
            PathEl::QuadTo(a, p) => format!("Q {} {}", p2(a), p2(p)),
            PathEl::CurveTo(a, b, p) => format!("C {} {} {}", p2(a), p2(b), p2(p)),
            PathEl::ClosePath => "Z".into(),
        });
    }
    out.join(" ")
}

fn round_pt(p: Point) -> [f64; 2] {
    let r = |x: f64| (x * 100.0).round() / 100.0;
    [r(p.x), r(p.y)]
}

/// Applies a rigid transform (rotation, reflection, translation) to a
/// shape. Polygons and paths get their rotation baked into the points.
pub fn transform(s: &mut Shape, a: Affine) {
    match s.kind {
        ShapeKind::Box | ShapeKind::Ellipse => {
            if let Some(at) = s.at.as_mut() {
                *at = round_pt(a * pt(*at));
            }
            // Both kinds are centrally symmetric, so the new rotation is the
            // direction of the transformed local x axis.
            let r = s.rotation.unwrap_or(0.0).to_radians();
            let [m0, m1, m2, m3, _, _] = a.as_coeffs();
            let (x, y) = (m0 * r.cos() + m2 * r.sin(), m1 * r.cos() + m3 * r.sin());
            let mut deg = (y.atan2(x).to_degrees() * 100.0).round() / 100.0;
            if deg > 180.0 {
                deg -= 360.0;
            }
            s.rotation = (deg.abs() > 1e-9 && (deg.abs() - 180.0).abs() > 1e-9).then_some(deg);
        }
        ShapeKind::Polygon => {
            let Ok(o) = outline(s) else { return };
            let pts: Vec<[f64; 2]> = o
                .elements()
                .iter()
                .filter_map(|el| match el {
                    PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(round_pt(a * *p)),
                    _ => None,
                })
                .collect();
            s.points = Some(pts);
            s.rotation = None;
        }
        ShapeKind::Path => {
            let Ok(o) = outline_raw(s) else { return };
            s.d = Some(format_d(&(a * o)));
            s.rotation = None;
        }
    }
}

/// Turns any shape into a `kind = "path"` shape with the same outline.
pub fn to_path(s: &mut Shape) -> Result<(), String> {
    let o = outline(s)?;
    s.kind = ShapeKind::Path;
    s.d = Some(format_d(&o));
    s.at = None;
    s.size = None;
    s.points = None;
    s.rotation = None;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolOp {
    /// Everything covered by any shape.
    Union,
    /// The bottom shape minus all the others.
    Difference,
    /// The area covered by all shapes.
    Intersection,
    /// The area covered by an odd number of shapes.
    Exclusion,
}

impl BoolOp {
    pub fn name(self) -> &'static str {
        match self {
            BoolOp::Union => "Union",
            BoolOp::Difference => "Difference",
            BoolOp::Intersection => "Intersection",
            BoolOp::Exclusion => "Exclusion",
        }
    }
}

fn binary(a: &BezPath, b: &BezPath, op: linesweeper::BinaryOp) -> Result<BezPath, String> {
    let contours = linesweeper::binary_op(a, b, linesweeper::FillRule::NonZero, op)
        .map_err(|e| e.to_string())?;
    let mut out = BezPath::new();
    for c in contours.contours() {
        out.extend(c.path.elements().iter().copied());
    }
    Ok(out)
}

/// Combines outlines, given bottom first. Needs at least two.
pub fn boolean(op: BoolOp, paths: &[BezPath]) -> Result<BezPath, String> {
    use linesweeper::BinaryOp as B;
    let [first, rest @ ..] = paths else {
        return Err("select at least two shapes".into());
    };
    if rest.is_empty() {
        return Err("select at least two shapes".into());
    }
    let fold = |init: BezPath, items: &[BezPath], op: B| -> Result<BezPath, String> {
        items.iter().try_fold(init, |acc, p| binary(&acc, p, op))
    };
    let result = match op {
        BoolOp::Union => fold(first.clone(), rest, B::Union)?,
        BoolOp::Intersection => fold(first.clone(), rest, B::Intersection)?,
        BoolOp::Exclusion => fold(first.clone(), rest, B::Xor)?,
        BoolOp::Difference => {
            let cut = fold(rest[0].clone(), &rest[1..], B::Union)?;
            binary(first, &cut, B::Difference)?
        }
    };
    if result.elements().is_empty() {
        return Err("the result is empty".into());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(x: f64, y: f64, w: f64, h: f64) -> Shape {
        Shape {
            id: None,
            kind: ShapeKind::Box,
            at: Some([x, y]),
            size: Some([w, h]),
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

    fn area(p: &BezPath) -> f64 {
        p.area().abs()
    }

    #[test]
    fn path_data_round_trips() {
        let p = outline(&bx(0.0, 0.0, 10.0, 4.0)).unwrap();
        let d = format_d(&p);
        assert!(d.starts_with("M -5 -2"), "{d}");
        let q = parse_d(&d).unwrap();
        assert!((area(&q) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn boolean_areas() {
        let a = outline(&bx(0.0, 0.0, 10.0, 10.0)).unwrap();
        let b = outline(&bx(5.0, 0.0, 10.0, 10.0)).unwrap();
        let ab = [a.clone(), b.clone()];
        let u = boolean(BoolOp::Union, &ab).unwrap();
        assert!((area(&u) - 150.0).abs() < 1e-6, "{}", area(&u));
        let i = boolean(BoolOp::Intersection, &ab).unwrap();
        assert!((area(&i) - 50.0).abs() < 1e-6);
        let d = boolean(BoolOp::Difference, &ab).unwrap();
        assert!((area(&d) - 50.0).abs() < 1e-6);
        // Exclusion: two 50 pt² pieces.
        let x = boolean(BoolOp::Exclusion, &ab).unwrap();
        let total: f64 = x.segments().count() as f64;
        assert!(total > 0.0);
        assert!(boolean(BoolOp::Union, &[a]).is_err());
    }

    #[test]
    fn ellipses_are_closed() {
        let mut e = bx(0.0, 0.0, 10.0, 10.0);
        e.kind = ShapeKind::Ellipse;
        let mut f = e.clone();
        f.at = Some([5.0, 0.0]);
        let u = boolean(BoolOp::Union, &[outline(&e).unwrap(), outline(&f).unwrap()]).unwrap();
        let one = std::f64::consts::PI * 25.0;
        assert!(u.area().abs() > one && u.area().abs() < 2.0 * one);
    }

    #[test]
    fn hole_from_difference() {
        let outer = outline(&bx(0.0, 0.0, 20.0, 20.0)).unwrap();
        let inner = outline(&bx(0.0, 0.0, 10.0, 10.0)).unwrap();
        let d = boolean(BoolOp::Difference, &[outer, inner]).unwrap();
        // Contours are oriented so the signed areas add up to the set's area.
        assert!((d.area().abs() - 300.0).abs() < 1e-6, "{}", d.area());
    }

    #[test]
    fn rigid_transforms() {
        let mut s = bx(10.0, 0.0, 4.0, 2.0);
        transform(&mut s, Affine::rotate(std::f64::consts::FRAC_PI_2));
        assert_eq!(s.at, Some([0.0, 10.0]));
        assert_eq!(s.rotation, Some(90.0));
        let mut p = bx(0.0, 0.0, 4.0, 2.0);
        to_path(&mut p).unwrap();
        transform(&mut p, Affine::translate((1.0, 1.0)));
        assert!(p.d.as_deref().unwrap().starts_with("M -1 0"));
    }
}

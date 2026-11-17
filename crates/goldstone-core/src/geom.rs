//! Geometry: base paths, arc-length parametrisation, wiggles, arrows.
//!
//! Coordinates are in pt with y pointing up. "Left" of a path is the side
//! obtained by rotating the tangent by +90°.

use kurbo::{
    Arc, BezPath, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathSeg, Point, Shape, Vec2,
};

/// Accuracy used for arc-length computations (pt).
const ARCLEN_ACC: f64 = 1e-6;
/// Tolerance for converting arcs to Bézier curves (pt).
pub const CURVE_TOL: f64 = 1e-4;

pub fn pt(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Left-pointing unit normal of a unit tangent.
pub fn left_normal(t: Vec2) -> Vec2 {
    Vec2::new(-t.y, t.x)
}

/// A straight segment.
pub fn line(a: Point, b: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(a);
    p.line_to(b);
    p
}

/// A circular arc from `a` to `b` whose tangent at `a` is the chord direction
/// rotated by `bend_deg` (positive: bulges to the left of a→b).
pub fn bent(a: Point, b: Point, bend_deg: f64) -> BezPath {
    let theta = bend_deg.to_radians();
    let d = b - a;
    let len = d.hypot();
    if theta.abs() < 1e-9 || len < 1e-9 {
        return line(a, b);
    }
    let n = left_normal(d / len);
    let mid = a.midpoint(b);
    // Center lies at distance (len/2)/tan(theta) to the right of the chord.
    let center = mid - n * ((len / 2.0) / theta.tan());
    let radius = (len / 2.0) / theta.sin().abs();
    let start = (a - center).atan2();
    let arc = Arc::new(center, Vec2::new(radius, radius), start, -2.0 * theta, 0.0);
    with_exact_ends(arc.to_path(CURVE_TOL), a, b)
}

/// A loop at `p`: a full circle of `radius` whose center lies in direction
/// `angle_deg` from `p`, traversed counter-clockwise starting and ending at `p`.
pub fn loop_at(p: Point, radius: f64, angle_deg: f64) -> BezPath {
    let dir = Vec2::from_angle(angle_deg.to_radians());
    let center = p + dir * radius;
    let start = (p - center).atan2();
    let arc = Arc::new(
        center,
        Vec2::new(radius, radius),
        start,
        std::f64::consts::TAU,
        0.0,
    );
    with_exact_ends(arc.to_path(CURVE_TOL), p, p)
}

/// A cubic Bézier.
pub fn cubic(a: Point, c1: Point, c2: Point, b: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(a);
    p.curve_to(c1, c2, b);
    p
}

/// Snaps the first and last point of a path onto the given points, removing
/// floating-point drift from arc conversion.
fn with_exact_ends(path: BezPath, a: Point, b: Point) -> BezPath {
    use kurbo::PathEl;
    let mut els: Vec<PathEl> = path.elements().to_vec();
    if let Some(PathEl::MoveTo(p)) = els.first_mut() {
        *p = a;
    }
    if let Some(PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p)) =
        els.last_mut()
    {
        *p = b;
    }
    BezPath::from_vec(els)
}

/// Arc-length parametrisation of a (single, open) path.
pub struct ArcLen {
    segs: Vec<(PathSeg, f64, f64)>, // segment, start s, length
    total: f64,
}

impl ArcLen {
    pub fn new(path: &BezPath) -> Self {
        let mut segs = Vec::new();
        let mut s = 0.0;
        for seg in path.segments() {
            let l = seg.arclen(ARCLEN_ACC);
            if l > 1e-12 {
                segs.push((seg, s, l));
                s += l;
            }
        }
        Self { segs, total: s }
    }

    pub fn length(&self) -> f64 {
        self.total
    }

    fn locate(&self, s: f64) -> Option<(PathSeg, f64)> {
        let s = s.clamp(0.0, self.total);
        let i = self
            .segs
            .iter()
            .position(|(_, s0, l)| s <= s0 + l)
            .unwrap_or(self.segs.len().checked_sub(1)?);
        let (seg, s0, l) = self.segs[i];
        let local = (s - s0).clamp(0.0, l);
        let t = if local >= l {
            1.0
        } else {
            seg.inv_arclen(local, ARCLEN_ACC)
        };
        Some((seg, t))
    }

    /// Point and unit tangent at arc length `s`.
    pub fn eval(&self, s: f64) -> (Point, Vec2) {
        let Some((seg, t)) = self.locate(s) else {
            return (Point::ZERO, Vec2::new(1.0, 0.0));
        };
        let p = seg.eval(t);
        let mut d = seg_deriv(seg, t);
        if d.hypot() < 1e-12 {
            // Degenerate control point at an end: fall back to a secant.
            let t2 = if t < 0.5 {
                (t + 1e-4).min(1.0)
            } else {
                (t - 1e-4).max(0.0)
            };
            d = if t < 0.5 {
                seg.eval(t2) - p
            } else {
                p - seg.eval(t2)
            };
        }
        let len = d.hypot();
        (
            p,
            if len > 0.0 {
                d / len
            } else {
                Vec2::new(1.0, 0.0)
            },
        )
    }

    /// The part of the path between arc lengths `s0` and `s1`.
    pub fn sub_path(&self, s0: f64, s1: f64) -> BezPath {
        let mut out = BezPath::new();
        let (s0, s1) = (s0.max(0.0), s1.min(self.total));
        if s1 <= s0 {
            return out;
        }
        for &(seg, a, l) in &self.segs {
            let b = a + l;
            if b <= s0 || a >= s1 {
                continue;
            }
            let t0 = if s0 > a {
                seg.inv_arclen(s0 - a, ARCLEN_ACC)
            } else {
                0.0
            };
            let t1 = if s1 < b {
                seg.inv_arclen(s1 - a, ARCLEN_ACC)
            } else {
                1.0
            };
            let sub = seg.subsegment(t0..t1);
            if out.elements().is_empty() {
                out.move_to(sub.start());
            }
            match sub {
                PathSeg::Line(l) => out.line_to(l.p1),
                PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
                PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
            }
        }
        out
    }
}

fn seg_deriv(seg: PathSeg, t: f64) -> Vec2 {
    match seg {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => q.deriv().eval(t).to_vec2(),
        PathSeg::Cubic(c) => c.deriv().eval(t).to_vec2(),
    }
}

/// Builds a smooth curve `q(s) = P(s) + T(s)·f(s).0 + N(s)·f(s).1` for
/// `s ∈ [0, L]`, sampled at `n` intervals and joined with cubic Hermite
/// segments.
pub fn displaced(base: &ArcLen, n: usize, f: impl Fn(f64) -> (f64, f64)) -> BezPath {
    let len = base.length();
    let n = n.max(1);
    let q = |s: f64| {
        let (p, t) = base.eval(s);
        let (dt, dn) = f(s);
        p + t * dt + left_normal(t) * dn
    };
    let h = (len / n as f64) * 1e-3;
    let deriv = |s: f64| {
        let (a, b) = ((s - h).max(0.0), (s + h).min(len));
        (q(b) - q(a)) / (b - a)
    };
    let ds = len / n as f64;
    let mut out = BezPath::new();
    let mut prev = q(0.0);
    let mut dprev = deriv(0.0);
    out.move_to(prev);
    for i in 1..=n {
        let s = ds * i as f64;
        let cur = q(s);
        let dcur = deriv(s);
        out.curve_to(prev + dprev * (ds / 3.0), cur - dcur * (ds / 3.0), cur);
        prev = cur;
        dprev = dcur;
    }
    out
}

// The wiggly strokes take an `offset`: the whole wiggle is shifted that far
// to the left of `base` (right if negative). Both lines of a double wiggly
// line are made this way from the same base, so they stay in phase even on
// arcs, where offset curves have different lengths.

/// Photon line: a sine wave with a whole number of half-waves.
pub fn wavy(base: &ArcLen, amplitude: f64, wavelength: f64, offset: f64) -> BezPath {
    let len = base.length();
    let halves = ((2.0 * len / wavelength).round() as usize).max(1);
    let k = std::f64::consts::PI * halves as f64 / len;
    displaced(base, halves * 6, |s| {
        (0.0, offset + amplitude * (k * s).sin())
    })
}

/// Gluon line: a coil (prolate trochoid) with a whole number of loops.
pub fn curly(base: &ArcLen, amplitude: f64, wavelength: f64, offset: f64) -> BezPath {
    let len = base.length();
    let loops = ((len / wavelength).round() as usize).max(1);
    let total = std::f64::consts::TAU * loops as f64;
    // Tangential radius; larger than the advance per radian (λ/2π) so loops form.
    let c = 0.35 * len / loops as f64;
    displaced(base, loops * 24, |s| {
        let u = total * s / len;
        (-c * u.sin(), offset + amplitude * 0.5 * (1.0 - u.cos()))
    })
}

/// Zigzag line with a whole number of teeth (straight segments).
pub fn zigzag(base: &ArcLen, amplitude: f64, wavelength: f64, offset: f64) -> BezPath {
    let len = base.length();
    let halves = ((2.0 * len / wavelength).round() as usize).max(1);
    let step = len / halves as f64;
    let at = |s: f64, d: f64| {
        let (p, t) = base.eval(s);
        p + left_normal(t) * d
    };
    let mut out = BezPath::new();
    out.move_to(at(0.0, offset));
    for i in 0..halves {
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        out.line_to(at(step * (i as f64 + 0.5), offset + amplitude * sign));
    }
    out.line_to(at(len, offset));
    out
}

/// Scales a dash pattern so the path starts and ends with a full dash.
pub fn fit_dash(len: f64, on: f64, off: f64) -> [f64; 2] {
    if len <= on || on + off <= 0.0 {
        return [on.max(len), off];
    }
    let n = ((len + off) / (on + off)).round().max(1.0);
    if n <= 1.0 {
        return [len, off];
    }
    let k = len / (n * on + (n - 1.0) * off);
    [on * k, off * k]
}

/// Spacing for a dotted line so dots sit exactly at both ends.
pub fn fit_dots(len: f64, gap: f64) -> f64 {
    let n = (len / gap).round().max(1.0);
    len / n
}

/// Filled, notched arrowhead centred at `center`, pointing along `dir`.
pub fn arrowhead(center: Point, dir: Vec2, length: f64, width: f64) -> BezPath {
    let n = left_normal(dir);
    let tip = center + dir * (length / 2.0);
    let back = center - dir * (length / 2.0);
    let notch = back + dir * (length * 0.25);
    let mut p = BezPath::new();
    p.move_to(tip);
    p.line_to(back + n * (width / 2.0));
    p.line_to(notch);
    p.line_to(back - n * (width / 2.0));
    p.close_path();
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Point, b: Point) -> bool {
        (a - b).hypot() < 1e-6
    }

    #[test]
    fn bent_hits_endpoints_and_bulges_left() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(40.0, 0.0);
        let p = bent(a, b, 30.0);
        let al = ArcLen::new(&p);
        assert!(close(al.eval(0.0).0, a));
        assert!(close(al.eval(al.length()).0, b));
        let mid = al.eval(al.length() / 2.0).0;
        assert!(
            mid.y > 0.0,
            "positive bend bulges left (up for a left-to-right chord)"
        );
        // start tangent rotated by +30°
        let t = al.eval(0.0).1;
        assert!((t.atan2().to_degrees() - 30.0).abs() < 0.1);
        // semicircle
        let semi = ArcLen::new(&bent(a, b, 90.0));
        assert!((semi.length() - std::f64::consts::PI * 20.0).abs() < 5e-3);
    }

    #[test]
    fn loop_is_closed_circle() {
        let p = Point::new(5.0, 5.0);
        let al = ArcLen::new(&loop_at(p, 10.0, 90.0));
        assert!(
            (al.length() - std::f64::consts::TAU * 10.0).abs() < 5e-3,
            "len {}",
            al.length()
        );
        assert!(close(al.eval(0.0).0, p));
        let top = al.eval(al.length() / 2.0).0;
        assert!(close(top, Point::new(5.0, 25.0)));
    }

    #[test]
    fn wiggles_end_on_base() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(53.0, 17.0);
        let base = ArcLen::new(&bent(a, b, 20.0));
        for path in [
            wavy(&base, 3.0, 8.0, 0.0),
            curly(&base, 4.0, 6.0, 0.0),
            zigzag(&base, 3.0, 6.0, 0.0),
        ] {
            let segs: Vec<_> = path.segments().collect();
            assert!(close(segs.first().unwrap().start(), a));
            assert!((segs.last().unwrap().end() - b).hypot() < 1e-6);
        }
    }

    #[test]
    fn dash_fits_exactly() {
        let [on, off] = fit_dash(50.0, 4.0, 3.0);
        let n = ((50.0 + off) / (on + off)).round();
        assert!((n * on + (n - 1.0) * off - 50.0).abs() < 1e-9);
    }

    #[test]
    fn sub_path_trims() {
        let al = ArcLen::new(&line(Point::new(0.0, 0.0), Point::new(10.0, 0.0)));
        let s = ArcLen::new(&al.sub_path(2.0, 7.0));
        assert!((s.length() - 5.0).abs() < 1e-9);
        assert!(close(s.eval(0.0).0, Point::new(2.0, 0.0)));
    }
}

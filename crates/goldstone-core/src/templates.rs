//! Built-in templates: a shape library in the spirit of an office suite's
//! (basic shapes, block arrows, stars, flowchart, callouts, equation
//! shapes) and many-body / Feynman diagrams. Every template is an ordinary
//! document; inserting one merges it into the drawing.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use crate::kurbo::{BezPath, Point, Shape as _};
use crate::model::{Document, Shape, ShapeKind};
use crate::shapes::format_d;

/// A named template in a category.
#[derive(Debug, Clone)]
pub struct Template {
    pub category: &'static str,
    pub name: String,
    pub doc: Document,
}

/// Category names, in menu order.
pub const CATEGORIES: &[&str] = &[
    "Basic Shapes",
    "Block Arrows",
    "Stars and Banners",
    "Flowchart",
    "Callouts",
    "Equation Shapes",
    "Many-Body Diagrams",
    "Feynman Diagrams",
];

const W: f64 = 60.0;
const H: f64 = 40.0;

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn shape(kind: ShapeKind) -> Shape {
    Shape {
        id: None,
        kind,
        at: None,
        size: None,
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

fn poly(pts: &[(f64, f64)]) -> Shape {
    let mut s = shape(ShapeKind::Polygon);
    s.points = Some(pts.iter().map(|(x, y)| [r2(*x), r2(*y)]).collect());
    s
}

fn path(p: &BezPath) -> Shape {
    let mut s = shape(ShapeKind::Path);
    s.d = Some(format_d(p));
    s
}

fn bx(w: f64, h: f64) -> Shape {
    let mut s = shape(ShapeKind::Box);
    s.at = Some([0.0, 0.0]);
    s.size = Some([w, h]);
    s
}

fn ellipse(w: f64, h: f64) -> Shape {
    let mut s = shape(ShapeKind::Ellipse);
    s.at = Some([0.0, 0.0]);
    s.size = Some([w, h]);
    s
}

fn doc(shapes: Vec<Shape>) -> Document {
    Document {
        shapes,
        ..Document::default()
    }
}

/// Polygon with corners on an ellipse (radii rx, ry), first corner up.
fn regular(n: usize, rx: f64, ry: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|k| {
            let a = FRAC_PI_2 + TAU * k as f64 / n as f64;
            (rx * a.cos(), ry * a.sin())
        })
        .collect()
}

fn star(n: usize, outer: f64, inner: f64) -> Vec<(f64, f64)> {
    (0..2 * n)
        .map(|k| {
            let r = if k % 2 == 0 { outer } else { inner };
            let a = FRAC_PI_2 + PI * k as f64 / n as f64;
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

fn rounded_rect(w: f64, h: f64, r: f64) -> BezPath {
    crate::kurbo::RoundedRect::new(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0, r).to_path(0.01)
}

fn circle_path(cx: f64, cy: f64, r: f64) -> BezPath {
    crate::kurbo::Circle::new((cx, cy), r).to_path(0.01)
}

/// A closed path through points with straight lines.
fn lines(p: &mut BezPath, pts: &[(f64, f64)]) {
    p.move_to(pts[0]);
    for q in &pts[1..] {
        p.line_to(*q);
    }
    p.close_path();
}

/// Horizontal wave from x0 to x1 around y (amplitude a, one period),
/// continuing the current subpath.
fn wave(p: &mut BezPath, x0: f64, x1: f64, y: f64, a: f64) {
    let n = 24;
    for k in 1..=n {
        let t = k as f64 / n as f64;
        let x = x0 + (x1 - x0) * t;
        p.line_to((x, y + a * (TAU * t).sin()));
    }
}

fn basic() -> Vec<(&'static str, Document)> {
    let w = W / 2.0;
    let h = H / 2.0;
    let mut frame = rounded_rect(W, H, 0.0);
    let mut inner = BezPath::new();
    lines(
        &mut inner,
        &[
            (-w + 6.0, -h + 6.0),
            (-w + 6.0, h - 6.0),
            (w - 6.0, h - 6.0),
            (w - 6.0, -h + 6.0),
        ],
    );
    frame.extend(inner);
    let mut donut = circle_path(0.0, 0.0, 20.0);
    donut.extend(
        crate::kurbo::Circle::new((0.0, 0.0), 11.0)
            .to_path(0.01)
            .reverse_subpaths(),
    );
    let mut heart = BezPath::new();
    heart.move_to((0.0, -20.0));
    heart.curve_to((-30.0, 2.0), (-18.0, 26.0), (0.0, 12.0));
    heart.curve_to((18.0, 26.0), (30.0, 2.0), (0.0, -20.0));
    heart.close_path();
    let mut moon = BezPath::new();
    moon.move_to((5.0, 20.0));
    moon.curve_to((-22.0, 20.0), (-22.0, -20.0), (5.0, -20.0));
    moon.curve_to((-8.0, -10.0), (-8.0, 10.0), (5.0, 20.0));
    moon.close_path();
    // Cloud: an ellipse with scalloped bumps.
    let mut cloud = BezPath::new();
    for k in 0..=240 {
        let t = TAU * k as f64 / 240.0;
        let f = 0.82 + 0.18 * (4.5 * t).sin().abs();
        let q = (31.0 * f * t.cos(), 22.0 * f * t.sin());
        if k == 0 {
            cloud.move_to(q);
        } else {
            cloud.line_to(q);
        }
    }
    cloud.close_path();
    let mut cylinder = BezPath::new();
    cylinder.move_to((-20.0, 14.0));
    cylinder.line_to((-20.0, -14.0));
    cylinder.curve_to((-20.0, -22.0), (20.0, -22.0), (20.0, -14.0));
    cylinder.line_to((20.0, 14.0));
    cylinder.curve_to((20.0, 22.0), (-20.0, 22.0), (-20.0, 14.0));
    cylinder.curve_to((-20.0, 6.0), (20.0, 6.0), (20.0, 14.0));
    let mut cube = BezPath::new();
    lines(
        &mut cube,
        &[(-20.0, -20.0), (10.0, -20.0), (10.0, 10.0), (-20.0, 10.0)],
    );
    cube.move_to((-20.0, 10.0));
    cube.line_to((-10.0, 20.0));
    cube.line_to((20.0, 20.0));
    cube.line_to((20.0, -10.0));
    cube.line_to((10.0, -20.0));
    cube.move_to((10.0, 10.0));
    cube.line_to((20.0, 20.0));
    let plus = |a: f64, b: f64| {
        vec![
            (-a, b),
            (-b, b),
            (-b, a),
            (b, a),
            (b, b),
            (a, b),
            (a, -b),
            (b, -b),
            (b, -a),
            (-b, -a),
            (-b, -b),
            (-a, -b),
        ]
    };
    vec![
        ("Rectangle", doc(vec![bx(W, H)])),
        (
            "Rounded Rectangle",
            doc(vec![path(&rounded_rect(W, H, 8.0))]),
        ),
        ("Square", doc(vec![bx(H, H)])),
        ("Ellipse", doc(vec![ellipse(W, H)])),
        ("Circle", doc(vec![ellipse(H, H)])),
        ("Triangle", doc(vec![poly(&[(-w, -h), (w, -h), (0.0, h)])])),
        (
            "Right Triangle",
            doc(vec![poly(&[(-w, -h), (w, -h), (-w, h)])]),
        ),
        (
            "Parallelogram",
            doc(vec![poly(&[
                (-w, -h),
                (w - 12.0, -h),
                (w, h),
                (-w + 12.0, h),
            ])]),
        ),
        (
            "Trapezoid",
            doc(vec![poly(&[
                (-w, -h),
                (w, -h),
                (w - 14.0, h),
                (-w + 14.0, h),
            ])]),
        ),
        (
            "Diamond",
            doc(vec![poly(&[(0.0, -h), (w, 0.0), (0.0, h), (-w, 0.0)])]),
        ),
        ("Pentagon", doc(vec![poly(&regular(5, 22.0, 22.0))])),
        (
            "Hexagon",
            doc(vec![poly(
                &regular(6, 22.0, 22.0)
                    .iter()
                    .map(|(x, y)| (*y, *x))
                    .collect::<Vec<_>>(),
            )]),
        ),
        ("Heptagon", doc(vec![poly(&regular(7, 22.0, 22.0))])),
        ("Octagon", doc(vec![poly(&star(4, 22.0, 22.0))])),
        ("Decagon", doc(vec![poly(&regular(10, 22.0, 22.0))])),
        ("Plus", doc(vec![poly(&plus(20.0, 7.0))])),
        ("Frame", doc(vec![path(&frame)])),
        ("Donut", doc(vec![path(&donut)])),
        ("Heart", doc(vec![path(&heart)])),
        ("Moon", doc(vec![path(&moon)])),
        ("Cloud", doc(vec![path(&cloud)])),
        (
            "Lightning Bolt",
            doc(vec![poly(&[
                (-6.0, 22.0),
                (8.0, 22.0),
                (2.0, 4.0),
                (12.0, 4.0),
                (-8.0, -22.0),
                (-2.0, -2.0),
                (-12.0, -2.0),
            ])]),
        ),
        ("Cylinder", doc(vec![path(&cylinder)])),
        ("Cube", doc(vec![path(&cube)])),
    ]
}

fn arrows() -> Vec<(&'static str, Document)> {
    // Right arrow: shaft half-height s, head from x = hx.
    let right = |l: f64, s: f64, hh: f64, hx: f64| {
        vec![
            (-l, -s),
            (hx, -s),
            (hx, -hh),
            (l, 0.0),
            (hx, hh),
            (hx, s),
            (-l, s),
        ]
    };
    let rot = |pts: &[(f64, f64)], k: i32| -> Vec<(f64, f64)> {
        pts.iter()
            .map(|&(x, y)| match k.rem_euclid(4) {
                0 => (x, y),
                1 => (-y, x),
                2 => (-x, -y),
                _ => (y, -x),
            })
            .collect()
    };
    let r = right(30.0, 8.0, 18.0, 10.0);
    let both = vec![
        (-30.0, 0.0),
        (-14.0, -18.0),
        (-14.0, -8.0),
        (14.0, -8.0),
        (14.0, -18.0),
        (30.0, 0.0),
        (14.0, 18.0),
        (14.0, 8.0),
        (-14.0, 8.0),
        (-14.0, 18.0),
    ];
    let mut quad = Vec::new();
    for k in 0..4 {
        // One quarter: from the arm's left shaft edge, around its head.
        let arm = [
            (5.0, 5.0),
            (5.0, 18.0),
            (11.0, 18.0),
            (0.0, 30.0),
            (-11.0, 18.0),
            (-5.0, 18.0),
        ];
        quad.extend(rot(&arm, k));
    }
    let mut circular = BezPath::new();
    let arc = crate::kurbo::Arc::new((0.0, 0.0), (20.0, 20.0), 0.3, 4.9, 0.0);
    let inner = crate::kurbo::Arc::new((0.0, 0.0), (12.0, 12.0), 0.3 + 4.9, -4.9, 0.0);
    let p0 = Point::new(20.0 * 0.3f64.cos(), 20.0 * 0.3f64.sin());
    circular.move_to(p0);
    arc.to_cubic_beziers(0.1, |a, b, c| circular.curve_to(a, b, c));
    let end: f64 = 0.3 + 4.9;
    let (c, s) = (end.cos(), end.sin());
    // Head at the end of the arc, pointing along the tangent.
    circular.line_to((24.0 * c, 24.0 * s));
    let tip_a = end + 0.45;
    circular.line_to((16.0 * tip_a.cos(), 16.0 * tip_a.sin()));
    circular.line_to((8.0 * c, 8.0 * s));
    circular.line_to((12.0 * c, 12.0 * s));
    inner.to_cubic_beziers(0.1, |a, b, c| circular.curve_to(a, b, c));
    circular.close_path();
    vec![
        ("Right Arrow", doc(vec![poly(&r)])),
        ("Left Arrow", doc(vec![poly(&rot(&r, 2))])),
        ("Up Arrow", doc(vec![poly(&rot(&r, 1))])),
        ("Down Arrow", doc(vec![poly(&rot(&r, 3))])),
        ("Left-Right Arrow", doc(vec![poly(&both)])),
        ("Up-Down Arrow", doc(vec![poly(&rot(&both, 1))])),
        ("Quad Arrow", doc(vec![poly(&quad)])),
        (
            "Notched Right Arrow",
            doc(vec![poly(&[
                (-30.0, -8.0),
                (10.0, -8.0),
                (10.0, -18.0),
                (30.0, 0.0),
                (10.0, 18.0),
                (10.0, 8.0),
                (-30.0, 8.0),
                (-22.0, 0.0),
            ])]),
        ),
        (
            "Pentagon Arrow",
            doc(vec![poly(&[
                (-30.0, -16.0),
                (14.0, -16.0),
                (30.0, 0.0),
                (14.0, 16.0),
                (-30.0, 16.0),
            ])]),
        ),
        (
            "Chevron",
            doc(vec![poly(&[
                (-30.0, -16.0),
                (14.0, -16.0),
                (30.0, 0.0),
                (14.0, 16.0),
                (-30.0, 16.0),
                (-14.0, 0.0),
            ])]),
        ),
        ("Circular Arrow", doc(vec![path(&circular)])),
    ]
}

fn stars() -> Vec<(&'static str, Document)> {
    let mut out: Vec<(&'static str, Document)> = [
        ("4-Point Star", 4, 0.38),
        ("5-Point Star", 5, 0.4),
        ("6-Point Star", 6, 0.55),
        ("7-Point Star", 7, 0.55),
        ("8-Point Star", 8, 0.6),
        ("10-Point Star", 10, 0.7),
        ("12-Point Star", 12, 0.75),
        ("16-Point Star", 16, 0.8),
        ("24-Point Star", 24, 0.85),
        ("32-Point Star", 32, 0.88),
    ]
    .into_iter()
    .map(|(name, n, ratio)| (name, doc(vec![poly(&star(n, 22.0, 22.0 * ratio))])))
    .collect();
    // Explosion: an irregular star.
    let radii = [
        24.0, 12.0, 20.0, 10.0, 26.0, 13.0, 18.0, 9.0, 23.0, 11.0, 21.0, 12.0, 25.0, 10.0, 19.0,
        12.0,
    ];
    let boom: Vec<(f64, f64)> = radii
        .iter()
        .enumerate()
        .map(|(k, r)| {
            let a = 0.2 + TAU * k as f64 / radii.len() as f64;
            (r * 1.3 * a.cos(), r * a.sin())
        })
        .collect();
    out.push(("Explosion", doc(vec![poly(&boom)])));
    let mut banner = BezPath::new();
    banner.move_to((-30.0, -10.0));
    wave(&mut banner, -30.0, 30.0, -10.0, 5.0);
    banner.line_to((30.0, 14.0));
    let mut top = BezPath::new();
    top.move_to((30.0, 14.0));
    wave(&mut top, 30.0, -30.0, 14.0, 5.0);
    banner.extend(top.elements().iter().skip(1).copied());
    banner.close_path();
    out.push(("Wave", doc(vec![path(&banner)])));
    out
}

fn flowchart() -> Vec<(&'static str, Document)> {
    let w = W / 2.0;
    let h = H / 2.0;
    type Seg = ((f64, f64), (f64, f64));
    let with_lines = |base: BezPath, segs: &[Seg]| {
        let mut p = base;
        for (a, b) in segs {
            p.move_to(*a);
            p.line_to(*b);
        }
        p
    };
    let rect = rounded_rect(W, H, 0.0);
    let mut document = BezPath::new();
    document.move_to((-w, h));
    document.line_to((-w, -h + 4.0));
    wave(&mut document, -w, w, -h + 4.0, -4.0);
    document.line_to((w, h));
    document.close_path();
    let mut tape = BezPath::new();
    tape.move_to((-w, -h + 4.0));
    wave(&mut tape, -w, w, -h + 4.0, 4.0);
    tape.line_to((w, h - 4.0));
    let mut back = BezPath::new();
    back.move_to((w, h - 4.0));
    wave(&mut back, w, -w, h - 4.0, 4.0);
    tape.extend(back.elements().iter().skip(1).copied());
    tape.close_path();
    let mut delay = BezPath::new();
    delay.move_to((-w, -h));
    delay.line_to((w - h, -h));
    delay.curve_to((w + 6.0, -h), (w + 6.0, h), (w - h, h));
    delay.line_to((-w, h));
    delay.close_path();
    let mut stored = BezPath::new();
    stored.move_to((-w + 8.0, h));
    stored.line_to((w, h));
    stored.curve_to((w - 12.0, h / 2.0), (w - 12.0, -h / 2.0), (w, -h));
    stored.line_to((-w + 8.0, -h));
    stored.curve_to((-w - 4.0, -h / 2.0), (-w - 4.0, h / 2.0), (-w + 8.0, h));
    stored.close_path();
    let mut display = BezPath::new();
    display.move_to((-w, 0.0));
    display.line_to((-w + 12.0, h));
    display.line_to((w - 10.0, h));
    display.curve_to((w + 4.0, h), (w + 4.0, -h), (w - 10.0, -h));
    display.line_to((-w + 12.0, -h));
    display.close_path();
    let circle = circle_path(0.0, 0.0, 18.0);
    let d = 18.0 * std::f64::consts::FRAC_1_SQRT_2;
    let mut sort = BezPath::new();
    lines(&mut sort, &[(0.0, -h), (w, 0.0), (0.0, h), (-w, 0.0)]);
    sort.move_to((-w, 0.0));
    sort.line_to((w, 0.0));
    vec![
        ("Process", doc(vec![bx(W, H)])),
        (
            "Alternate Process",
            doc(vec![path(&rounded_rect(W, H, 8.0))]),
        ),
        (
            "Decision",
            doc(vec![poly(&[(0.0, -h), (w, 0.0), (0.0, h), (-w, 0.0)])]),
        ),
        (
            "Data",
            doc(vec![poly(&[
                (-w, -h),
                (w - 12.0, -h),
                (w, h),
                (-w + 12.0, h),
            ])]),
        ),
        (
            "Predefined Process",
            doc(vec![path(&with_lines(
                rect.clone(),
                &[
                    ((-w + 7.0, -h), (-w + 7.0, h)),
                    ((w - 7.0, -h), (w - 7.0, h)),
                ],
            ))]),
        ),
        (
            "Internal Storage",
            doc(vec![path(&with_lines(
                rect,
                &[
                    ((-w + 8.0, -h), (-w + 8.0, h)),
                    ((-w, h - 8.0), (w, h - 8.0)),
                ],
            ))]),
        ),
        ("Document", doc(vec![path(&document)])),
        (
            "Terminator",
            doc(vec![path(&rounded_rect(W, H * 0.7, H * 0.35))]),
        ),
        (
            "Preparation",
            doc(vec![poly(&[
                (-w, 0.0),
                (-w + 12.0, h),
                (w - 12.0, h),
                (w, 0.0),
                (w - 12.0, -h),
                (-w + 12.0, -h),
            ])]),
        ),
        (
            "Manual Input",
            doc(vec![poly(&[(-w, -h), (w, -h), (w, h), (-w, h - 10.0)])]),
        ),
        (
            "Manual Operation",
            doc(vec![poly(&[
                (-w, h),
                (w, h),
                (w - 12.0, -h),
                (-w + 12.0, -h),
            ])]),
        ),
        ("Connector", doc(vec![ellipse(24.0, 24.0)])),
        (
            "Off-Page Connector",
            doc(vec![poly(&[
                (-14.0, 18.0),
                (14.0, 18.0),
                (14.0, -6.0),
                (0.0, -18.0),
                (-14.0, -6.0),
            ])]),
        ),
        (
            "Card",
            doc(vec![poly(&[
                (-w + 10.0, h),
                (w, h),
                (w, -h),
                (-w, -h),
                (-w, h - 10.0),
            ])]),
        ),
        ("Punched Tape", doc(vec![path(&tape)])),
        (
            "Summing Junction",
            doc(vec![path(&with_lines(
                circle.clone(),
                &[((-d, -d), (d, d)), ((-d, d), (d, -d))],
            ))]),
        ),
        (
            "Or",
            doc(vec![path(&with_lines(
                circle,
                &[((-18.0, 0.0), (18.0, 0.0)), ((0.0, -18.0), (0.0, 18.0))],
            ))]),
        ),
        (
            "Collate",
            doc(vec![poly(&[
                (-16.0, 20.0),
                (16.0, 20.0),
                (-16.0, -20.0),
                (16.0, -20.0),
            ])]),
        ),
        ("Sort", doc(vec![path(&sort)])),
        (
            "Extract",
            doc(vec![poly(&[(-20.0, -16.0), (20.0, -16.0), (0.0, 18.0)])]),
        ),
        (
            "Merge",
            doc(vec![poly(&[(-20.0, 16.0), (20.0, 16.0), (0.0, -18.0)])]),
        ),
        ("Delay", doc(vec![path(&delay)])),
        ("Stored Data", doc(vec![path(&stored)])),
        ("Display", doc(vec![path(&display)])),
    ]
}

fn callouts() -> Vec<(&'static str, Document)> {
    let w = W / 2.0;
    let h = H / 2.0;
    let rect_callout = poly(&[
        (-w, h),
        (w, h),
        (w, -h),
        (-6.0, -h),
        (-16.0, -h - 14.0),
        (-16.0, -h),
        (-w, -h),
    ]);
    let mut rounded = BezPath::new();
    let r = 7.0;
    rounded.move_to((-w + r, h));
    rounded.line_to((w - r, h));
    rounded.quad_to((w, h), (w, h - r));
    rounded.line_to((w, -h + r));
    rounded.quad_to((w, -h), (w - r, -h));
    rounded.line_to((-6.0, -h));
    rounded.line_to((-16.0, -h - 14.0));
    rounded.line_to((-16.0, -h));
    rounded.line_to((-w + r, -h));
    rounded.quad_to((-w, -h), (-w, -h + r));
    rounded.line_to((-w, h - r));
    rounded.quad_to((-w, h), (-w + r, h));
    rounded.close_path();
    // Oval with a tail: the ellipse arc leaves a gap where the tail joins.
    let mut oval = BezPath::new();
    let (a0, a1) = (-1.95f64, -1.45f64); // angles of the gap (radians)
    let el = crate::kurbo::Arc::new((0.0, 0.0), (w, h), a1, TAU - (a1 - a0), 0.0);
    oval.move_to((w * a1.cos(), h * a1.sin()));
    el.to_cubic_beziers(0.1, |p1, p2, p3| oval.curve_to(p1, p2, p3));
    oval.line_to((-20.0, -h - 14.0));
    oval.close_path();
    let mut thought = crate::kurbo::Ellipse::new((0.0, 4.0), (w, h - 2.0), 0.0).to_path(0.01);
    thought.extend(circle_path(-14.0, -h - 2.0, 4.0));
    thought.extend(circle_path(-20.0, -h - 10.0, 2.5));
    let mut line = rounded_rect(W, H, 0.0);
    line.move_to((-w, -h));
    line.line_to((-w - 16.0, -h - 14.0));
    vec![
        ("Rectangular Callout", doc(vec![rect_callout])),
        ("Rounded Rectangular Callout", doc(vec![path(&rounded)])),
        ("Oval Callout", doc(vec![path(&oval)])),
        ("Thought Bubble", doc(vec![path(&thought)])),
        ("Line Callout", doc(vec![path(&line)])),
    ]
}

fn equation_shapes() -> Vec<(&'static str, Document)> {
    let bar = |y: f64| {
        [
            (-20.0, y - 3.5),
            (20.0, y - 3.5),
            (20.0, y + 3.5),
            (-20.0, y + 3.5),
        ]
    };
    let mut divide = BezPath::new();
    lines(&mut divide, &bar(0.0));
    divide.extend(circle_path(0.0, 11.0, 4.0));
    divide.extend(circle_path(0.0, -11.0, 4.0));
    let mut equal = BezPath::new();
    lines(&mut equal, &bar(7.0));
    lines(&mut equal, &bar(-7.0));
    let mut not_equal = equal.clone();
    lines(
        &mut not_equal,
        &[(4.0, 20.0), (10.0, 20.0), (-4.0, -20.0), (-10.0, -20.0)],
    );
    let plus = |a: f64, b: f64| {
        vec![
            (-a, b),
            (-b, b),
            (-b, a),
            (b, a),
            (b, b),
            (a, b),
            (a, -b),
            (b, -b),
            (b, -a),
            (-b, -a),
            (-b, -b),
            (-a, -b),
        ]
    };
    let times: Vec<(f64, f64)> = plus(20.0, 3.5)
        .iter()
        .map(|(x, y)| {
            let c = std::f64::consts::FRAC_1_SQRT_2;
            (c * (x - y), c * (x + y))
        })
        .collect();
    vec![
        ("Plus Sign", doc(vec![poly(&plus(20.0, 3.5))])),
        ("Minus Sign", doc(vec![poly(&bar(0.0))])),
        ("Multiply Sign", doc(vec![poly(&times)])),
        ("Division Sign", doc(vec![path(&divide)])),
        ("Equal Sign", doc(vec![path(&equal)])),
        ("Not Equal Sign", doc(vec![path(&not_equal)])),
    ]
}

/// Many-body diagrams, drawn from their equations (they stay editable in
/// the LaTeX tab).
const MANY_BODY: &[(&str, &str)] = &[
    (
        "MBPT(2) energy",
        r"E^{(2)} = \frac{1}{4} \sum_{ijab} V_{ijab} V_{abij}",
    ),
    (
        "CC energy",
        r"E = \sum_{ia} f_{ia} t_{ai} + \frac{1}{4} \sum_{ijab} V_{ijab} t_{abij} + \frac{1}{2} \sum_{ijab} V_{ijab} t_{ai} t_{bj}",
    ),
    ("T1 amplitude", r"Z_{ai} = t_{ai}"),
    ("T2 amplitude", r"Z_{abij} = t_{abij}"),
    ("T3 amplitude", r"Z_{abcijk} = t_{abcijk}"),
    ("One-body operator", r"Z_{ai} = f_{ai}"),
    ("Two-body interaction", r"Z_{abij} = V_{abij}"),
    (
        "Particle-particle ladder",
        r"Z_{abij} = \frac{1}{2} \sum_{ef} V_{abef} t_{efij}",
    ),
    (
        "Hole-hole ladder",
        r"Z_{abij} = \frac{1}{2} \sum_{kl} V_{klij} t_{abkl}",
    ),
    (
        "Ring (particle-hole)",
        r"Z_{abij} = \sum_{kc} V_{kbcj} t_{acik}",
    ),
    ("Fock particle line", r"Z_{abij} = \sum_{e} f_{be} t_{aeij}"),
    ("Fock hole line", r"Z_{abij} = - \sum_{m} f_{mj} t_{abim}"),
    (
        "Quadratic T2 T2",
        r"Z_{abij} = \frac{1}{4} \sum_{klcd} V_{klcd} t_{cdij} t_{abkl}",
    ),
    (
        "T1 from T2 (particle)",
        r"Z_{ai} = \frac{1}{2} \sum_{kcd} V_{akcd} t_{cdik}",
    ),
    (
        "IMSRG(2) energy flow",
        r"\frac{dE}{ds} = \frac{1}{4} \sum_{abcd} \eta_{abcd} \Gamma_{cdab} n_a n_b \bar{n}_c \bar{n}_d",
    ),
    (
        "IMSRG one-body flow",
        r"\frac{df_{pq}}{ds} = \sum_{r} \eta_{pr} f_{rq}",
    ),
];

/// Feynman diagrams as `.gold` text (vertices named by position).
const FEYNMAN: &[(&str, &str)] = &[
    (
        "Fermion propagator",
        r#"vertex = [{ id = "a", at = [0, 0] }, { id = "b", at = [60, 0] }]
edge = [{ from = "a", to = "b", arrow = "forward" }]"#,
    ),
    (
        "Photon propagator",
        r#"vertex = [{ id = "a", at = [0, 0] }, { id = "b", at = [60, 0] }]
edge = [{ from = "a", to = "b", stroke = "wavy" }]"#,
    ),
    (
        "Gluon propagator",
        r#"vertex = [{ id = "a", at = [0, 0] }, { id = "b", at = [60, 0] }]
edge = [{ from = "a", to = "b", stroke = "curly" }]"#,
    ),
    (
        "Scalar propagator",
        r#"vertex = [{ id = "a", at = [0, 0] }, { id = "b", at = [60, 0] }]
edge = [{ from = "a", to = "b", stroke = "dashed" }]"#,
    ),
    (
        "QED vertex",
        r#"vertex = [{ id = "i", at = [-30, -26] }, { id = "o", at = [-30, 26] }, { id = "v", at = [0, 0], kind = "dot" }, { id = "g", at = [40, 0] }]
edge = [
  { from = "i", to = "v", arrow = "forward", label = "$e^-$" },
  { from = "v", to = "o", arrow = "forward", label = "$e^-$", label_side = "right" },
  { from = "v", to = "g", stroke = "wavy", label = "$\\gamma$" },
]"#,
    ),
    (
        "s-channel (annihilation)",
        r#"vertex = [
  { id = "a", at = [-50, -30] }, { id = "b", at = [-50, 30] }, { id = "v1", at = [-20, 0], kind = "dot" },
  { id = "v2", at = [20, 0], kind = "dot" }, { id = "c", at = [50, -30] }, { id = "d", at = [50, 30] },
]
edge = [
  { from = "a", to = "v1", arrow = "forward", label = "$e^-$", label_side = "right" },
  { from = "v1", to = "b", arrow = "forward", label = "$e^+$", label_side = "right" },
  { from = "v1", to = "v2", stroke = "wavy", label = "$\\gamma$" },
  { from = "c", to = "v2", arrow = "forward", label = "$\\mu^+$" },
  { from = "v2", to = "d", arrow = "forward", label = "$\\mu^-$" },
]"#,
    ),
    (
        "t-channel (scattering)",
        r#"vertex = [
  { id = "a", at = [-40, -40] }, { id = "b", at = [40, -40] }, { id = "v1", at = [-20, 0], kind = "dot" },
  { id = "v2", at = [20, 0], kind = "dot" }, { id = "c", at = [-40, 40] }, { id = "d", at = [40, 40] },
]
edge = [
  { from = "a", to = "v1", arrow = "forward", label = "$e^-$" },
  { from = "v1", to = "c", arrow = "forward", label = "$e^-$" },
  { from = "b", to = "v2", arrow = "forward", label = "$e^-$", label_side = "right" },
  { from = "v2", to = "d", arrow = "forward", label = "$e^-$", label_side = "right" },
  { from = "v1", to = "v2", stroke = "wavy", label = "$\\gamma$" },
]"#,
    ),
    (
        "Electron self-energy",
        r#"vertex = [{ id = "a", at = [-50, 0] }, { id = "v1", at = [-20, 0], kind = "dot" }, { id = "v2", at = [20, 0], kind = "dot" }, { id = "b", at = [50, 0] }]
edge = [
  { from = "a", to = "v1", arrow = "forward" },
  { from = "v1", to = "v2", arrow = "forward" },
  { from = "v2", to = "b", arrow = "forward" },
  { from = "v1", to = "v2", stroke = "wavy", bend = 90 },
]"#,
    ),
    (
        "Vacuum polarization",
        r#"vertex = [{ id = "a", at = [-50, 0] }, { id = "v1", at = [-20, 0], kind = "dot" }, { id = "v2", at = [20, 0], kind = "dot" }, { id = "b", at = [50, 0] }]
edge = [
  { from = "a", to = "v1", stroke = "wavy" },
  { from = "v1", to = "v2", arrow = "forward", bend = 90 },
  { from = "v2", to = "v1", arrow = "forward", bend = 90 },
  { from = "v2", to = "b", stroke = "wavy" },
]"#,
    ),
    (
        "Tadpole",
        r#"vertex = [{ id = "a", at = [-40, 0] }, { id = "v", at = [0, 0], kind = "dot" }, { id = "b", at = [40, 0] }]
edge = [
  { from = "a", to = "v", stroke = "dashed" },
  { from = "v", to = "b", stroke = "dashed" },
  { from = "v", to = "v", arrow = "forward", loop = { radius = 14, angle = 90 } },
]"#,
    ),
    (
        "Compton scattering",
        r#"vertex = [
  { id = "a", at = [-50, -30] }, { id = "g1", at = [-50, 30] }, { id = "v1", at = [-20, 0], kind = "dot" },
  { id = "v2", at = [20, 0], kind = "dot" }, { id = "b", at = [50, -30] }, { id = "g2", at = [50, 30] },
]
edge = [
  { from = "a", to = "v1", arrow = "forward", label = "$e^-$", label_side = "right" },
  { from = "g1", to = "v1", stroke = "wavy", label = "$\\gamma$" },
  { from = "v1", to = "v2", arrow = "forward" },
  { from = "v2", to = "b", arrow = "forward", label = "$e^-$" },
  { from = "v2", to = "g2", stroke = "wavy", label = "$\\gamma$", label_side = "right" },
]"#,
    ),
];

/// All built-in templates.
pub fn builtin() -> Vec<Template> {
    let mut out = Vec::new();
    let mut add = |category: &'static str, items: Vec<(&'static str, Document)>| {
        for (name, doc) in items {
            out.push(Template {
                category,
                name: name.to_string(),
                doc,
            });
        }
    };
    add("Basic Shapes", basic());
    add("Block Arrows", arrows());
    add("Stars and Banners", stars());
    add("Flowchart", flowchart());
    add("Callouts", callouts());
    add("Equation Shapes", equation_shapes());
    let many: Vec<(&'static str, Document)> = MANY_BODY
        .iter()
        .filter_map(|(name, tex)| Some((*name, crate::expr::from_latex_editable(tex).ok()?.doc)))
        .collect();
    add("Many-Body Diagrams", many);
    let feyn: Vec<(&'static str, Document)> = FEYNMAN
        .iter()
        .filter_map(|(name, src)| {
            Some((*name, crate::io::parse(&format!("format = 1\n{src}")).ok()?))
        })
        .collect();
    add("Feynman Diagrams", feyn);
    out
}

/// `doc` moved by `d`, with `prefix` before every id (so several copies
/// can share a document).
pub fn placed(doc: &Document, prefix: &str, d: crate::kurbo::Vec2) -> Document {
    let mut doc = doc.clone();
    let ren = |id: &mut String| *id = format!("{prefix}{id}");
    let mv = |p: &mut [f64; 2]| {
        p[0] = r2(p[0] + d.x);
        p[1] = r2(p[1] + d.y);
    };
    for v in &mut doc.vertices {
        ren(&mut v.id);
        mv(&mut v.at);
    }
    for e in &mut doc.edges {
        ren(&mut e.from);
        ren(&mut e.to);
        if let Some(id) = e.id.as_mut() {
            ren(id);
        }
        if let Some(cs) = e.controls.as_mut() {
            cs.iter_mut().for_each(mv);
        }
    }
    for t in &mut doc.texts {
        if let Some(id) = t.id.as_mut() {
            ren(id);
        }
        mv(&mut t.at);
    }
    for a in &mut doc.axes {
        if let Some(id) = a.id.as_mut() {
            ren(id);
        }
        mv(&mut a.origin);
    }
    for s in &mut doc.shapes {
        if let Some(id) = s.id.as_mut() {
            ren(id);
        }
        crate::shapes::transform(s, crate::kurbo::Affine::translate(d));
    }
    for g in &mut doc.groups {
        ren(&mut g.id);
        g.members.iter_mut().for_each(ren);
    }
    doc
}

/// All templates on one sheet, by category, each with its name below it.
pub fn sheet(templates: &[Template]) -> Document {
    let mut out = Document::default();
    let mut y = 0.0;
    let text =
        |id: String, at: (f64, f64), content: String, size: f64, anchor| crate::model::Text {
            id: Some(id),
            at: [r2(at.0), r2(at.1)],
            content,
            class: None,
            anchor: Some(anchor),
            size: Some(size),
            rotation: None,
            color: None,
            layer: None,
        };
    let mut n = 0;
    for cat in CATEGORIES {
        let items: Vec<&Template> = templates.iter().filter(|t| t.category == *cat).collect();
        if items.is_empty() {
            continue;
        }
        out.texts.push(text(
            format!("cat{n}"),
            (0.0, y),
            cat.to_string(),
            12.0,
            crate::model::Anchor::West,
        ));
        y -= 20.0;
        let (mut x, mut row_h) = (0.0f64, 0.0f64);
        for t in items {
            n += 1;
            let Some(b) = crate::scene::build(&t.doc)
                .ok()
                .and_then(|s| s.path_bounds())
            else {
                continue;
            };
            // Labels stick out of the lines (a caption like E = 1/4 on the left).
            let b = t.doc.texts.iter().fold(b, |b, x| {
                let w = 5.0 * x.content.len() as f64;
                b.union(crate::kurbo::Rect::new(
                    x.at[0] - w.min(60.0),
                    x.at[1] - 8.0,
                    x.at[0] + 8.0,
                    x.at[1] + 8.0,
                ))
            });
            let w = b.width().max(70.0);
            if x > 0.0 && x + w > 900.0 {
                x = 0.0;
                y -= row_h + 34.0;
                row_h = 0.0;
            }
            let d = crate::kurbo::Vec2::new(x + (w - b.width()) / 2.0 - b.x0, y - b.y1);
            let p = placed(&t.doc, &format!("t{n}_"), d);
            for (k, v) in p.styles {
                out.styles.entry(k).or_insert(v);
            }
            out.vertices.extend(p.vertices);
            out.edges.extend(p.edges);
            out.texts.extend(p.texts);
            out.axes.extend(p.axes);
            out.shapes.extend(p.shapes);
            out.groups.extend(p.groups);
            out.texts.push(text(
                format!("name{n}"),
                (x + w / 2.0, y - b.height() - 8.0),
                t.name.clone(),
                7.0,
                crate::model::Anchor::North,
            ));
            row_h = row_h.max(b.height());
            x += w + 24.0;
        }
        y -= row_h + 50.0;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_is_valid() {
        let all = builtin();
        for (name, _) in MANY_BODY {
            assert!(all.iter().any(|t| t.name == *name), "{name} did not draw");
        }
        for (name, src) in FEYNMAN {
            crate::io::parse(&format!("format = 1\n{src}"))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
        }
        for t in &all {
            let s = crate::scene::build(&t.doc)
                .unwrap_or_else(|e| panic!("{} / {}: {e}", t.category, t.name));
            assert!(s.path_bounds().is_some(), "{} draws nothing", t.name);
            assert!(CATEGORIES.contains(&t.category));
        }
        assert!(all.len() > 100, "{} templates", all.len());
        crate::scene::build(&sheet(&all)).expect("the overview sheet is valid");
    }
}

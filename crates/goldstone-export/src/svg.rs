//! SVG backend. Output units are pt (1 user unit = 1 pt).

use std::fmt::Write as _;

use goldstone_core::kurbo::{Affine, BezPath, PathEl, Point, Rect, Vec2};
use goldstone_core::scene::{Cap, Join, Mark, PathItem, Scene, TextItem};

use crate::labels::{LabelBox, Labels};
use crate::{ExportError, num};

/// Namespace of the embedded `.gold` source in `<metadata>`.
pub const SOURCE_NS: &str = "urn:goldstone:source";

/// A text item together with its typeset box and placement.
pub(crate) struct PlacedText<'a> {
    pub item: &'a TextItem,
    pub label: LabelBox,
    /// Bottom-left corner of the (unrotated) box, y up.
    pub origin: Point,
}

impl PlacedText<'_> {
    /// Corners of the (possibly rotated) box, y up.
    fn corners(&self) -> [Point; 4] {
        let (w, h) = (self.label.width, self.label.height);
        let o = self.origin;
        let rot = Affine::rotate_about(self.item.rotation.to_radians(), self.item.at);
        [
            o,
            o + Vec2::new(w, 0.0),
            o + Vec2::new(w, h),
            o + Vec2::new(0.0, h),
        ]
        .map(|p| rot * p)
    }
}

pub(crate) fn place_texts<'a>(
    scene: &'a Scene,
    labels: &mut Labels,
) -> Result<Vec<PlacedText<'a>>, ExportError> {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for item in &scene.items {
        if let Mark::Text(t) = &item.mark {
            match labels.typeset(&t.content, t.size, t.color) {
                Ok(label) => {
                    let (ux, uy) = t.anchor.unit_offset();
                    let center = t.at - Vec2::new(ux * label.width / 2.0, uy * label.height / 2.0);
                    let origin = center - Vec2::new(label.width / 2.0, label.height / 2.0);
                    out.push(PlacedText {
                        item: t,
                        label,
                        origin,
                    });
                }
                Err(e) => errors.push(format!("{} ({}): {e}", item.owner, t.content)),
            }
        }
    }
    if errors.is_empty() {
        Ok(out)
    } else {
        Err(ExportError::Labels(errors))
    }
}

/// Bounding box of the drawing including typeset text, before the margin.
pub(crate) fn full_bounds(scene: &Scene, texts: &[PlacedText]) -> Rect {
    let mut r = scene.path_bounds();
    for t in texts {
        for c in t.corners() {
            let pr = Rect::from_points(c, c);
            r = Some(r.map_or(pr, |a| a.union(pr)));
        }
    }
    r.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0))
}

/// SVG path data with y flipped (SVG y points down).
fn path_data(path: &BezPath) -> String {
    let f = |p: Point| format!("{} {}", num(p.x), num(-p.y));
    let mut d = String::new();
    for el in path.elements() {
        if !d.is_empty() {
            d.push(' ');
        }
        match *el {
            PathEl::MoveTo(p) => write!(d, "M{}", f(p)),
            PathEl::LineTo(p) => write!(d, "L{}", f(p)),
            PathEl::QuadTo(a, p) => write!(d, "Q{} {}", f(a), f(p)),
            PathEl::CurveTo(a, b, p) => write!(d, "C{} {} {}", f(a), f(b), f(p)),
            PathEl::ClosePath => write!(d, "Z"),
        }
        .unwrap();
    }
    d
}

fn path_element(p: &PathItem) -> String {
    let mut attrs = format!("d=\"{}\"", path_data(&p.path));
    match p.fill {
        Some(c) => write!(attrs, " fill=\"{}\"", c.hex()).unwrap(),
        None => attrs.push_str(" fill=\"none\""),
    }
    if let Some(s) = &p.stroke {
        write!(
            attrs,
            " stroke=\"{}\" stroke-width=\"{}\"",
            s.color.hex(),
            num(s.width)
        )
        .unwrap();
        if s.cap == Cap::Round {
            attrs.push_str(" stroke-linecap=\"round\"");
        }
        if s.join == Join::Round {
            attrs.push_str(" stroke-linejoin=\"round\"");
        }
        if let Some([on, off]) = s.dash {
            write!(attrs, " stroke-dasharray=\"{} {}\"", num(on), num(off)).unwrap();
        }
    }
    format!("<path {attrs}/>")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Renders a scene to a standalone SVG document. If `source` is given, the
/// `.gold` text is embedded in `<metadata>` so the figure can be reopened.
pub fn render(
    scene: &Scene,
    labels: &mut Labels,
    source: Option<&str>,
) -> Result<String, ExportError> {
    let texts = place_texts(scene, labels)?;
    let b = full_bounds(scene, &texts).inflate(scene.margin, scene.margin);
    let (w, h) = (b.width(), b.height());

    let mut s = String::new();
    writeln!(s, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>").unwrap();
    writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" \
         width=\"{}pt\" height=\"{}pt\" viewBox=\"{} {} {} {}\">",
        num(w),
        num(h),
        num(b.x0),
        num(-b.y1),
        num(w),
        num(h)
    )
    .unwrap();
    if let Some(src) = source {
        let cdata = src.replace("]]>", "]]]]><![CDATA[>");
        writeln!(s, "<metadata><goldstone:source xmlns:goldstone=\"{SOURCE_NS}\"><![CDATA[{cdata}]]></goldstone:source></metadata>")
            .unwrap();
    }
    if let Some(bg) = scene.background {
        writeln!(
            s,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            num(b.x0),
            num(-b.y1),
            num(w),
            num(h),
            bg.hex()
        )
        .unwrap();
    }

    let mut text_iter = texts.iter();
    let mut n_label = 0usize;
    for item in &scene.items {
        match &item.mark {
            Mark::Path(p) => {
                writeln!(
                    s,
                    "<!-- {} -->{}",
                    xml_escape(&item.owner).replace("--", "- -"),
                    path_element(p)
                )
                .unwrap();
            }
            Mark::Text(_) => {
                let t = text_iter.next().expect("one placement per text item");
                n_label += 1;
                let prefix = format!("L{n_label}-");
                let body = t
                    .label
                    .svg
                    .replace("id=\"", &format!("id=\"{prefix}"))
                    .replace("href=\"#", &format!("href=\"#{prefix}"))
                    .replace("url(#", &format!("url(#{prefix}"));
                let top_left = (t.origin.x, -(t.origin.y + t.label.height));
                let mut transform = String::new();
                if t.item.rotation != 0.0 {
                    write!(
                        transform,
                        "rotate({} {} {}) ",
                        num(-t.item.rotation),
                        num(t.item.at.x),
                        num(-t.item.at.y)
                    )
                    .unwrap();
                }
                write!(
                    transform,
                    "translate({} {})",
                    num(top_left.0),
                    num(top_left.1)
                )
                .unwrap();
                writeln!(
                    s,
                    "<g transform=\"{transform}\"><title>{}</title>{body}</g>",
                    xml_escape(&t.item.content)
                )
                .unwrap();
            }
        }
    }
    s.push_str("</svg>\n");
    Ok(s)
}

/// Extracts an embedded `.gold` source from an SVG written by [`render`].
pub fn extract_source(svg: &str) -> Option<String> {
    let start = svg.find("<goldstone:source")?;
    let rest = &svg[start..];
    let open = rest.find("<![CDATA[")? + "<![CDATA[".len();
    let close = rest.find("</goldstone:source>")?;
    let raw = &rest[open..close];
    let raw = raw.strip_suffix("]]>")?;
    Some(raw.replace("]]]]><![CDATA[>", "]]>"))
}

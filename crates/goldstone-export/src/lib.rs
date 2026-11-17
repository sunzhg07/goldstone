//! Goldstone export backends: SVG, PDF, PNG and TikZ.
//!
//! All backends draw the same [`Scene`], so they agree with each other and
//! with the editor canvas.

use std::fmt;

use goldstone_core::scene::Scene;

pub mod labels;
pub mod svg;
pub mod tikz;

pub use labels::Labels;

/// Output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Svg,
    Pdf,
    Png,
    /// A complete `standalone` LaTeX document (`.tex`).
    Tex,
    /// A bare `tikzpicture` to `\input` into a paper (`.tikz`).
    Tikz,
}

impl Format {
    pub const ALL: [Format; 5] = [
        Format::Svg,
        Format::Pdf,
        Format::Png,
        Format::Tex,
        Format::Tikz,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Pdf => "pdf",
            Format::Png => "png",
            Format::Tex => "tex",
            Format::Tikz => "tikz",
        }
    }

    pub fn from_extension(ext: &str) -> Option<Format> {
        Format::ALL
            .into_iter()
            .find(|f| f.extension().eq_ignore_ascii_case(ext))
    }
}

#[derive(Debug)]
pub enum ExportError {
    /// Labels that could not be typeset.
    Labels(Vec<String>),
    Pdf(String),
    Png(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Labels(es) => {
                for (i, e) in es.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "error: label {e}")?;
                }
                Ok(())
            }
            ExportError::Pdf(e) => write!(f, "error: PDF export failed: {e}"),
            ExportError::Png(e) => write!(f, "error: PNG export failed: {e}"),
        }
    }
}

impl std::error::Error for ExportError {}

/// Export options.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    /// The `.gold` source, embedded in SVG output.
    pub source: Option<&'a str>,
    /// File name of the source, mentioned in TikZ headers.
    pub source_name: Option<&'a str>,
    /// Resolution for PNG output.
    pub dpi: f32,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            source: None,
            source_name: None,
            dpi: 300.0,
        }
    }
}

/// Renders a scene in the given format.
pub fn export(
    scene: &Scene,
    format: Format,
    labels: &mut Labels,
    opts: &Options,
) -> Result<Vec<u8>, ExportError> {
    Ok(match format {
        Format::Svg => svg::render(scene, labels, opts.source)?.into_bytes(),
        Format::Tex => tikz::render_standalone(scene, opts.source_name).into_bytes(),
        Format::Tikz => tikz::render_picture(scene, opts.source_name).into_bytes(),
        Format::Pdf => {
            let svg = svg::render(scene, labels, None)?;
            let tree = parse_svg(&svg).map_err(ExportError::Pdf)?;
            // usvg works in CSS px (96 per inch); 96 dpi makes 1 pt in → 1 pt out.
            let page = svg2pdf::PageOptions { dpi: 96.0 };
            svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), page)
                .map_err(|e| ExportError::Pdf(format!("{e:?}")))?
        }
        Format::Png => {
            let svg = svg::render(scene, labels, None)?;
            let tree = parse_svg(&svg).map_err(ExportError::Png)?;
            let scale = opts.dpi / 96.0;
            let size = tree
                .size()
                .to_int_size()
                .scale_by(scale)
                .ok_or_else(|| ExportError::Png("image too small".into()))?;
            let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
                .ok_or_else(|| {
                    ExportError::Png(format!(
                        "cannot allocate {}×{} image",
                        size.width(),
                        size.height()
                    ))
                })?;
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(scale, scale),
                &mut pixmap.as_mut(),
            );
            pixmap
                .encode_png()
                .map_err(|e| ExportError::Png(e.to_string()))?
        }
    })
}

fn parse_svg(svg: &str) -> Result<svg2pdf::usvg::Tree, String> {
    svg2pdf::usvg::Tree::from_str(svg, &svg2pdf::usvg::Options::default())
        .map_err(|e| e.to_string())
}

/// Formats a coordinate compactly: at most 3 decimals, no trailing zeros.
pub(crate) fn num(x: f64) -> String {
    let s = format!("{:.3}", x);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use goldstone_core::{io, scene};

    fn example(name: &str) -> (String, Scene) {
        let path = format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(path).unwrap();
        let sc = scene::build(&io::parse(&src).unwrap()).unwrap();
        (src, sc)
    }

    #[test]
    fn numbers() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(2.50049), "2.5");
        assert_eq!(num(-1.23456), "-1.235");
    }

    #[test]
    fn all_examples_export_in_all_formats() {
        let mut labels = Labels::new();
        for name in ["mbpt2.gold", "onebody.gold", "gallery.gold"] {
            let (src, sc) = example(name);
            for f in Format::ALL {
                let opts = Options {
                    source: Some(&src),
                    source_name: Some(name),
                    dpi: 72.0,
                };
                let out = export(&sc, f, &mut labels, &opts)
                    .unwrap_or_else(|e| panic!("{name} as {f:?}: {e}"));
                assert!(!out.is_empty());
            }
        }
    }

    #[test]
    fn svg_embeds_source() {
        let (src, sc) = example("mbpt2.gold");
        let tricky = format!("{src}\n# ]]> inside a comment\n");
        let svg = svg::render(&sc, &mut Labels::new(), Some(&tricky)).unwrap();
        assert_eq!(svg::extract_source(&svg).unwrap(), tricky);
    }

    #[test]
    fn pdf_page_size_is_in_points() {
        let (_, sc) = example("mbpt2.gold");
        let mut labels = Labels::new();
        let svg = svg::render(&sc, &mut labels, None).unwrap();
        let w: f64 = svg
            .split("width=\"")
            .nth(1)
            .unwrap()
            .split("pt")
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let pdf = export(&sc, Format::Pdf, &mut labels, &Options::default()).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        let mb = text
            .split("/MediaBox [")
            .nth(1)
            .expect("MediaBox")
            .split(']')
            .next()
            .unwrap();
        let nums: Vec<f64> = mb.split_whitespace().map(|n| n.parse().unwrap()).collect();
        assert!(
            (nums[2] - nums[0] - w).abs() < 0.5,
            "MediaBox {mb} vs SVG width {w}"
        );
    }
}

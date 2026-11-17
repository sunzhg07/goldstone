//! Typesetting of label text (`plain text with $LaTeX math$`).
//!
//! LaTeX math is converted to Typst math by `mitex` and typeset by the Typst
//! compiler with the bundled New Computer Modern fonts, entirely in-process.
//! The result is an SVG fragment with glyph outlines (no font needed to view it).

use std::collections::HashMap;
use std::sync::OnceLock;

use goldstone_core::color::Color;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_layout::PagedDocument;

/// A typeset label.
#[derive(Debug, Clone)]
pub struct LabelBox {
    /// Size in pt.
    pub width: f64,
    pub height: f64,
    /// SVG markup (without a root element) in a coordinate system with the
    /// origin at the top-left corner of the box, y down, units of pt.
    pub svg: String,
}

/// Padding around the ink box, comparable to TikZ's `inner sep`.
pub const INNER_SEP: f64 = 1.0;

struct Shared {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
}

fn shared() -> &'static Shared {
    static SHARED: OnceLock<Shared> = OnceLock::new();
    SHARED.get_or_init(|| {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        Shared {
            library: LazyHash::new(Library::default()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
        }
    })
}

struct LabelWorld {
    main: FileId,
    source: Source,
}

impl World for LabelWorld {
    fn library(&self) -> &LazyHash<Library> {
        &shared().library
    }
    fn book(&self) -> &LazyHash<FontBook> {
        &shared().book
    }
    fn main(&self) -> FileId {
        self.main
    }
    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main {
            Ok(self.source.clone())
        } else {
            Err(FileError::AccessDenied)
        }
    }
    fn file(&self, _id: FileId) -> FileResult<Bytes> {
        Err(FileError::AccessDenied)
    }
    fn font(&self, index: usize) -> Option<Font> {
        shared().fonts.get(index).cloned()
    }
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

/// Escapes plain text for Typst markup.
fn escape_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if "\\*_#$[]`<>@~=-+/'\"".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Converts `text $math$ text` into Typst markup.
pub fn to_typst(content: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut parts = content.split('$');
    let mut in_math = false;
    // Splitting on '$' alternates text / math; an even number of '$' is required.
    if !content.matches('$').count().is_multiple_of(2) {
        return Err("unbalanced `$`: math must be enclosed in a pair of `$`".into());
    }
    for part in parts.by_ref() {
        if in_math {
            let m = mitex::convert_math(part, None)
                .map_err(|e| format!("cannot read LaTeX `{part}`: {e}"))?;
            out.push('$');
            out.push_str(&m);
            out.push('$');
        } else {
            out.push_str(&escape_markup(part));
        }
        in_math = !in_math;
    }
    Ok(out)
}

/// Typesets labels, caching results.
#[derive(Default)]
pub struct Labels {
    cache: HashMap<(String, u64, Color), Result<LabelBox, String>>,
}

impl Labels {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn typeset(&mut self, content: &str, size: f64, color: Color) -> Result<LabelBox, String> {
        let key = (content.to_string(), size.to_bits(), color);
        if let Some(r) = self.cache.get(&key) {
            return r.clone();
        }
        let r = typeset_uncached(content, size, color);
        self.cache.insert(key, r.clone());
        r
    }
}

fn typeset_uncached(content: &str, size: f64, color: Color) -> Result<LabelBox, String> {
    let body = to_typst(content)?;
    let src = format!(
        "#set page(width: auto, height: auto, margin: {m}pt, fill: none)\n\
         #set text(font: \"New Computer Modern\", size: {size}pt, fill: rgb(\"{hex}\"), top-edge: \"bounds\", bottom-edge: \"bounds\")\n\
         #show math.equation: set text(font: \"New Computer Modern Math\")\n\
         {body}\n",
        m = INNER_SEP,
        hex = color.hex(),
    );
    let main = RootedPath::new(
        VirtualRoot::Project,
        VirtualPath::new("main.typ").expect("valid path"),
    )
    .intern();
    let world = LabelWorld {
        main,
        source: Source::new(main, src),
    };
    let doc: PagedDocument = typst::compile(&world).output.map_err(|errs| {
        let msgs: Vec<String> = errs.iter().map(|e| e.message.to_string()).collect();
        format!("cannot typeset label {content:?}: {}", msgs.join("; "))
    })?;
    let page = doc
        .pages()
        .first()
        .ok_or_else(|| format!("label {content:?} produced no output"))?;
    let svg = typst_svg::svg(page, &typst_svg::SvgOptions::default());
    Ok(LabelBox {
        width: page.frame.width().to_pt(),
        height: page.frame.height().to_pt(),
        svg: inner_svg(&svg),
    })
}

/// Strips the root `<svg …>` element, keeping its children.
fn inner_svg(svg: &str) -> String {
    let start = svg
        .find("<svg")
        .and_then(|i| svg[i..].find('>').map(|j| i + j + 1))
        .unwrap_or(0);
    let end = svg.rfind("</svg>").unwrap_or(svg.len());
    svg[start..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_mixed_text() {
        let t = to_typst(r"order $t_{ij}^{ab}$ #1").unwrap();
        assert!(t.starts_with("order $"), "{t}");
        assert!(t.ends_with(r"\#1"), "{t}");
        assert!(to_typst("$a").is_err());
    }

    #[test]
    fn typesets_math() {
        let mut l = Labels::new();
        let b = l
            .typeset(r"$\bar{t}_{ij}^{ab}$", 10.0, Color::BLACK)
            .unwrap();
        assert!(b.width > 5.0 && b.width < 40.0, "width {}", b.width);
        assert!(b.height > 5.0 && b.height < 25.0, "height {}", b.height);
        assert!(
            b.svg.contains("<path") || b.svg.contains("<use"),
            "{}",
            &b.svg[..b.svg.len().min(400)]
        );
        // Larger font → larger box.
        let big = l
            .typeset(r"$\bar{t}_{ij}^{ab}$", 20.0, Color::BLACK)
            .unwrap();
        assert!(big.width > 1.8 * (b.width - 2.0 * INNER_SEP));
    }
}

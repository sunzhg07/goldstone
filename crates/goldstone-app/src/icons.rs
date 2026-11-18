//! Editor icons. Each icon is a Goldstone diagram (`icons/*.gold`, a 24 × 24
//! pt canvas) rendered by the same pipeline as the canvas and the exports.
//! Black, white and greys are re-tinted for the UI theme (black becomes the
//! text color, white the panel color); colored parts keep their color.

use std::collections::HashMap;

use goldstone_core::kurbo::Rect;
use goldstone_export::Labels;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        /// (name, `.gold` source) of every icon.
        pub const SOURCES: &[(&str, &str)] = &[
            $(($name, include_str!(concat!("../icons/", $name, ".gold"))),)*
        ];
    };
}

icons!(
    "undo",
    "redo",
    "select",
    "edit",
    "edge",
    "arc",
    "vertex",
    "cross",
    "text",
    "rect",
    "ellipse",
    "group",
    "ungroup",
    "union",
    "difference",
    "intersection",
    "exclusion",
    "to-path",
    "flip-h",
    "flip-v",
    "rotate-ccw",
    "rotate-cw",
    "align-left",
    "align-center-x",
    "align-right",
    "align-top",
    "align-center-y",
    "align-bottom",
    "distribute-x",
    "distribute-y",
);

/// Side of the icon canvas in pt.
pub const ICON_PT: f64 = 24.0;

/// Renders an icon to premultiplied RGBA (`px` × `px`), untinted.
pub fn render(name: &str, px: u32, labels: &mut Labels) -> Result<Vec<u8>, String> {
    let src = SOURCES
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("no icon {name:?}"))?
        .1;
    let doc = goldstone_core::io::parse(src).map_err(|e| e.to_string())?;
    let scene = goldstone_core::scene::build(&doc).map_err(|e| e.to_string())?;
    let (pixels, errors) = goldstone_export::rasterize_view(
        &scene,
        labels,
        Rect::new(0.0, 0.0, ICON_PT, ICON_PT),
        px,
        px,
    );
    if let Some(e) = errors.first() {
        return Err(e.clone());
    }
    Ok(pixels)
}

/// Re-tints neutral (grey) pixels: black → `fg`, white → `bg`.
pub fn tint(pixels: &mut [u8], fg: [u8; 3], bg: [u8; 3]) {
    for p in pixels.as_chunks_mut::<4>().0 {
        let a = p[3];
        if a == 0 {
            continue;
        }
        let un = |c: u8| (c as f32 * 255.0 / a as f32).min(255.0);
        let (r, g, b) = (un(p[0]), un(p[1]), un(p[2]));
        if r.max(g).max(b) - r.min(g).min(b) > 24.0 {
            continue; // colored: keep
        }
        let l = (r + g + b) / (3.0 * 255.0);
        for k in 0..3 {
            let c = fg[k] as f32 * (1.0 - l) + bg[k] as f32 * l;
            p[k] = (c * a as f32 / 255.0).round() as u8;
        }
    }
}

/// Icon textures, rendered on first use and again when the theme changes.
#[derive(Default)]
pub struct Icons {
    cache: HashMap<&'static str, egui::TextureHandle>,
    key: Option<([u8; 3], [u8; 3], u32)>,
}

impl Icons {
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        name: &'static str,
        labels: &mut Labels,
    ) -> Option<egui::TextureId> {
        let v = ctx.global_style().visuals.clone();
        let fg = v.text_color();
        let bg = v.panel_fill;
        let px = (ICON_PT as f32 * ctx.pixels_per_point()).round().max(8.0) as u32;
        let key = ([fg.r(), fg.g(), fg.b()], [bg.r(), bg.g(), bg.b()], px);
        if self.key != Some(key) {
            self.cache.clear();
            self.key = Some(key);
        }
        if let Some(t) = self.cache.get(name) {
            return Some(t.id());
        }
        let mut pixels = render(name, px, labels).ok()?;
        tint(&mut pixels, key.0, key.1);
        let img = egui::ColorImage::from_rgba_premultiplied([px as usize, px as usize], &pixels);
        let tex = ctx.load_texture(format!("icon-{name}"), img, egui::TextureOptions::LINEAR);
        let id = tex.id();
        self.cache.insert(name, tex);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_renders() {
        let mut labels = Labels::new();
        for (name, _) in SOURCES {
            let px = render(name, 48, &mut labels).unwrap_or_else(|e| panic!("{name}: {e}"));
            let ink = px.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
            assert!(ink > 48, "{name} is (almost) empty");
        }
    }

    #[test]
    fn tint_keeps_colors() {
        let mut px = vec![0, 0, 0, 255, 255, 255, 255, 255, 53, 132, 228, 255];
        tint(&mut px, [200, 200, 200], [30, 30, 30]);
        assert_eq!(&px[0..3], &[200, 200, 200]);
        assert_eq!(&px[4..7], &[30, 30, 30]);
        assert_eq!(&px[8..11], &[53, 132, 228]);
    }
}

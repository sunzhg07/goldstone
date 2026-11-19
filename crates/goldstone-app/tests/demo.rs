//! Records the demo GIF (docs/images/demo.gif) by driving the editor with
//! simulated mouse and keyboard input and rendering every step.
//! Run with: GOLDSTONE_DEMO=$PWD/docs/images/demo.gif cargo test --release -p goldstone-app --test demo -- --ignored
//! With GOLDSTONE_DEMO_FRAMES=dir it also writes a PNG at the end of every scene.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect};
use egui_kittest::Harness;
use egui_kittest::kittest::{AccessKitNode, Queryable};
use goldstone_app::app::GoldstoneApp;
use image::RgbaImage;

const W: f32 = 1280.0;
const H: f32 = 820.0;
const CAPTION_H: f32 = 52.0;
/// One step of the app, one frame of the GIF.
const TICK_MS: u32 = 50;

const CMD_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};

/// What the caption strip under the window shows.
#[derive(Default, Clone, PartialEq)]
struct Caption {
    text: String,
    keys: String,
}

struct Demo {
    h: Harness<'static, GoldstoneApp>,
    cap_h: Harness<'static>,
    caption: Rc<RefCell<Caption>>,
    cap_cache: Option<(Caption, RgbaImage)>,
    /// Frames shown so long the key hint stays visible.
    keys_left: u32,
    pointer: Pos2,
    gif: GifWriter,
    shots: Option<PathBuf>,
    scene: u32,
}

impl Demo {
    fn new(out: PathBuf) -> Self {
        let mut h = Harness::builder()
            .with_size([W, H])
            .with_theme(egui::Theme::Light)
            .with_step_dt(TICK_MS as f32 / 1000.0)
            .wgpu()
            .build_eframe(|cc| GoldstoneApp::new(cc, None));
        h.run_steps(6);
        let caption = Rc::new(RefCell::new(Caption::default()));
        let c = caption.clone();
        let cap_h = Harness::builder()
            .with_size([W, CAPTION_H])
            .with_theme(egui::Theme::Light)
            .with_render_cursor(false)
            .with_context_setup(|ctx| {
                goldstone_app::i18n::install_fonts(ctx, false);
            })
            .wgpu()
            .build_ui(move |ui| {
                let c = c.borrow();
                let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(W, CAPTION_H));
                let p = ui.ctx().layer_painter(egui::LayerId::background());
                p.rect_filled(rect, 0.0, egui::Color32::from_rgb(32, 36, 44));
                p.text(
                    rect.left_center() + egui::vec2(24.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &c.text,
                    egui::FontId::proportional(22.0),
                    egui::Color32::WHITE,
                );
                if !c.keys.is_empty() {
                    let font = egui::FontId::monospace(20.0);
                    let galley =
                        p.layout_no_wrap(c.keys.clone(), font, egui::Color32::from_rgb(32, 36, 44));
                    let size = galley.size() + egui::vec2(20.0, 10.0);
                    let r = Rect::from_min_size(
                        rect.right_center() - egui::vec2(24.0 + size.x, size.y / 2.0),
                        size,
                    );
                    p.rect_filled(r, 6.0, egui::Color32::from_rgb(250, 210, 90));
                    p.galley(r.min + egui::vec2(10.0, 5.0), galley, egui::Color32::BLACK);
                }
            });
        Demo {
            h,
            cap_h,
            caption,
            cap_cache: None,
            keys_left: 0,
            pointer: Pos2::new(W / 2.0, H / 2.0),
            gif: GifWriter::new(out, W as u16, (H + CAPTION_H) as u16),
            shots: std::env::var("GOLDSTONE_DEMO_FRAMES")
                .ok()
                .map(PathBuf::from),
            scene: 0,
        }
    }

    /// Steps the app once and adds the rendered window, with the caption, to the GIF.
    fn tick(&mut self) {
        self.h.step();
        if self.keys_left > 0 {
            self.keys_left -= 1;
            if self.keys_left == 0 {
                self.caption.borrow_mut().keys.clear();
            }
        }
        let img = self.compose();
        self.gif.push(&img, TICK_MS);
    }

    fn compose(&mut self) -> RgbaImage {
        let mut app = self.h.render().expect("render");
        // Some areas (the rulers) render translucent: flatten onto white, as an image viewer would.
        for p in app.pixels_mut() {
            let a = p.0[3] as u32;
            if a < 255 {
                for c in &mut p.0[..3] {
                    *c = ((*c as u32 * a + 255 * (255 - a)) / 255) as u8;
                }
                p.0[3] = 255;
            }
        }
        let want = self.caption.borrow().clone();
        if self.cap_cache.as_ref().is_none_or(|(c, _)| *c != want) {
            self.cap_h.run_steps(2);
            let img = self.cap_h.render().expect("render caption");
            self.cap_cache = Some((want, img));
        }
        let cap = &self.cap_cache.as_ref().unwrap().1;
        let mut out = RgbaImage::new(app.width(), app.height() + cap.height());
        image::imageops::replace(&mut out, &app, 0, 0);
        image::imageops::replace(&mut out, cap, 0, app.height() as i64);
        out
    }

    fn wait(&mut self, ms: u32) {
        for _ in 0..ms.div_ceil(TICK_MS) {
            self.tick();
        }
    }

    fn caption(&mut self, text: &str) {
        self.caption.borrow_mut().text = text.to_string();
    }

    /// Ends a scene: optionally saves its last frame for checking.
    fn shot(&mut self, name: &str) {
        self.scene += 1;
        if let Some(dir) = self.shots.clone() {
            let img = self.compose();
            img.save(dir.join(format!("{:02}-{name}.png", self.scene)))
                .expect("save");
        }
    }

    fn show_keys(&mut self, keys: &str) {
        self.caption.borrow_mut().keys = keys.to_string();
        self.keys_left = 24;
    }

    // ---- pointer --------------------------------------------------------------

    /// Moves the pointer to `to` along an eased path (about 30 px per frame).
    fn move_to(&mut self, to: Pos2) {
        let from = self.pointer;
        let dist = from.distance(to);
        let n = ((dist / 30.0).ceil() as usize).clamp(3, 18);
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let e = t * t * (3.0 - 2.0 * t);
            let p = from + (to - from) * e;
            self.h.event(Event::PointerMoved(p));
            self.tick();
        }
        self.pointer = to;
    }

    fn button(&mut self, pressed: bool) {
        self.h.event(Event::PointerButton {
            pos: self.pointer,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }

    fn click_at(&mut self, p: Pos2) {
        self.move_to(p);
        self.wait(150);
        self.button(true);
        self.tick();
        self.button(false);
        self.tick();
        self.wait(100);
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.move_to(from);
        self.wait(150);
        self.button(true);
        self.tick();
        let n = 16;
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let e = t * t * (3.0 - 2.0 * t);
            let p = from + (to - from) * e;
            self.h.event(Event::PointerMoved(p));
            self.tick();
        }
        self.pointer = to;
        self.button(false);
        self.tick();
        self.wait(150);
    }

    /// The canvas point (x, y) on the screen.
    fn at(&self, x: f64, y: f64) -> Pos2 {
        self.h.state().screen_pos(x, y)
    }

    fn rect_of(&self, f: impl Fn(&AccessKitNode<'_>) -> bool + 'static) -> Rect {
        self.h
            .get_all_by(f)
            .last()
            .unwrap_or_else(|| panic!("node not found"))
            .rect()
    }

    /// Clicks the last widget labelled `label` (exact), or starting with it if `prefix`.
    fn click_label(&mut self, label: &str) {
        let label = label.to_string();
        let r = self.rect_of(move |n| n.label().is_some_and(|l| l == label));
        self.click_at(r.center());
    }

    fn click_prefix(&mut self, prefix: &str) {
        let prefix = prefix.to_string();
        let r = self.rect_of(move |n| n.label().is_some_and(|l| l.starts_with(&prefix)));
        self.click_at(r.center());
    }

    fn click_prefix_hover(&mut self, prefix: &str) {
        let prefix = prefix.to_string();
        let r = self.rect_of(move |n| n.label().is_some_and(|l| l.starts_with(&prefix)));
        self.move_to(r.center());
    }

    fn hover_label(&mut self, label: &str) {
        let label = label.to_string();
        let r = self.rect_of(move |n| n.label().is_some_and(|l| l == label));
        self.move_to(r.center());
        self.wait(250);
    }

    fn click_button(&mut self, label: &str) {
        let label = label.to_string();
        let r = self
            .rect_of(move |n| n.label().is_some_and(|l| l == label) && n.role() == Role::Button);
        self.click_at(r.center());
    }

    /// The end points (canvas) of the last line labelled `label`.
    fn edge_ends(&self, label: &str) -> ([f64; 2], [f64; 2]) {
        let d = self.h.state().document();
        let e = d
            .edges
            .iter()
            .rev()
            .find(|e| e.label.as_deref() == Some(label))
            .unwrap_or_else(|| panic!("no line labelled {label}"));
        let at = |id: &str| d.vertices.iter().find(|v| v.id == id).unwrap().at;
        (at(&e.from), at(&e.to))
    }

    /// The middle of the last line labelled `label`, on the screen.
    fn edge_mid(&self, label: &str) -> Pos2 {
        let (a, b) = self.edge_ends(label);
        self.at((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0)
    }

    /// The `i`-th color swatch of the bottom bar.
    fn swatch(&self, i: usize) -> Pos2 {
        let r = self
            .h
            .get_all_by_label("Color")
            .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
            .unwrap()
            .rect();
        Pos2::new(r.right() + 15.0 + 26.0 * i as f32, r.center().y)
    }

    /// Opens the submenu `sub` of the open menu and clicks `item` in it,
    /// moving across first so the submenu stays open.
    fn submenu(&mut self, sub: &str, item: &str) {
        self.hover_label(sub);
        let item = item.to_string();
        let r = self.rect_of(move |n| n.label().is_some_and(|l| l == item));
        self.move_to(Pos2::new(r.left() + 30.0, self.pointer.y));
        self.click_at(Pos2::new(r.left() + 30.0, r.center().y));
    }

    // ---- keyboard -------------------------------------------------------------

    fn key(&mut self, m: Modifiers, key: Key, shown: &str) {
        self.show_keys(shown);
        self.h.key_press_modifiers(m, key);
        self.tick();
        self.wait(200);
    }

    /// Types `s` one character per frame (spaces get an extra frame).
    fn type_text(&mut self, s: &str) {
        for ch in s.chars() {
            self.h.event(Event::Text(ch.to_string()));
            self.tick();
            if ch == ' ' {
                self.tick();
            }
        }
    }

    fn backspace(&mut self, n: usize) {
        self.show_keys("⌫");
        for _ in 0..n {
            self.h.key_press(Key::Backspace);
            self.tick();
        }
    }

    /// Clicks into the last multi-line text box and selects all of it.
    fn focus_text(&mut self) {
        let r = self.rect_of(|n| n.role() == Role::MultilineTextInput);
        self.click_at(r.center());
    }
}

// ---- GIF ------------------------------------------------------------------------

/// Writes frames as they come; unchanged frames lengthen the previous one, and
/// each frame stores only the rectangle that changed.
struct GifWriter {
    enc: gif::Encoder<std::io::BufWriter<std::fs::File>>,
    prev: Option<RgbaImage>,
    /// The frame waiting for its delay to be known.
    pending: Option<gif::Frame<'static>>,
    pending_ms: u32,
    frames: usize,
}

impl GifWriter {
    fn new(path: PathBuf, w: u16, h: u16) -> Self {
        let file = std::io::BufWriter::new(std::fs::File::create(path).expect("create gif"));
        let mut enc = gif::Encoder::new(file, w, h, &[]).expect("gif");
        enc.set_repeat(gif::Repeat::Infinite).expect("gif");
        GifWriter {
            enc,
            prev: None,
            pending: None,
            pending_ms: 0,
            frames: 0,
        }
    }

    fn push(&mut self, img: &RgbaImage, ms: u32) {
        let (x0, y0, x1, y1) = match &self.prev {
            None => (0, 0, img.width(), img.height()),
            Some(prev) => match changed(prev, img) {
                None => {
                    self.pending_ms += ms;
                    return;
                }
                Some(r) => r,
            },
        };
        let sub = image::imageops::crop_imm(img, x0, y0, x1 - x0, y1 - y0).to_image();
        // Pixels that did not change are transparent: they compress to almost nothing.
        let keep: Vec<bool> = match &self.prev {
            Some(prev) => sub
                .enumerate_pixels()
                .map(|(x, y, p)| prev.get_pixel(x0 + x, y0 + y) != p)
                .collect(),
            None => vec![true; sub.len() / 4],
        };
        let mut f = quantize(&sub, &keep);
        f.left = x0 as u16;
        f.top = y0 as u16;
        f.dispose = gif::DisposalMethod::Keep;
        self.flush();
        self.pending = Some(f);
        self.pending_ms = ms;
        self.prev = Some(img.clone());
    }

    fn flush(&mut self) {
        if let Some(mut f) = self.pending.take() {
            f.delay = (self.pending_ms / 10).max(2) as u16;
            self.enc.write_frame(&f).expect("gif frame");
            self.frames += 1;
        }
    }

    fn finish(mut self, last_ms: u32) -> usize {
        self.pending_ms += last_ms;
        self.flush();
        self.frames
    }
}

/// A GIF frame of `img` with the pixels not in `keep` transparent (index 255).
/// Up to 255 colors are kept exactly; more go through NeuQuant.
fn quantize(img: &RgbaImage, keep: &[bool]) -> gif::Frame<'static> {
    const CLEAR: u8 = 255;
    let pixels: Vec<[u8; 3]> = img.pixels().map(|p| [p.0[0], p.0[1], p.0[2]]).collect();
    let mut exact: std::collections::HashMap<[u8; 3], u8> = std::collections::HashMap::new();
    for (p, k) in pixels.iter().zip(keep) {
        if *k && !exact.contains_key(p) {
            if exact.len() == CLEAR as usize {
                exact.clear();
                break;
            }
            let n = exact.len() as u8;
            exact.insert(*p, n);
        }
    }
    let mut palette = vec![0u8; 256 * 3];
    let buffer: Vec<u8> = if !exact.is_empty() || !keep.iter().any(|k| *k) {
        for (c, i) in &exact {
            palette[*i as usize * 3..*i as usize * 3 + 3].copy_from_slice(c);
        }
        pixels
            .iter()
            .zip(keep)
            .map(|(p, k)| if *k { exact[p] } else { CLEAR })
            .collect()
    } else {
        let sample: Vec<u8> = pixels
            .iter()
            .zip(keep)
            .filter(|(_, k)| **k)
            .flat_map(|(p, _)| [p[0], p[1], p[2], 255])
            .collect();
        let nq = color_quant::NeuQuant::new(10, CLEAR as usize, &sample);
        palette[..CLEAR as usize * 3].copy_from_slice(&nq.color_map_rgb());
        pixels
            .iter()
            .zip(keep)
            .map(|(p, k)| {
                if *k {
                    nq.index_of(&[p[0], p[1], p[2], 255]) as u8
                } else {
                    CLEAR
                }
            })
            .collect()
    };
    gif::Frame {
        width: img.width() as u16,
        height: img.height() as u16,
        buffer: buffer.into(),
        palette: Some(palette),
        transparent: Some(CLEAR),
        ..gif::Frame::default()
    }
}

/// The bounding box of the pixels that differ, as (x0, y0, x1, y1).
fn changed(a: &RgbaImage, b: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = b.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if a.get_pixel(x, y) != b.get_pixel(x, y) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0).then_some((x0, y0, x1, y1))
}

// ---- the demo -------------------------------------------------------------------

#[test]
#[ignore = "renders with wgpu and writes a GIF; run explicitly"]
fn demo_gif() {
    let Ok(out) = std::env::var("GOLDSTONE_DEMO") else {
        return;
    };
    goldstone_app::i18n::set_lang(goldstone_app::i18n::Lang::En);
    let mut d = Demo::new(PathBuf::from(out));

    // 1. The empty editor.
    d.caption("Goldstone: many-body diagrams from LaTeX");
    d.wait(1500);
    d.shot("start");

    // 2. IMSRG: an equation becomes diagrams.
    d.caption("IMSRG: Insert ▸ IMSRG Diagram from LaTeX…");
    d.click_label("Insert");
    d.wait(300);
    d.click_prefix("IMSRG Diagram from LaTeX…");
    d.wait(400);
    d.caption("Type the flow equation");
    d.focus_text();
    d.key(Modifiers::COMMAND, Key::A, "⌘A");
    d.type_text(r"\frac{dE}{ds} = \sum_{ab} (n_a - n_b) \eta_{ab} f_{ba}");
    d.wait(1200);
    d.shot("srg-typed");
    d.caption("Occupation factors decide particle or hole: one diagram each");
    d.click_button("Insert");
    d.h.run_steps(4);
    d.key(Modifiers::NONE, Key::Num4, "4  Zoom Drawing");
    d.wait(2000);
    d.shot("srg-inserted");

    // 3. Edit the equation in the LaTeX tab.
    d.caption("Edit the equation in the LaTeX tab: the drawing follows");
    d.focus_text();
    d.key(Modifiers::COMMAND, Key::End, "⌘ End");
    d.type_text(r" + \frac{1}{4} \sum_{abcd} \eta_{abcd} \Gamma_{cdab} n_a n_b \bar n_c \bar n_d");
    d.wait(600);
    d.key(Modifiers::NONE, Key::Escape, "Esc");
    d.key(Modifiers::NONE, Key::Num4, "4  Zoom Drawing");
    let p = d.edge_mid("$\\Gamma$");
    d.click_at(p);
    d.wait(2200);
    d.shot("srg-edited");

    // 4. CC: a template.
    d.caption("Coupled cluster: start from a template…");
    d.key(Modifiers::COMMAND, Key::N, "⌘N  New tab");
    d.click_label("Templates");
    d.wait(200);
    d.submenu("Many-Body Diagrams", "CC energy");
    d.wait(300);
    d.key(Modifiers::NONE, Key::Num4, "4  Zoom Drawing");
    d.wait(1800);
    d.shot("cc-template");

    // 5. CC: a term typed, with a mistake on the way.
    d.caption("…or type any CC term: a–g particles, h–n holes");
    d.key(CMD_SHIFT, Key::T, "⌘⇧T  CC Diagram from LaTeX");
    d.focus_text();
    d.key(Modifiers::COMMAND, Key::A, "⌘A");
    d.type_text(r"Z_{abij} = \frac{1}{2} V_abef T_{efij}");
    d.caption("Strict rules: anything unclear is rejected, with a hint");
    d.wait(2600);
    d.shot("cc-error");
    d.backspace("abef T_{efij}".len());
    d.caption("Fix it and insert");
    d.type_text("{abef} T_{efij}");
    d.wait(1000);
    d.click_button("Insert");
    d.h.run_steps(2);
    d.key(Modifiers::NONE, Key::Num3, "3  Zoom Selection");
    d.wait(1800);
    d.shot("cc-inserted");

    // 6. Hand edits: ungroup, bend a line, restyle the interaction.
    d.caption("Every line is an ordinary object: ungroup, then edit");
    let p = d.edge_mid("$V$");
    d.click_at(p);
    d.key(CMD_SHIFT, Key::G, "⌘⇧G  Ungroup");
    let p = d.edge_mid("$e$");
    d.click_at(p);
    d.key(Modifiers::NONE, Key::N, "N  Edit tool");
    // The bend handle sits at a quarter of a line with its arrow in the middle.
    let (a, b) = d.edge_ends("$e$");
    let q = [a[0] + (b[0] - a[0]) / 4.0, a[1] + (b[1] - a[1]) / 4.0];
    d.caption("Bend a line into an arc");
    let (from, to) = (d.at(q[0], q[1]), d.at(q[0] + 12.0, q[1]));
    d.drag(from, to);
    d.wait(600);
    d.caption("Restyle: wavy, colored, double, …");
    d.key(Modifiers::NONE, Key::S, "S  Select tool");
    let p = d.edge_mid("$V$");
    d.click_at(p);
    d.click_label("wavy");
    let p = d.swatch(7);
    d.click_at(p);
    d.wait(1500);
    d.shot("restyled");

    // 7. Drawing by hand.
    d.caption("Or draw by hand: lines, arcs, crosses ---X");
    d.key(Modifiers::NONE, Key::X, "X  Cross tool");
    let (_, v) = d.edge_ends("$T$");
    let (from, to) = (d.at(v[0], v[1]), d.at(v[0] + 30.0, v[1] - 12.0));
    d.drag(from, to);
    d.wait(1200);
    d.shot("cross");

    // 8. Undo and redo.
    d.caption("Undo and redo, per tab");
    d.key(Modifiers::NONE, Key::S, "S  Select tool");
    for _ in 0..2 {
        d.click_button("Undo");
        d.wait(500);
    }
    for _ in 0..2 {
        d.click_button("Redo");
        d.wait(500);
    }
    d.wait(800);
    d.shot("undo-redo");

    // 9. A function plot.
    d.caption("Function plots, with axes and LaTeX labels");
    d.key(Modifiers::COMMAND, Key::N, "⌘N  New tab");
    d.click_label("Insert");
    d.wait(200);
    d.click_prefix("Function Plot…");
    d.wait(1200);
    d.click_button("Insert");
    d.h.run_steps(2);
    d.key(Modifiers::NONE, Key::Num4, "4  Zoom Drawing");
    d.wait(2000);
    d.shot("plot");

    // 10. Export.
    d.caption("Export to PDF, SVG, PNG or TikZ for papers and talks");
    d.click_label("File");
    d.wait(200);
    d.click_prefix_hover("Export…");
    d.wait(2200);
    d.shot("export");
    d.click_label("File");
    d.caption("Goldstone · github.com/sunzhg07/goldstone");
    d.click_label("*Untitled");
    d.key(Modifiers::NONE, Key::Escape, "Esc");
    d.wait(2500);
    d.shot("end");

    let frames = d.gif.finish(1500);
    eprintln!("{frames} GIF frames");
}

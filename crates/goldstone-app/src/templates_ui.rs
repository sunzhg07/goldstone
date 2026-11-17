//! Templates: the built-in shape and diagram library, the user's own
//! templates (`.gold` files in the settings folder), the Templates menu and
//! the Browse Templates window.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use egui::{RichText, TextEdit, Ui};
use goldstone_core::kurbo::{Rect, Vec2};
use goldstone_core::model::Document;
use goldstone_core::templates::{self, CATEGORIES, Template};

use crate::app::GoldstoneApp;
use crate::doc_ops;

/// Category of templates saved without one.
pub const MINE: &str = "My Templates";
const THUMB_PT: f32 = 56.0;

/// The built-in templates (built once).
pub fn builtin() -> &'static [Template] {
    static B: OnceLock<Vec<Template>> = OnceLock::new();
    B.get_or_init(templates::builtin)
}

/// The folder of the user's templates: `templates/` in the settings folder
/// (macOS: ~/Library/Application Support/Goldstone, Windows:
/// %APPDATA%\Goldstone, else $XDG_CONFIG_HOME or ~/.config/goldstone).
pub fn user_dir() -> Option<PathBuf> {
    let base = if cfg!(target_os = "macos") {
        PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/Goldstone")
    } else if cfg!(windows) {
        PathBuf::from(std::env::var_os("APPDATA")?).join("Goldstone")
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?
            .join("goldstone")
    };
    Some(base.join("templates"))
}

/// A template of the user: `<dir>/<category>/<name>.gold`, or
/// `<dir>/<name>.gold` for "My Templates".
#[derive(Clone)]
pub struct UserTemplate {
    pub category: String,
    pub name: String,
    pub doc: Document,
    pub path: PathBuf,
}

fn read_user(dir: &Path) -> Vec<UserTemplate> {
    let mut out = Vec::new();
    let mut scan = |d: &Path, category: &str| {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut files: Vec<PathBuf> = rd.filter_map(|e| Some(e.ok()?.path())).collect();
        files.sort();
        for p in files {
            if p.extension().is_some_and(|e| e == "gold") {
                let Ok(text) = std::fs::read_to_string(&p) else {
                    continue;
                };
                let Ok(doc) = goldstone_core::io::parse(&text) else {
                    continue;
                };
                out.push(UserTemplate {
                    category: category.to_string(),
                    name: p
                        .file_stem()
                        .map_or(String::new(), |s| s.to_string_lossy().into_owned()),
                    doc,
                    path: p,
                });
            }
        }
    };
    scan(dir, MINE);
    if let Ok(rd) = std::fs::read_dir(dir) {
        let mut subs: Vec<PathBuf> = rd
            .filter_map(|e| Some(e.ok()?.path()))
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        for s in subs {
            let cat = s
                .file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned());
            scan(&s, &cat);
        }
    }
    out
}

/// A file name without characters that are not allowed on some systems.
fn file_safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Browse Templates window and Add to Templates dialog.
#[derive(Default)]
pub struct TemplatesState {
    pub browse: bool,
    query: String,
    /// `None`: all categories.
    category: Option<String>,
    user: Option<Vec<UserTemplate>>,
    thumbs: HashMap<String, egui::TextureHandle>,
    /// Add to Templates: (name, category) being typed.
    pub add: Option<(String, String)>,
}

/// Renders a template to a square thumbnail.
fn thumbnail(
    doc: &Document,
    labels: &mut goldstone_export::Labels,
    px: u32,
) -> Option<egui::ColorImage> {
    let scene = goldstone_core::scene::build(doc).ok()?;
    let mut b = scene.path_bounds()?;
    for (_, q) in goldstone_export::svg::text_boxes(&scene, labels) {
        for c in q {
            b = b.union(Rect::from_points(c, c));
        }
    }
    let side = b.width().max(b.height()) * 1.08 + 2.0;
    let c = b.center();
    let view = Rect::from_center_size(c, (side, side));
    let (pixels, _) = goldstone_export::rasterize_view(&scene, labels, view, px, px);
    Some(egui::ColorImage::from_rgba_premultiplied(
        [px as usize, px as usize],
        &pixels,
    ))
}

impl GoldstoneApp {
    fn user_templates(&mut self) -> &[UserTemplate] {
        if self.templates.user.is_none() {
            self.templates.user = Some(user_dir().map(|d| read_user(&d)).unwrap_or_default());
        }
        self.templates.user.as_deref().unwrap_or_default()
    }

    /// Adds a template's objects at the center of the view, selected.
    pub(crate) fn insert_template(&mut self, doc: Document, name: &str) {
        let center = self.view_center();
        let new = doc_ops::merge(&mut self.doc, doc);
        if let Some(b) = doc_ops::bounds(&doc_ops::selection_points(&self.doc, &new)) {
            let g = self.grid();
            let d = center - b.center();
            let d = Vec2::new((d.x / g).round() * g, (d.y / g).round() * g);
            doc_ops::translate(&mut self.doc, &new, d);
        }
        self.selection = new;
        self.message = Some(format!("Inserted template {name}"));
    }

    /// The Templates menu: browse, add, and every category.
    pub(crate) fn templates_menu(&mut self, ui: &mut Ui) {
        if ui.button("Browse Templates…").clicked() {
            self.templates.browse = true;
            ui.close();
        }
        if ui
            .add_enabled(
                !self.selection.is_empty(),
                egui::Button::new("Add Selection to Templates…"),
            )
            .clicked()
        {
            self.start_add_template();
            ui.close();
        }
        if ui.button("Open Templates Folder").clicked() {
            self.open_templates_folder();
            ui.close();
        }
        ui.separator();
        let mut chosen: Option<(Document, String)> = None;
        for cat in CATEGORIES {
            ui.menu_button(*cat, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(500.0)
                    .show(ui, |ui| {
                        for t in builtin().iter().filter(|t| t.category == *cat) {
                            if ui.button(&t.name).clicked() {
                                chosen = Some((t.doc.clone(), t.name.clone()));
                                ui.close();
                            }
                        }
                    });
            });
        }
        let user: Vec<UserTemplate> = self.user_templates().to_vec();
        let mut cats: Vec<String> = user.iter().map(|t| t.category.clone()).collect();
        cats.dedup();
        if !cats.is_empty() {
            ui.separator();
        }
        for cat in cats {
            ui.menu_button(&cat, |ui| {
                for t in user.iter().filter(|t| t.category == cat) {
                    if ui.button(&t.name).clicked() {
                        chosen = Some((t.doc.clone(), t.name.clone()));
                        ui.close();
                    }
                }
            });
        }
        if let Some((doc, name)) = chosen {
            self.insert_template(doc, &name);
        }
    }

    pub(crate) fn start_add_template(&mut self) {
        if self.selection.is_empty() {
            self.message = Some("Select the objects to keep as a template first".into());
            return;
        }
        self.templates.add = Some((String::new(), MINE.into()));
    }

    fn open_templates_folder(&mut self) {
        let Some(dir) = user_dir() else {
            self.message = Some("No settings folder on this system".into());
            return;
        };
        let _ = std::fs::create_dir_all(&dir);
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else if cfg!(windows) {
            "explorer"
        } else {
            "xdg-open"
        };
        match std::process::Command::new(opener).arg(&dir).spawn() {
            Ok(_) => self.message = Some(format!("Templates folder: {}", dir.display())),
            Err(e) => self.message = Some(format!("{}: {e}", dir.display())),
        }
    }

    fn save_template(&mut self, name: &str, category: &str) -> Result<PathBuf, String> {
        let dir = user_dir().ok_or("no settings folder on this system")?;
        let name = file_safe(name);
        if name.is_empty() {
            return Err("give the template a name".into());
        }
        let cat = file_safe(category);
        let dir = if cat.is_empty() || cat == MINE {
            dir
        } else {
            dir.join(cat)
        };
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = dir.join(format!("{name}.gold"));
        let part = doc_ops::extract(&self.doc, &self.selection);
        let text = format!(
            "# Goldstone template\n{}",
            goldstone_core::io::to_toml(&part)
        );
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.templates.user = None; // re-read
        Ok(path)
    }

    /// Add to Templates dialog.
    pub(crate) fn add_template_window(&mut self, ctx: &egui::Context) {
        let Some((mut name, mut cat)) = self.templates.add.clone() else {
            return;
        };
        let cats: Vec<String> = {
            let mut c: Vec<String> = self
                .user_templates()
                .iter()
                .map(|t| t.category.clone())
                .collect();
            c.push(MINE.into());
            c.sort();
            c.dedup();
            c
        };
        let mut open = true;
        let mut done = false;
        egui::Window::new("Add to Templates")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "Keep the {} selected object(s) as a template.",
                    self.selection.len()
                ));
                egui::Grid::new("add-template")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Name");
                        let r = ui.add(TextEdit::singleline(&mut name).hint_text("e.g. my ladder"));
                        r.request_focus();
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            done = true;
                        }
                        ui.end_row();
                        ui.label("Category");
                        ui.horizontal(|ui| {
                            ui.add(TextEdit::singleline(&mut cat).desired_width(140.0));
                            egui::ComboBox::from_id_salt("tpl-cat")
                                .selected_text("")
                                .width(20.0)
                                .show_ui(ui, |ui| {
                                    for c in &cats {
                                        ui.selectable_value(&mut cat, c.clone(), c);
                                    }
                                });
                        });
                        ui.end_row();
                    });
                if let Some(dir) = user_dir() {
                    ui.weak(format!("Saved as a .gold file in {}", dir.display()));
                }
                if ui.button("Save").clicked() {
                    done = true;
                }
            });
        if done {
            match self.save_template(&name, &cat) {
                Ok(p) => {
                    self.message = Some(format!("Saved template {}", p.display()));
                    self.templates.add = None;
                    return;
                }
                Err(e) => self.message = Some(e),
            }
        }
        self.templates.add = open.then_some((name, cat));
    }

    /// Browse Templates window: search, categories, thumbnails.
    pub(crate) fn templates_window(&mut self, ctx: &egui::Context) {
        if !self.templates.browse {
            return;
        }
        let user: Vec<UserTemplate> = self.user_templates().to_vec();
        // (key, category, name, doc, user file)
        let mut all: Vec<(String, String, String, &Document, Option<PathBuf>)> = builtin()
            .iter()
            .map(|t| {
                (
                    format!("b/{}/{}", t.category, t.name),
                    t.category.to_string(),
                    t.name.clone(),
                    &t.doc,
                    None,
                )
            })
            .collect();
        all.extend(user.iter().map(|t| {
            (
                format!("u/{}", t.path.display()),
                t.category.clone(),
                t.name.clone(),
                &t.doc,
                Some(t.path.clone()),
            )
        }));
        let mut cats: Vec<String> = CATEGORIES.iter().map(|c| c.to_string()).collect();
        for t in &user {
            if !cats.contains(&t.category) {
                cats.push(t.category.clone());
            }
        }
        let mut open = true;
        let mut insert: Option<(Document, String, bool)> = None;
        let mut delete: Option<PathBuf> = None;
        let px = (THUMB_PT * ctx.pixels_per_point()).round() as u32;
        let mut rendered = 0;
        egui::Window::new("Templates")
            .open(&mut open)
            .default_size([640.0, 520.0])
            .max_width(900.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.add(
                        TextEdit::singleline(&mut self.templates.query)
                            .hint_text("star, arrow, ladder, QED…")
                            .desired_width(220.0),
                    );
                    if ui.button("Add Selection…").clicked() {
                        self.start_add_template();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .selectable_label(self.templates.category.is_none(), "All")
                        .clicked()
                    {
                        self.templates.category = None;
                    }
                    for c in &cats {
                        let on = self.templates.category.as_deref() == Some(c.as_str());
                        if ui.selectable_label(on, c).clicked() {
                            self.templates.category = Some(c.clone());
                        }
                    }
                });
                ui.separator();
                {
                    let q = self.templates.query.to_lowercase();
                    let shown: Vec<&(String, String, String, &Document, Option<PathBuf>)> = all
                        .iter()
                        .filter(|t| self.templates.category.as_ref().is_none_or(|c| *c == t.1))
                        .filter(|t| {
                            q.is_empty()
                                || t.2.to_lowercase().contains(&q)
                                || t.1.to_lowercase().contains(&q)
                        })
                        .collect();
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if shown.is_empty() {
                                ui.weak("No template matches.");
                            }
                            for row in shown.chunks(7) {
                                ui.horizontal_top(|ui| {
                                    for (key, cat, name, doc, path) in row.iter().copied() {
                                        if !self.templates.thumbs.contains_key(key) && rendered < 12
                                        {
                                            rendered += 1;
                                            if let Some(img) = thumbnail(doc, &mut self.labels, px)
                                            {
                                                let tex = ctx.load_texture(
                                                    key,
                                                    img,
                                                    egui::TextureOptions::LINEAR,
                                                );
                                                self.templates.thumbs.insert(key.clone(), tex);
                                            }
                                        }
                                        ui.vertical(|ui| {
                                            ui.set_width(THUMB_PT + 24.0);
                                            let r = match self.templates.thumbs.get(key) {
                                                Some(t) => ui.add(egui::Button::image(
                                                    egui::Image::new((
                                                        t.id(),
                                                        egui::vec2(THUMB_PT, THUMB_PT),
                                                    ))
                                                    .bg_fill(egui::Color32::WHITE)
                                                    .corner_radius(3.0),
                                                )),
                                                None => ui.add_sized(
                                                    [THUMB_PT + 8.0, THUMB_PT + 8.0],
                                                    egui::Button::new("…"),
                                                ),
                                            };
                                            let r = r.on_hover_text(format!(
                                                "{name}\n{cat}\nClick to insert"
                                            ));
                                            if r.clicked() {
                                                insert =
                                                    Some(((*doc).clone(), name.clone(), false));
                                            }
                                            r.context_menu(|ui| {
                                                if ui.button("Insert").clicked() {
                                                    insert =
                                                        Some(((*doc).clone(), name.clone(), false));
                                                    ui.close();
                                                }
                                                if ui.button("Insert in New Tab").clicked() {
                                                    insert =
                                                        Some(((*doc).clone(), name.clone(), true));
                                                    ui.close();
                                                }
                                                if let Some(p) = path
                                                    && ui.button("Delete Template").clicked()
                                                {
                                                    delete = Some(p.clone());
                                                    ui.close();
                                                }
                                            });
                                            ui.add(
                                                egui::Label::new(RichText::new(name).small())
                                                    .wrap(),
                                            );
                                        });
                                    }
                                });
                            }
                        });
                }
            });
        if rendered > 0 {
            ctx.request_repaint();
        }
        if let Some((doc, name, new_tab)) = insert {
            if new_tab {
                self.insert_in_new_tab(doc);
                self.message = Some(format!("Inserted template {name}"));
            } else {
                self.insert_template(doc, &name);
            }
        }
        if let Some(p) = delete {
            match std::fs::remove_file(&p) {
                Ok(()) => self.message = Some(format!("Deleted template {}", p.display())),
                Err(e) => self.message = Some(format!("{}: {e}", p.display())),
            }
            self.templates.user = None;
            self.templates.thumbs.retain(|k, _| !k.starts_with("u/"));
        }
        self.templates.browse = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_templates_round_trip() {
        let dir = std::env::temp_dir().join(format!("goldstone-tpl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Ladders")).unwrap();
        let doc = &builtin()
            .iter()
            .find(|t| t.name == "5-Point Star")
            .unwrap()
            .doc;
        let text = goldstone_core::io::to_toml(doc);
        std::fs::write(dir.join("star.gold"), &text).unwrap();
        std::fs::write(dir.join("Ladders/pp.gold"), &text).unwrap();
        std::fs::write(dir.join("broken.gold"), "not toml [").unwrap();
        let u = read_user(&dir);
        let names: Vec<(&str, &str)> = u
            .iter()
            .map(|t| (t.category.as_str(), t.name.as_str()))
            .collect();
        assert_eq!(names, vec![(MINE, "star"), ("Ladders", "pp")]);
        assert_eq!(file_safe(" a/b:c "), "a_b_c");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

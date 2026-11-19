//! Menus, commands bar, toolbox, tool controls, docked dialogs, palettes and
//! status bar.

use crate::i18n::t;
use std::ops::RangeInclusive;

use egui::{Color32, ComboBox, DragValue, Grid, RichText, ScrollArea, TextEdit, Ui};
use goldstone_core::color::{Color, NAMED};
use goldstone_core::kurbo::Vec2;
use goldstone_core::model::*;
use goldstone_core::scene::{ObjRef, defaults as d};
use goldstone_core::shapes::BoolOp;
use goldstone_core::{inspect, io, scene};

use crate::app::{ACCENT, GoldstoneApp, Tab, Tool};
use crate::doc_ops;
use crate::keys::{self, Action, AlignOp, BINDINGS, MOUSE, Same};

pub(crate) const STROKES: [(Stroke, &str); 6] = [
    (Stroke::Plain, "plain"),
    (Stroke::Dashed, "dashed"),
    (Stroke::Dotted, "dotted"),
    (Stroke::Wavy, "wavy"),
    (Stroke::Curly, "curly"),
    (Stroke::Zigzag, "zigzag"),
];
pub(crate) const ARROWS: [(ArrowDir, &str); 3] = [
    (ArrowDir::None, "none"),
    (ArrowDir::Forward, "forward"),
    (ArrowDir::Backward, "backward"),
];
pub(crate) const KINDS: [(VertexKind, &str); 6] = [
    (VertexKind::None, "none (invisible)"),
    (VertexKind::Dot, "dot"),
    (VertexKind::Cross, "cross ✕"),
    (VertexKind::Ocross, "circled cross ⊗"),
    (VertexKind::Circle, "circle"),
    (VertexKind::Square, "square"),
];
const MARKS: [(u32, &str); 4] = [(0, "none"), (1, "1 bar"), (2, "2 bars"), (3, "3 bars")];
const SIDES: [(Side, &str); 2] = [(Side::Left, "left"), (Side::Right, "right")];
const ANCHORS: [(Anchor, &str); 9] = [
    (Anchor::Center, "center"),
    (Anchor::North, "north"),
    (Anchor::South, "south"),
    (Anchor::East, "east"),
    (Anchor::West, "west"),
    (Anchor::NorthEast, "north-east"),
    (Anchor::NorthWest, "north-west"),
    (Anchor::SouthEast, "south-east"),
    (Anchor::SouthWest, "south-west"),
];

/// Buttons of the commands bar: (icon, label, action). `None` is a separator.
const COMMANDS: &[Option<(&str, &str, Action)>] = &[
    Some(("undo", "Undo", Action::Undo)),
    Some(("redo", "Redo", Action::Redo)),
    None,
    Some(("group", "Group", Action::Group)),
    Some(("ungroup", "Ungroup", Action::Ungroup)),
    None,
    Some(("union", "Union", Action::Bool(BoolOp::Union))),
    Some(("difference", "Difference", Action::Bool(BoolOp::Difference))),
    Some((
        "intersection",
        "Intersection",
        Action::Bool(BoolOp::Intersection),
    )),
    Some(("exclusion", "Exclusion", Action::Bool(BoolOp::Exclusion))),
    Some(("to-path", "Object to Path", Action::ObjectToPath)),
    None,
    Some(("flip-h", "Flip Horizontal", Action::FlipH)),
    Some(("flip-v", "Flip Vertical", Action::FlipV)),
    Some((
        "rotate-ccw",
        "Rotate 90° Counterclockwise",
        Action::Rotate(90),
    )),
    Some(("rotate-cw", "Rotate 90° Clockwise", Action::Rotate(-90))),
    None,
    Some(("align-left", "Align & Distribute", Action::ShowAlign)),
];

pub(crate) const ALIGN: &[(&str, &str, AlignOp)] = &[
    ("align-left", "Align left edges", AlignOp::Left),
    (
        "align-center-x",
        "Center on vertical axis",
        AlignOp::CenterX,
    ),
    ("align-right", "Align right edges", AlignOp::Right),
    ("align-top", "Align top edges", AlignOp::Top),
    (
        "align-center-y",
        "Center on horizontal axis",
        AlignOp::CenterY,
    ),
    ("align-bottom", "Align bottom edges", AlignOp::Bottom),
    (
        "distribute-x",
        "Distribute centers horizontally at equal distances",
        AlignOp::DistributeX,
    ),
    (
        "distribute-y",
        "Distribute centers vertically at equal distances",
        AlignOp::DistributeY,
    ),
];

fn to_c32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

// ---- small widgets for optional (inheritable) fields --------------------------
//
// `mixed`: the selected objects disagree on this field. The widget then says
// so, and a change applies the new value to all of them.

fn reset(ui: &mut Ui, set: bool) -> bool {
    if set {
        ui.small_button(t("↺"))
            .on_hover_text(t("Reset to the class / default value"))
            .clicked()
    } else {
        ui.weak(t("·"))
            .on_hover_text(t("Inherited from the class or the default"));
        false
    }
}

fn trim(n: f64) -> String {
    format!("{n:.2}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn num(
    ui: &mut Ui,
    label: &str,
    v: &mut Option<f64>,
    inherited: f64,
    speed: f64,
    range: RangeInclusive<f64>,
    mixed: bool,
) {
    let l = ui.label(t(label)).id;
    ui.horizontal(|ui| {
        let mut x = v.unwrap_or(inherited);
        let mut dv = DragValue::new(&mut x)
            .speed(speed)
            .range(range)
            .max_decimals(2);
        if mixed {
            dv = dv.custom_formatter(|_, _| t("mixed").into());
        } else if v.is_none() {
            dv = dv.custom_formatter(|n, _| trim(n));
        }
        let r = ui.add(dv).labelled_by(l);
        if r.changed() {
            *v = Some(doc_ops::round(x));
        }
        if reset(ui, v.is_some() || mixed) {
            *v = None;
        }
    });
    ui.end_row();
}

fn choice<T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    v: &mut Option<T>,
    inherited: T,
    options: &[(T, &str)],
    mixed: bool,
) {
    ui.label(t(label));
    ui.horizontal(|ui| {
        let cur = v.unwrap_or(inherited);
        let name = options
            .iter()
            .find(|(o, _)| *o == cur)
            .map_or("?", |(_, n)| *n);
        let text = if mixed {
            RichText::new(t("mixed")).italics()
        } else if v.is_some() {
            RichText::new(t(name))
        } else {
            RichText::new(t(name)).weak()
        };
        ComboBox::from_id_salt(label)
            .selected_text(text)
            .show_ui(ui, |ui| {
                for (o, n) in options {
                    if ui.selectable_label(!mixed && cur == *o, t(n)).clicked() {
                        *v = Some(*o);
                    }
                }
            });
        if reset(ui, v.is_some() || mixed) {
            *v = None;
        }
    });
    ui.end_row();
}

fn flag(ui: &mut Ui, label: &str, v: &mut Option<bool>, inherited: bool, mixed: bool) {
    let l = ui.label(t(label)).id;
    ui.horizontal(|ui| {
        let mut x = v.unwrap_or(inherited);
        if ui
            .add(egui::Checkbox::new(&mut x, "").indeterminate(mixed))
            .labelled_by(l)
            .changed()
        {
            *v = Some(x);
        }
        if reset(ui, v.is_some() || mixed) {
            *v = None;
        }
    });
    ui.end_row();
}

fn color(
    ui: &mut Ui,
    label: &str,
    v: &mut Option<String>,
    inherited: Option<Color>,
    allow_none: bool,
    mixed: bool,
) {
    let l = ui.label(t(label)).id;
    ui.horizontal(|ui| {
        let cur = v.as_deref().and_then(|s| Color::parse(s).ok()).or(
            if v.as_deref().is_some_and(|s| s.eq_ignore_ascii_case("none")) {
                None
            } else {
                inherited
            },
        );
        let mut rgb = cur.map_or([255, 255, 255], |c| [c.r, c.g, c.b]);
        if ui.color_edit_button_srgb(&mut rgb).labelled_by(l).changed() {
            *v = Some(Color::rgb(rgb[0], rgb[1], rgb[2]).hex());
        }
        let shown = if mixed {
            t("mixed").to_string()
        } else {
            v.clone()
                .unwrap_or_else(|| inherited.map_or(t("none").into(), |c| c.hex()))
        };
        let text = if v.is_some() && !mixed {
            RichText::new(shown).monospace()
        } else {
            RichText::new(shown).monospace().weak()
        };
        ui.label(text);
        if allow_none && (cur.is_some() || mixed) && ui.small_button(t("none")).clicked() {
            *v = Some("none".into());
        }
        if reset(ui, v.is_some() || mixed) {
            *v = None;
        }
    });
    ui.end_row();
}

fn opt_text(
    ui: &mut Ui,
    label: &str,
    v: &mut Option<String>,
    hint: &str,
    mixed: bool,
) -> egui::Response {
    let l = ui.label(t(label)).id;
    let mut s = if mixed {
        String::new()
    } else {
        v.clone().unwrap_or_default()
    };
    let r = ui
        .add(
            TextEdit::singleline(&mut s)
                .hint_text(if mixed { t("mixed") } else { hint })
                .desired_width(f32::INFINITY),
        )
        .labelled_by(l);
    if r.changed() {
        *v = (!s.is_empty()).then_some(s);
    }
    ui.end_row();
    r
}

fn class_choice(ui: &mut Ui, v: &mut Option<String>, styles: &[String], mixed: bool) {
    ui.label(t("Class"));
    let text = if mixed {
        RichText::new(t("mixed")).italics()
    } else {
        RichText::new(v.clone().unwrap_or_else(|| "(none)".into()))
    };
    ComboBox::from_id_salt("class")
        .selected_text(text)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(!mixed && v.is_none(), t("(none)"))
                .clicked()
            {
                *v = None;
            }
            for s in styles {
                if ui
                    .selectable_label(!mixed && v.as_deref() == Some(s.as_str()), s)
                    .clicked()
                {
                    *v = Some(s.clone());
                }
            }
        });
    ui.end_row();
}

/// Height and radius of an arc with chord length `l`, as alternatives to
/// its bend angle (which is what the file stores).
fn arc_fields(ui: &mut Ui, bend: &mut Option<f64>, l: f64) {
    let b = bend.unwrap_or(0.0);
    let l_h = ui.label(t("Arc height (pt)")).id;
    let mut h = doc_ops::arc_height(l, b);
    let r = ui
        .add(DragValue::new(&mut h).speed(0.2).max_decimals(2))
        .labelled_by(l_h)
        .on_hover_text(
            "How far the arc bulges from the straight line (signed: + is left of from → to)",
        );
    if r.changed() {
        *bend = Some(doc_ops::round(doc_ops::bend_from_height(l, h)));
    }
    ui.end_row();
    let l_r = ui.label(t("Arc radius (pt)")).id;
    ui.horizontal(|ui| match doc_ops::arc_radius(l, b) {
        Some(mut rad) => {
            let resp = ui
                .add(
                    DragValue::new(&mut rad)
                        .speed(0.2)
                        .range(l / 2.0..=1.0e5)
                        .max_decimals(2),
                )
                .labelled_by(l_r)
                .on_hover_text(crate::i18n::tr(&format!(
                    "At least half the chord ({} pt)",
                    trim(l / 2.0)
                )));
            let mut deep = b.abs() > 90.0;
            let deep_resp = ui
                .checkbox(&mut deep, t("deep"))
                .on_hover_text(t("More than half a circle (the same radius fits two arcs)"));
            if resp.changed() || deep_resp.changed() {
                *bend = Some(doc_ops::round(doc_ops::bend_from_radius(
                    l,
                    rad,
                    deep,
                    b.signum(),
                )));
            }
        }
        None => {
            ui.weak(t("straight")).labelled_by(l_r);
        }
    });
    ui.end_row();
}

fn style_color(st: &Style) -> Option<Color> {
    Some(
        st.color
            .as_deref()
            .and_then(|c| Color::parse(c).ok())
            .unwrap_or(Color::BLACK),
    )
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", t(if n == 1 { one } else { many }))
}

/// Indices of the selected objects of one kind.
fn of_kind(sel: &[ObjRef], f: fn(&ObjRef) -> Option<usize>) -> Vec<usize> {
    sel.iter().filter_map(f).collect()
}

impl GoldstoneApp {
    /// An icon image of `size` points.
    pub(crate) fn icon(
        &mut self,
        ctx: &egui::Context,
        name: &'static str,
        size: f32,
    ) -> Option<egui::Image<'static>> {
        let id = self.icons.texture(ctx, name, &mut self.labels)?;
        Some(egui::Image::new((id, egui::vec2(size, size))))
    }

    fn shortcut(&self, a: Action) -> String {
        keys::shortcut(a, self.os)
    }

    pub(crate) fn menu_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut m = crate::menus::Items::new(
            self,
            &ctx,
            &[
                "undo",
                "redo",
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
            ],
        );
        let (show_grid, show_rulers, show_ids) = (self.show_grid, self.show_rulers, self.show_ids);
        let mut about = false;
        let mut language = None;
        egui::MenuBar::new().ui(ui, |ui| {
            let mut item = |ui: &mut Ui, icon: Option<&str>, label: &str, a: Action| {
                m.item(ui, icon, label, a)
            };
            ui.menu_button(t("File"), |ui| {
                item(ui, None, "New", Action::New);
                item(ui, None, "Open…", Action::Open);
                item(ui, None, "Save", Action::Save);
                item(ui, None, "Save As…", Action::SaveAs);
                item(ui, None, "Close", Action::CloseTab);
                ui.separator();
                item(ui, None, "Export…", Action::Export);
                if !crate::platform::WEB {
                    ui.separator();
                    item(ui, None, "Quit", Action::Quit);
                }
            });
            ui.menu_button(t("Insert"), |ui| {
                item(ui, None, "CC Diagram from LaTeX…", Action::FromLatex);
                item(ui, None, "IMSRG Diagram from LaTeX…", Action::FromLatexSrg);
                item(
                    ui,
                    None,
                    "IMSRG Hugenholtz Diagram from LaTeX…",
                    Action::FromLatexSrgHug,
                );
                item(ui, None, "Function Plot…", Action::FunctionPlot);
                item(ui, None, "Templates…", Action::Templates);
            });
            ui.menu_button(t("Templates"), |ui| self.templates_menu(ui));
            ui.menu_button(t("Edit"), |ui| {
                item(ui, Some("undo"), "Undo", Action::Undo);
                item(ui, Some("redo"), "Redo", Action::Redo);
                ui.separator();
                item(ui, None, "Cut", Action::Cut);
                item(ui, None, "Copy", Action::Copy);
                item(ui, None, "Paste", Action::Paste);
                item(ui, None, "Paste In Place", Action::PasteInPlace);
                item(ui, None, "Duplicate", Action::Duplicate);
                item(ui, None, "Delete", Action::Delete);
                ui.separator();
                item(ui, None, "Select All", Action::SelectAll);
                item(ui, None, "Deselect", Action::Deselect);
                ui.menu_button(t("Select Same"), |ui| {
                    item(
                        ui,
                        None,
                        "Fill and Stroke",
                        Action::SelectSame(Same::FillAndStroke),
                    );
                    item(
                        ui,
                        None,
                        "Stroke Color",
                        Action::SelectSame(Same::StrokeColor),
                    );
                    item(
                        ui,
                        None,
                        "Stroke Style",
                        Action::SelectSame(Same::StrokeStyle),
                    );
                    item(ui, None, "Class", Action::SelectSame(Same::Class));
                    item(ui, None, "Object Type", Action::SelectSame(Same::Kind));
                });
            });
            ui.menu_button(t("View"), |ui| {
                item(ui, None, "Zoom In", Action::ZoomIn);
                item(ui, None, "Zoom Out", Action::ZoomOut);
                item(ui, None, "Zoom 1:1", Action::Zoom1);
                item(ui, None, "Zoom Selection", Action::ZoomSelection);
                item(ui, None, "Zoom Drawing", Action::ZoomDrawing);
                ui.separator();
                let check = |on: bool, s: &str| format!("{} {s}", if on { "☑" } else { "☐" });
                item(ui, None, &check(show_grid, "Grid"), Action::ToggleGrid);
                item(
                    ui,
                    None,
                    &check(show_rulers, "Rulers"),
                    Action::ToggleRulers,
                );
                item(
                    ui,
                    None,
                    &check(show_ids, "Object Ids and Coordinates"),
                    Action::ToggleIds,
                );
                ui.separator();
                ui.menu_button(t("Language"), |ui| {
                    for l in crate::i18n::Lang::ALL {
                        if ui
                            .selectable_label(crate::i18n::lang() == l, l.name())
                            .clicked()
                        {
                            language = Some(l);
                            ui.close();
                        }
                    }
                });
            });
            ui.menu_button(t("Object"), |ui| {
                item(ui, Some("group"), "Group", Action::Group);
                item(ui, Some("ungroup"), "Ungroup", Action::Ungroup);
                ui.separator();
                item(
                    ui,
                    Some("rotate-ccw"),
                    "Rotate 90° Counterclockwise",
                    Action::Rotate(90),
                );
                item(
                    ui,
                    Some("rotate-cw"),
                    "Rotate 90° Clockwise",
                    Action::Rotate(-90),
                );
                item(ui, Some("flip-h"), "Flip Horizontal", Action::FlipH);
                item(ui, Some("flip-v"), "Flip Vertical", Action::FlipV);
                ui.separator();
                item(
                    ui,
                    Some("align-left"),
                    "Align & Distribute…",
                    Action::ShowAlign,
                );
            });
            ui.menu_button(t("Path"), |ui| {
                item(ui, Some("to-path"), "Object to Path", Action::ObjectToPath);
                ui.separator();
                item(ui, Some("union"), "Union", Action::Bool(BoolOp::Union));
                item(
                    ui,
                    Some("difference"),
                    "Difference",
                    Action::Bool(BoolOp::Difference),
                );
                item(
                    ui,
                    Some("intersection"),
                    "Intersection",
                    Action::Bool(BoolOp::Intersection),
                );
                item(
                    ui,
                    Some("exclusion"),
                    "Exclusion",
                    Action::Bool(BoolOp::Exclusion),
                );
            });
            ui.menu_button(t("Dialogs"), |ui| {
                item(ui, None, "Fill & Stroke", Action::ShowFillStroke);
                item(ui, None, "Objects", Action::ShowObjects);
                item(ui, None, "Align & Distribute", Action::ShowAlign);
                item(ui, None, "Source", Action::ShowSource);
                item(ui, None, "LaTeX / Plot Source", Action::ShowCode);
            });
            ui.menu_button(t("Help"), |ui| {
                item(ui, None, "Keyboard and Mouse", Action::Shortcuts);
                if !crate::platform::WEB {
                    item(ui, None, "Check for Updates…", Action::CheckUpdates);
                }
                if ui.button(t("About Goldstone")).clicked() {
                    about = true;
                    ui.close();
                }
            });
        });
        if let Some(a) = m.chosen {
            self.do_action(a, &ctx);
        }
        self.show_about |= about;
        if let Some(l) = language {
            self.set_language(&ctx, l);
        }
    }

    /// Inkscape's commands bar: icon buttons for object and path commands.
    pub(crate) fn commands_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut chosen = None;
        ui.horizontal(|ui| {
            for c in COMMANDS {
                let Some((icon, label, action)) = *c else {
                    ui.separator();
                    continue;
                };
                let tip = match self.shortcut(action) {
                    s if s.is_empty() => t(label).to_string(),
                    s => format!("{} ({s})", t(label)),
                };
                let enabled = match action {
                    Action::Undo => self.can_undo(),
                    Action::Redo => self.can_redo(),
                    _ => true,
                };
                let r = match self.icon(&ctx, icon, 22.0) {
                    Some(img) => ui.add_enabled(
                        enabled,
                        egui::Button::image(img).min_size(egui::vec2(28.0, 28.0)),
                    ),
                    None => ui.add_enabled(enabled, egui::Button::new(t(label))),
                };
                // Icon buttons carry their name for screen readers (and tests).
                r.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, t(label)));
                if r.on_hover_text(tip).clicked() {
                    chosen = Some(action);
                }
            }
        });
        if let Some(a) = chosen {
            self.do_action(a, &ctx);
        }
    }

    pub(crate) fn toolbox(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            for t in Tool::ALL {
                let tip = format!("{} ({})\n{}", t.name(), t.key(), self.os.fill(t.hint()));
                let b = match self.icon(&ctx, t.icon(), 26.0) {
                    Some(img) => egui::Button::selectable(self.tool == t, img),
                    None => egui::Button::selectable(self.tool == t, t.name()),
                };
                let r = ui
                    .add(b.min_size(egui::vec2(36.0, 36.0)))
                    .on_hover_text(crate::i18n::tr(&format!("{tip}\nRight-click: options")));
                if r.clicked() {
                    self.set_tool(t);
                }
                r.context_menu(|ui| self.tool_menu(ui, t));
            }
        });
    }

    pub(crate) fn tool_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong(self.tool.name());
            ui.separator();
            match self.tool {
                Tool::Edge | Tool::Arc => {
                    let styles: Vec<String> = self.doc.styles.keys().cloned().collect();
                    ui.label(t("Class"));
                    let text = self.opts.class.clone().unwrap_or_else(|| "(none)".into());
                    ComboBox::from_id_salt("tc-class")
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(self.opts.class.is_none(), t("(none)"))
                                .clicked()
                            {
                                self.opts.class = None;
                            }
                            for s in &styles {
                                if ui
                                    .selectable_label(
                                        self.opts.class.as_deref() == Some(s.as_str()),
                                        s,
                                    )
                                    .clicked()
                                {
                                    self.opts.class = Some(s.clone());
                                }
                            }
                        });
                    ui.label(t("Stroke"));
                    let name = STROKES
                        .iter()
                        .find(|(s, _)| *s == self.opts.stroke)
                        .map_or("", |(_, n)| *n);
                    ComboBox::from_id_salt("tc-stroke")
                        .selected_text(name)
                        .show_ui(ui, |ui| {
                            for (s, n) in STROKES {
                                ui.selectable_value(&mut self.opts.stroke, s, n);
                            }
                        });
                    ui.label(t("Arrow"));
                    let name = match self.opts.arrow {
                        None => "from class",
                        Some(a) => ARROWS.iter().find(|(x, _)| *x == a).map_or("", |(_, n)| *n),
                    };
                    ComboBox::from_id_salt("tc-arrow")
                        .selected_text(name)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.opts.arrow, None, t("from class"));
                            for (a, n) in ARROWS {
                                ui.selectable_value(&mut self.opts.arrow, Some(a), n);
                            }
                        });
                    ui.checkbox(&mut self.opts.double, t("Double"))
                        .on_hover_text(t("Draw new lines as double lines"));
                    if self.opts.double {
                        let l = ui.label(t("gap")).id;
                        let mut g = self.opts.double_sep.unwrap_or(d::DOUBLE_SEP);
                        if ui
                            .add(DragValue::new(&mut g).speed(0.1).range(0.1..=20.0).suffix(" pt"))
                            .labelled_by(l)
                            .changed()
                        {
                            self.opts.double_sep = Some(doc_ops::round(g));
                        }
                    }
                }
                Tool::Vertex => {
                    ui.label(t("Kind"));
                    let name = KINDS
                        .iter()
                        .find(|(k, _)| *k == self.opts.vertex_kind)
                        .map_or("", |(_, n)| *n);
                    ComboBox::from_id_salt("tc-kind")
                        .selected_text(name)
                        .show_ui(ui, |ui| {
                            for (k, n) in KINDS {
                                ui.selectable_value(&mut self.opts.vertex_kind, k, n);
                            }
                        });
                }
                Tool::Select | Tool::Edit => {
                    // Position of the selection (its reference points), in pt.
                    if let Some(b) =
                        doc_ops::bounds(&doc_ops::selection_points(&self.doc, &self.selection))
                    {
                        let (mut x, mut y) = (b.x0, b.y0);
                        let lx = ui.label(t("X")).id;
                        let rx = ui
                            .add(DragValue::new(&mut x).speed(0.5).max_decimals(2))
                            .labelled_by(lx);
                        let ly = ui.label(t("Y")).id;
                        let ry = ui
                            .add(DragValue::new(&mut y).speed(0.5).max_decimals(2))
                            .labelled_by(ly);
                        if rx.changed() || ry.changed() {
                            doc_ops::translate(
                                &mut self.doc,
                                &self.selection,
                                Vec2::new(x - b.x0, y - b.y0),
                            );
                        }
                        ui.label(t("pt")).on_hover_text(
                            "X, Y: lower-left corner of the selection's vertices and anchor points, in pt (y up)",
                        );
                        // W, H: resize about the lower-left corner.
                        let (w0, h0) = (b.width(), b.height());
                        let (mut w, mut h) = (w0, h0);
                        let lw = ui.label(t("W")).id;
                        let rw = ui
                            .add_enabled(
                                w0 > 1e-6,
                                DragValue::new(&mut w).speed(0.5).range(0.1..=1e5).max_decimals(2),
                            )
                            .labelled_by(lw);
                        ui.toggle_value(&mut self.lock_ratio, t("lock"))
                            .on_hover_text(t("W and H change together"));
                        let lh = ui.label(t("H")).id;
                        let rh = ui
                            .add_enabled(
                                h0 > 1e-6,
                                DragValue::new(&mut h).speed(0.5).range(0.1..=1e5).max_decimals(2),
                            )
                            .labelled_by(lh);
                        if rw.changed() || rh.changed() {
                            let mut sx = if w0 > 1e-6 { w / w0 } else { 1.0 };
                            let mut sy = if h0 > 1e-6 { h / h0 } else { 1.0 };
                            if self.lock_ratio {
                                if rw.changed() {
                                    sy = sx;
                                } else {
                                    sx = sy;
                                }
                            }
                            doc_ops::scale(
                                &mut self.doc,
                                &self.selection,
                                b.origin(),
                                sx,
                                sy,
                                self.scale_widths,
                            );
                        }
                        ui.checkbox(&mut self.scale_widths, t("widths & text")).on_hover_text(
                            "Resizing also scales line widths and text sizes",
                        );
                    }
                }
                Tool::Rect | Tool::Ellipse => {
                    ui.weak(self.os.fill("Drag on the canvas; hold {cmd}for a square / circle"));
                }
                _ => {}
            }
            ui.separator();
            ui.checkbox(&mut self.show_grid, t("Grid"));
            let mut g = self.grid();
            let l = ui.label(t("spacing")).id;
            if ui
                .add(DragValue::new(&mut g).range(1.0..=100.0).suffix(" pt"))
                .labelled_by(l)
                .changed()
            {
                self.doc.canvas.grid = Some(g);
            }
            ui.checkbox(&mut self.show_ids, t("Ids"))
                .on_hover_text(t("Show object ids and coordinates on the canvas (I)"));
        });
    }

    pub(crate) fn palettes(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(t("Color"));
            for (name, c) in NAMED {
                let (r, resp) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                ui.painter().rect_filled(r, 2.0, to_c32(*c));
                ui.painter().rect_stroke(
                    r,
                    2.0,
                    egui::Stroke::new(1.0, Color32::from_gray(150)),
                    egui::StrokeKind::Inside,
                );
                let resp = resp.on_hover_text(crate::i18n::tr(&format!(
                    "{name}: click sets the stroke color, {} the fill",
                    self.os.fill("{shift}click")
                )));
                if resp.clicked() {
                    let fill = ui.input(|i| i.modifiers.shift);
                    self.apply_color(name, fill);
                }
            }
            // The last color picked from the screen.
            if let Some(hex) = self.picked.clone()
                && let Ok(c) = Color::parse(&hex)
            {
                let (r, resp) =
                    ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::click());
                ui.painter().rect_filled(r, 2.0, to_c32(c));
                ui.painter().rect_stroke(
                    r,
                    2.0,
                    egui::Stroke::new(1.5, ACCENT),
                    egui::StrokeKind::Inside,
                );
                let resp = resp.on_hover_text(crate::i18n::tr(&format!(
                    "{hex} (picked): click sets the stroke color, {} the fill",
                    self.os.fill("{shift}click")
                )));
                if resp.clicked() {
                    let fill = ui.input(|i| i.modifiers.shift);
                    self.apply_color(&hex, fill);
                }
            }
            let picking = self.picker.active();
            let b = ui
                .add_enabled(!picking, egui::Button::new(t(if picking { "Picking…" } else { "Pick" })))
                .on_hover_text(crate::i18n::tr(&format!(
                    "Pick a color anywhere on the screen. Click: stroke color of the selection, {}: fill",
                    self.os.fill("{shift}click")
                )));
            if b.clicked() {
                let fill = ui.input(|i| i.modifiers.shift);
                self.picker.start(ui.ctx(), fill);
                self.message = Some(if cfg!(windows) {
                    "Click anywhere on the screen to pick its color (Esc cancels)".into()
                } else {
                    "Click anywhere on the screen to pick its color".into()
                });
            }
            if let Some(r) = self.picker.poll() {
                match r {
                    Ok([r, g, b]) => {
                        let hex = format!("#{r:02x}{g:02x}{b:02x}");
                        if self.selection.is_empty() {
                            self.message = Some(format!(
                                "Picked {hex}: select objects and click its swatch to use it"
                            ));
                        } else {
                            self.apply_color(&hex, self.picker.fill);
                            self.message = Some(format!(
                                "Picked {hex} for the {}",
                                if self.picker.fill { "fill" } else { "stroke" }
                            ));
                        }
                        self.picked = Some(hex);
                    }
                    Err(e) => self.message = Some(format!("No color picked: {e}")),
                }
            }
            ui.separator();
            ui.label(t("Stroke"));
            for (s, n) in STROKES {
                if ui
                    .selectable_label(self.opts.stroke == s, t(n))
                    .on_hover_text(t("Set the stroke of selected edges and of new edges"))
                    .clicked()
                {
                    self.opts.stroke = s;
                    for o in &self.selection {
                        if let ObjRef::Edge(i) = *o {
                            self.doc.edges[i].stroke = (s != Stroke::Plain
                                || self.doc.edges[i].class.is_some())
                            .then_some(s);
                        }
                    }
                }
            }
            ui.separator();
            let edges: Vec<usize> = self
                .selection
                .iter()
                .filter_map(|o| {
                    if let ObjRef::Edge(i) = o {
                        Some(*i)
                    } else {
                        None
                    }
                })
                .collect();
            // "double" shows the state of the selected edges (or of new lines).
            let on = if edges.is_empty() {
                self.opts.double
            } else {
                edges
                    .iter()
                    .all(|&i| self.doc.edges[i].double == Some(true))
            };
            if ui
                .selectable_label(on, t("double"))
                .on_hover_text(t("Double line on/off for selected edges and new lines (any stroke)"))
                .clicked()
            {
                self.opts.double = !on;
                for &i in &edges {
                    self.doc.edges[i].double = (!on).then_some(true);
                }
            }
            let ticked =
                !edges.is_empty() && edges.iter().all(|&i| self.doc.edges[i].ticks == Some(true));
            if ui
                .selectable_label(ticked, t("ticks"))
                .on_hover_text(
                    "Ticks along the selected edges on/off (spacing and marks: Fill & Stroke)",
                )
                .clicked()
            {
                for &i in &edges {
                    self.doc.edges[i].ticks = (!ticked).then_some(true);
                }
            }
            let mut g = edges
                .first()
                .and_then(|&i| self.doc.edges[i].double_sep)
                .or(self.opts.double_sep)
                .unwrap_or(d::DOUBLE_SEP);
            let l = ui.label(t("gap")).id;
            if ui
                .add(
                    DragValue::new(&mut g)
                        .speed(0.1)
                        .range(0.1..=20.0)
                        .suffix(" pt"),
                )
                .labelled_by(l)
                .on_hover_text(t("Separation of double lines: selected edges and new lines"))
                .changed()
            {
                let g = doc_ops::round(g);
                self.opts.double_sep = Some(g);
                for &i in &edges {
                    self.doc.edges[i].double_sep = Some(g);
                }
            }
        });
    }

    fn apply_color(&mut self, name: &str, fill: bool) {
        let v = Some(name.to_string());
        for o in self.selection.clone() {
            match o {
                ObjRef::Vertex(i) if fill => self.doc.vertices[i].fill = v.clone(),
                ObjRef::Vertex(i) => self.doc.vertices[i].color = v.clone(),
                ObjRef::Edge(i) if !fill => self.doc.edges[i].color = v.clone(),
                ObjRef::Text(i) if !fill => self.doc.texts[i].color = v.clone(),
                ObjRef::Axes(i) if !fill => self.doc.axes[i].color = v.clone(),
                ObjRef::Shape(i) if fill => self.doc.shapes[i].fill = v.clone(),
                ObjRef::Shape(i) => self.doc.shapes[i].color = v.clone(),
                _ => {}
            }
        }
    }

    pub(crate) fn status_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(crate::i18n::tr(&self.status_text()));
            ui.separator();
            let errors: Vec<String> = self
                .scene_errors
                .iter()
                .chain(&self.label_errors)
                .cloned()
                .collect();
            if !errors.is_empty() {
                let r = ui.label(
                    RichText::new(crate::i18n::tr(&format!("⚠ {} error(s)", errors.len())))
                        .color(Color32::from_rgb(190, 30, 30)),
                );
                r.on_hover_text(errors.join("\n"));
                ui.separator();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(self.pointer_text());
                ui.separator();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if let Some(t) = self.object_text() {
                        ui.label(RichText::new(t).monospace().color(ACCENT));
                        ui.separator();
                    }
                    if let Some(m) = &self.message {
                        ui.label(RichText::new(crate::i18n::tr(m)).italics());
                    } else {
                        ui.weak(self.os.fill(self.tool.hint()));
                    }
                });
            });
        });
    }

    pub(crate) fn dialogs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::FillStroke, t("Fill & Stroke"));
            ui.selectable_value(&mut self.tab, Tab::Objects, t("Objects"));
            ui.selectable_value(&mut self.tab, Tab::Align, t("Align"));
            ui.selectable_value(&mut self.tab, Tab::Source, t("Source"));
            ui.selectable_value(&mut self.tab, Tab::Code, t("LaTeX"))
                .on_hover_text(t("Edit the equation or plot settings of the selected drawing; it is redrawn as you type"));
        });
        ui.separator();
        match self.tab {
            Tab::FillStroke => {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.fill_stroke(ui));
            }
            Tab::Objects => {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.objects(ui));
            }
            Tab::Align => self.align_tab(ui),
            Tab::Source => self.source_tab(ui),
            Tab::Code => {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.code_tab(ui));
            }
        }
    }

    // ---- Fill & Stroke --------------------------------------------------------

    fn fill_stroke(&mut self, ui: &mut Ui) {
        let styles: Vec<String> = self.doc.styles.keys().cloned().collect();
        let sel: Vec<ObjRef> = self.selection.iter().copied().collect();
        if sel.is_empty() {
            ui.weak(t("Select an object to edit its properties."));
            ui.add_space(8.0);
            ui.label(RichText::new(t("Document")).strong());
            Grid::new("doc").num_columns(2).show(ui, |ui| {
                num(
                    ui,
                    "Margin (pt)",
                    &mut self.doc.canvas.margin,
                    d::MARGIN,
                    0.2,
                    0.0..=100.0,
                    false,
                );
                color(
                    ui,
                    "Background",
                    &mut self.doc.canvas.background,
                    None,
                    true,
                    false,
                );
            });
            return;
        }
        if sel.len() > 1 {
            ui.label(crate::i18n::tr(&format!(
                "{} objects selected. A change applies to all of them; \"mixed\" marks values that differ.",
                sel.len()
            )));
            ui.add_space(4.0);
        }
        let vs = of_kind(&sel, |o| {
            if let ObjRef::Vertex(i) = o {
                Some(*i)
            } else {
                None
            }
        });
        let es = of_kind(&sel, |o| {
            if let ObjRef::Edge(i) = o {
                Some(*i)
            } else {
                None
            }
        });
        let ts = of_kind(&sel, |o| {
            if let ObjRef::Text(i) = o {
                Some(*i)
            } else {
                None
            }
        });
        let ax = of_kind(&sel, |o| {
            if let ObjRef::Axes(i) = o {
                Some(*i)
            } else {
                None
            }
        });
        let ss = of_kind(&sel, |o| {
            if let ObjRef::Shape(i) = o {
                Some(*i)
            } else {
                None
            }
        });
        let mut sep = false;
        let mut section = |ui: &mut Ui| {
            if sep {
                ui.add_space(6.0);
                ui.separator();
            }
            sep = true;
        };
        if !vs.is_empty() {
            section(ui);
            ui.push_id("vertices", |ui| self.vertex_props(ui, &vs, &styles));
        }
        if !es.is_empty() {
            section(ui);
            ui.push_id("edges", |ui| self.edge_props(ui, &es, &styles));
        }
        if !ts.is_empty() {
            section(ui);
            ui.push_id("texts", |ui| self.text_props(ui, &ts, &styles));
        }
        if !ss.is_empty() {
            section(ui);
            ui.push_id("shapes", |ui| self.shape_props(ui, &ss, &styles));
        }
        if !ax.is_empty() {
            section(ui);
            ui.push_id("axes", |ui| self.axes_props(ui, &ax));
        }
    }

    fn vertex_props(&mut self, ui: &mut Ui, idx: &[usize], styles: &[String]) {
        let items: Vec<Vertex> = idx.iter().map(|&i| self.doc.vertices[i].clone()).collect();
        let before = items[0].clone();
        macro_rules! mx {
            ($f:ident) => {
                items.iter().any(|x| x.$f != before.$f)
            };
        }
        let st = before
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        let mut v = before.clone();
        let single = idx.len() == 1;
        ui.label(
            RichText::new(if single {
                format!("Vertex {}", before.id)
            } else {
                plural(idx.len(), "vertex", "vertices")
            })
            .strong(),
        );
        Grid::new("vertex").num_columns(2).show(ui, |ui| {
            if single {
                let i = idx[0];
                ui.label(t("Id"));
                let cur = before.id.clone();
                let buf = match &mut self.rename {
                    Some((j, b)) if *j == i => b,
                    _ => {
                        self.rename = Some((i, cur.clone()));
                        &mut self.rename.as_mut().expect("just set").1
                    }
                };
                let r = ui.add(TextEdit::singleline(buf).desired_width(120.0));
                if r.lost_focus() {
                    let new = buf.clone();
                    if let Err(e) = doc_ops::rename_vertex(&mut self.doc, i, &new) {
                        self.message = Some(e);
                        self.rename = Some((i, cur));
                    }
                }
                ui.end_row();
                ui.label(crate::i18n::t("Position"));
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut v.at[0]).speed(0.5).prefix("x "));
                    ui.add(DragValue::new(&mut v.at[1]).speed(0.5).prefix("y "));
                });
                ui.end_row();
            }
            class_choice(ui, &mut v.class, styles, mx!(class));
            let kind = v.kind.or(st.kind).unwrap_or(VertexKind::None);
            choice(
                ui,
                "Kind",
                &mut v.kind,
                st.kind.unwrap_or(VertexKind::None),
                &KINDS,
                mx!(kind),
            );
            num(
                ui,
                "Size",
                &mut v.size,
                st.size.unwrap_or(d::vertex_size(kind)),
                0.1,
                0.0..=100.0,
                mx!(size),
            );
            color(
                ui,
                "Color",
                &mut v.color,
                style_color(&st),
                false,
                mx!(color),
            );
            color(ui, "Fill", &mut v.fill, None, true, mx!(fill));
            num(
                ui,
                "Line width",
                &mut v.width,
                st.width.unwrap_or(d::WIDTH),
                0.05,
                0.05..=20.0,
                mx!(width),
            );
            num(
                ui,
                "Rotation (°)",
                &mut v.rotation,
                0.0,
                1.0,
                -360.0..=360.0,
                mx!(rotation),
            );
            opt_text(ui, "Label", &mut v.label, "$V$", mx!(label));
            choice(
                ui,
                "Label anchor",
                &mut v.label_anchor,
                Anchor::SouthWest,
                &ANCHORS,
                mx!(label_anchor),
            );
        });
        macro_rules! sync {
            ($($f:ident),*) => {
                $(if v.$f != before.$f {
                    for &i in idx {
                        self.doc.vertices[i].$f = v.$f.clone();
                    }
                })*
            };
        }
        sync!(
            at,
            class,
            kind,
            size,
            color,
            fill,
            width,
            rotation,
            label,
            label_anchor
        );
    }

    fn edge_props(&mut self, ui: &mut Ui, idx: &[usize], styles: &[String]) {
        let items: Vec<Edge> = idx.iter().map(|&i| self.doc.edges[i].clone()).collect();
        let before = items[0].clone();
        macro_rules! mx {
            ($f:ident) => {
                items.iter().any(|x| x.$f != before.$f)
            };
        }
        let single = idx.len() == 1;
        let ids = scene::ids(&self.doc);
        ui.label(
            RichText::new(if single {
                format!(
                    "Edge {}  {} → {}",
                    ids.of(&self.doc, ObjRef::Edge(idx[0])),
                    before.from,
                    before.to
                )
            } else {
                plural(idx.len(), "edge", "edges")
            })
            .strong(),
        );
        let st = before
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        let all_loops = items.iter().all(|e| e.from == e.to);
        let all_arcs = items.iter().all(|e| e.from != e.to && e.controls.is_none());
        let loop_r = |e: &Edge| e.loop_.as_ref().and_then(|l| l.radius);
        let loop_a = |e: &Edge| e.loop_.as_ref().and_then(|l| l.angle);
        let (mut lr, mut la) = (loop_r(&before), loop_a(&before));
        let (lr0, la0) = (lr, la);
        // Chord length of a single arc, for its height and radius fields.
        let chord = (single && all_arcs)
            .then(|| {
                let a = doc_ops::vertex_index(&self.doc, &before.from)?;
                let b = doc_ops::vertex_index(&self.doc, &before.to)?;
                let l = (doc_ops::pt(self.doc.vertices[a].at)
                    - doc_ops::pt(self.doc.vertices[b].at))
                .hypot();
                (l > 1e-9).then_some(l)
            })
            .flatten();
        let mut e = before.clone();
        Grid::new("edge").num_columns(2).show(ui, |ui| {
            class_choice(ui, &mut e.class, styles, mx!(class));
            let stroke = e.stroke.or(st.stroke).unwrap_or(Stroke::Plain);
            choice(
                ui,
                "Stroke",
                &mut e.stroke,
                st.stroke.unwrap_or(Stroke::Plain),
                &STROKES,
                mx!(stroke),
            );
            color(
                ui,
                "Color",
                &mut e.color,
                style_color(&st),
                false,
                mx!(color),
            );
            num(
                ui,
                "Width",
                &mut e.width,
                st.width.unwrap_or(d::WIDTH),
                0.05,
                0.05..=20.0,
                mx!(width),
            );
            match stroke {
                Stroke::Dashed | Stroke::Dotted => {
                    let [on, off] = e.dash.or(st.dash).unwrap_or(d::DASH);
                    let (mut on2, mut off2) = (Some(on), Some(off));
                    num(ui, "Dash length", &mut on2, on, 0.1, 0.1..=50.0, mx!(dash));
                    num(ui, "Dash gap", &mut off2, off, 0.1, 0.1..=50.0, mx!(dash));
                    let new = [on2.unwrap_or(d::DASH[0]), off2.unwrap_or(d::DASH[1])];
                    if new != [on, off] {
                        e.dash = Some(new);
                    }
                }
                Stroke::Wavy | Stroke::Curly | Stroke::Zigzag => {
                    let (a, w) = match stroke {
                        Stroke::Wavy => d::WAVY,
                        Stroke::Curly => d::CURLY,
                        _ => d::ZIGZAG,
                    };
                    num(
                        ui,
                        "Amplitude",
                        &mut e.amplitude,
                        st.amplitude.unwrap_or(a),
                        0.1,
                        0.1..=50.0,
                        mx!(amplitude),
                    );
                    num(
                        ui,
                        "Wavelength",
                        &mut e.wavelength,
                        st.wavelength.unwrap_or(w),
                        0.1,
                        0.5..=100.0,
                        mx!(wavelength),
                    );
                }
                Stroke::Plain => {}
            }
            flag(
                ui,
                "Double",
                &mut e.double,
                st.double.unwrap_or(false),
                mx!(double),
            );
            if e.double.or(st.double).unwrap_or(false) || mx!(double) {
                num(
                    ui,
                    "Double gap (pt)",
                    &mut e.double_sep,
                    st.double_sep.unwrap_or(d::DOUBLE_SEP),
                    0.1,
                    0.1..=20.0,
                    mx!(double_sep),
                );
            }
            flag(
                ui,
                "Ticks",
                &mut e.ticks,
                st.ticks.unwrap_or(false),
                mx!(ticks),
            );
            if e.ticks.or(st.ticks).unwrap_or(false) || mx!(ticks) {
                num(
                    ui,
                    "Tick spacing (pt)",
                    &mut e.tick_spacing,
                    st.tick_spacing.unwrap_or(d::EDGE_TICK_SPACING),
                    0.1,
                    1.0..=100.0,
                    mx!(tick_spacing),
                );
            }
            choice(
                ui,
                "Marks",
                &mut e.marks,
                st.marks.unwrap_or(0),
                &MARKS,
                mx!(marks),
            );
            let has_marks = e.marks.or(st.marks).unwrap_or(0) > 0 || mx!(marks);
            if has_marks {
                num(
                    ui,
                    "Mark position",
                    &mut e.mark_pos,
                    st.mark_pos.unwrap_or(0.5),
                    0.01,
                    0.0..=1.0,
                    mx!(mark_pos),
                );
            }
            if has_marks || e.ticks.or(st.ticks).unwrap_or(false) || mx!(ticks) {
                num(
                    ui,
                    "Tick length (pt)",
                    &mut e.tick_length,
                    st.tick_length.unwrap_or(d::EDGE_TICK_LENGTH),
                    0.1,
                    0.5..=50.0,
                    mx!(tick_length),
                );
            }
            choice(
                ui,
                "Arrow",
                &mut e.arrow,
                st.arrow.unwrap_or(ArrowDir::None),
                &ARROWS,
                mx!(arrow),
            );
            num(
                ui,
                "Arrow position",
                &mut e.arrow_pos,
                st.arrow_pos.unwrap_or(d::ARROW_POS),
                0.01,
                0.0..=1.0,
                mx!(arrow_pos),
            );
            num(
                ui,
                "Arrow length",
                &mut e.arrow_length,
                st.arrow_length.unwrap_or(d::ARROW_LENGTH),
                0.1,
                0.5..=100.0,
                mx!(arrow_length),
            );
            num(
                ui,
                "Arrow width",
                &mut e.arrow_width,
                st.arrow_width.unwrap_or(d::ARROW_WIDTH),
                0.1,
                0.5..=100.0,
                mx!(arrow_width),
            );
            if all_loops {
                let mr = items.iter().any(|x| loop_r(x) != lr0);
                let ma = items.iter().any(|x| loop_a(x) != la0);
                num(
                    ui,
                    "Loop radius",
                    &mut lr,
                    d::LOOP_RADIUS,
                    0.2,
                    1.0..=500.0,
                    mr,
                );
                num(
                    ui,
                    "Loop angle (°)",
                    &mut la,
                    d::LOOP_ANGLE,
                    1.0,
                    -360.0..=360.0,
                    ma,
                );
            } else if all_arcs {
                num(
                    ui,
                    "Bend (°)",
                    &mut e.bend,
                    0.0,
                    1.0,
                    -179.0..=179.0,
                    mx!(bend),
                );
                if let Some(l) = chord {
                    arc_fields(ui, &mut e.bend, l);
                }
                if e.bend == Some(0.0) {
                    e.bend = None;
                }
            }
            opt_text(ui, "Label", &mut e.label, "$p$", mx!(label));
            choice(
                ui,
                "Label side",
                &mut e.label_side,
                st.label_side.unwrap_or(Side::Left),
                &SIDES,
                mx!(label_side),
            );
            num(
                ui,
                "Label position",
                &mut e.label_pos,
                st.label_pos.unwrap_or(d::LABEL_POS),
                0.01,
                0.0..=1.0,
                mx!(label_pos),
            );
            num(
                ui,
                "Label offset",
                &mut e.label_offset,
                st.label_offset.unwrap_or(d::LABEL_OFFSET),
                0.1,
                -50.0..=50.0,
                mx!(label_offset),
            );
        });
        macro_rules! sync {
            ($($f:ident),*) => {
                $(if e.$f != before.$f {
                    for &i in idx {
                        self.doc.edges[i].$f = e.$f.clone();
                    }
                })*
            };
        }
        sync!(
            class,
            stroke,
            color,
            width,
            dash,
            amplitude,
            wavelength,
            double,
            double_sep,
            ticks,
            tick_spacing,
            tick_length,
            marks,
            mark_pos,
            arrow,
            arrow_pos,
            arrow_length,
            arrow_width,
            bend,
            label,
            label_side,
            label_pos,
            label_offset
        );
        if lr != lr0 || la != la0 {
            for &i in idx {
                let ed = &mut self.doc.edges[i];
                let l = ed.loop_.get_or_insert(LoopSpec {
                    radius: None,
                    angle: None,
                });
                if lr != lr0 {
                    l.radius = lr;
                }
                if la != la0 {
                    l.angle = la;
                }
                if l.radius.is_none() && l.angle.is_none() {
                    ed.loop_ = None;
                }
            }
        }
        if single {
            self.bezier_buttons(ui, idx[0]);
        }
    }

    fn bezier_buttons(&mut self, ui: &mut Ui, i: usize) {
        let e = &self.doc.edges[i];
        if e.from == e.to {
            return;
        }
        ui.add_space(4.0);
        if e.controls.is_none() {
            if ui
                .button(t("Make Bézier curve"))
                .on_hover_text(t(
                    "Replace the arc by a curve with two control points (Edit tool)",
                ))
                .clicked()
            {
                let (a, b) = (
                    doc_ops::vertex_index(&self.doc, &e.from),
                    doc_ops::vertex_index(&self.doc, &e.to),
                );
                if let (Some(a), Some(b)) = (a, b) {
                    let (pa, pb) = (
                        doc_ops::pt(self.doc.vertices[a].at),
                        doc_ops::pt(self.doc.vertices[b].at),
                    );
                    let n = goldstone_core::geom::left_normal((pb - pa).normalize())
                        * ((pb - pa).hypot() * 0.25);
                    let c1 = pa + (pb - pa) / 3.0 + n;
                    let c2 = pa + (pb - pa) * (2.0 / 3.0) + n;
                    let e = &mut self.doc.edges[i];
                    e.bend = None;
                    e.controls = Some([
                        [doc_ops::round(c1.x), doc_ops::round(c1.y)],
                        [doc_ops::round(c2.x), doc_ops::round(c2.y)],
                    ]);
                }
            }
        } else if ui.button(t("Make straight")).clicked() {
            self.doc.edges[i].controls = None;
        }
    }

    fn text_props(&mut self, ui: &mut Ui, idx: &[usize], styles: &[String]) {
        let items: Vec<Text> = idx.iter().map(|&i| self.doc.texts[i].clone()).collect();
        let before = items[0].clone();
        macro_rules! mx {
            ($f:ident) => {
                items.iter().any(|x| x.$f != before.$f)
            };
        }
        let single = idx.len() == 1;
        ui.label(
            RichText::new(if single {
                "Text".into()
            } else {
                plural(idx.len(), "text", "texts")
            })
            .strong(),
        );
        let focus = std::mem::take(&mut self.focus_text_content);
        let st = before
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        let mut t = before.clone();
        ui.label(crate::i18n::t("Content (text with $LaTeX math$)"));
        let mixed_content = mx!(content);
        let mut content = if mixed_content {
            String::new()
        } else {
            t.content.clone()
        };
        let r = ui.add(
            TextEdit::multiline(&mut content)
                .hint_text(if mixed_content {
                    crate::i18n::t("mixed")
                } else {
                    ""
                })
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .code_editor(),
        );
        if r.changed() {
            t.content = content;
        }
        if focus {
            r.request_focus();
        }
        Grid::new("text").num_columns(2).show(ui, |ui| {
            if single {
                ui.label(crate::i18n::t("Position"));
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut t.at[0]).speed(0.5).prefix("x "));
                    ui.add(DragValue::new(&mut t.at[1]).speed(0.5).prefix("y "));
                });
                ui.end_row();
            }
            class_choice(ui, &mut t.class, styles, mx!(class));
            choice(
                ui,
                "Anchor",
                &mut t.anchor,
                Anchor::Center,
                &ANCHORS,
                mx!(anchor),
            );
            num(
                ui,
                "Size (pt)",
                &mut t.size,
                st.text_size.unwrap_or(d::TEXT_SIZE),
                0.1,
                1.0..=200.0,
                mx!(size),
            );
            color(
                ui,
                "Color",
                &mut t.color,
                style_color(&st),
                false,
                mx!(color),
            );
            num(
                ui,
                "Rotation (°)",
                &mut t.rotation,
                0.0,
                1.0,
                -360.0..=360.0,
                mx!(rotation),
            );
        });
        macro_rules! sync {
            ($($f:ident),*) => {
                $(if t.$f != before.$f {
                    for &i in idx {
                        self.doc.texts[i].$f = t.$f.clone();
                    }
                })*
            };
        }
        sync!(content, at, class, anchor, size, color, rotation);
    }

    fn shape_props(&mut self, ui: &mut Ui, idx: &[usize], styles: &[String]) {
        let items: Vec<Shape> = idx.iter().map(|&i| self.doc.shapes[i].clone()).collect();
        let before = items[0].clone();
        macro_rules! mx {
            ($f:ident) => {
                items.iter().any(|x| x.$f != before.$f)
            };
        }
        let single = idx.len() == 1;
        let title = if single {
            let ids = scene::ids(&self.doc);
            format!(
                "Shape {}  ({})",
                ids.of(&self.doc, ObjRef::Shape(idx[0])),
                goldstone_core::shapes::kind_name(before.kind)
            )
        } else {
            plural(idx.len(), "shape", "shapes")
        };
        ui.label(RichText::new(title).strong());
        let st = before
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        let mut s = before.clone();
        Grid::new("shape").num_columns(2).show(ui, |ui| {
            if single && let (Some(at), Some(size)) = (s.at.as_mut(), s.size.as_mut()) {
                ui.label(t("Center"));
                ui.horizontal(|ui| {
                    ui.add(DragValue::new(&mut at[0]).speed(0.5).prefix("x "));
                    ui.add(DragValue::new(&mut at[1]).speed(0.5).prefix("y "));
                });
                ui.end_row();
                ui.label(t("Size"));
                ui.horizontal(|ui| {
                    ui.add(
                        DragValue::new(&mut size[0])
                            .speed(0.5)
                            .range(0.1..=10000.0)
                            .prefix("w "),
                    );
                    ui.add(
                        DragValue::new(&mut size[1])
                            .speed(0.5)
                            .range(0.1..=10000.0)
                            .prefix("h "),
                    );
                });
                ui.end_row();
            }
            class_choice(ui, &mut s.class, styles, mx!(class));
            color(
                ui,
                "Stroke color",
                &mut s.color,
                style_color(&st),
                false,
                mx!(color),
            );
            color(ui, "Fill", &mut s.fill, None, true, mx!(fill));
            num(
                ui,
                "Width",
                &mut s.width,
                st.width.unwrap_or(d::WIDTH),
                0.05,
                0.0..=20.0,
                mx!(width),
            );
            num(
                ui,
                "Rotation (°)",
                &mut s.rotation,
                0.0,
                1.0,
                -360.0..=360.0,
                mx!(rotation),
            );
        });
        macro_rules! sync {
            ($($f:ident),*) => {
                $(if s.$f != before.$f {
                    for &i in idx {
                        self.doc.shapes[i].$f = s.$f.clone();
                    }
                })*
            };
        }
        sync!(at, size, class, color, fill, width, rotation);
    }

    fn axes_props(&mut self, ui: &mut Ui, idx: &[usize]) {
        if idx.len() > 1 {
            ui.label(RichText::new(plural(idx.len(), "axes", "axes")).strong());
            ui.weak(t("Select one set of axes to edit it."));
            return;
        }
        ui.label(RichText::new(t("Axes")).strong());
        let a = &mut self.doc.axes[idx[0]];
        Grid::new("axes").num_columns(2).show(ui, |ui| {
            ui.label(t("Origin"));
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut a.origin[0]).speed(0.5).prefix("x "));
                ui.add(DragValue::new(&mut a.origin[1]).speed(0.5).prefix("y "));
            });
            ui.end_row();
            color(ui, "Color", &mut a.color, Some(Color::BLACK), false, false);
            num(
                ui,
                "Width",
                &mut a.width,
                d::WIDTH,
                0.05,
                0.05..=20.0,
                false,
            );
        });
        for (name, axis) in [("x axis", &mut a.x), ("y axis", &mut a.y)] {
            ui.add_space(6.0);
            ui.label(RichText::new(name).strong());
            let ax = axis.get_or_insert_with(Axis::default);
            Grid::new(name).num_columns(2).show(ui, |ui| {
                flag(ui, "Show", &mut ax.show, true, false);
                num(
                    ui,
                    "Length",
                    &mut ax.length,
                    d::AXIS_LENGTH,
                    0.5,
                    1.0..=10000.0,
                    false,
                );
                flag(ui, "Arrow", &mut ax.arrow, true, false);
                opt_text(ui, "Label", &mut ax.label, "$t$", false);
                flag(ui, "Ticks", &mut ax.ticks, false, false);
                num(
                    ui,
                    "Tick spacing",
                    &mut ax.tick_spacing,
                    d::TICK_SPACING,
                    0.5,
                    0.5..=1000.0,
                    false,
                );
                num(
                    ui,
                    "Tick length",
                    &mut ax.tick_length,
                    d::TICK_LENGTH,
                    0.1,
                    0.0..=50.0,
                    false,
                );
            });
            if *ax == Axis::default() {
                *axis = None;
            }
        }
    }

    // ---- Objects ----------------------------------------------------------------

    fn objects(&mut self, ui: &mut Ui) {
        let shift = ui.input(|i| i.modifiers.shift);
        let mut clicked: Option<doc_ops::Selection> = None;
        let ids = scene::ids(&self.doc);
        let d = &self.doc;
        let mut section = |ui: &mut Ui,
                           title: &str,
                           items: Vec<(doc_ops::Selection, String)>,
                           sel: &doc_ops::Selection| {
            if items.is_empty() {
                return;
            }
            egui::CollapsingHeader::new(format!("{title} ({})", items.len()))
                .default_open(true)
                .show(ui, |ui| {
                    for (objs, name) in items {
                        let on = objs.iter().all(|o| sel.contains(o)) && !objs.is_empty();
                        if ui
                            .selectable_label(on, RichText::new(name).monospace())
                            .clicked()
                        {
                            clicked = Some(objs);
                        }
                    }
                });
        };
        let row = |o: ObjRef| -> (doc_ops::Selection, String) {
            (
                [o].into(),
                format!("{}  {}", ids.of(d, o), inspect::summary(d, o)),
            )
        };
        section(
            ui,
            "Vertices",
            (0..d.vertices.len())
                .map(|i| row(ObjRef::Vertex(i)))
                .collect(),
            &self.selection,
        );
        section(
            ui,
            "Edges",
            (0..d.edges.len()).map(|i| row(ObjRef::Edge(i))).collect(),
            &self.selection,
        );
        section(
            ui,
            "Texts",
            (0..d.texts.len()).map(|i| row(ObjRef::Text(i))).collect(),
            &self.selection,
        );
        section(
            ui,
            "Axes",
            (0..d.axes.len()).map(|i| row(ObjRef::Axes(i))).collect(),
            &self.selection,
        );
        section(
            ui,
            "Shapes",
            (0..d.shapes.len()).map(|i| row(ObjRef::Shape(i))).collect(),
            &self.selection,
        );
        section(
            ui,
            "Groups",
            (0..d.groups.len())
                .map(|g| {
                    (
                        doc_ops::group_members(d, g),
                        format!("{}  {}", d.groups[g].id, d.groups[g].members.join(", ")),
                    )
                })
                .collect(),
            &self.selection,
        );
        if let Some(objs) = clicked {
            if shift {
                if objs.iter().all(|o| self.selection.contains(o)) {
                    objs.iter().for_each(|o| {
                        self.selection.remove(o);
                    });
                } else {
                    self.selection.extend(objs);
                }
            } else {
                self.selection = objs;
            }
        }
        if self.doc.vertices.is_empty()
            && self.doc.texts.is_empty()
            && self.doc.axes.is_empty()
            && self.doc.shapes.is_empty()
        {
            ui.weak(t("The diagram is empty. Draw with the Line tool (B), the Arc tool (A) or the Vertex tool (P)."));
        }
    }

    // ---- Align & Distribute -------------------------------------------------------

    fn align_tab(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        ui.weak(t("Relative to the bounding box of the selection. Groups move as one; a vertex shared by two objects moves once."));
        ui.add_space(6.0);
        let mut chosen = None;
        for (row, title) in [(&ALIGN[..6], "Align"), (&ALIGN[6..], "Distribute")] {
            ui.label(RichText::new(title).strong());
            ui.horizontal_wrapped(|ui| {
                for (icon, tip, op) in row {
                    let r = match self.icon(&ctx, icon, 24.0) {
                        Some(img) => {
                            ui.add(egui::Button::image(img).min_size(egui::vec2(32.0, 32.0)))
                        }
                        None => ui.button(*tip),
                    };
                    if r.on_hover_text(*tip).clicked() {
                        chosen = Some(*op);
                    }
                }
            });
            ui.add_space(6.0);
        }
        if self.selection.len() < 2 {
            ui.weak(t(
                "Select two or more objects (three or more to distribute).",
            ));
        }
        if let Some(op) = chosen {
            self.do_action(Action::Align(op), &ctx);
        }
    }

    // ---- Source -----------------------------------------------------------------

    fn source_tab(&mut self, ui: &mut Ui) {
        ui.weak(t(
            "The .gold text of this diagram. Edits apply as soon as the text is valid.",
        ));
        if let Some(e) = &self.source.error {
            ui.label(
                RichText::new(e)
                    .color(Color32::from_rgb(190, 30, 30))
                    .monospace(),
            );
        }
        let id = egui::Id::unique("goldstone-source-editor");
        let focused = ui.ctx().memory(|m| m.has_focus(id));
        if !focused && self.source.for_doc.as_ref() != Some(&self.doc) {
            self.source.text = io::save(&self.doc, self.loaded_text.as_deref());
            self.source.for_doc = Some(self.doc.clone());
            self.source.error = None;
        }
        let r = ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add(
                    TextEdit::multiline(&mut self.source.text)
                        .id(id)
                        .code_editor()
                        .desired_width(f32::INFINITY)
                        .desired_rows(30),
                )
            })
            .inner;
        if r.changed() {
            match io::parse(&self.source.text).and_then(|d| scene::build(&d).map(|_| d)) {
                Ok(d) => {
                    self.doc = d.clone();
                    self.source.for_doc = Some(d);
                    self.source.error = None;
                }
                Err(e) => self.source.error = Some(e.to_string()),
            }
        }
    }

    // ---- windows ----------------------------------------------------------------

    pub(crate) fn shortcuts_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_shortcuts;
        let os = self.os;
        egui::Window::new(t("Keyboard and mouse"))
            .open(&mut open)
            .default_size([560.0, 560.0])
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    ui.weak(crate::i18n::tr(&format!(
                        "Shortcuts for {}. The full manual is docs/MANUAL.md.",
                        os.name()
                    )));
                    let mut group = "";
                    for b in BINDINGS.iter().filter(|b| b.listed && b.only.includes(os)) {
                        if b.group != group {
                            group = b.group;
                            ui.add_space(6.0);
                            ui.label(RichText::new(t(group)).strong().color(ACCENT));
                        }
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [120.0, 16.0],
                                egui::Label::new(
                                    RichText::new(keys::keys_label(b, os)).monospace(),
                                ),
                            );
                            ui.label(t(b.desc));
                        });
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new(t("Mouse")).strong().color(ACCENT));
                    for (w, input, desc, _) in MOUSE {
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [120.0, 16.0],
                                egui::Label::new(RichText::new(t(w)).weak()),
                            );
                            ui.label(RichText::new(os.fill(t(input))).monospace());
                            ui.label(os.fill(t(desc)));
                        });
                    }
                });
            });
        self.show_shortcuts = open;
    }

    pub(crate) fn latex_window(&mut self, ctx: &egui::Context) {
        if !self.latex.open {
            return;
        }
        let mut open = true;
        let mut action: Option<bool> = None;
        let srg = self.latex.rule.srg();
        let hug = self.latex.rule.hugenholtz();
        egui::Window::new(t(match self.latex.rule {
            goldstone_core::expr::Rule::Cc => "CC Diagram from LaTeX",
            goldstone_core::expr::Rule::Srg => "IMSRG Diagram from LaTeX",
            goldstone_core::expr::Rule::SrgHug => "IMSRG Hugenholtz Diagram from LaTeX",
        }))
            .open(&mut open)
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(t("Write a tensor expression; Goldstone draws its diagram."));
                let r = ui.add(
                    TextEdit::multiline(&mut self.latex.text)
                        .code_editor()
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );
                if self.latex.for_text.as_deref() != Some(self.latex.text.as_str()) {
                    self.latex.for_text = Some(self.latex.text.clone());
                    let mut warnings = Vec::new();
                    self.latex.status = goldstone_core::expr::parse_with(&self.latex.text, self.latex.rule).and_then(|eq| {
                        let drawn = goldstone_core::expr::diagram(&eq)?;
                        warnings = drawn.warnings;
                        let ops: usize = eq.terms.iter().map(|t| t.tensors.len()).sum();
                        Ok(format!(
                            "{} term(s), {ops} operator(s){}",
                            eq.terms.len() - warnings.len(),
                            if eq.external.is_empty() {
                                String::new()
                            } else {
                                format!(", open: {}", eq.external.join(" "))
                            }
                        ))
                    });
                    self.latex.warnings = warnings;
                }
                match &self.latex.status {
                    Ok(s) if !s.is_empty() => {
                        ui.label(RichText::new(crate::i18n::tr(&format!("✓ {s}"))).color(Color32::from_rgb(40, 150, 60)));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        ui.label(RichText::new(e).color(Color32::from_rgb(190, 30, 30)));
                    }
                }
                for w in &self.latex.warnings {
                    ui.label(RichText::new(crate::i18n::tr(&format!("⚠ {w}"))).color(Color32::from_rgb(200, 120, 0)));
                }
                let ok = self.latex.status.is_ok();
                ui.horizontal(|ui| {
                    if ui.add_enabled(ok, egui::Button::new(t("Insert"))).on_hover_text(t("Add the diagram to the right of the drawing")).clicked() {
                        action = Some(false);
                    }
                    if ui
                        .add_enabled(ok, egui::Button::new(t("Replace drawing")))
                        .on_hover_text(t("Replace everything with the diagram (undo brings it back)"))
                        .clicked()
                    {
                        action = Some(true);
                    }
                });
                if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command) && ok {
                    action = Some(false);
                }
                ui.add_space(4.0);
                egui::CollapsingHeader::new(t("Conventions")).default_open(false).show(ui, |ui| {
                    ui.label(t("An operator is X^{label}_{pqrs} = ⟨pq|X|rs⟩: the first half of the indices are outgoing (bra), the second incoming (ket); the optional superscript is part of the name."));
                    if srg {
                        ui.label(t("IMSRG: an index is a, a_1 or a digit (1). Every line is a general orbital (no arrow) unless an occupation factor says otherwise: n_a hole, \\bar n_a or (1 - n_a) particle; factors such as (n_a - n_b) give one diagram per particle–hole assignment. Permutations (1+P_{12}) are left out."));
                    } else {
                        ui.label(t("An index is a or a_1 (nothing else). a–g are particles (arrow up), h–n holes (arrow down), numbered or not; other letters are general. An index used twice is an internal line; one used once is external and must stand on the left."));
                    }
                    if hug {
                        ui.label(t("Hugenholtz: every two- and three-body operator is one dot with all its lines; one-body operators are drawn as below."));
                    }
                    ui.label(t("V W v w g H F f U \\Gamma are interactions (dashed line; one-body: ---X); other names (T, S, R, …) are amplitudes (thick bar)."));
                    ui.label(t("Terms: + and -; prefactors such as \\frac{1}{4} are kept; \\sum, *, \\cdot are ignored."));
                    if srg {
                        ui.monospace("\\frac{dE}{ds} = \\sum_{ab} (n_a - n_b) \\eta_{ab} f_{ba}\n\\frac{df_{12}}{ds} = \\sum_{a} (1+P_{12}) \\eta_{1a} f_{a2}\nZ_{12} = \\frac{1}{2} \\sum_{abc} (n_a n_b \\bar n_c + \\bar n_a \\bar n_b n_c) \\eta_{c1ab} \\Gamma_{abc2}");
                    } else {
                        ui.monospace("E2 = \\sum_{abij} V_{ijab} T_{abij}\nZ_{abic} = V_{abef} S_{efic}\nZ_{abij} = \\frac{1}{2} V_{abef} T_{efij}");
                    }
                });
            });
        if let Some(replace) = action {
            self.insert_latex(replace);
        }
        self.latex.open = open && action.is_none_or(|_| self.latex.status.is_err());
    }

    /// Web: Export downloads the drawing in a format chosen here.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn export_window(&mut self, ctx: &egui::Context) {
        if !self.show_export {
            return;
        }
        let mut open = true;
        let mut chosen = None;
        egui::Window::new(t("Export"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(t("Download the drawing as:"));
                for (ext, what) in [
                    ("pdf", "PDF (vector)"),
                    ("svg", "SVG (vector, reopenable in Goldstone)"),
                    ("png", "PNG (300 dpi)"),
                    ("tikz", "TikZ picture (\\input into a paper)"),
                    ("tex", "LaTeX document"),
                ] {
                    if ui.button(t(what)).clicked() {
                        chosen = Some(ext);
                    }
                }
            });
        if let Some(ext) = chosen {
            self.export_download(ext);
            open = false;
        }
        self.show_export = open;
    }

    pub(crate) fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new(t("About Goldstone"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr(&format!(
                    "Goldstone {}",
                    env!("CARGO_PKG_VERSION")
                )));
                ui.label(t("Many-body diagrams: coupled cluster, IMSRG, MBPT."));
                ui.weak(t("MIT license."));
                ui.add_space(4.0);
                ui.weak(t(
                    "Includes third-party software and fonts under their own licenses.",
                ));
                if ui.button(t("Third-party licenses")).clicked() {
                    self.show_licenses = true;
                }
            });
        self.show_about = open;
        self.licenses_window(ctx);
    }

    /// The licenses and notices of everything compiled into the program.
    fn licenses_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_licenses;
        egui::Window::new(t("Third-party licenses"))
            .open(&mut open)
            .default_size([640.0, 520.0])
            .show(ctx, |ui| {
                let lines: Vec<&str> = crate::THIRD_PARTY_LICENSES.lines().collect();
                let row = ui.text_style_height(&egui::TextStyle::Monospace);
                ScrollArea::both().auto_shrink([false, false]).show_rows(
                    ui,
                    row,
                    lines.len(),
                    |ui, range| {
                        for line in &lines[range] {
                            ui.label(RichText::new(*line).monospace().size(11.0));
                        }
                    },
                );
            });
        self.show_licenses = open;
    }
}

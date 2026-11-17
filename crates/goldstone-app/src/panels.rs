//! Menus, toolbox, tool controls, docked dialogs, palettes and status bar.

use std::ops::RangeInclusive;

use egui::{Color32, ComboBox, DragValue, Grid, RichText, ScrollArea, TextEdit, Ui};
use goldstone_core::color::{Color, NAMED};
use goldstone_core::model::*;
use goldstone_core::scene::{ObjRef, defaults as d};
use goldstone_core::{io, scene};

use crate::app::{ACCENT, GoldstoneApp, Tab, Tool};
use crate::doc_ops;
use crate::keys::{Action, BINDINGS, MOUSE};

const STROKES: [(Stroke, &str); 6] = [
    (Stroke::Plain, "plain"),
    (Stroke::Dashed, "dashed"),
    (Stroke::Dotted, "dotted"),
    (Stroke::Wavy, "wavy"),
    (Stroke::Curly, "curly"),
    (Stroke::Zigzag, "zigzag"),
];
const ARROWS: [(ArrowDir, &str); 3] = [
    (ArrowDir::None, "none"),
    (ArrowDir::Forward, "forward"),
    (ArrowDir::Backward, "backward"),
];
const KINDS: [(VertexKind, &str); 6] = [
    (VertexKind::None, "none (invisible)"),
    (VertexKind::Dot, "dot"),
    (VertexKind::Cross, "cross ✕"),
    (VertexKind::Ocross, "circled cross ⊗"),
    (VertexKind::Circle, "circle"),
    (VertexKind::Square, "square"),
];
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

fn shortcut(action: Action) -> String {
    BINDINGS
        .iter()
        .find(|b| b.action == action && !b.keys.is_empty())
        .map_or(String::new(), |b| b.keys.to_string())
}

fn to_c32(c: Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

// ---- small widgets for optional (inheritable) fields --------------------------

fn reset(ui: &mut Ui, set: bool) -> bool {
    if set {
        ui.small_button("↺")
            .on_hover_text("Reset to the class / default value")
            .clicked()
    } else {
        ui.weak("·")
            .on_hover_text("Inherited from the class or the default");
        false
    }
}

fn num(
    ui: &mut Ui,
    label: &str,
    v: &mut Option<f64>,
    inherited: f64,
    speed: f64,
    range: RangeInclusive<f64>,
) {
    let l = ui.label(label).id;
    ui.horizontal(|ui| {
        let mut x = v.unwrap_or(inherited);
        let mut dv = DragValue::new(&mut x)
            .speed(speed)
            .range(range)
            .max_decimals(2);
        if v.is_none() {
            dv = dv.custom_formatter(|n, _| {
                format!("{n:.2}")
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_string()
            });
        }
        let r = ui.add(dv).labelled_by(l);
        if r.changed() {
            *v = Some(doc_ops::round(x));
        }
        if reset(ui, v.is_some()) {
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
) {
    ui.label(label);
    ui.horizontal(|ui| {
        let cur = v.unwrap_or(inherited);
        let name = options
            .iter()
            .find(|(o, _)| *o == cur)
            .map_or("?", |(_, n)| *n);
        let text = if v.is_some() {
            RichText::new(name)
        } else {
            RichText::new(name).weak()
        };
        ComboBox::from_id_salt(label)
            .selected_text(text)
            .show_ui(ui, |ui| {
                for (o, n) in options {
                    if ui.selectable_label(cur == *o, *n).clicked() {
                        *v = Some(*o);
                    }
                }
            });
        if reset(ui, v.is_some()) {
            *v = None;
        }
    });
    ui.end_row();
}

fn flag(ui: &mut Ui, label: &str, v: &mut Option<bool>, inherited: bool) {
    let l = ui.label(label).id;
    ui.horizontal(|ui| {
        let mut x = v.unwrap_or(inherited);
        if ui.checkbox(&mut x, "").labelled_by(l).changed() {
            *v = Some(x);
        }
        if reset(ui, v.is_some()) {
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
) {
    let l = ui.label(label).id;
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
        let shown = v
            .clone()
            .unwrap_or_else(|| inherited.map_or("none".into(), |c| c.hex()));
        let text = if v.is_some() {
            RichText::new(shown).monospace()
        } else {
            RichText::new(shown).monospace().weak()
        };
        ui.label(text);
        if allow_none && cur.is_some() && ui.small_button("none").clicked() {
            *v = Some("none".into());
        }
        if reset(ui, v.is_some()) {
            *v = None;
        }
    });
    ui.end_row();
}

fn opt_text(ui: &mut Ui, label: &str, v: &mut Option<String>, hint: &str) -> egui::Response {
    let l = ui.label(label).id;
    let mut s = v.clone().unwrap_or_default();
    let r = ui
        .add(
            TextEdit::singleline(&mut s)
                .hint_text(hint)
                .desired_width(f32::INFINITY),
        )
        .labelled_by(l);
    if r.changed() {
        *v = (!s.is_empty()).then_some(s);
    }
    ui.end_row();
    r
}

fn class_choice(ui: &mut Ui, v: &mut Option<String>, styles: &[String]) {
    ui.label("Class");
    let text = v.clone().unwrap_or_else(|| "(none)".into());
    ComboBox::from_id_salt("class")
        .selected_text(text)
        .show_ui(ui, |ui| {
            if ui.selectable_label(v.is_none(), "(none)").clicked() {
                *v = None;
            }
            for s in styles {
                if ui
                    .selectable_label(v.as_deref() == Some(s.as_str()), s)
                    .clicked()
                {
                    *v = Some(s.clone());
                }
            }
        });
    ui.end_row();
}

impl GoldstoneApp {
    pub(crate) fn menu_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let show_grid = self.show_grid;
        let mut chosen: Option<Action> = None;
        let mut about = false;
        egui::MenuBar::new().ui(ui, |ui| {
            let mut item = |ui: &mut Ui, label: &str, a: Action| {
                if ui
                    .add(egui::Button::new(label).shortcut_text(shortcut(a)))
                    .clicked()
                {
                    chosen = Some(a);
                    ui.close();
                }
            };
            ui.menu_button("File", |ui| {
                item(ui, "New", Action::New);
                item(ui, "Open…", Action::Open);
                item(ui, "Save", Action::Save);
                item(ui, "Save As…", Action::SaveAs);
                ui.separator();
                item(ui, "Export…", Action::Export);
            });
            ui.menu_button("Edit", |ui| {
                item(ui, "Undo", Action::Undo);
                item(ui, "Redo", Action::Redo);
                ui.separator();
                item(ui, "Delete", Action::Delete);
                item(ui, "Select All", Action::SelectAll);
                item(ui, "Deselect", Action::Deselect);
                ui.separator();
                item(ui, "Flip Horizontal", Action::FlipH);
                item(ui, "Flip Vertical", Action::FlipV);
            });
            ui.menu_button("View", |ui| {
                item(ui, "Zoom In", Action::ZoomIn);
                item(ui, "Zoom Out", Action::ZoomOut);
                item(ui, "Zoom 1:1", Action::Zoom1);
                item(ui, "Zoom Selection", Action::ZoomSelection);
                item(ui, "Zoom Drawing", Action::ZoomDrawing);
                ui.separator();
                item(
                    ui,
                    if show_grid { "Hide Grid" } else { "Show Grid" },
                    Action::ToggleGrid,
                );
            });
            ui.menu_button("Dialogs", |ui| {
                item(ui, "Fill & Stroke", Action::ShowFillStroke);
                item(ui, "Objects", Action::ShowObjects);
                item(ui, "Source", Action::ShowSource);
            });
            ui.menu_button("Help", |ui| {
                item(ui, "Keyboard and Mouse", Action::Shortcuts);
                if ui.button("About Goldstone").clicked() {
                    about = true;
                    ui.close();
                }
            });
        });
        if let Some(a) = chosen {
            self.do_action(a, &ctx);
        }
        self.show_about |= about;
    }

    pub(crate) fn toolbox(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        for t in Tool::ALL {
            let text = RichText::new(format!("{}\n{}", t.name(), t.key())).size(12.0);
            let b = egui::Button::selectable(self.tool == t, text).min_size(egui::vec2(60.0, 40.0));
            if ui
                .add(b)
                .on_hover_text(format!("{} ({})\n{}", t.name(), t.key(), t.hint()))
                .clicked()
            {
                self.set_tool(t);
            }
        }
    }

    pub(crate) fn tool_controls(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.strong(self.tool.name());
            ui.separator();
            match self.tool {
                Tool::Edge => {
                    let styles: Vec<String> = self.doc.styles.keys().cloned().collect();
                    ui.label("Class");
                    let text = self.opts.class.clone().unwrap_or_else(|| "(none)".into());
                    ComboBox::from_id_salt("tc-class")
                        .selected_text(text)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(self.opts.class.is_none(), "(none)")
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
                    ui.label("Stroke");
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
                    ui.label("Arrow");
                    let name = match self.opts.arrow {
                        None => "from class",
                        Some(a) => ARROWS.iter().find(|(x, _)| *x == a).map_or("", |(_, n)| *n),
                    };
                    ComboBox::from_id_salt("tc-arrow")
                        .selected_text(name)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.opts.arrow, None, "from class");
                            for (a, n) in ARROWS {
                                ui.selectable_value(&mut self.opts.arrow, Some(a), n);
                            }
                        });
                }
                Tool::Vertex => {
                    ui.label("Kind");
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
                    if let Some(b) =
                        doc_ops::bounds(&doc_ops::selection_points(&self.doc, &self.selection))
                    {
                        ui.label(format!(
                            "X {:.1}  Y {:.1}  W {:.1}  H {:.1} pt",
                            b.x0,
                            b.y0,
                            b.width(),
                            b.height()
                        ));
                    }
                }
                _ => {}
            }
            ui.separator();
            ui.checkbox(&mut self.show_grid, "Grid");
            let mut g = self.grid();
            let l = ui.label("spacing").id;
            if ui
                .add(DragValue::new(&mut g).range(1.0..=100.0).suffix(" pt"))
                .labelled_by(l)
                .changed()
            {
                self.doc.canvas.grid = Some(g);
            }
        });
    }

    pub(crate) fn palettes(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Color");
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
                let resp = resp.on_hover_text(format!(
                    "{name}: click sets the stroke color, Shift+click the fill"
                ));
                if resp.clicked() {
                    let fill = ui.input(|i| i.modifiers.shift);
                    self.apply_color(name, fill);
                }
            }
            ui.separator();
            ui.label("Stroke");
            for (s, n) in STROKES {
                if ui
                    .selectable_label(self.opts.stroke == s, n)
                    .on_hover_text("Set the stroke of selected edges and of new edges")
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
            if ui
                .button("double")
                .on_hover_text("Toggle double line on selected edges")
                .clicked()
            {
                for o in &self.selection {
                    if let ObjRef::Edge(i) = *o {
                        let e = &mut self.doc.edges[i];
                        e.double = if e.double == Some(true) {
                            None
                        } else {
                            Some(true)
                        };
                    }
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
            ui.label(self.status_text());
            ui.separator();
            let errors: Vec<String> = self
                .scene_errors
                .iter()
                .chain(&self.label_errors)
                .cloned()
                .collect();
            if !errors.is_empty() {
                let r = ui.label(
                    RichText::new(format!("⚠ {} error(s)", errors.len()))
                        .color(Color32::from_rgb(190, 30, 30)),
                );
                r.on_hover_text(errors.join("\n"));
                ui.separator();
            }
            if let Some(m) = &self.message {
                ui.label(RichText::new(m).italics());
            } else {
                ui.weak(self.tool.hint());
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.monospace(self.pointer_text());
            });
        });
    }

    pub(crate) fn dialogs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::FillStroke, "Fill & Stroke");
            ui.selectable_value(&mut self.tab, Tab::Objects, "Objects");
            ui.selectable_value(&mut self.tab, Tab::Source, "Source");
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
            Tab::Source => self.source_tab(ui),
        }
    }

    // ---- Fill & Stroke --------------------------------------------------------

    fn fill_stroke(&mut self, ui: &mut Ui) {
        let styles: Vec<String> = self.doc.styles.keys().cloned().collect();
        let sel: Vec<ObjRef> = self.selection.iter().copied().collect();
        match sel.as_slice() {
            [] => {
                ui.weak("Select an object to edit its properties.");
                ui.add_space(8.0);
                ui.label(RichText::new("Document").strong());
                Grid::new("doc").num_columns(2).show(ui, |ui| {
                    num(
                        ui,
                        "Margin (pt)",
                        &mut self.doc.canvas.margin,
                        d::MARGIN,
                        0.2,
                        0.0..=100.0,
                    );
                    color(
                        ui,
                        "Background",
                        &mut self.doc.canvas.background,
                        None,
                        true,
                    );
                });
            }
            [ObjRef::Vertex(i)] => self.vertex_props(ui, *i, &styles),
            [ObjRef::Edge(i)] => self.edge_props(ui, *i, &styles),
            [ObjRef::Text(i)] => self.text_props(ui, *i, &styles),
            [ObjRef::Axes(i)] => {
                ui.label(RichText::new("Axes").strong());
                let a = &mut self.doc.axes[*i];
                Grid::new("axes").num_columns(2).show(ui, |ui| {
                    ui.label("Origin");
                    ui.horizontal(|ui| {
                        ui.add(DragValue::new(&mut a.origin[0]).speed(0.5).prefix("x "));
                        ui.add(DragValue::new(&mut a.origin[1]).speed(0.5).prefix("y "));
                    });
                    ui.end_row();
                    color(ui, "Color", &mut a.color, Some(Color::BLACK), false);
                    num(ui, "Width", &mut a.width, d::WIDTH, 0.05, 0.05..=20.0);
                });
                for (name, axis) in [("x axis", &mut a.x), ("y axis", &mut a.y)] {
                    ui.add_space(6.0);
                    ui.label(RichText::new(name).strong());
                    let ax = axis.get_or_insert_with(Axis::default);
                    Grid::new(name).num_columns(2).show(ui, |ui| {
                        flag(ui, "Show", &mut ax.show, true);
                        num(
                            ui,
                            "Length",
                            &mut ax.length,
                            d::AXIS_LENGTH,
                            0.5,
                            1.0..=10000.0,
                        );
                        flag(ui, "Arrow", &mut ax.arrow, true);
                        opt_text(ui, "Label", &mut ax.label, "$t$");
                        flag(ui, "Ticks", &mut ax.ticks, false);
                        num(
                            ui,
                            "Tick spacing",
                            &mut ax.tick_spacing,
                            d::TICK_SPACING,
                            0.5,
                            0.5..=1000.0,
                        );
                        num(
                            ui,
                            "Tick length",
                            &mut ax.tick_length,
                            d::TICK_LENGTH,
                            0.1,
                            0.0..=50.0,
                        );
                    });
                    if *ax == Axis::default() {
                        *axis = None;
                    }
                }
            }
            [ObjRef::Shape(i)] => {
                ui.label(RichText::new("Shape").strong());
                let s = &mut self.doc.shapes[*i];
                Grid::new("shape").num_columns(2).show(ui, |ui| {
                    class_choice(ui, &mut s.class, &styles);
                    color(ui, "Stroke color", &mut s.color, Some(Color::BLACK), false);
                    color(ui, "Fill", &mut s.fill, None, true);
                    num(ui, "Width", &mut s.width, d::WIDTH, 0.05, 0.0..=20.0);
                    num(
                        ui,
                        "Rotation (°)",
                        &mut s.rotation,
                        0.0,
                        1.0,
                        -360.0..=360.0,
                    );
                });
            }
            many => {
                ui.label(format!("{} objects selected.", many.len()));
                ui.weak("Use the palettes below to set color or stroke for all of them. Editing every shared property at once comes next (M2).");
                ui.add_space(6.0);
                let edges: Vec<usize> = many
                    .iter()
                    .filter_map(|o| {
                        if let ObjRef::Edge(i) = o {
                            Some(*i)
                        } else {
                            None
                        }
                    })
                    .collect();
                if !edges.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(format!("Width of {} edge(s)", edges.len()));
                        let mut w = self.doc.edges[edges[0]].width.unwrap_or(d::WIDTH);
                        if ui
                            .add(DragValue::new(&mut w).speed(0.05).range(0.05..=20.0))
                            .changed()
                        {
                            for &i in &edges {
                                self.doc.edges[i].width = Some(doc_ops::round(w));
                            }
                        }
                    });
                }
            }
        }
    }

    fn vertex_props(&mut self, ui: &mut Ui, i: usize, styles: &[String]) {
        ui.label(RichText::new("Vertex").strong());
        let st = self.doc.vertices[i]
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        Grid::new("vertex").num_columns(2).show(ui, |ui| {
            ui.label("Id");
            let cur = self.doc.vertices[i].id.clone();
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
            let v = &mut self.doc.vertices[i];
            ui.label("Position");
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut v.at[0]).speed(0.5).prefix("x "));
                ui.add(DragValue::new(&mut v.at[1]).speed(0.5).prefix("y "));
            });
            ui.end_row();
            class_choice(ui, &mut v.class, styles);
            let kind = v.kind.or(st.kind).unwrap_or(VertexKind::None);
            choice(
                ui,
                "Kind",
                &mut v.kind,
                st.kind.unwrap_or(VertexKind::None),
                &KINDS,
            );
            num(
                ui,
                "Size",
                &mut v.size,
                st.size.unwrap_or(d::vertex_size(kind)),
                0.1,
                0.0..=100.0,
            );
            color(
                ui,
                "Color",
                &mut v.color,
                Some(
                    st.color
                        .as_deref()
                        .and_then(|c| Color::parse(c).ok())
                        .unwrap_or(Color::BLACK),
                ),
                false,
            );
            color(ui, "Fill", &mut v.fill, None, true);
            num(
                ui,
                "Line width",
                &mut v.width,
                st.width.unwrap_or(d::WIDTH),
                0.05,
                0.05..=20.0,
            );
            num(
                ui,
                "Rotation (°)",
                &mut v.rotation,
                0.0,
                1.0,
                -360.0..=360.0,
            );
            opt_text(ui, "Label", &mut v.label, "$V$");
            choice(
                ui,
                "Label anchor",
                &mut v.label_anchor,
                Anchor::SouthWest,
                &ANCHORS,
            );
        });
    }

    fn edge_props(&mut self, ui: &mut Ui, i: usize, styles: &[String]) {
        let e = &self.doc.edges[i];
        ui.label(RichText::new(format!("Edge  {} → {}", e.from, e.to)).strong());
        let st = e
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        let e = &mut self.doc.edges[i];
        Grid::new("edge").num_columns(2).show(ui, |ui| {
            class_choice(ui, &mut e.class, styles);
            let stroke = e.stroke.or(st.stroke).unwrap_or(Stroke::Plain);
            choice(
                ui,
                "Stroke",
                &mut e.stroke,
                st.stroke.unwrap_or(Stroke::Plain),
                &STROKES,
            );
            color(
                ui,
                "Color",
                &mut e.color,
                Some(
                    st.color
                        .as_deref()
                        .and_then(|c| Color::parse(c).ok())
                        .unwrap_or(Color::BLACK),
                ),
                false,
            );
            num(
                ui,
                "Width",
                &mut e.width,
                st.width.unwrap_or(d::WIDTH),
                0.05,
                0.05..=20.0,
            );
            match stroke {
                Stroke::Dashed | Stroke::Dotted => {
                    let [on, off] = e.dash.or(st.dash).unwrap_or(d::DASH);
                    let (mut on2, mut off2) = (Some(on), Some(off));
                    num(ui, "Dash length", &mut on2, on, 0.1, 0.1..=50.0);
                    num(ui, "Dash gap", &mut off2, off, 0.1, 0.1..=50.0);
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
                    );
                    num(
                        ui,
                        "Wavelength",
                        &mut e.wavelength,
                        st.wavelength.unwrap_or(w),
                        0.1,
                        0.5..=100.0,
                    );
                }
                Stroke::Plain => {}
            }
            if matches!(stroke, Stroke::Plain | Stroke::Dashed | Stroke::Dotted) {
                flag(ui, "Double", &mut e.double, st.double.unwrap_or(false));
                num(
                    ui,
                    "Double gap",
                    &mut e.double_sep,
                    st.double_sep.unwrap_or(d::DOUBLE_SEP),
                    0.1,
                    0.1..=20.0,
                );
            }
            choice(
                ui,
                "Arrow",
                &mut e.arrow,
                st.arrow.unwrap_or(ArrowDir::None),
                &ARROWS,
            );
            num(
                ui,
                "Arrow position",
                &mut e.arrow_pos,
                st.arrow_pos.unwrap_or(d::ARROW_POS),
                0.01,
                0.0..=1.0,
            );
            num(
                ui,
                "Arrow length",
                &mut e.arrow_length,
                st.arrow_length.unwrap_or(d::ARROW_LENGTH),
                0.1,
                0.5..=100.0,
            );
            num(
                ui,
                "Arrow width",
                &mut e.arrow_width,
                st.arrow_width.unwrap_or(d::ARROW_WIDTH),
                0.1,
                0.5..=100.0,
            );
            if e.from == e.to {
                let lp = e.loop_.get_or_insert(LoopSpec {
                    radius: None,
                    angle: None,
                });
                num(
                    ui,
                    "Loop radius",
                    &mut lp.radius,
                    d::LOOP_RADIUS,
                    0.2,
                    1.0..=500.0,
                );
                num(
                    ui,
                    "Loop angle (°)",
                    &mut lp.angle,
                    d::LOOP_ANGLE,
                    1.0,
                    -360.0..=360.0,
                );
                if lp.radius.is_none() && lp.angle.is_none() {
                    e.loop_ = None;
                }
            } else if e.controls.is_none() {
                num(ui, "Bend (°)", &mut e.bend, 0.0, 1.0, -179.0..=179.0);
                if e.bend == Some(0.0) {
                    e.bend = None;
                }
            }
            opt_text(ui, "Label", &mut e.label, "$p$");
            choice(
                ui,
                "Label side",
                &mut e.label_side,
                st.label_side.unwrap_or(Side::Left),
                &SIDES,
            );
            num(
                ui,
                "Label position",
                &mut e.label_pos,
                st.label_pos.unwrap_or(d::LABEL_POS),
                0.01,
                0.0..=1.0,
            );
            num(
                ui,
                "Label offset",
                &mut e.label_offset,
                st.label_offset.unwrap_or(d::LABEL_OFFSET),
                0.1,
                -50.0..=50.0,
            );
        });
        let e = &mut self.doc.edges[i];
        if e.from != e.to {
            ui.add_space(4.0);
            if e.controls.is_none() {
                if ui
                    .button("Make Bézier curve")
                    .on_hover_text("Replace the arc by a curve with two control points (Edit tool)")
                    .clicked()
                {
                    let (a, b) = (
                        doc_ops::vertex_index(&self.doc, &self.doc.edges[i].from),
                        doc_ops::vertex_index(&self.doc, &self.doc.edges[i].to),
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
            } else if ui.button("Make straight").clicked() {
                e.controls = None;
            }
        }
    }

    fn text_props(&mut self, ui: &mut Ui, i: usize, styles: &[String]) {
        ui.label(RichText::new("Text").strong());
        let focus = std::mem::take(&mut self.focus_text_content);
        let t = &mut self.doc.texts[i];
        let st = t
            .class
            .as_ref()
            .and_then(|c| self.doc.styles.get(c))
            .cloned()
            .unwrap_or_default();
        ui.label("Content (text with $LaTeX math$)");
        let r = ui.add(
            TextEdit::multiline(&mut t.content)
                .desired_rows(2)
                .desired_width(f32::INFINITY)
                .code_editor(),
        );
        if focus {
            r.request_focus();
        }
        Grid::new("text").num_columns(2).show(ui, |ui| {
            ui.label("Position");
            ui.horizontal(|ui| {
                ui.add(DragValue::new(&mut t.at[0]).speed(0.5).prefix("x "));
                ui.add(DragValue::new(&mut t.at[1]).speed(0.5).prefix("y "));
            });
            ui.end_row();
            class_choice(ui, &mut t.class, styles);
            choice(ui, "Anchor", &mut t.anchor, Anchor::Center, &ANCHORS);
            num(
                ui,
                "Size (pt)",
                &mut t.size,
                st.text_size.unwrap_or(d::TEXT_SIZE),
                0.1,
                1.0..=200.0,
            );
            color(
                ui,
                "Color",
                &mut t.color,
                Some(
                    st.color
                        .as_deref()
                        .and_then(|c| Color::parse(c).ok())
                        .unwrap_or(Color::BLACK),
                ),
                false,
            );
            num(
                ui,
                "Rotation (°)",
                &mut t.rotation,
                0.0,
                1.0,
                -360.0..=360.0,
            );
        });
    }

    // ---- Objects ----------------------------------------------------------------

    fn objects(&mut self, ui: &mut Ui) {
        let shift = ui.input(|i| i.modifiers.shift);
        let mut clicked: Option<ObjRef> = None;
        let mut section =
            |ui: &mut Ui, title: &str, items: Vec<(ObjRef, String)>, sel: &doc_ops::Selection| {
                if items.is_empty() {
                    return;
                }
                egui::CollapsingHeader::new(format!("{title} ({})", items.len()))
                    .default_open(true)
                    .show(ui, |ui| {
                        for (o, name) in items {
                            if ui.selectable_label(sel.contains(&o), name).clicked() {
                                clicked = Some(o);
                            }
                        }
                    });
            };
        let d = &self.doc;
        let kind_name = |v: &Vertex| {
            let st = v.class.as_ref().and_then(|c| d.styles.get(c));
            match v
                .kind
                .or(st.and_then(|s| s.kind))
                .unwrap_or(VertexKind::None)
            {
                VertexKind::None => "point",
                VertexKind::Dot => "dot",
                VertexKind::Cross => "cross",
                VertexKind::Ocross => "⊗",
                VertexKind::Circle => "circle",
                VertexKind::Square => "square",
            }
        };
        section(
            ui,
            "Vertices",
            d.vertices
                .iter()
                .enumerate()
                .map(|(i, v)| (ObjRef::Vertex(i), format!("{}  ({})", v.id, kind_name(v))))
                .collect(),
            &self.selection,
        );
        section(
            ui,
            "Edges",
            d.edges
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    let mut s = format!("{} → {}", e.from, e.to);
                    if let Some(c) = &e.class {
                        s.push_str(&format!("  [{c}]"));
                    }
                    if let Some(l) = &e.label {
                        s.push_str(&format!("  {l}"));
                    }
                    (ObjRef::Edge(i), s)
                })
                .collect(),
            &self.selection,
        );
        section(
            ui,
            "Texts",
            d.texts
                .iter()
                .enumerate()
                .map(|(i, t)| (ObjRef::Text(i), t.content.clone()))
                .collect(),
            &self.selection,
        );
        section(
            ui,
            "Axes",
            d.axes
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    (
                        ObjRef::Axes(i),
                        format!("axes at ({}, {})", a.origin[0], a.origin[1]),
                    )
                })
                .collect(),
            &self.selection,
        );
        section(
            ui,
            "Shapes",
            d.shapes
                .iter()
                .enumerate()
                .map(|(i, s)| (ObjRef::Shape(i), format!("{:?}", s.kind).to_lowercase()))
                .collect(),
            &self.selection,
        );
        if let Some(o) = clicked {
            if shift {
                if !self.selection.remove(&o) {
                    self.selection.insert(o);
                }
            } else {
                self.selection = [o].into();
            }
        }
        if self.doc.vertices.is_empty()
            && self.doc.texts.is_empty()
            && self.doc.axes.is_empty()
            && self.doc.shapes.is_empty()
        {
            ui.weak("The diagram is empty. Draw with the Edge tool (B) or the Vertex tool (P).");
        }
    }

    // ---- Source -----------------------------------------------------------------

    fn source_tab(&mut self, ui: &mut Ui) {
        ui.weak("The .gold text of this diagram. Edits apply as soon as the text is valid.");
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
        egui::Window::new("Keyboard and mouse")
            .open(&mut open)
            .default_size([520.0, 520.0])
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    let mut group = "";
                    for b in BINDINGS.iter().filter(|b| !b.keys.is_empty()) {
                        if b.group != group {
                            group = b.group;
                            ui.add_space(6.0);
                            ui.label(RichText::new(group).strong().color(ACCENT));
                        }
                        ui.horizontal(|ui| {
                            ui.add_sized(
                                [110.0, 16.0],
                                egui::Label::new(RichText::new(b.keys).monospace()),
                            );
                            ui.label(b.desc);
                        });
                    }
                    ui.add_space(6.0);
                    ui.label(RichText::new("Mouse").strong().color(ACCENT));
                    for (w, input, desc, _) in MOUSE {
                        ui.horizontal(|ui| {
                            ui.add_sized([110.0, 16.0], egui::Label::new(RichText::new(*w).weak()));
                            ui.label(RichText::new(*input).monospace());
                            ui.label(*desc);
                        });
                    }
                    ui.add_space(6.0);
                    ui.weak("Ctrl means Cmd on macOS. The full manual is docs/MANUAL.md.");
                });
            });
        self.show_shortcuts = open;
    }

    pub(crate) fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new("About Goldstone")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("Goldstone {}", env!("CARGO_PKG_VERSION")));
                ui.label("Many-body diagrams: coupled cluster, IMSRG, MBPT.");
                ui.weak("MIT license.");
            });
        self.show_about = open;
    }
}

//! Document tabs and right-click menus (canvas, toolbox, tabs).

use crate::i18n::t;
use std::collections::HashMap;

use egui::{DragValue, Ui};
use goldstone_core::scene::defaults as d;
use goldstone_core::shapes::BoolOp;

use crate::app::{GoldstoneApp, Tool};
use crate::doc_ops;
use crate::keys::{self, Action, Os, Same};
use crate::panels::{ALIGN, ARROWS, KINDS, STROKES};

/// Menu entries that run an action, with icon and shortcut; the chosen
/// action is run after the menu is drawn.
pub(crate) struct Items {
    imgs: HashMap<&'static str, egui::Image<'static>>,
    os: Os,
    pub chosen: Option<Action>,
}

impl Items {
    pub fn new(app: &mut GoldstoneApp, ctx: &egui::Context, icons: &[&'static str]) -> Self {
        let mut imgs = HashMap::new();
        for name in icons {
            if let Some(i) = app.icon(ctx, name, 16.0) {
                imgs.insert(*name, i);
            }
        }
        Self {
            imgs,
            os: app.os,
            chosen: None,
        }
    }

    pub fn item(&mut self, ui: &mut Ui, icon: Option<&str>, label: &str, a: Action) {
        let label = crate::i18n::tr(label);
        let label = label.as_str();
        let sc = keys::shortcut(a, self.os);
        let b = match icon.and_then(|i| self.imgs.get(i)) {
            Some(img) => egui::Button::image_and_text(img.clone(), label),
            None => egui::Button::new(label),
        };
        if ui.add(b.shortcut_text(sc)).clicked() {
            self.chosen = Some(a);
            ui.close();
        }
    }
}

fn tool_action(t: Tool) -> Action {
    match t {
        Tool::Select => Action::ToolSelect,
        Tool::Edit => Action::ToolEdit,
        Tool::Edge => Action::ToolEdge,
        Tool::Arc => Action::ToolArc,
        Tool::Vertex => Action::ToolVertex,
        Tool::Cross => Action::ToolCross,
        Tool::Text => Action::ToolText,
        Tool::Rect => Action::ToolRect,
        Tool::Ellipse => Action::ToolEllipse,
    }
}

const ICONS: &[&str] = &[
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
    "align-center-x",
    "align-right",
    "align-top",
    "align-center-y",
    "align-bottom",
    "distribute-x",
    "distribute-y",
    "select",
    "edit",
    "edge",
    "arc",
    "vertex",
    "cross",
    "text",
    "rect",
    "ellipse",
];

fn check(on: bool, s: &str) -> String {
    format!("{} {s}", if on { "☑" } else { "☐" })
}

impl GoldstoneApp {
    /// The row of document tabs above the canvas.
    pub(crate) fn tab_bar(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let titles = self.tab_titles();
        let (mut switch, mut close, mut new) = (None, None, false);
        let close_key = keys::shortcut(Action::CloseTab, self.os);
        egui::Frame::NONE
            .fill(ui.visuals().panel_fill)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::horizontal()
                    .id_salt("doc-tabs")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (i, (name, dirty)) in titles.iter().enumerate() {
                                let text = format!("{}{name}", if *dirty { "*" } else { "" });
                                let tip = self
                                    .tab_path(i)
                                    .map_or(name.clone(), |p| p.display().to_string());
                                let r = ui
                                    .selectable_label(i == self.active, text)
                                    .on_hover_text(tip);
                                if r.clicked() {
                                    switch = Some(i);
                                }
                                if r.middle_clicked() {
                                    close = Some(i);
                                }
                                r.context_menu(|ui| {
                                    if ui.button(t("Close")).clicked() {
                                        close = Some(i);
                                        ui.close();
                                    }
                                    if ui.button(t("New Diagram")).clicked() {
                                        new = true;
                                        ui.close();
                                    }
                                });
                                if ui
                                    .small_button(t("×"))
                                    .on_hover_text(crate::i18n::tr(&format!("Close ({close_key})")))
                                    .clicked()
                                {
                                    close = Some(i);
                                }
                                ui.add_space(6.0);
                            }
                            if ui
                                .small_button(t("+"))
                                .on_hover_text(t("New diagram"))
                                .clicked()
                            {
                                new = true;
                            }
                        });
                    });
            });
        if let Some(i) = switch {
            self.switch_tab(i);
        }
        if let Some(i) = close {
            self.close_tab(i, &ctx);
        }
        if new {
            self.do_action(Action::New, &ctx);
        }
    }

    /// Options of `tool` for new objects, as menu entries.
    pub(crate) fn tool_options_menu(&mut self, ui: &mut Ui, tool: Tool) {
        match tool {
            Tool::Edge | Tool::Arc => {
                let styles: Vec<String> = self.doc.styles.keys().cloned().collect();
                let class = self.opts.class.clone().unwrap_or_else(|| "(none)".into());
                ui.menu_button(crate::i18n::tr(&format!("Class: {class}")), |ui| {
                    if ui
                        .selectable_label(self.opts.class.is_none(), t("(none)"))
                        .clicked()
                    {
                        self.opts.class = None;
                        ui.close();
                    }
                    for s in &styles {
                        let on = self.opts.class.as_deref() == Some(s.as_str());
                        if ui.selectable_label(on, s).clicked() {
                            self.opts.class = Some(s.clone());
                            ui.close();
                        }
                    }
                });
                let stroke = STROKES
                    .iter()
                    .find(|(s, _)| *s == self.opts.stroke)
                    .map_or("", |(_, n)| *n);
                ui.menu_button(crate::i18n::tr(&format!("Stroke: {stroke}")), |ui| {
                    for (s, n) in STROKES {
                        if ui.selectable_label(self.opts.stroke == s, n).clicked() {
                            self.opts.stroke = s;
                            ui.close();
                        }
                    }
                });
                let arrow = match self.opts.arrow {
                    None => "from class",
                    Some(a) => ARROWS.iter().find(|(x, _)| *x == a).map_or("", |(_, n)| *n),
                };
                ui.menu_button(crate::i18n::tr(&format!("Arrow: {arrow}")), |ui| {
                    if ui
                        .selectable_label(self.opts.arrow.is_none(), t("from class"))
                        .clicked()
                    {
                        self.opts.arrow = None;
                        ui.close();
                    }
                    for (a, n) in ARROWS {
                        if ui.selectable_label(self.opts.arrow == Some(a), n).clicked() {
                            self.opts.arrow = Some(a);
                            ui.close();
                        }
                    }
                });
                ui.checkbox(&mut self.opts.double, t("Double line"));
                if self.opts.double {
                    ui.horizontal(|ui| {
                        let l = ui.label(t("Gap")).id;
                        let mut g = self.opts.double_sep.unwrap_or(d::DOUBLE_SEP);
                        if ui
                            .add(
                                DragValue::new(&mut g)
                                    .speed(0.1)
                                    .range(0.1..=20.0)
                                    .suffix(" pt"),
                            )
                            .labelled_by(l)
                            .changed()
                        {
                            self.opts.double_sep = Some(doc_ops::round(g));
                        }
                    });
                }
            }
            Tool::Vertex => {
                let kind = KINDS
                    .iter()
                    .find(|(k, _)| *k == self.opts.vertex_kind)
                    .map_or("", |(_, n)| *n);
                ui.menu_button(crate::i18n::tr(&format!("Kind: {kind}")), |ui| {
                    for (k, n) in KINDS {
                        if ui.selectable_label(self.opts.vertex_kind == k, n).clicked() {
                            self.opts.vertex_kind = k;
                            ui.close();
                        }
                    }
                });
            }
            Tool::Rect | Tool::Ellipse => {
                ui.weak(
                    self.os
                        .fill("Hold {cmd}while dragging for a square / circle"),
                );
            }
            _ => {
                ui.weak(self.os.fill(tool.hint()));
            }
        }
        ui.separator();
        ui.checkbox(&mut self.show_grid, t("Snap to grid"));
        ui.horizontal(|ui| {
            let l = ui.label(t("Grid spacing")).id;
            let mut g = self.grid();
            if ui
                .add(DragValue::new(&mut g).range(1.0..=100.0).suffix(" pt"))
                .labelled_by(l)
                .changed()
            {
                self.doc.canvas.grid = Some(g);
            }
        });
    }

    /// Right-click menu of a toolbox button.
    pub(crate) fn tool_menu(&mut self, ui: &mut Ui, tool: Tool) {
        let key = tool.key();
        if ui
            .add(
                egui::Button::new(crate::i18n::tr(&format!("Use {}", tool.name())))
                    .shortcut_text(key),
            )
            .clicked()
        {
            self.set_tool(tool);
            ui.close();
        }
        ui.separator();
        self.tool_options_menu(ui, tool);
    }

    /// Right-click menu of the canvas: commands for the selection, tools
    /// and their options, view.
    pub(crate) fn canvas_menu(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let mut m = Items::new(self, &ctx, ICONS);
        let any = !self.selection.is_empty();
        let several = self.selection.len() > 1;
        m.item(ui, Some("undo"), "Undo", Action::Undo);
        m.item(ui, Some("redo"), "Redo", Action::Redo);
        ui.separator();
        if any {
            m.item(ui, None, "Cut", Action::Cut);
            m.item(ui, None, "Copy", Action::Copy);
        }
        m.item(ui, None, "Paste", Action::Paste);
        if any {
            m.item(ui, None, "Paste In Place", Action::PasteInPlace);
            m.item(ui, None, "Duplicate", Action::Duplicate);
            m.item(ui, None, "Delete", Action::Delete);
            ui.separator();
            if several {
                m.item(ui, Some("group"), "Group", Action::Group);
            }
            m.item(ui, Some("ungroup"), "Ungroup", Action::Ungroup);
            ui.menu_button(t("Transform"), |ui| {
                m.item(
                    ui,
                    Some("rotate-ccw"),
                    "Rotate 90° Counterclockwise",
                    Action::Rotate(90),
                );
                m.item(
                    ui,
                    Some("rotate-cw"),
                    "Rotate 90° Clockwise",
                    Action::Rotate(-90),
                );
                m.item(ui, Some("flip-h"), "Flip Horizontal", Action::FlipH);
                m.item(ui, Some("flip-v"), "Flip Vertical", Action::FlipV);
            });
            if several {
                ui.menu_button(t("Align & Distribute"), |ui| {
                    for (icon, label, op) in ALIGN {
                        m.item(ui, Some(icon), label, Action::Align(*op));
                    }
                });
            }
            ui.menu_button(t("Path"), |ui| {
                m.item(ui, Some("to-path"), "Object to Path", Action::ObjectToPath);
                ui.separator();
                for (icon, label, op) in [
                    ("union", "Union", BoolOp::Union),
                    ("difference", "Difference", BoolOp::Difference),
                    ("intersection", "Intersection", BoolOp::Intersection),
                    ("exclusion", "Exclusion", BoolOp::Exclusion),
                ] {
                    m.item(ui, Some(icon), label, Action::Bool(op));
                }
            });
            ui.menu_button(t("Select Same"), |ui| {
                for (label, same) in [
                    ("Fill and Stroke", Same::FillAndStroke),
                    ("Stroke Color", Same::StrokeColor),
                    ("Stroke Style", Same::StrokeStyle),
                    ("Class", Same::Class),
                    ("Object Type", Same::Kind),
                ] {
                    m.item(ui, None, label, Action::SelectSame(same));
                }
            });
            m.item(ui, None, "Fill & Stroke…", Action::ShowFillStroke);
        } else {
            m.item(ui, None, "Select All", Action::SelectAll);
        }
        ui.separator();
        let current = self.tool;
        ui.menu_button(t("Tools"), |ui| {
            for t in Tool::ALL {
                let label = if t == current {
                    format!("● {}", t.name())
                } else {
                    t.name().to_string()
                };
                m.item(ui, Some(t.icon()), &label, tool_action(t));
            }
        });
        let mut options = false;
        ui.menu_button(
            crate::i18n::tr(&format!("{} Tool Options", current.name())),
            |ui| {
                options = true;
                self.tool_options_menu(ui, current);
            },
        );
        ui.menu_button(t("View"), |ui| {
            m.item(ui, None, "Zoom In", Action::ZoomIn);
            m.item(ui, None, "Zoom Out", Action::ZoomOut);
            m.item(ui, None, "Zoom 1:1", Action::Zoom1);
            if any {
                m.item(ui, None, "Zoom Selection", Action::ZoomSelection);
            }
            m.item(ui, None, "Zoom Drawing", Action::ZoomDrawing);
            ui.separator();
            m.item(ui, None, &check(self.show_grid, "Grid"), Action::ToggleGrid);
            m.item(
                ui,
                None,
                &check(self.show_rulers, "Rulers"),
                Action::ToggleRulers,
            );
            m.item(
                ui,
                None,
                &check(self.show_ids, "Object Ids and Coordinates"),
                Action::ToggleIds,
            );
        });
        ui.separator();
        m.item(ui, None, "CC Diagram from LaTeX…", Action::FromLatex);
        m.item(ui, None, "IMSRG Diagram from LaTeX…", Action::FromLatexSrg);
        m.item(ui, None, "Function Plot…", Action::FunctionPlot);
        if any {
            m.item(ui, None, "Add Selection to Templates…", Action::AddTemplate);
        }
        ui.menu_button(t("Templates"), |ui| self.templates_menu(ui));
        if let Some(a) = m.chosen {
            self.do_action(a, &ctx);
        }
    }
}

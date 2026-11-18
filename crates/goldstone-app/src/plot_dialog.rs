//! Function plots (Insert ▸ Function Plot…) and the LaTeX tab of the
//! right panel, where the equation or plot settings of a drawing made by
//! Goldstone are edited and the drawing follows as you type.

use crate::i18n::t;
use egui::{Color32, DragValue, RichText, TextEdit, Ui};
use goldstone_core::kurbo::Vec2;
use goldstone_core::model::{Document, PlotSource};
use goldstone_core::plot;

use crate::app::GoldstoneApp;
use crate::doc_ops;
use crate::keys::Action;

/// Insert ▸ Function Plot… dialog.
pub struct PlotDialog {
    pub open: bool,
    pub src: PlotSource,
    status: Result<String, String>,
    for_src: Option<PlotSource>,
}

impl Default for PlotDialog {
    fn default() -> Self {
        Self {
            open: false,
            src: PlotSource {
                functions: "sin(x); cos(x)".into(),
                ..PlotSource::default()
            },
            status: Ok(String::new()),
            for_src: None,
        }
    }
}

/// What the LaTeX tab is editing: the generated group (by id) and the
/// text being typed.
#[derive(Default)]
pub struct CodeState {
    group: Option<String>,
    latex: String,
    plot: PlotSource,
    error: Option<String>,
    warnings: Vec<String>,
}

const OK: Color32 = Color32::from_rgb(40, 150, 60);
const BAD: Color32 = Color32::from_rgb(190, 30, 30);

fn summary(p: &plot::Plot) -> String {
    let curves = p
        .doc
        .shapes
        .iter()
        .filter(|s| s.id.as_deref().is_some_and(|i| i.starts_with("plot_f")))
        .count();
    format!("{curves} curve(s)")
}

/// The form for plot settings; true when something changed.
fn plot_form(ui: &mut Ui, src: &mut PlotSource) -> bool {
    let mut changed = false;
    ui.label(t(
        "Functions of x (one per line or separated by ;), plain or LaTeX:",
    ));
    changed |= ui
        .add(
            TextEdit::multiline(&mut src.functions)
                .code_editor()
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
        .changed();
    egui::Grid::new("plot-form").num_columns(2).show(ui, |ui| {
        ui.label(t("x from"));
        ui.horizontal(|ui| {
            changed |= ui
                .add(TextEdit::singleline(&mut src.x[0]).desired_width(70.0))
                .changed();
            ui.label(t("to"));
            changed |= ui
                .add(TextEdit::singleline(&mut src.x[1]).desired_width(70.0))
                .changed();
        });
        ui.end_row();
        ui.label(t("y"));
        ui.horizontal(|ui| {
            let mut auto = src.y.is_none();
            if ui.checkbox(&mut auto, t("automatic")).changed() {
                src.y = if auto {
                    None
                } else {
                    Some(["-1".into(), "1".into()])
                };
                changed = true;
            }
            if let Some(y) = &mut src.y {
                changed |= ui
                    .add(TextEdit::singleline(&mut y[0]).desired_width(50.0))
                    .changed();
                ui.label(t("to"));
                changed |= ui
                    .add(TextEdit::singleline(&mut y[1]).desired_width(50.0))
                    .changed();
            }
        });
        ui.end_row();
        ui.label(t("Size"));
        ui.horizontal(|ui| {
            changed |= ui
                .add(
                    DragValue::new(&mut src.size[0])
                        .range(20.0..=2000.0)
                        .suffix(" pt"),
                )
                .changed();
            ui.label(t("×"));
            changed |= ui
                .add(
                    DragValue::new(&mut src.size[1])
                        .range(20.0..=2000.0)
                        .suffix(" pt"),
                )
                .changed();
        });
        ui.end_row();
        ui.label(t("Axis labels"));
        ui.horizontal(|ui| {
            let mut x = src.x_label.clone().unwrap_or_else(|| "$x$".into());
            let mut y = src.y_label.clone().unwrap_or_else(|| "$y$".into());
            if ui
                .add(TextEdit::singleline(&mut x).desired_width(60.0))
                .changed()
            {
                src.x_label = Some(x);
                changed = true;
            }
            if ui
                .add(TextEdit::singleline(&mut y).desired_width(60.0))
                .changed()
            {
                src.y_label = Some(y);
                changed = true;
            }
        });
        ui.end_row();
        ui.label(t("Curve labels"));
        ui.horizontal(|ui| {
            for (v, name) in [(None, "auto"), (Some(true), "on"), (Some(false), "off")] {
                if ui.selectable_label(src.labels == v, name).clicked() {
                    src.labels = v;
                    changed = true;
                }
            }
        });
        ui.end_row();
    });
    ui.weak(t("Ranges may use pi, e.g. -2pi or \\pi/2. Examples: x^3 - 2x;  e^{-x^2/2};  \\frac{1}{1+x^2};  |x|;  sqrt(x)."));
    changed
}

fn status_line(ui: &mut Ui, r: &Result<String, String>) {
    match r {
        Ok(s) if !s.is_empty() => {
            ui.label(RichText::new(crate::i18n::tr(&format!("✓ {s}"))).color(OK));
        }
        Ok(_) => {}
        Err(e) => {
            ui.label(RichText::new(e).color(BAD));
        }
    }
}

impl GoldstoneApp {
    pub(crate) fn plot_window(&mut self, ctx: &egui::Context) {
        if !self.plot.open {
            return;
        }
        let mut open = true;
        let mut insert: Option<bool> = None;
        egui::Window::new(t("Function Plot"))
            .open(&mut open)
            .default_width(460.0)
            .show(ctx, |ui| {
                plot_form(ui, &mut self.plot.src);
                if self.plot.for_src.as_ref() != Some(&self.plot.src) {
                    self.plot.for_src = Some(self.plot.src.clone());
                    self.plot.status = plot::from_source(&self.plot.src).map(|p| {
                        let mut s = summary(&p);
                        for w in p.warnings {
                            s.push_str(&format!("; {w}"));
                        }
                        s
                    });
                }
                status_line(ui, &self.plot.status);
                let ok = self.plot.status.is_ok();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(ok, egui::Button::new(t("Insert")))
                        .on_hover_text(t("Add the plot to the right of the drawing"))
                        .clicked()
                    {
                        insert = Some(false);
                    }
                    if ui
                        .add_enabled(ok, egui::Button::new(t("In New Tab")))
                        .clicked()
                    {
                        insert = Some(true);
                    }
                });
                ui.weak(t("The plot is a group of ordinary objects. Select it and edit its settings in the LaTeX tab of the right panel; the plot is redrawn as you type."));
            });
        if let Some(new_tab) = insert
            && let Ok(p) = plot::from_source(&self.plot.src)
        {
            if new_tab {
                self.insert_in_new_tab(p.doc);
            } else {
                self.insert_doc(p.doc);
            }
            self.tab = crate::app::Tab::Code;
            self.message = Some(format!("Plotted {}", self.plot.src.functions.trim()));
            open = false;
        }
        self.plot.open = open;
    }

    /// The generated group the selection belongs to (drawn from LaTeX or a
    /// function plot).
    fn selected_generated(&self) -> Option<usize> {
        let o = *self.selection.first()?;
        let g = doc_ops::group_of(&self.doc, o)?;
        let grp = &self.doc.groups[g];
        (grp.latex.is_some() || grp.plot.is_some()).then_some(g)
    }

    /// Replaces the objects of group `g` by `part` (which holds one group),
    /// keeping the lower-left corner and the group's id.
    fn replace_generated(&mut self, g: usize, mut part: Document) {
        let old = doc_ops::group_members(&self.doc, g);
        let corner = doc_ops::bounds(&doc_ops::selection_points(&self.doc, &old));
        let id = self.doc.groups[g].id.clone();
        self.doc.groups.remove(g);
        doc_ops::delete(&mut self.doc, &old);
        if let Some(pg) = part.groups.first_mut() {
            pg.id = id;
        }
        let new = doc_ops::merge(&mut self.doc, part);
        if let (Some(a), Some(b)) = (
            corner,
            doc_ops::bounds(&doc_ops::selection_points(&self.doc, &new)),
        ) {
            doc_ops::translate(&mut self.doc, &new, Vec2::new(a.x0 - b.x0, a.y0 - b.y0));
        }
        self.selection = new;
    }

    /// The LaTeX tab: edit the source of the selected generated drawing.
    pub(crate) fn code_tab(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let Some(g) = self.selected_generated() else {
            self.code.group = None;
            ui.weak(t("Select a diagram drawn from LaTeX, or a function plot, to edit its equation here; the drawing is redrawn as you type."));
            ui.add_space(8.0);
            if ui.button(t("CC Diagram from LaTeX…")).clicked() {
                self.do_action(Action::FromLatex, &ctx);
            }
            if ui.button(t("IMSRG Diagram from LaTeX…")).clicked() {
                self.do_action(Action::FromLatexSrg, &ctx);
            }
            if ui.button(t("Function Plot…")).clicked() {
                self.do_action(Action::FunctionPlot, &ctx);
            }
            return;
        };
        let grp = self.doc.groups[g].clone();
        if self.code.group.as_deref() != Some(grp.id.as_str()) {
            self.code = CodeState {
                group: Some(grp.id.clone()),
                latex: grp.latex.clone().unwrap_or_default(),
                plot: grp.plot.clone().unwrap_or_default(),
                error: None,
                warnings: Vec::new(),
            };
        }
        let mut redraw: Option<Result<Document, String>> = None;
        if grp.latex.is_some() {
            let rule = goldstone_core::expr::Rule::from_stored(grp.latex_rule.as_deref());
            let kind = if rule == goldstone_core::expr::Rule::Srg {
                "IMSRG"
            } else {
                "CC"
            };
            ui.label(
                RichText::new(crate::i18n::tr(&format!(
                    "{kind} diagram from LaTeX (group {})",
                    grp.id
                )))
                .strong(),
            );
            let r = ui.add(
                TextEdit::multiline(&mut self.code.latex)
                    .code_editor()
                    .desired_rows(6)
                    .desired_width(f32::INFINITY),
            );
            if r.changed() {
                redraw = Some(
                    goldstone_core::expr::from_latex_editable_with(&self.code.latex, rule).map(
                        |d| {
                            self.code.warnings = d.warnings;
                            d.doc
                        },
                    ),
                );
            }
        } else {
            ui.label(
                RichText::new(crate::i18n::tr(&format!(
                    "Function plot (group {})",
                    grp.id
                )))
                .strong(),
            );
            if plot_form(ui, &mut self.code.plot) {
                redraw = Some(plot::from_source(&self.code.plot).map(|p| {
                    self.code.warnings = p.warnings;
                    p.doc
                }));
            }
        }
        match redraw {
            Some(Ok(part)) => {
                self.code.error = None;
                self.replace_generated(g, part);
            }
            Some(Err(e)) => self.code.error = Some(e),
            None => {}
        }
        match &self.code.error {
            Some(e) => {
                ui.label(RichText::new(e).color(BAD));
                ui.weak(t("The drawing shows the last version that worked."));
            }
            None => {
                ui.label(RichText::new(t("✓ up to date")).color(OK));
            }
        }
        for w in &self.code.warnings {
            ui.label(
                RichText::new(crate::i18n::tr(&format!("⚠ {w}")))
                    .color(Color32::from_rgb(180, 120, 0)),
            );
        }
        ui.add_space(8.0);
        ui.weak(t(
            "Ungroup the drawing to edit its objects one by one (it then forgets its source).",
        ));
    }
}

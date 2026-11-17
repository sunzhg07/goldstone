//! The editor: state, tools, canvas and file handling.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use egui::{
    Align2, Color32, CornerRadius, FontId, Key, PointerButton, Pos2, Sense, Stroke as EStroke,
    StrokeKind,
};
use goldstone_core::kurbo::{BezPath, PathEl, Point, Rect, Vec2};
use goldstone_core::model::{ArrowDir, Document, LoopSpec, Stroke, VertexKind};
use goldstone_core::scene::{self, ObjRef, Scene};
use goldstone_core::{geom, io};
use goldstone_export::{Format, Labels};

use crate::doc_ops::{self, Selection};
use crate::keys::{Action, BINDINGS, Step, Trigger};
use crate::view::{self, TextBoxes, View};

pub const ACCENT: Color32 = Color32::from_rgb(30, 120, 230);
const DESK: Color32 = Color32::from_rgb(222, 222, 226);
const PICK_PX: f64 = 6.0;
const SNAP_PX: f64 = 9.0;
const HANDLE_PX: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Edit,
    Edge,
    Vertex,
    Cross,
    Text,
}

impl Tool {
    pub const ALL: [Tool; 6] = [
        Tool::Select,
        Tool::Edit,
        Tool::Edge,
        Tool::Vertex,
        Tool::Cross,
        Tool::Text,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Edit => "Edit",
            Tool::Edge => "Edge",
            Tool::Vertex => "Vertex",
            Tool::Cross => "Cross",
            Tool::Text => "Text",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "S",
            Tool::Edit => "N",
            Tool::Edge => "B",
            Tool::Vertex => "P",
            Tool::Cross => "X",
            Tool::Text => "T",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => {
                "Click to select, Shift+click to add/remove, drag to move; drag on empty canvas to select a region"
            }
            Tool::Edit => {
                "Drag the round handle to bend a line or resize a loop, the diamond to move the arrow"
            }
            Tool::Edge => {
                "Click, click: line (ends snap to vertices). Drag at the second point: arc. Same vertex twice: loop. Esc cancels"
            }
            Tool::Vertex => "Click to place a vertex",
            Tool::Cross => "Click to place a cross; drag from a vertex to draw ---X",
            Tool::Text => "Click to place a LaTeX label, then type it in Fill & Stroke",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    FillStroke,
    Objects,
    Source,
}

/// Defaults for objects created by the tools (shown in the tool controls bar).
#[derive(Debug, Clone)]
pub struct ToolOptions {
    pub stroke: Stroke,
    pub class: Option<String>,
    /// `None`: take the arrow from the class (or none).
    pub arrow: Option<ArrowDir>,
    pub vertex_kind: VertexKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EdgeEnd {
    Vertex(usize),
    Point(Point),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RubberMode {
    Replace,
    Add,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HandleKind {
    Bend,
    Control(usize),
    /// Handle at this percentage of the loop's length.
    Loop(u8),
    Arrow,
}

enum Drag {
    Pan,
    Move {
        base: Document,
        anchor: Point,
        start: Point,
    },
    Rubber {
        start: Pos2,
        mode: RubberMode,
    },
    Handle {
        base: Document,
        edge: usize,
        kind: HandleKind,
    },
    CrossFrom {
        vertex: usize,
    },
    EdgeTo {
        start: EdgeEnd,
        end: EdgeEnd,
    },
    EdgeFree {
        start: EdgeEnd,
    },
}

/// What the canvas texture was rendered for: scene version, view, pixel size.
type TextureKey = (u64, [u64; 3], [u32; 2]);

/// Undo history. Changes are committed when the user is not in the middle
/// of an interaction (no button held, no text field focused), so a drag or
/// typing a label is one undo step.
struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    committed: Document,
}

impl History {
    fn new(doc: &Document) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            committed: doc.clone(),
        }
    }

    fn settle(&mut self, doc: &Document) {
        if *doc != self.committed {
            self.undo
                .push(std::mem::replace(&mut self.committed, doc.clone()));
            self.redo.clear();
            if self.undo.len() > 1000 {
                self.undo.remove(0);
            }
        }
    }

    fn undo(&mut self, doc: &mut Document) -> bool {
        if *doc != self.committed {
            *doc = self.committed.clone();
            return true;
        }
        match self.undo.pop() {
            Some(prev) => {
                self.redo
                    .push(std::mem::replace(&mut self.committed, prev.clone()));
                *doc = prev;
                true
            }
            None => false,
        }
    }

    fn redo(&mut self, doc: &mut Document) -> bool {
        self.settle(doc);
        match self.redo.pop() {
            Some(next) => {
                self.undo
                    .push(std::mem::replace(&mut self.committed, next.clone()));
                *doc = next;
                true
            }
            None => false,
        }
    }
}

/// An action waiting for "discard unsaved changes?".
#[derive(Debug, Clone)]
enum Pending {
    New,
    Open(Option<PathBuf>),
    Quit,
}

pub struct SourceState {
    pub text: String,
    pub for_doc: Option<Document>,
    pub error: Option<String>,
}

pub struct GoldstoneApp {
    pub(crate) doc: Document,
    pub(crate) file: Option<PathBuf>,
    /// Text the document was loaded from or last saved as (comments and
    /// layout are preserved from it on save).
    pub(crate) loaded_text: Option<String>,
    saved_doc: Document,
    history: History,
    pub(crate) scene: Scene,
    scene_for: Option<Document>,
    pub(crate) scene_errors: Vec<String>,
    scene_version: u64,
    pub(crate) labels: Labels,
    text_boxes: TextBoxes,
    pub(crate) label_errors: Vec<String>,
    view: View,
    view_fitted: bool,
    pub(crate) tool: Tool,
    pub(crate) opts: ToolOptions,
    pub(crate) selection: Selection,
    drag: Option<Drag>,
    pending_edge: Option<EdgeEnd>,
    texture: Option<(egui::TextureHandle, TextureKey)>,
    pub(crate) tab: Tab,
    pub(crate) show_grid: bool,
    pub(crate) message: Option<String>,
    pub(crate) show_shortcuts: bool,
    pub(crate) show_about: bool,
    pub(crate) source: SourceState,
    confirm: Option<Pending>,
    pub(crate) focus_text_content: bool,
    pub(crate) rename: Option<(usize, String)>,
    canvas_rect: egui::Rect,
    pointer_doc: Option<Point>,
    allow_close: bool,
    title: String,
}

impl GoldstoneApp {
    pub fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        if let Some(data) = goldstone_export::labels::symbol_font() {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "symbols".into(),
                std::sync::Arc::new(egui::FontData::from_static(data)),
            );
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(fam)
                    .or_default()
                    .push("symbols".into());
            }
            cc.egui_ctx.set_fonts(fonts);
        }
        let doc = Document::default();
        let mut app = Self {
            saved_doc: doc.clone(),
            history: History::new(&doc),
            scene: scene::build(&doc).expect("empty document is valid"),
            doc,
            file: None,
            loaded_text: None,
            scene_for: None,
            scene_errors: Vec::new(),
            scene_version: 0,
            labels: Labels::new(),
            text_boxes: Vec::new(),
            label_errors: Vec::new(),
            view: View::default(),
            view_fitted: false,
            tool: Tool::Select,
            opts: ToolOptions {
                stroke: Stroke::Plain,
                class: None,
                arrow: None,
                vertex_kind: VertexKind::Dot,
            },
            selection: Selection::new(),
            drag: None,
            pending_edge: None,
            texture: None,
            tab: Tab::FillStroke,
            show_grid: true,
            message: None,
            show_shortcuts: false,
            show_about: false,
            source: SourceState {
                text: String::new(),
                for_doc: None,
                error: None,
            },
            confirm: None,
            focus_text_content: false,
            rename: None,
            canvas_rect: egui::Rect::NOTHING,
            pointer_doc: None,
            allow_close: false,
            title: String::new(),
        };
        if let Some(p) = path {
            app.load(&p);
        }
        app
    }

    /// The document being edited.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The selected objects.
    pub fn selected(&self) -> Vec<ObjRef> {
        self.selection.iter().copied().collect()
    }

    /// Screen position of a document point on the canvas (for tests).
    pub fn screen_pos(&self, x: f64, y: f64) -> Pos2 {
        self.view.to_screen(self.canvas_rect, Point::new(x, y))
    }

    // ---- document lifecycle -------------------------------------------------

    fn dirty(&self) -> bool {
        self.doc != self.saved_doc
    }

    fn reset_with(&mut self, doc: Document, file: Option<PathBuf>, text: Option<String>) {
        self.saved_doc = doc.clone();
        self.history = History::new(&doc);
        self.doc = doc;
        self.file = file;
        self.loaded_text = text;
        self.selection.clear();
        self.drag = None;
        self.pending_edge = None;
        self.view_fitted = false;
        self.source.for_doc = None;
        self.rename = None;
    }

    pub(crate) fn load(&mut self, path: &Path) {
        let result = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                let is_svg = path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("svg"));
                let src = if is_svg {
                    goldstone_export::svg::extract_source(&text)
                        .ok_or("this SVG was not made by Goldstone (no embedded source)")?
                } else {
                    text
                };
                let doc = io::parse(&src).map_err(|e| e.to_string())?;
                Ok((doc, src, is_svg))
            });
        match result {
            Ok((doc, src, is_svg)) => {
                // A figure opened from an SVG is saved as a new .gold file.
                let file = if is_svg {
                    None
                } else {
                    Some(path.to_path_buf())
                };
                self.reset_with(doc, file, Some(src));
                self.message = Some(format!("Opened {}", path.display()));
            }
            Err(e) => self.message = Some(format!("Cannot open {}: {e}", path.display())),
        }
    }

    fn save_to(&mut self, path: PathBuf) {
        let text = io::save(&self.doc, self.loaded_text.as_deref());
        match std::fs::write(&path, &text) {
            Ok(()) => {
                self.saved_doc = self.doc.clone();
                self.loaded_text = Some(text);
                self.message = Some(format!("Saved {}", path.display()));
                self.file = Some(path);
            }
            Err(e) => self.message = Some(format!("Cannot save {}: {e}", path.display())),
        }
    }

    fn save(&mut self) {
        match self.file.clone() {
            Some(p) => self.save_to(p),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) {
        let name = self
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("diagram.gold".into(), |n| n.to_string_lossy().into_owned());
        if let Some(mut p) = rfd::FileDialog::new()
            .add_filter("Goldstone diagram", &["gold"])
            .set_file_name(name)
            .save_file()
        {
            if p.extension().is_none() {
                p.set_extension("gold");
            }
            self.save_to(p);
        }
    }

    fn export(&mut self) {
        if !self.scene_errors.is_empty() {
            self.message = Some("Fix the errors in the diagram before exporting".into());
            return;
        }
        let stem = self
            .file
            .as_ref()
            .and_then(|p| p.file_stem())
            .map_or("diagram".into(), |n| n.to_string_lossy().into_owned());
        let Some(path) = rfd::FileDialog::new()
            .add_filter("PDF", &["pdf"])
            .add_filter("SVG (reopenable in Goldstone)", &["svg"])
            .add_filter("PNG", &["png"])
            .add_filter("TikZ picture (\\input into a paper)", &["tikz"])
            .add_filter("LaTeX document", &["tex"])
            .set_file_name(format!("{stem}.pdf"))
            .save_file()
        else {
            return;
        };
        let Some(format) = path
            .extension()
            .and_then(|e| Format::from_extension(&e.to_string_lossy()))
        else {
            self.message =
                Some("Choose a file name ending in .pdf, .svg, .png, .tikz or .tex".into());
            return;
        };
        let source = io::save(&self.doc, self.loaded_text.as_deref());
        let name = self
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned());
        let opts = goldstone_export::Options {
            source: Some(&source),
            source_name: name.as_deref(),
            dpi: 300.0,
        };
        self.message = Some(
            match goldstone_export::export(&self.scene, format, &mut self.labels, &opts) {
                Ok(bytes) => match std::fs::write(&path, bytes) {
                    Ok(()) => format!("Exported {}", path.display()),
                    Err(e) => format!("Cannot write {}: {e}", path.display()),
                },
                Err(e) => format!("Export failed: {e}"),
            },
        );
    }

    fn request(&mut self, p: Pending, ctx: &egui::Context) {
        if self.dirty() {
            self.confirm = Some(p);
        } else {
            self.perform(p, ctx);
        }
    }

    fn perform(&mut self, p: Pending, ctx: &egui::Context) {
        match p {
            Pending::New => {
                self.reset_with(Document::default(), None, None);
                self.message = Some("New diagram".into());
            }
            Pending::Open(path) => {
                let path = path.or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("Goldstone diagram", &["gold", "svg"])
                        .pick_file()
                });
                if let Some(p) = path {
                    self.load(&p);
                }
            }
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    // ---- scene --------------------------------------------------------------

    fn sync_scene(&mut self) {
        if self.scene_for.as_ref() == Some(&self.doc) {
            return;
        }
        match scene::build(&self.doc) {
            Ok(s) => {
                self.scene = s;
                self.scene_errors.clear();
                self.text_boxes = goldstone_export::svg::text_boxes(&self.scene, &mut self.labels);
                self.scene_version += 1;
            }
            Err(d) => self.scene_errors = d.0,
        }
        self.scene_for = Some(self.doc.clone());
        self.selection.retain(|r| doc_ops::exists(&self.doc, *r));
    }

    /// Export area: drawing bounds including labels, plus the margin.
    fn page(&self) -> Option<Rect> {
        let mut r = self.scene.path_bounds();
        for (_, q) in &self.text_boxes {
            for c in q {
                let pr = Rect::from_points(*c, *c);
                r = Some(r.map_or(pr, |a| a.union(pr)));
            }
        }
        r.map(|r| r.inflate(self.scene.margin, self.scene.margin))
    }

    fn selection_bounds(&self) -> Option<Rect> {
        let all = view::object_bounds(&self.doc, &self.scene, &self.text_boxes);
        all.iter()
            .filter(|(o, _)| self.selection.contains(o))
            .map(|(_, r)| *r)
            .reduce(|a, b| a.union(b))
    }

    pub(crate) fn grid(&self) -> f64 {
        self.doc.canvas.grid.filter(|g| *g > 0.0).unwrap_or(10.0)
    }

    /// Snaps a document point: to a vertex nearby, else to the grid (when
    /// the grid is shown). Shift disables snapping.
    fn snap(&self, p: Point, free: bool, exclude: &BTreeSet<usize>) -> (Point, Option<usize>) {
        if free {
            return (Point::new(doc_ops::round(p.x), doc_ops::round(p.y)), None);
        }
        let tol = SNAP_PX / self.view.zoom;
        let near = self
            .doc
            .vertices
            .iter()
            .enumerate()
            .filter(|(i, _)| !exclude.contains(i))
            .map(|(i, v)| (i, (doc_ops::pt(v.at) - p).hypot()))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = near {
            return (doc_ops::pt(self.doc.vertices[i].at), Some(i));
        }
        if self.show_grid {
            let g = self.grid();
            (
                Point::new((p.x / g).round() * g, (p.y / g).round() * g),
                None,
            )
        } else {
            (Point::new(doc_ops::round(p.x), doc_ops::round(p.y)), None)
        }
    }

    // ---- actions ------------------------------------------------------------

    pub(crate) fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        self.pending_edge = None;
    }

    pub(crate) fn do_action(&mut self, a: Action, ctx: &egui::Context) {
        let canvas = self.canvas_rect;
        match a {
            Action::ToolSelect => self.set_tool(Tool::Select),
            Action::ToolEdit => self.set_tool(Tool::Edit),
            Action::ToolEdge => self.set_tool(Tool::Edge),
            Action::ToolVertex => self.set_tool(Tool::Vertex),
            Action::ToolCross => self.set_tool(Tool::Cross),
            Action::ToolText => self.set_tool(Tool::Text),
            Action::Undo => {
                if !self.history.undo(&mut self.doc) {
                    self.message = Some("Nothing to undo".into());
                }
            }
            Action::Redo => {
                if !self.history.redo(&mut self.doc) {
                    self.message = Some("Nothing to redo".into());
                }
            }
            Action::Delete => {
                let sel = std::mem::take(&mut self.selection);
                doc_ops::delete(&mut self.doc, &sel);
            }
            Action::SelectAll => self.selection = doc_ops::all_objects(&self.doc),
            Action::Deselect => {
                if self.pending_edge.take().is_none() && self.drag.take().is_none() {
                    self.selection.clear();
                }
            }
            Action::FlipH => doc_ops::flip(&mut self.doc, &self.selection, true),
            Action::FlipV => doc_ops::flip(&mut self.doc, &self.selection, false),
            Action::Nudge { dx, dy, step } => {
                let s = match step {
                    Step::Normal => 2.0,
                    Step::Large => 20.0,
                    Step::Pixel => 1.0 / self.view.zoom,
                };
                doc_ops::translate(
                    &mut self.doc,
                    &self.selection,
                    Vec2::new(dx as f64 * s, dy as f64 * s),
                );
            }
            Action::ZoomIn => self.view.zoom_at(canvas, canvas.center(), 1.25),
            Action::ZoomOut => self.view.zoom_at(canvas, canvas.center(), 0.8),
            Action::Zoom1 => self.view.zoom = 1.0,
            Action::ZoomSelection => {
                if let Some(r) = self.selection_bounds() {
                    self.view.fit(canvas, r);
                }
            }
            Action::ZoomDrawing | Action::ZoomPage => {
                if let Some(r) = self.page() {
                    self.view.fit(canvas, r);
                }
            }
            Action::ToggleGrid => self.show_grid = !self.show_grid,
            Action::New => self.request(Pending::New, ctx),
            Action::Open => self.request(Pending::Open(None), ctx),
            Action::Save => self.save(),
            Action::SaveAs => self.save_as(),
            Action::Export => self.export(),
            Action::ShowFillStroke => self.tab = Tab::FillStroke,
            Action::ShowObjects => self.tab = Tab::Objects,
            Action::ShowSource => self.tab = Tab::Source,
            Action::Shortcuts => self.show_shortcuts = true,
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut fired = Vec::new();
        ctx.input_mut(|i| {
            for bnd in BINDINGS {
                let hit = match bnd.trigger {
                    Trigger::Key(mods, key) => {
                        let mut hit = false;
                        i.events.retain(|e| {
                            if let egui::Event::Key {
                                key: k,
                                pressed: true,
                                modifiers: m,
                                ..
                            } = e
                                && !hit
                                && *k == key
                                && m.command == mods.command
                                && m.alt == mods.alt
                                && (m.shift == mods.shift
                                    || matches!(key, Key::Plus | Key::Equals | Key::Minus))
                            {
                                hit = true;
                                return false;
                            }
                            true
                        });
                        hit
                    }
                    Trigger::Text(t) => {
                        let before = i.events.len();
                        i.events
                            .retain(|e| !matches!(e, egui::Event::Text(s) if s == t));
                        i.events.len() != before
                    }
                };
                if hit {
                    fired.push(bnd.action);
                }
            }
        });
        for a in fired {
            self.do_action(a, ctx);
        }
    }

    // ---- creating objects ---------------------------------------------------

    fn add_vertex(&mut self, at: Point, kind: VertexKind) -> usize {
        let id = doc_ops::fresh_id(&self.doc, "v");
        let kind = (kind != VertexKind::None).then_some(kind);
        self.doc.vertices.push(doc_ops::new_vertex(id, at, kind));
        self.doc.vertices.len() - 1
    }

    fn end_vertex(&mut self, end: EdgeEnd) -> String {
        match end {
            EdgeEnd::Vertex(i) => self.doc.vertices[i].id.clone(),
            EdgeEnd::Point(p) => {
                let i = self.add_vertex(p, VertexKind::None);
                self.doc.vertices[i].id.clone()
            }
        }
    }

    fn end_point(&self, end: EdgeEnd) -> Point {
        match end {
            EdgeEnd::Vertex(i) => doc_ops::pt(self.doc.vertices[i].at),
            EdgeEnd::Point(p) => p,
        }
    }

    fn finish_edge(&mut self, start: EdgeEnd, end: EdgeEnd, through: Option<Point>) {
        let (a, b) = (self.end_point(start), self.end_point(end));
        let is_loop = matches!((start, end), (EdgeEnd::Vertex(x), EdgeEnd::Vertex(y)) if x == y);
        if !is_loop && (a - b).hypot() < 1e-6 {
            return;
        }
        let from = self.end_vertex(start);
        let to = if is_loop {
            from.clone()
        } else {
            self.end_vertex(end)
        };
        let mut e = doc_ops::new_edge(from, to);
        e.class = self.opts.class.clone();
        e.stroke = (self.opts.stroke != Stroke::Plain).then_some(self.opts.stroke);
        e.arrow = self.opts.arrow;
        if is_loop {
            let angle = through.map_or(90.0, |p| (p - a).atan2().to_degrees().round());
            e.loop_ = Some(LoopSpec {
                radius: None,
                angle: Some(angle),
            });
        } else if let Some(p) = through {
            let bend = doc_ops::bend_through(a, b, p);
            e.bend = (bend != 0.0).then_some(bend);
        }
        self.doc.edges.push(e);
        self.selection = [ObjRef::Edge(self.doc.edges.len() - 1)].into();
    }

    // ---- edit handles -------------------------------------------------------

    fn selected_edge(&self) -> Option<usize> {
        match (self.selection.len(), self.selection.first()) {
            (1, Some(ObjRef::Edge(i))) => Some(*i),
            _ => None,
        }
    }

    /// Handles of the selected edge in the Edit tool: (kind, position).
    fn handles(&self) -> Vec<(HandleKind, Point)> {
        let Some(i) = self.selected_edge() else {
            return vec![];
        };
        let (Some(e), Some(Some(path))) = (self.doc.edges.get(i), self.scene.edge_paths.get(i))
        else {
            return vec![];
        };
        let al = geom::ArcLen::new(path);
        let len = al.length();
        let mut out = Vec::new();
        // The arrow handle sits on the arrowhead itself; the bend/loop handle
        // moves to a quarter of the line when the arrow is near the middle
        // (any point of an arc determines the same arc).
        let class = e.class.as_ref().and_then(|c| self.doc.styles.get(c));
        let arrow = e
            .arrow
            .or(class.and_then(|s| s.arrow))
            .unwrap_or(ArrowDir::None);
        let pos = e
            .arrow_pos
            .or(class.and_then(|s| s.arrow_pos))
            .unwrap_or(scene::defaults::ARROW_POS);
        if arrow != ArrowDir::None {
            out.push((HandleKind::Arrow, al.eval(pos * len).0));
        }
        let f: u8 = if arrow != ArrowDir::None && (pos - 0.5).abs() < 0.2 {
            25
        } else {
            50
        };
        if let Some([c1, c2]) = e.controls {
            out.push((HandleKind::Control(0), doc_ops::pt(c1)));
            out.push((HandleKind::Control(1), doc_ops::pt(c2)));
        } else if e.from == e.to {
            out.push((HandleKind::Loop(f), al.eval(len * f as f64 / 100.0).0));
        } else {
            out.push((HandleKind::Bend, al.eval(len * f as f64 / 100.0).0));
        }
        out
    }

    fn apply_handle(
        &mut self,
        base: &Document,
        edge: usize,
        kind: HandleKind,
        p: Point,
        ctrl: bool,
        free: bool,
    ) {
        let mut doc = base.clone();
        let Some(e) = doc.edges.get(edge).cloned() else {
            return;
        };
        let (Some(a), Some(b)) = (
            doc_ops::vertex_index(&doc, &e.from),
            doc_ops::vertex_index(&doc, &e.to),
        ) else {
            return;
        };
        let (pa, pb) = (
            doc_ops::pt(doc.vertices[a].at),
            doc_ops::pt(doc.vertices[b].at),
        );
        let e = &mut doc.edges[edge];
        match kind {
            HandleKind::Bend => {
                let mut bend = doc_ops::bend_through(pa, pb, p);
                if ctrl {
                    bend = (bend / 15.0).round() * 15.0;
                }
                e.bend = (bend != 0.0).then_some(bend);
            }
            HandleKind::Control(k) => {
                let (q, _) = self.snap(p, free, &BTreeSet::new());
                if let Some(c) = e.controls.as_mut() {
                    c[k] = [q.x, q.y];
                }
            }
            HandleKind::Loop(f) => {
                // The handle is at fraction f of a circle through the vertex:
                // chord = 2r·sin(θ/2), and the center lies (π−θ)/2 to the
                // left of the chord, with θ = 2πf.
                let d = p - pa;
                let theta = std::f64::consts::TAU * f as f64 / 100.0;
                let r = d.hypot() / (2.0 * (theta / 2.0).sin());
                let mut angle = (d.atan2() + (std::f64::consts::PI - theta) / 2.0).to_degrees();
                if ctrl {
                    angle = (angle / 15.0).round() * 15.0;
                }
                e.loop_ = Some(LoopSpec {
                    radius: Some(doc_ops::round(r).max(1.0)),
                    angle: Some(angle.round().rem_euclid(360.0)),
                });
            }
            HandleKind::Arrow => {
                if let Some(Some(path)) = self.scene.edge_paths.get(edge) {
                    let f = (view::closest_fraction(path, p) * 100.0).round() / 100.0;
                    e.arrow_pos = Some(f);
                }
            }
        }
        self.doc = doc;
    }

    // ---- canvas -------------------------------------------------------------

    fn hit(&self, p: Point) -> Option<ObjRef> {
        view::hit(
            &self.doc,
            &self.scene,
            &self.text_boxes,
            p,
            PICK_PX / self.view.zoom,
        )
    }

    fn click_select(&mut self, p: Point, shift: bool) {
        match self.hit(p) {
            Some(o) if shift => {
                if !self.selection.remove(&o) {
                    self.selection.insert(o);
                }
            }
            Some(o) => self.selection = [o].into(),
            None if !shift => self.selection.clear(),
            None => {}
        }
    }

    fn start_move(&mut self, o: ObjRef, press: Point, shift: bool) {
        if !self.selection.contains(&o) {
            if shift {
                self.selection.insert(o);
            } else {
                self.selection = [o].into();
            }
        }
        let anchor = match o {
            ObjRef::Vertex(i) => doc_ops::pt(self.doc.vertices[i].at),
            ObjRef::Text(i) => doc_ops::pt(self.doc.texts[i].at),
            _ => {
                let (q, _) = self.snap(press, false, &BTreeSet::new());
                if (q - press).hypot() * self.view.zoom < SNAP_PX {
                    q
                } else {
                    press
                }
            }
        };
        self.drag = Some(Drag::Move {
            base: self.doc.clone(),
            anchor,
            start: press,
        });
    }

    fn canvas(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        self.canvas_rect = rect;
        if !self.view_fitted && rect.width() > 10.0 {
            self.sync_scene();
            match self.page() {
                Some(r) => self.view.fit(rect, r),
                None => self.view = View::default(),
            }
            self.view.zoom = self.view.zoom.min(4.0);
            self.view_fitted = true;
        }
        let (mods, scroll, zoom_delta, space) = ui.input(|i| {
            (
                i.modifiers,
                i.smooth_scroll_delta,
                i.zoom_delta(),
                i.key_down(Key::Space),
            )
        });
        let space = space && !ui.ctx().egui_wants_keyboard_input();
        let hover = resp.hover_pos();
        self.pointer_doc = hover.map(|h| self.view.to_doc(rect, h));

        // ---- view navigation
        if let Some(h) = hover {
            if zoom_delta != 1.0 {
                self.view.zoom_at(rect, h, zoom_delta as f64);
            } else if scroll != egui::Vec2::ZERO {
                self.view.pan_screen(scroll);
            }
        }
        if resp.dragged_by(PointerButton::Middle)
            || (space && resp.dragged_by(PointerButton::Primary))
        {
            self.view.pan_screen(resp.drag_delta());
            if resp.drag_started() {
                self.drag = Some(Drag::Pan);
            }
        } else {
            self.tool_input(ui, rect, &resp, mods);
        }
        if resp.secondary_clicked() {
            self.pending_edge = None;
        }

        self.sync_scene();
        self.paint(ui, rect);
    }

    fn tool_input(
        &mut self,
        ui: &egui::Ui,
        rect: egui::Rect,
        resp: &egui::Response,
        mods: egui::Modifiers,
    ) {
        let to_doc = |p: Pos2| self.view.to_doc(rect, p);
        let pointer = resp.interact_pointer_pos().or(resp.hover_pos()).map(to_doc);
        let press = ui.input(|i| i.pointer.press_origin()).map(to_doc);
        let (shift, ctrl) = (mods.shift, mods.command);

        if resp.clicked_by(PointerButton::Primary) {
            let Some(p) = pointer else { return };
            match self.tool {
                Tool::Select | Tool::Edit => self.click_select(p, shift),
                Tool::Edge => {
                    let (q, v) = self.snap(p, shift, &BTreeSet::new());
                    let end = v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex);
                    match self.pending_edge.take() {
                        None => self.pending_edge = Some(end),
                        Some(start) => self.finish_edge(start, end, None),
                    }
                }
                Tool::Vertex | Tool::Cross => {
                    if let Some(o @ ObjRef::Vertex(_)) = self.hit(p) {
                        self.selection = [o].into();
                    } else {
                        let (q, _) = self.snap(p, shift, &BTreeSet::new());
                        let kind = if self.tool == Tool::Cross {
                            VertexKind::Cross
                        } else {
                            self.opts.vertex_kind
                        };
                        let i = self.add_vertex(q, kind);
                        self.selection = [ObjRef::Vertex(i)].into();
                    }
                }
                Tool::Text => {
                    if let Some(o @ ObjRef::Text(_)) = self.hit(p) {
                        self.selection = [o].into();
                    } else {
                        let (q, _) = self.snap(p, shift, &BTreeSet::new());
                        self.doc.texts.push(doc_ops::new_text(q, "$x$".into()));
                        self.selection = [ObjRef::Text(self.doc.texts.len() - 1)].into();
                        self.tab = Tab::FillStroke;
                        self.focus_text_content = true;
                    }
                }
            }
            return;
        }

        if resp.drag_started_by(PointerButton::Primary) {
            let Some(p) = press else { return };
            self.drag = match self.tool {
                Tool::Select | Tool::Edit => {
                    let tol = HANDLE_PX as f64 * 1.6 / self.view.zoom;
                    let handle = if self.tool == Tool::Edit {
                        self.handles()
                            .into_iter()
                            .find(|(_, h)| (*h - p).hypot() <= tol)
                            .map(|(k, _)| k)
                    } else {
                        None
                    };
                    if let (Some(kind), Some(edge)) = (handle, self.selected_edge()) {
                        Some(Drag::Handle {
                            base: self.doc.clone(),
                            edge,
                            kind,
                        })
                    } else if let Some(o) = self.hit(p) {
                        self.start_move(o, p, shift);
                        self.drag.take()
                    } else {
                        let mode = match (shift, ctrl) {
                            (true, true) => RubberMode::Remove,
                            (true, false) => RubberMode::Add,
                            _ => RubberMode::Replace,
                        };
                        Some(Drag::Rubber {
                            start: self.view.to_screen(rect, p),
                            mode,
                        })
                    }
                }
                Tool::Edge => {
                    let (q, v) = self.snap(p, shift, &BTreeSet::new());
                    let at = v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex);
                    match self.pending_edge.take() {
                        Some(start) => Some(Drag::EdgeTo { start, end: at }),
                        None => Some(Drag::EdgeFree { start: at }),
                    }
                }
                Tool::Cross => match self.hit(p) {
                    Some(ObjRef::Vertex(i)) => Some(Drag::CrossFrom { vertex: i }),
                    _ => None,
                },
                Tool::Vertex | Tool::Text => match self.hit(p) {
                    Some(o @ (ObjRef::Vertex(_) | ObjRef::Text(_))) => {
                        self.start_move(o, p, shift);
                        self.drag.take()
                    }
                    _ => None,
                },
            };
        }

        if resp.dragged_by(PointerButton::Primary)
            && let Some(p) = pointer
        {
            match self.drag.take() {
                Some(Drag::Move {
                    base,
                    anchor,
                    start,
                }) => {
                    let mut raw = p - start;
                    if ctrl {
                        if raw.x.abs() > raw.y.abs() {
                            raw.y = 0.0;
                        } else {
                            raw.x = 0.0;
                        }
                    }
                    let moved = doc_ops::moved_vertices(&base, &self.selection);
                    let (target, _) = self.snap(anchor + raw, shift, &moved);
                    let mut d = target - anchor;
                    if ctrl {
                        if raw.x == 0.0 {
                            d.x = 0.0;
                        } else {
                            d.y = 0.0;
                        }
                    }
                    let mut doc = base.clone();
                    doc_ops::translate(&mut doc, &self.selection, d);
                    self.doc = doc;
                    self.drag = Some(Drag::Move {
                        base,
                        anchor,
                        start,
                    });
                }
                Some(Drag::Handle { base, edge, kind }) => {
                    self.apply_handle(&base, edge, kind, p, ctrl, shift);
                    self.drag = Some(Drag::Handle { base, edge, kind });
                }
                other => self.drag = other,
            }
        }

        if resp.drag_stopped_by(PointerButton::Primary) {
            let end = pointer;
            match self.drag.take() {
                Some(Drag::Rubber { start, mode }) => {
                    if let Some(e) = resp.interact_pointer_pos().or(resp.hover_pos()) {
                        let (a, b) = (self.view.to_doc(rect, start), self.view.to_doc(rect, e));
                        let r = Rect::from_points(a, b);
                        let inside: Vec<ObjRef> =
                            view::object_bounds(&self.doc, &self.scene, &self.text_boxes)
                                .into_iter()
                                .filter(|(_, b)| {
                                    r.contains(b.origin()) && r.contains(Point::new(b.x1, b.y1))
                                })
                                .map(|(o, _)| o)
                                .collect();
                        match mode {
                            RubberMode::Replace => self.selection = inside.into_iter().collect(),
                            RubberMode::Add => self.selection.extend(inside),
                            RubberMode::Remove => inside.iter().for_each(|o| {
                                self.selection.remove(o);
                            }),
                        }
                    }
                }
                Some(Drag::CrossFrom { vertex }) => {
                    if let Some(p) = end {
                        let (q, _) = self.snap(p, shift, &BTreeSet::new());
                        if (q - doc_ops::pt(self.doc.vertices[vertex].at)).hypot() > 1e-6 {
                            let x = self.add_vertex(q, VertexKind::Cross);
                            let mut e = doc_ops::new_edge(
                                self.doc.vertices[vertex].id.clone(),
                                self.doc.vertices[x].id.clone(),
                            );
                            e.stroke = Some(Stroke::Dashed);
                            self.doc.edges.push(e);
                            self.selection = [ObjRef::Vertex(x)].into();
                        }
                    }
                }
                Some(Drag::EdgeTo { start, end: e }) => {
                    if let Some(p) = end {
                        self.finish_edge(start, e, Some(p));
                    }
                }
                Some(Drag::EdgeFree { start }) => {
                    if let Some(p) = end {
                        let (q, v) = self.snap(p, shift, &BTreeSet::new());
                        self.finish_edge(start, v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex), None);
                    }
                }
                _ => {}
            }
        }
    }

    fn screen_path(&self, rect: egui::Rect, path: &BezPath) -> Vec<Vec<Pos2>> {
        let mut out: Vec<Vec<Pos2>> = Vec::new();
        let tol = 0.3 / self.view.zoom;
        let mut start = Pos2::ZERO;
        goldstone_core::kurbo::flatten(path, tol, |el| match el {
            PathEl::MoveTo(p) => {
                start = self.view.to_screen(rect, p);
                out.push(vec![start]);
            }
            PathEl::LineTo(p) => {
                if let Some(v) = out.last_mut() {
                    v.push(self.view.to_screen(rect, p));
                }
            }
            PathEl::ClosePath => {
                if let Some(v) = out.last_mut() {
                    v.push(start);
                }
            }
            _ => {}
        });
        out
    }

    fn paint(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, DESK);

        // Page (export area).
        if let Some(page) = self.page() {
            let a = self.view.to_screen(rect, Point::new(page.x0, page.y1));
            let b = self.view.to_screen(rect, Point::new(page.x1, page.y0));
            let pr = egui::Rect::from_two_pos(a, b);
            painter.rect_filled(
                pr.translate(egui::vec2(2.0, 2.0)),
                0.0,
                Color32::from_black_alpha(40),
            );
            painter.rect_filled(pr, 0.0, Color32::WHITE);
        }

        // Grid.
        if self.show_grid {
            let g = self.grid();
            let step = g * self.view.zoom;
            if step >= 6.0 {
                let vis = self.view.visible(rect);
                let col = Color32::from_rgba_unmultiplied(80, 110, 200, 40);
                let mut x = (vis.x0 / g).ceil() * g;
                while x <= vis.x1 {
                    let s = self.view.to_screen(rect, Point::new(x, 0.0)).x;
                    painter.line_segment(
                        [Pos2::new(s, rect.top()), Pos2::new(s, rect.bottom())],
                        EStroke::new(1.0, col),
                    );
                    x += g;
                }
                let mut y = (vis.y0 / g).ceil() * g;
                while y <= vis.y1 {
                    let s = self.view.to_screen(rect, Point::new(0.0, y)).y;
                    painter.line_segment(
                        [Pos2::new(rect.left(), s), Pos2::new(rect.right(), s)],
                        EStroke::new(1.0, col),
                    );
                    y += g;
                }
            }
        }

        // The drawing itself, rendered by the export pipeline.
        let ppp = ui.ctx().pixels_per_point();
        let w = (rect.width() * ppp).round().max(1.0) as u32;
        let h = (rect.height() * ppp).round().max(1.0) as u32;
        let key = (
            self.scene_version,
            [
                self.view.center.x.to_bits(),
                self.view.center.y.to_bits(),
                self.view.zoom.to_bits(),
            ],
            [w, h],
        );
        if self.texture.as_ref().is_none_or(|(_, k)| *k != key) {
            let (pixels, errors) = goldstone_export::rasterize_view(
                &self.scene,
                &mut self.labels,
                self.view.visible(rect),
                w,
                h,
            );
            self.label_errors = errors;
            let img = egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], &pixels);
            match self.texture.as_mut() {
                Some((tex, k)) => {
                    tex.set(img, egui::TextureOptions::LINEAR);
                    *k = key;
                }
                None => {
                    let tex = ui.ctx().load_texture(
                        "goldstone-canvas",
                        img,
                        egui::TextureOptions::LINEAR,
                    );
                    self.texture = Some((tex, key));
                }
            }
        }
        if let Some((tex, _)) = &self.texture {
            painter.image(
                tex.id(),
                rect,
                egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }

        // Invisible vertices: small markers so they can be found.
        for v in &self.doc.vertices {
            let class_kind = v
                .class
                .as_ref()
                .and_then(|c| self.doc.styles.get(c))
                .and_then(|s| s.kind);
            if v.kind.or(class_kind).unwrap_or(VertexKind::None) == VertexKind::None {
                let s = self.view.to_screen(rect, doc_ops::pt(v.at));
                painter.circle_stroke(
                    s,
                    2.5,
                    EStroke::new(1.0, Color32::from_rgba_unmultiplied(30, 120, 230, 110)),
                );
            }
        }

        // Selection.
        let sel_stroke = EStroke::new(2.5, Color32::from_rgba_unmultiplied(30, 120, 230, 150));
        for item in &self.scene.items {
            if self.selection.contains(&item.obj)
                && !matches!(item.obj, ObjRef::Vertex(_))
                && let scene::Mark::Path(p) = &item.mark
            {
                for line in self.screen_path(rect, &p.path) {
                    painter.add(egui::Shape::line(line, sel_stroke));
                }
            }
        }
        for o in &self.selection {
            match *o {
                ObjRef::Vertex(i) => {
                    let s = self
                        .view
                        .to_screen(rect, doc_ops::pt(self.doc.vertices[i].at));
                    let r = egui::Rect::from_center_size(s, egui::vec2(9.0, 9.0));
                    painter.rect_stroke(r, 0.0, EStroke::new(1.5, ACCENT), StrokeKind::Middle);
                }
                ObjRef::Text(_) | ObjRef::Axes(_) | ObjRef::Shape(_) => {
                    for (t, q) in &self.text_boxes {
                        if t == o {
                            let mut pts: Vec<Pos2> =
                                q.iter().map(|c| self.view.to_screen(rect, *c)).collect();
                            pts.push(pts[0]);
                            painter.extend(egui::Shape::dashed_line(
                                &pts,
                                EStroke::new(1.0, ACCENT),
                                4.0,
                                3.0,
                            ));
                        }
                    }
                }
                ObjRef::Edge(_) => {}
            }
        }
        if self.selection.len() > 1
            && let Some(b) = self.selection_bounds()
        {
            let a = self.view.to_screen(rect, Point::new(b.x0, b.y1));
            let c = self.view.to_screen(rect, Point::new(b.x1, b.y0));
            let r = egui::Rect::from_two_pos(a, c).expand(6.0);
            let pts = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            painter.extend(egui::Shape::dashed_line(
                &pts,
                EStroke::new(1.0, Color32::from_gray(60)),
                5.0,
                4.0,
            ));
        }

        // Edit-tool handles.
        if self.tool == Tool::Edit {
            if let (Some(i), Some(e)) = (
                self.selected_edge(),
                self.selected_edge().and_then(|i| self.doc.edges.get(i)),
            ) {
                if let Some([c1, c2]) = e.controls
                    && let (Some(a), Some(b)) = (
                        doc_ops::vertex_index(&self.doc, &e.from),
                        doc_ops::vertex_index(&self.doc, &e.to),
                    )
                {
                    let st = EStroke::new(1.0, ACCENT);
                    let pa = self
                        .view
                        .to_screen(rect, doc_ops::pt(self.doc.vertices[a].at));
                    let pb = self
                        .view
                        .to_screen(rect, doc_ops::pt(self.doc.vertices[b].at));
                    painter.line_segment([pa, self.view.to_screen(rect, doc_ops::pt(c1))], st);
                    painter.line_segment([pb, self.view.to_screen(rect, doc_ops::pt(c2))], st);
                }
                let _ = i;
            }
            for (kind, p) in self.handles() {
                let s = self.view.to_screen(rect, p);
                match kind {
                    HandleKind::Arrow => {
                        let r = HANDLE_PX;
                        let pts = vec![
                            s + egui::vec2(0.0, -r),
                            s + egui::vec2(r, 0.0),
                            s + egui::vec2(0.0, r),
                            s + egui::vec2(-r, 0.0),
                        ];
                        painter.add(egui::Shape::convex_polygon(
                            pts,
                            Color32::WHITE,
                            EStroke::new(1.5, ACCENT),
                        ));
                    }
                    _ => {
                        painter.circle(s, HANDLE_PX, Color32::WHITE, EStroke::new(1.5, ACCENT));
                    }
                }
            }
        }

        // Edge being drawn.
        if self.tool == Tool::Edge
            && let (Some(start), Some(p)) = (self.pending_edge, self.pointer_doc)
        {
            let shift = ui.input(|i| i.modifiers.shift);
            let (q, v) = self.snap(p, shift, &BTreeSet::new());
            let a = self.view.to_screen(rect, self.end_point(start));
            let b = self.view.to_screen(rect, q);
            painter.line_segment([a, b], EStroke::new(1.5, ACCENT));
            if v.is_some() {
                painter.circle_stroke(b, 6.0, EStroke::new(1.5, ACCENT));
            }
        }
        match &self.drag {
            Some(Drag::Rubber { start, mode }) => {
                if let Some(e) = ui.input(|i| i.pointer.latest_pos()) {
                    let r = egui::Rect::from_two_pos(*start, e);
                    let col = if *mode == RubberMode::Remove {
                        Color32::from_rgb(200, 60, 60)
                    } else {
                        ACCENT
                    };
                    painter.rect_filled(r, 0.0, col.gamma_multiply(0.08));
                    painter.rect_stroke(r, 0.0, EStroke::new(1.0, col), StrokeKind::Middle);
                }
            }
            Some(Drag::EdgeFree { start }) | Some(Drag::EdgeTo { start, .. }) => {
                if let Some(p) = self.pointer_doc {
                    let a = self.view.to_screen(rect, self.end_point(*start));
                    painter
                        .line_segment([a, self.view.to_screen(rect, p)], EStroke::new(1.5, ACCENT));
                }
            }
            Some(Drag::CrossFrom { vertex }) => {
                if let Some(p) = self.pointer_doc {
                    let a = self
                        .view
                        .to_screen(rect, doc_ops::pt(self.doc.vertices[*vertex].at));
                    let b = self.view.to_screen(rect, p);
                    painter.extend(egui::Shape::dashed_line(
                        &[a, b],
                        EStroke::new(1.5, ACCENT),
                        5.0,
                        4.0,
                    ));
                    let d = 5.0;
                    painter.line_segment(
                        [b + egui::vec2(-d, -d), b + egui::vec2(d, d)],
                        EStroke::new(1.5, ACCENT),
                    );
                    painter.line_segment(
                        [b + egui::vec2(-d, d), b + egui::vec2(d, -d)],
                        EStroke::new(1.5, ACCENT),
                    );
                }
            }
            _ => {}
        }

        if !self.scene_errors.is_empty() {
            painter.text(
                rect.left_top() + egui::vec2(10.0, 10.0),
                Align2::LEFT_TOP,
                "The diagram has errors; showing the last valid version. See the status bar.",
                FontId::proportional(13.0),
                Color32::from_rgb(190, 30, 30),
            );
        }
        let _ = CornerRadius::ZERO;
    }

    // ---- frame --------------------------------------------------------------

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(p) = self.confirm.clone() else {
            return;
        };
        let mut choice = None;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("The diagram has unsaved changes.");
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Some(0);
                    }
                    if ui.button("Discard").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => {
                self.save();
                self.confirm = None;
                if !self.dirty() {
                    self.perform(p, ctx);
                }
            }
            Some(1) => {
                self.confirm = None;
                self.perform(p, ctx);
            }
            Some(2) => self.confirm = None,
            _ => {}
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self
            .file
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or("Untitled".into(), |n| n.to_string_lossy().into_owned());
        let title = format!("{}{name} — Goldstone", if self.dirty() { "*" } else { "" });
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    pub(crate) fn status_text(&self) -> String {
        let n = |f: fn(&ObjRef) -> bool| self.selection.iter().filter(|o| f(o)).count();
        let parts: Vec<String> = [
            (n(|o| matches!(o, ObjRef::Vertex(_))), "vertex", "vertices"),
            (n(|o| matches!(o, ObjRef::Edge(_))), "edge", "edges"),
            (n(|o| matches!(o, ObjRef::Text(_))), "text", "texts"),
            (n(|o| matches!(o, ObjRef::Axes(_))), "axes", "axes"),
            (n(|o| matches!(o, ObjRef::Shape(_))), "shape", "shapes"),
        ]
        .iter()
        .filter(|(c, _, _)| *c > 0)
        .map(|(c, one, many)| format!("{c} {}", if *c == 1 { one } else { many }))
        .collect();
        if parts.is_empty() {
            "Nothing selected".into()
        } else {
            parts.join(", ")
        }
    }

    pub(crate) fn pointer_text(&self) -> String {
        let pos = self
            .pointer_doc
            .map_or(String::new(), |p| format!("x {:.1}  y {:.1} pt", p.x, p.y));
        format!("{pos}   {:.0}%", self.view.zoom * 100.0)
    }
}

impl eframe::App for GoldstoneApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty() && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Pending::Quit);
        }
        if let Some(path) =
            ctx.input(|i| i.raw.dropped_files.first().map(|f| f.path().to_path_buf()))
        {
            self.request(Pending::Open(Some(path)), &ctx);
        }
        self.handle_keys(&ctx);
        self.sync_scene();

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("tool-controls").show(ui, |ui| self.tool_controls(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("palettes").show(ui, |ui| self.palettes(ui));
        egui::Panel::left("toolbox")
            .resizable(false)
            .exact_size(72.0)
            .show(ui, |ui| self.toolbox(ui));
        egui::Panel::right("dialogs")
            .resizable(true)
            .default_size(320.0)
            .min_size(240.0)
            .show(ui, |ui| self.dialogs(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.canvas(ui));

        self.confirm_dialog(&ctx);
        self.shortcuts_window(&ctx);
        self.about_window(&ctx);

        let interacting =
            ctx.input(|i| i.pointer.any_down()) || ctx.memory(|m| m.focused().is_some());
        if !interacting {
            self.history.settle(&self.doc);
        }
        self.sync_scene();
        self.update_title(&ctx);
    }
}

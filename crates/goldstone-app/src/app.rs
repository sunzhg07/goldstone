//! The editor: state, tools, canvas and file handling.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use egui::{
    Align2, Color32, CornerRadius, FontId, Key, PointerButton, Pos2, Sense, Stroke as EStroke,
    StrokeKind,
};
use goldstone_core::kurbo::{BezPath, PathEl, Point, Rect, Vec2};
use goldstone_core::model::{ArrowDir, Document, LoopSpec, ShapeKind, Stroke, VertexKind};
use goldstone_core::scene::{self, ObjRef, Scene};
use goldstone_core::{geom, inspect, io};
use goldstone_export::{Format, Labels};

use crate::doc_ops::{self, Selection};
use crate::icons::Icons;
use crate::keys::{self, Action, BINDINGS, Os, Step, Trigger};
use crate::view::{self, TextBoxes, View};

pub const ACCENT: Color32 = Color32::from_rgb(30, 120, 230);
const DESK: Color32 = Color32::from_rgb(222, 222, 226);
const PICK_PX: f64 = 6.0;
const SNAP_PX: f64 = 9.0;
const HANDLE_PX: f32 = 5.0;
/// Width of the rulers in screen points.
pub const RULER: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Edit,
    Edge,
    Arc,
    Vertex,
    Cross,
    Text,
    Rect,
    Ellipse,
}

impl Tool {
    pub const ALL: [Tool; 9] = [
        Tool::Select,
        Tool::Edit,
        Tool::Edge,
        Tool::Arc,
        Tool::Vertex,
        Tool::Cross,
        Tool::Text,
        Tool::Rect,
        Tool::Ellipse,
    ];

    /// Name of the tool's icon (`icons/<name>.gold`).
    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Edit => "edit",
            Tool::Edge => "edge",
            Tool::Arc => "arc",
            Tool::Vertex => "vertex",
            Tool::Cross => "cross",
            Tool::Text => "text",
            Tool::Rect => "rect",
            Tool::Ellipse => "ellipse",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Tool::Select => "Select",
            Tool::Edit => "Edit",
            Tool::Edge => "Line",
            Tool::Arc => "Arc",
            Tool::Vertex => "Vertex",
            Tool::Cross => "Cross",
            Tool::Text => "Text",
            Tool::Rect => "Rectangle",
            Tool::Ellipse => "Ellipse",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "S",
            Tool::Edit => "N",
            Tool::Edge => "B",
            Tool::Arc => "A",
            Tool::Vertex => "P",
            Tool::Cross => "X",
            Tool::Text => "T",
            Tool::Rect => "R",
            Tool::Ellipse => "E",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            Tool::Select => {
                "Click to select (Shift: add/remove), drag to move; drag on empty canvas to select a region, Alt+drag to touch-select"
            }
            Tool::Edit => {
                "Drag the round handle to bend a line or resize a loop, the diamond to move the arrow"
            }
            Tool::Edge => {
                "Click, click: straight line (ends snap to vertices, or make invisible vertices). Esc cancels"
            }
            Tool::Arc => {
                "Click the start, click the end, then move to set the bulge and click. Same vertex twice: loop. Esc cancels"
            }
            Tool::Vertex => "Click to place a vertex",
            Tool::Cross => "Click to place a cross; drag from a vertex to draw ---X",
            Tool::Text => "Click to place a LaTeX label, then type it in Fill & Stroke",
            Tool::Rect => "Drag to draw a rectangle (Ctrl: square)",
            Tool::Ellipse => "Drag to draw an ellipse (Ctrl: circle)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    FillStroke,
    Objects,
    Align,
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
    /// New lines are double lines.
    pub double: bool,
    /// Separation of new double lines (`None`: class / default).
    pub double_sep: Option<f64>,
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
    /// Touch selection: the screen path drawn so far.
    Touch {
        points: Vec<Pos2>,
        add: bool,
    },
    ShapeNew {
        start: Point,
        kind: ShapeKind,
    },
}

/// What the canvas texture was rendered for: scene version, view, pixel size.
type TextureKey = (u64, [u64; 3], [u32; 2]);

/// Undo history. Changes are committed when the user is not in the middle
/// of an interaction (no button held, no text field focused), so a drag or
/// typing a label is one undo step.
#[derive(Default)]
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
    /// Close the active tab.
    Close,
    Quit,
}

/// One open diagram (a tab). The active one lives in the
/// `GoldstoneApp` fields; the others are kept here.
#[derive(Default)]
struct Page {
    doc: Document,
    file: Option<PathBuf>,
    loaded_text: Option<String>,
    saved_doc: Document,
    history: History,
    view: View,
    view_fitted: bool,
    selection: Selection,
    source: SourceState,
    /// "Untitled N" for a diagram without a file.
    untitled: usize,
}

impl Page {
    fn dirty(&self) -> bool {
        self.doc != self.saved_doc
    }
}

/// Diagram from LaTeX dialog.
pub struct LatexDialog {
    pub open: bool,
    pub text: String,
    /// Result of parsing `text`: a summary, or the error.
    pub status: Result<String, String>,
    /// Terms left out because they are not valid contractions.
    pub warnings: Vec<String>,
    pub(crate) for_text: Option<String>,
}

impl Default for LatexDialog {
    fn default() -> Self {
        Self {
            open: false,
            text: r"E2 = \sum_{abij} V_{ijab} T_{abij}".into(),
            status: Ok(String::new()),
            warnings: Vec::new(),
            for_text: None,
        }
    }
}

#[derive(Default)]
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
    /// Arc tool: both ends chosen, the bulge follows the pointer.
    arc_end: Option<EdgeEnd>,
    texture: Option<(egui::TextureHandle, TextureKey)>,
    pub(crate) tab: Tab,
    pub(crate) show_grid: bool,
    pub(crate) message: Option<String>,
    pub(crate) show_shortcuts: bool,
    pub(crate) show_about: bool,
    pub(crate) show_licenses: bool,
    pub(crate) latex: LatexDialog,
    pub(crate) source: SourceState,
    confirm: Option<Pending>,
    pub(crate) focus_text_content: bool,
    pub(crate) rename: Option<(usize, String)>,
    canvas_rect: egui::Rect,
    pointer_doc: Option<Point>,
    allow_close: bool,
    title: String,
    pub(crate) icons: Icons,
    pub(crate) show_rulers: bool,
    pub(crate) show_ids: bool,
    /// Key conventions and labels (Cmd on macOS, Ctrl elsewhere).
    pub(crate) os: Os,
    /// Last copied objects as `.gold` text.
    clipboard: Option<String>,
    /// Text delivered by the platform's paste shortcut.
    paste_text: Option<String>,
    pub(crate) hover: Option<ObjRef>,
    /// All tabs; the entry of the active tab is a placeholder while its
    /// state is in the fields above.
    pages: Vec<Page>,
    pub(crate) active: usize,
    untitled: usize,
    next_untitled: usize,
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
                double: false,
                double_sep: None,
            },
            selection: Selection::new(),
            drag: None,
            pending_edge: None,
            arc_end: None,
            texture: None,
            tab: Tab::FillStroke,
            show_grid: true,
            message: None,
            show_shortcuts: false,
            show_about: false,
            show_licenses: false,
            latex: LatexDialog::default(),
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
            icons: Icons::default(),
            show_rulers: true,
            show_ids: false,
            os: Os::current(),
            clipboard: None,
            paste_text: None,
            hover: None,
            pages: vec![Page::default()],
            active: 0,
            untitled: 1,
            next_untitled: 2,
        };
        if let Some(p) = path {
            app.open_path(&p);
        }
        app
    }

    /// Opens more files, each in its own tab.
    pub fn open_paths(&mut self, paths: &[PathBuf]) {
        for p in paths {
            self.open_path(p);
        }
    }

    // ---- tabs -----------------------------------------------------------------

    /// Number of open tabs.
    pub fn tab_count(&self) -> usize {
        self.pages.len()
    }

    /// Index of the active tab.
    pub fn active_tab(&self) -> usize {
        self.active
    }

    /// Moves the active diagram's state out of the app fields.
    fn take_page(&mut self) -> Page {
        self.drag = None;
        self.pending_edge = None;
        self.arc_end = None;
        self.hover = None;
        self.rename = None;
        Page {
            doc: std::mem::take(&mut self.doc),
            file: self.file.take(),
            loaded_text: self.loaded_text.take(),
            saved_doc: std::mem::take(&mut self.saved_doc),
            history: std::mem::take(&mut self.history),
            view: std::mem::take(&mut self.view),
            view_fitted: self.view_fitted,
            selection: std::mem::take(&mut self.selection),
            source: std::mem::take(&mut self.source),
            untitled: self.untitled,
        }
    }

    fn put_page(&mut self, p: Page) {
        self.doc = p.doc;
        self.file = p.file;
        self.loaded_text = p.loaded_text;
        self.saved_doc = p.saved_doc;
        self.history = p.history;
        self.view = p.view;
        self.view_fitted = p.view_fitted;
        self.selection = p.selection;
        self.source = p.source;
        self.untitled = p.untitled;
        // The canvas is rebuilt for the new document.
        self.scene_for = None;
        self.scene_errors.clear();
        self.scene_version += 1;
    }

    /// Makes tab `i` the active one.
    pub fn switch_tab(&mut self, i: usize) {
        if i == self.active || i >= self.pages.len() {
            return;
        }
        let cur = self.take_page();
        self.pages[self.active] = cur;
        let next = std::mem::take(&mut self.pages[i]);
        self.active = i;
        self.put_page(next);
        self.sync_scene();
    }

    /// Opens a new tab with `doc` and makes it active.
    fn new_tab(&mut self, doc: Document, file: Option<PathBuf>, text: Option<String>) {
        let cur = self.take_page();
        self.pages[self.active] = cur;
        self.pages.push(Page::default());
        self.active = self.pages.len() - 1;
        self.put_page(Page::default());
        self.untitled = if file.is_none() {
            self.next_untitled += 1;
            self.next_untitled - 1
        } else {
            0
        };
        self.reset_with(doc, file, text);
    }

    /// Closes the active tab without asking; the last tab becomes empty.
    fn close_active(&mut self) {
        if self.pages.len() == 1 {
            self.reset_with(Document::default(), None, None);
            self.untitled = 1;
            self.next_untitled = 2;
            return;
        }
        self.take_page();
        self.pages.remove(self.active);
        self.active = self.active.min(self.pages.len() - 1);
        let next = std::mem::take(&mut self.pages[self.active]);
        self.put_page(next);
        self.sync_scene();
    }

    /// Closes tab `i`, asking first if it has unsaved changes.
    pub fn close_tab(&mut self, i: usize, ctx: &egui::Context) {
        self.switch_tab(i);
        self.request(Pending::Close, ctx);
    }

    /// Tab titles with an unsaved-changes mark, in order.
    pub fn tab_titles(&self) -> Vec<(String, bool)> {
        (0..self.pages.len())
            .map(|i| {
                let (file, untitled, dirty) = if i == self.active {
                    (&self.file, self.untitled, self.dirty())
                } else {
                    let p = &self.pages[i];
                    (&p.file, p.untitled, p.dirty())
                };
                (page_name(file.as_deref(), untitled), dirty)
            })
            .collect()
    }

    /// Full path of tab `i`'s file.
    pub(crate) fn tab_path(&self, i: usize) -> Option<&Path> {
        if i == self.active {
            self.file.as_deref()
        } else {
            self.pages.get(i)?.file.as_deref()
        }
    }

    fn any_dirty(&self) -> Option<usize> {
        if self.dirty() {
            return Some(self.active);
        }
        self.pages
            .iter()
            .enumerate()
            .find(|(i, p)| *i != self.active && p.dirty())
            .map(|(i, _)| i)
    }

    /// Whether the active tab is an untouched empty diagram (a file opened
    /// now replaces it instead of getting a tab of its own).
    fn active_is_blank(&self) -> bool {
        self.file.is_none() && !self.dirty() && self.doc == Document::default()
    }

    /// The document being edited.
    pub fn document(&self) -> &Document {
        &self.doc
    }

    /// The selected objects.
    pub fn selected(&self) -> Vec<ObjRef> {
        self.selection.iter().copied().collect()
    }

    /// The `.gold` text of the last copy, if any.
    pub fn clipboard_text(&self) -> Option<&str> {
        self.clipboard.as_deref()
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
        self.arc_end = None;
        self.view_fitted = false;
        self.source.for_doc = None;
        self.rename = None;
    }

    /// Opens a file in a tab: the tab already showing it, the blank
    /// active tab, or a new one.
    pub(crate) fn open_path(&mut self, path: &Path) {
        let canon = |p: &Path| std::fs::canonicalize(p).ok();
        if let Some(c) = canon(path)
            && let Some(i) =
                (0..self.pages.len()).find(|i| self.tab_path(*i).and_then(canon) == Some(c.clone()))
        {
            self.switch_tab(i);
            self.message = Some(format!("{} is already open", path.display()));
            return;
        }
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
                if self.active_is_blank() {
                    self.untitled = if file.is_none() { self.untitled } else { 0 };
                    self.reset_with(doc, file, Some(src));
                } else {
                    self.new_tab(doc, file, Some(src));
                }
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
                self.untitled = 0;
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

    /// Runs `p`, first asking about unsaved changes (for Quit: of each
    /// tab in turn, which is shown while asking).
    fn request(&mut self, p: Pending, ctx: &egui::Context) {
        let dirty = match p {
            Pending::Close => self.dirty().then_some(self.active),
            Pending::Quit => self.any_dirty(),
        };
        match dirty {
            Some(i) => {
                self.switch_tab(i);
                self.confirm = Some(p);
            }
            None => self.perform(p, ctx),
        }
    }

    fn new_diagram(&mut self) {
        self.new_tab(Document::default(), None, None);
        self.message = Some("New diagram".into());
    }

    fn open_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Goldstone diagram", &["gold", "svg"])
            .pick_files()
        {
            self.open_paths(&paths);
        }
    }

    fn perform(&mut self, p: Pending, ctx: &egui::Context) {
        match p {
            Pending::Close => self.close_active(),
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
        self.arc_end = None;
    }

    pub(crate) fn do_action(&mut self, a: Action, ctx: &egui::Context) {
        let canvas = self.canvas_rect;
        match a {
            Action::ToolSelect => self.set_tool(Tool::Select),
            Action::ToolEdit => self.set_tool(Tool::Edit),
            Action::ToolEdge => self.set_tool(Tool::Edge),
            Action::ToolArc => self.set_tool(Tool::Arc),
            Action::ToolVertex => self.set_tool(Tool::Vertex),
            Action::ToolCross => self.set_tool(Tool::Cross),
            Action::ToolText => self.set_tool(Tool::Text),
            Action::ToolRect => self.set_tool(Tool::Rect),
            Action::ToolEllipse => self.set_tool(Tool::Ellipse),
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
                let drawing = self.pending_edge.take().is_some() | self.arc_end.take().is_some();
                if !drawing && self.drag.take().is_none() {
                    self.selection.clear();
                }
            }
            Action::FlipH => doc_ops::flip(&mut self.doc, &self.selection, true),
            Action::FlipV => doc_ops::flip(&mut self.doc, &self.selection, false),
            Action::Rotate(deg) => doc_ops::rotate(&mut self.doc, &self.selection, deg as f64),
            Action::Copy => {
                self.copy(ctx);
            }
            Action::Cut => {
                if self.copy(ctx) {
                    let sel = std::mem::take(&mut self.selection);
                    doc_ops::delete(&mut self.doc, &sel);
                }
            }
            Action::Paste | Action::PasteInPlace => {
                let text = self.paste_text.take().or_else(|| self.clipboard.clone());
                match text {
                    Some(t) => self.paste(&t, a == Action::PasteInPlace),
                    None => self.message = Some("Nothing to paste".into()),
                }
            }
            Action::Duplicate => {
                if !self.selection.is_empty() {
                    let part = doc_ops::extract(&self.doc, &self.selection);
                    self.selection = doc_ops::merge(&mut self.doc, part);
                }
            }
            Action::SelectNext | Action::SelectPrev => {
                let all: Vec<ObjRef> = doc_ops::all_objects(&self.doc).into_iter().collect();
                if !all.is_empty() {
                    let n = all.len();
                    let next = match self
                        .selection
                        .last()
                        .and_then(|o| all.iter().position(|x| x == o))
                    {
                        None if a == Action::SelectNext => 0,
                        None => n - 1,
                        Some(i) if a == Action::SelectNext => {
                            // Skip the rest of the current group.
                            (1..=n)
                                .map(|k| (i + k) % n)
                                .find(|j| !self.selection.contains(&all[*j]))
                                .unwrap_or(i)
                        }
                        Some(_) => {
                            let first = self
                                .selection
                                .first()
                                .and_then(|o| all.iter().position(|x| x == o))
                                .unwrap_or(0);
                            (first + n - 1) % n
                        }
                    };
                    self.selection = doc_ops::with_group(&self.doc, all[next]);
                }
            }
            Action::SelectSame(same) => {
                if self.selection.is_empty() {
                    self.message = Some("Select an object first".into());
                } else {
                    self.selection = doc_ops::select_same(&self.doc, &self.selection, same);
                }
            }
            Action::Group => match doc_ops::group(&mut self.doc, &self.selection) {
                Some(id) => {
                    self.message = Some(format!("Grouped {} objects as {id}", self.selection.len()))
                }
                None => self.message = Some("Select at least two objects to group".into()),
            },
            Action::Ungroup => {
                let n = doc_ops::ungroup(&mut self.doc, &self.selection);
                self.message = Some(match n {
                    0 => "No group selected".into(),
                    1 => "Ungrouped 1 group".into(),
                    n => format!("Ungrouped {n} groups"),
                });
            }
            Action::Align(op) => {
                let units = self.align_units();
                doc_ops::align(&mut self.doc, &units, op);
            }
            Action::Bool(op) => match doc_ops::boolean(&mut self.doc, &self.selection, op) {
                Ok(i) => self.selection = [ObjRef::Shape(i)].into(),
                Err(e) => self.message = Some(e),
            },
            Action::ObjectToPath => {
                let n = doc_ops::object_to_path(&mut self.doc, &self.selection);
                self.message = Some(format!("Converted {n} shape(s) to paths"));
            }
            Action::ToggleRulers => self.show_rulers = !self.show_rulers,
            Action::ToggleIds => self.show_ids = !self.show_ids,
            Action::ShowAlign => self.tab = Tab::Align,
            Action::Quit => self.request(Pending::Quit, ctx),
            Action::FromLatex => self.latex.open = true,
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
            Action::New => self.new_diagram(),
            Action::Open => self.open_dialog(),
            Action::CloseTab => self.request(Pending::Close, ctx),
            Action::NextTab => self.switch_tab((self.active + 1) % self.pages.len()),
            Action::PrevTab => {
                self.switch_tab((self.active + self.pages.len() - 1) % self.pages.len())
            }
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
        let os = self.os;
        let mut fired = Vec::new();
        let mut paste = None;
        ctx.input_mut(|i| {
            for bnd in BINDINGS.iter().filter(|b| b.only.includes(os)) {
                let mut hit = false;
                i.events.retain(|e| {
                    if hit {
                        return true;
                    }
                    hit = match (bnd.trigger, e) {
                        (
                            Trigger::Key(mods, key),
                            egui::Event::Key {
                                key: k,
                                pressed: true,
                                modifiers: m,
                                ..
                            },
                        ) => *k == key && keys::matches(mods, key, *m, os),
                        (Trigger::Text(t), egui::Event::Text(s)) => s == t,
                        (Trigger::Copy, egui::Event::Copy) => true,
                        (Trigger::Cut, egui::Event::Cut) => true,
                        (Trigger::Paste, egui::Event::Paste(text)) => {
                            paste = Some(text.clone());
                            true
                        }
                        _ => false,
                    };
                    !hit
                });
                if hit {
                    fired.push(bnd.action);
                }
            }
        });
        if paste.is_some() {
            self.paste_text = paste;
        }
        for a in fired {
            self.do_action(a, ctx);
        }
    }

    // ---- clipboard ------------------------------------------------------------

    /// Copies the selection; the clipboard gets its `.gold` text.
    fn copy(&mut self, ctx: &egui::Context) -> bool {
        if self.selection.is_empty() {
            self.message = Some("Nothing selected to copy".into());
            return false;
        }
        let part = doc_ops::extract(&self.doc, &self.selection);
        let text = io::to_toml(&part);
        ctx.copy_text(text.clone());
        self.clipboard = Some(text);
        self.message = Some(format!(
            "Copied {} object(s); the clipboard holds their .gold text",
            self.selection.len()
        ));
        true
    }

    /// Pastes `.gold` text: at the pointer, or where it was (`in_place`).
    fn paste(&mut self, text: &str, in_place: bool) {
        let Ok(part) = io::parse(text) else {
            self.message = Some("The clipboard does not hold Goldstone objects".into());
            return;
        };
        let new = doc_ops::merge(&mut self.doc, part);
        if !in_place {
            let target = match self.pointer_doc {
                Some(p) => self.snap(p, false, &BTreeSet::new()).0,
                None => {
                    let g = self.grid();
                    doc_ops::bounds(&doc_ops::selection_points(&self.doc, &new))
                        .map_or(Point::ZERO, |b| b.center() + Vec2::new(g, -g))
                }
            };
            if let Some(b) = doc_ops::bounds(&doc_ops::selection_points(&self.doc, &new)) {
                let c = self.snap(b.center(), !self.show_grid, &BTreeSet::new()).0;
                doc_ops::translate(&mut self.doc, &new, target - c);
            }
        }
        self.message = Some(format!("Pasted {} object(s)", new.len()));
        self.selection = new;
    }

    /// Adds the diagram of the LaTeX expression in the dialog: to the
    /// right of the drawing, or as the whole document (`replace`).
    pub(crate) fn insert_latex(&mut self, replace: bool) {
        let part = match goldstone_core::expr::from_latex(&self.latex.text) {
            Ok(d) => d.doc,
            Err(e) => {
                self.latex.status = Err(e);
                return;
            }
        };
        if replace {
            self.doc.vertices.clear();
            self.doc.edges.clear();
            self.doc.texts.clear();
            self.doc.axes.clear();
            self.doc.shapes.clear();
            self.doc.groups.clear();
        }
        let existing = self.page();
        let new = doc_ops::merge(&mut self.doc, part);
        if let (Some(old), Some(b)) = (
            existing.filter(|_| !replace),
            doc_ops::bounds(&doc_ops::selection_points(&self.doc, &new)),
        ) {
            let g = self.grid();
            let x = ((old.x1 + 40.0 - b.x0) / g).round() * g;
            let y = ((old.y0 + self.scene.margin - b.y0) / g).round() * g;
            doc_ops::translate(&mut self.doc, &new, Vec2::new(x, y));
        }
        self.message = Some(format!(
            "Drew {}: {} lines",
            self.latex.text.trim(),
            new.iter().filter(|o| matches!(o, ObjRef::Edge(_))).count()
        ));
        self.selection = new;
        self.view_fitted = false;
    }

    /// Units for Align & Distribute: groups move as one; a selected edge
    /// moves its vertices unless those are selected themselves.
    fn align_units(&self) -> Vec<(Selection, Rect)> {
        let bounds: std::collections::BTreeMap<ObjRef, Rect> =
            view::object_bounds(&self.doc, &self.scene, &self.text_boxes)
                .into_iter()
                .collect();
        let mut units: Vec<Selection> = Vec::new();
        let mut seen = Selection::new();
        for o in &self.selection {
            if seen.contains(o) {
                continue;
            }
            let unit = match doc_ops::group_of(&self.doc, *o) {
                Some(g) => doc_ops::group_members(&self.doc, g),
                None => [*o].into(),
            };
            seen.extend(unit.iter().copied());
            units.push(unit);
        }
        let selected_vertices = doc_ops::moved_vertices(
            &self.doc,
            &self
                .selection
                .iter()
                .filter(|o| matches!(o, ObjRef::Vertex(_)))
                .copied()
                .collect(),
        );
        units
            .into_iter()
            .filter(|u| {
                // A lone edge whose ends are aligned on their own is skipped.
                !(u.len() == 1
                    && matches!(u.first(), Some(ObjRef::Edge(_)))
                    && doc_ops::moved_vertices(&self.doc, u)
                        .iter()
                        .any(|v| selected_vertices.contains(v)))
            })
            .filter_map(|u| {
                let r = u
                    .iter()
                    .filter_map(|o| bounds.get(o).copied())
                    .reduce(|a, b| a.union(b))?;
                Some((u, r))
            })
            .collect()
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

    /// The point the arc passes through for pointer `p`; with Ctrl the
    /// bend snaps to 15° steps.
    fn arc_through(&self, start: EdgeEnd, end: EdgeEnd, p: Point, ctrl: bool) -> Point {
        let (a, b) = (self.end_point(start), self.end_point(end));
        if !ctrl || start == end {
            return p;
        }
        let bend = (doc_ops::bend_through(a, b, p) / 15.0).round() * 15.0;
        if bend == 0.0 {
            return a.midpoint(b);
        }
        let al = geom::ArcLen::new(&geom::bent(a, b, bend));
        al.eval(al.length() / 2.0).0
    }

    /// Preview path of the arc (or loop) being drawn.
    fn arc_preview(&self, start: EdgeEnd, end: EdgeEnd, through: Point) -> BezPath {
        let (a, b) = (self.end_point(start), self.end_point(end));
        if start == end {
            let r = ((through - a).hypot() / 2.0).max(1.0);
            geom::loop_at(a, r, (through - a).atan2().to_degrees())
        } else {
            geom::bent(a, b, doc_ops::bend_through(a, b, through))
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
        if self.opts.double {
            e.double = Some(true);
            e.double_sep = self.opts.double_sep;
        }
        if is_loop {
            let angle = through.map_or(90.0, |p| (p - a).atan2().to_degrees().round());
            let radius = through
                .map(|p| doc_ops::round((p - a).hypot() / 2.0).max(1.0))
                .filter(|r| *r >= 2.0);
            e.loop_ = Some(LoopSpec {
                radius,
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

    /// `o` with its group, unless `inside` (Ctrl+click enters groups).
    fn unit(&self, o: ObjRef, inside: bool) -> Selection {
        if inside {
            [o].into()
        } else {
            doc_ops::with_group(&self.doc, o)
        }
    }

    fn click_select(&mut self, p: Point, shift: bool, inside: bool) {
        match self.hit(p) {
            Some(o) if shift => {
                let unit = self.unit(o, inside);
                if self.selection.contains(&o) {
                    unit.iter().for_each(|u| {
                        self.selection.remove(u);
                    });
                } else {
                    self.selection.extend(unit);
                }
            }
            Some(o) => self.selection = self.unit(o, inside),
            None if !shift => self.selection.clear(),
            None => {}
        }
    }

    fn start_move(&mut self, o: ObjRef, press: Point, shift: bool) {
        if !self.selection.contains(&o) {
            let unit = self.unit(o, false);
            if shift {
                self.selection.extend(unit);
            } else {
                self.selection = unit;
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
        let (full, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let rect = if self.show_rulers {
            egui::Rect::from_min_max(full.min + egui::vec2(RULER, RULER), full.max)
        } else {
            full
        };
        let resp = ui.interact(
            rect,
            ui.scope_id().with("goldstone-canvas"),
            Sense::click_and_drag(),
        );
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
        self.hover = self.pointer_doc.and_then(|p| self.hit(p));

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
            let drawing = self.pending_edge.take().is_some() | self.arc_end.take().is_some();
            // Right-click on an object outside the selection selects it,
            // so the menu acts on what was clicked (as in Inkscape).
            if !drawing && let Some(o) = self.hover.filter(|o| !self.selection.contains(o)) {
                self.selection = self.unit(o, false);
            }
        }
        resp.context_menu(|ui| self.canvas_menu(ui));

        self.sync_scene();
        self.paint(ui, rect);
        if self.show_rulers {
            self.paint_rulers(ui, full, rect);
        }
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
        let (shift, ctrl, alt) = (mods.shift, mods.command, mods.alt);

        if resp.clicked_by(PointerButton::Primary) {
            let Some(p) = pointer else { return };
            match self.tool {
                Tool::Select | Tool::Edit => self.click_select(p, shift, ctrl),
                Tool::Rect | Tool::Ellipse => self.click_select(p, shift, ctrl),
                Tool::Edge => {
                    let (q, v) = self.snap(p, shift, &BTreeSet::new());
                    let end = v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex);
                    match self.pending_edge.take() {
                        None => self.pending_edge = Some(end),
                        Some(start) if start == end => {
                            self.pending_edge = Some(start);
                            self.message = Some("For a loop use the Arc tool (A)".into());
                        }
                        Some(start) => self.finish_edge(start, end, None),
                    }
                }
                Tool::Arc => {
                    if let (Some(start), Some(end)) = (self.pending_edge, self.arc_end) {
                        let through = self.arc_through(start, end, p, ctrl);
                        self.pending_edge = None;
                        self.arc_end = None;
                        self.finish_edge(start, end, Some(through));
                    } else {
                        let (q, v) = self.snap(p, shift, &BTreeSet::new());
                        let end = v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex);
                        match self.pending_edge {
                            None => self.pending_edge = Some(end),
                            Some(start) => {
                                let loop_ = matches!(end, EdgeEnd::Vertex(_)) && start == end;
                                if loop_ || (self.end_point(start) - q).hypot() > 1e-6 {
                                    self.arc_end = Some(end);
                                }
                            }
                        }
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
                Tool::Select if alt => Some(Drag::Touch {
                    points: vec![self.view.to_screen(rect, p)],
                    add: shift,
                }),
                Tool::Rect | Tool::Ellipse => {
                    let (q, _) = self.snap(p, shift, &BTreeSet::new());
                    Some(Drag::ShapeNew {
                        start: q,
                        kind: if self.tool == Tool::Rect {
                            ShapeKind::Box
                        } else {
                            ShapeKind::Ellipse
                        },
                    })
                }
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
                        Some(start) => Some(Drag::EdgeFree { start }),
                        None => Some(Drag::EdgeFree { start: at }),
                    }
                }
                Tool::Arc if self.arc_end.is_none() => {
                    let (q, v) = self.snap(p, shift, &BTreeSet::new());
                    let at = v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex);
                    match self.pending_edge.take() {
                        // Press at the end point and drag: arc through the release point.
                        Some(start) => Some(Drag::EdgeTo { start, end: at }),
                        // Drag from start to end; then set the bulge.
                        None => Some(Drag::EdgeFree { start: at }),
                    }
                }
                Tool::Arc => None,
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
                Some(Drag::Touch { mut points, add }) => {
                    let s = self.view.to_screen(rect, p);
                    if points.last().is_none_or(|l| l.distance(s) >= 1.0) {
                        points.push(s);
                    }
                    self.drag = Some(Drag::Touch { points, add });
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
                        let inside: Selection =
                            view::object_bounds(&self.doc, &self.scene, &self.text_boxes)
                                .into_iter()
                                .filter(|(_, b)| {
                                    r.contains(b.origin()) && r.contains(Point::new(b.x1, b.y1))
                                })
                                .map(|(o, _)| o)
                                .collect();
                        // A group is picked only when all of it is inside.
                        let inside: Vec<ObjRef> = inside
                            .iter()
                            .filter(|o| {
                                doc_ops::with_group(&self.doc, **o)
                                    .iter()
                                    .all(|m| inside.contains(m))
                            })
                            .copied()
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
                        let through = self.arc_through(start, e, p, ctrl);
                        self.finish_edge(start, e, Some(through));
                    }
                }
                Some(Drag::EdgeFree { start }) => {
                    if let Some(p) = end {
                        let (q, v) = self.snap(p, shift, &BTreeSet::new());
                        let e = v.map_or(EdgeEnd::Point(q), EdgeEnd::Vertex);
                        if self.tool == Tool::Arc {
                            if start == e && matches!(e, EdgeEnd::Point(_)) {
                                self.pending_edge = Some(start);
                            } else {
                                self.pending_edge = Some(start);
                                self.arc_end = Some(e);
                            }
                        } else if start != e {
                            self.finish_edge(start, e, None);
                        }
                    }
                }
                Some(Drag::Touch { points, add }) => {
                    let tol = PICK_PX / self.view.zoom;
                    let mut touched = Selection::new();
                    for w in points.windows(2) {
                        let n = (w[0].distance(w[1]) / 2.0).ceil().max(1.0) as usize;
                        for k in 0..=n {
                            let s = w[0] + (w[1] - w[0]) * (k as f32 / n as f32);
                            let p = self.view.to_doc(rect, s);
                            touched.extend(view::hit_all(
                                &self.doc,
                                &self.scene,
                                &self.text_boxes,
                                p,
                                tol,
                            ));
                        }
                    }
                    let touched: Selection = touched
                        .into_iter()
                        .flat_map(|o| doc_ops::with_group(&self.doc, o))
                        .collect();
                    if add {
                        self.selection.extend(touched);
                    } else {
                        self.selection = touched;
                    }
                }
                Some(Drag::ShapeNew { start, kind }) => {
                    if let Some(p) = end {
                        let b = self.shape_corner(start, p, shift, ctrl);
                        if (b.x - start.x).abs() > 1e-6 && (b.y - start.y).abs() > 1e-6 {
                            self.doc.shapes.push(doc_ops::new_shape(kind, start, b));
                            self.selection = [ObjRef::Shape(self.doc.shapes.len() - 1)].into();
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Opposite corner of a shape being drawn: snapped, and square with Ctrl.
    fn shape_corner(&self, start: Point, p: Point, shift: bool, ctrl: bool) -> Point {
        let (mut q, _) = self.snap(p, shift, &BTreeSet::new());
        if ctrl {
            let d = q - start;
            let m = d.x.abs().max(d.y.abs());
            q = start + Vec2::new(m.copysign(d.x), m.copysign(d.y));
        }
        q
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

        // Arc with both ends chosen: the bulge follows the pointer.
        if self.tool == Tool::Arc
            && let (Some(start), Some(end), Some(p)) =
                (self.pending_edge, self.arc_end, self.pointer_doc)
        {
            let ctrl = ui.input(|i| i.modifiers.command);
            let path = self.arc_preview(start, end, self.arc_through(start, end, p, ctrl));
            for line in self.screen_path(rect, &path) {
                painter.add(egui::Shape::line(line, EStroke::new(1.5, ACCENT)));
            }
            for e in [start, end] {
                painter.circle_stroke(
                    self.view.to_screen(rect, self.end_point(e)),
                    4.0,
                    EStroke::new(1.5, ACCENT),
                );
            }
        } else if matches!(self.tool, Tool::Edge | Tool::Arc)
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
            Some(Drag::EdgeFree { start }) => {
                if let Some(p) = self.pointer_doc {
                    let a = self.view.to_screen(rect, self.end_point(*start));
                    painter
                        .line_segment([a, self.view.to_screen(rect, p)], EStroke::new(1.5, ACCENT));
                }
            }
            Some(Drag::EdgeTo { start, end }) => {
                if let Some(p) = self.pointer_doc {
                    let ctrl = ui.input(|i| i.modifiers.command);
                    let path =
                        self.arc_preview(*start, *end, self.arc_through(*start, *end, p, ctrl));
                    for line in self.screen_path(rect, &path) {
                        painter.add(egui::Shape::line(line, EStroke::new(1.5, ACCENT)));
                    }
                }
            }
            Some(Drag::Touch { points, .. }) => {
                painter.add(egui::Shape::line(
                    points.clone(),
                    EStroke::new(1.5, Color32::from_rgb(220, 40, 40)),
                ));
            }
            Some(Drag::ShapeNew { start, kind }) => {
                if let Some(p) = self.pointer_doc {
                    let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
                    let b = self.shape_corner(*start, p, shift, ctrl);
                    let r = egui::Rect::from_two_pos(
                        self.view.to_screen(rect, *start),
                        self.view.to_screen(rect, b),
                    );
                    let st = EStroke::new(1.5, ACCENT);
                    if *kind == ShapeKind::Box {
                        painter.rect_stroke(r, 0.0, st, StrokeKind::Middle);
                    } else {
                        painter.add(egui::Shape::ellipse_stroke(r.center(), r.size() / 2.0, st));
                    }
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

        if self.show_ids {
            self.paint_ids(&painter, rect);
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

    /// Ids and coordinates next to every object.
    fn paint_ids(&self, painter: &egui::Painter, rect: egui::Rect) {
        let ids = scene::ids(&self.doc);
        let font = FontId::monospace(10.0);
        let bg = painter
            .ctx()
            .global_style()
            .visuals
            .extreme_bg_color
            .gamma_multiply(0.85);
        let tag = |at: Point, text: String| {
            let s = self.view.to_screen(rect, at) + egui::vec2(5.0, -5.0);
            let galley = painter.layout_no_wrap(text, font.clone(), ACCENT);
            let r = egui::Rect::from_min_size(s - egui::vec2(0.0, galley.size().y), galley.size())
                .expand(1.5);
            painter.rect_filled(r, 2.0, bg);
            painter.galley(r.min + egui::vec2(1.5, 1.5), galley, ACCENT);
        };
        for v in &self.doc.vertices {
            tag(
                doc_ops::pt(v.at),
                format!("{} {}", v.id, inspect::point(v.at)),
            );
        }
        for (i, path) in self.scene.edge_paths.iter().enumerate() {
            if let Some(path) = path {
                let al = geom::ArcLen::new(path);
                let mid = al.eval(al.length() / 2.0).0;
                tag(mid, ids.of(&self.doc, ObjRef::Edge(i)));
            }
        }
        for (i, t) in self.doc.texts.iter().enumerate() {
            tag(
                doc_ops::pt(t.at),
                format!(
                    "{} {}",
                    ids.of(&self.doc, ObjRef::Text(i)),
                    inspect::point(t.at)
                ),
            );
        }
        for (i, a) in self.doc.axes.iter().enumerate() {
            tag(
                doc_ops::pt(a.origin),
                format!(
                    "{} {}",
                    ids.of(&self.doc, ObjRef::Axes(i)),
                    inspect::point(a.origin)
                ),
            );
        }
        for (i, s) in self.doc.shapes.iter().enumerate() {
            if let Ok(o) = goldstone_core::shapes::outline(s) {
                use goldstone_core::kurbo::Shape as _;
                let b = o.bounding_box();
                tag(
                    Point::new(b.x0, b.y1),
                    format!(
                        "{} {}",
                        ids.of(&self.doc, ObjRef::Shape(i)),
                        inspect::point([b.x0, b.y1])
                    ),
                );
            }
        }
    }

    /// Inkscape-style rulers along the top and left, in pt.
    fn paint_rulers(&self, ui: &egui::Ui, full: egui::Rect, rect: egui::Rect) {
        let painter = ui.painter_at(full);
        let vis = ui.visuals();
        let bg = vis.faint_bg_color;
        let fg = vis.text_color();
        let tick = EStroke::new(1.0, fg.gamma_multiply(0.7));
        let top = egui::Rect::from_min_max(
            Pos2::new(rect.left(), full.top()),
            Pos2::new(full.right(), rect.top()),
        );
        let left = egui::Rect::from_min_max(
            Pos2::new(full.left(), rect.top()),
            Pos2::new(rect.left(), full.bottom()),
        );
        painter.rect_filled(egui::Rect::from_min_max(full.min, rect.min), 0.0, bg);
        painter.rect_filled(top, 0.0, bg);
        painter.rect_filled(left, 0.0, bg);
        painter.line_segment([top.left_bottom(), top.right_bottom()], tick);
        painter.line_segment([left.right_top(), left.right_bottom()], tick);
        painter.text(
            full.min + egui::vec2(RULER / 2.0, RULER / 2.0),
            Align2::CENTER_CENTER,
            "pt",
            FontId::proportional(8.0),
            fg,
        );
        // Major step: 1, 2 or 5 × 10^k pt, at least 60 screen points apart.
        let raw = 60.0 / self.view.zoom;
        let mag = 10f64.powf(raw.log10().floor());
        let step = [1.0, 2.0, 5.0, 10.0]
            .iter()
            .map(|m| m * mag)
            .find(|s| *s >= raw)
            .unwrap_or(10.0 * mag);
        let minor = step
            / if (step / mag).round() == 2.0 {
                4.0
            } else {
                5.0
            };
        let v = self.view.visible(rect);
        let font = FontId::proportional(9.0);
        let label = |x: f64| inspect::num(x);
        let painter_top = ui.painter_at(top);
        let mut x = (v.x0 / minor).floor() * minor;
        while x <= v.x1 {
            let sx = self.view.to_screen(rect, Point::new(x, 0.0)).x;
            let major = ((x / step).round() * step - x).abs() < minor * 0.01;
            let h = if major { RULER * 0.7 } else { RULER * 0.25 };
            painter_top.line_segment(
                [Pos2::new(sx, top.bottom()), Pos2::new(sx, top.bottom() - h)],
                tick,
            );
            if major {
                painter_top.text(
                    Pos2::new(sx + 2.0, top.top() + 1.0),
                    Align2::LEFT_TOP,
                    label(x),
                    font.clone(),
                    fg,
                );
            }
            x += minor;
        }
        let painter_left = ui.painter_at(left);
        let mut y = (v.y0 / minor).floor() * minor;
        while y <= v.y1 {
            let sy = self.view.to_screen(rect, Point::new(0.0, y)).y;
            let major = ((y / step).round() * step - y).abs() < minor * 0.01;
            let w = if major { RULER * 0.7 } else { RULER * 0.25 };
            painter_left.line_segment(
                [Pos2::new(left.right(), sy), Pos2::new(left.right() - w, sy)],
                tick,
            );
            if major {
                let galley = painter_left.layout_no_wrap(label(y), font.clone(), fg);
                painter_left.add(
                    egui::epaint::TextShape::new(
                        Pos2::new(left.left() + 1.0, sy - 2.0),
                        galley,
                        fg,
                    )
                    .with_angle(-std::f32::consts::FRAC_PI_2),
                );
            }
            y += minor;
        }
        // Pointer position.
        if let Some(p) = self.pointer_doc {
            let s = self.view.to_screen(rect, p);
            let mark = EStroke::new(1.5, ACCENT);
            painter_top.line_segment(
                [Pos2::new(s.x, top.top()), Pos2::new(s.x, top.bottom())],
                mark,
            );
            painter_left.line_segment(
                [Pos2::new(left.left(), s.y), Pos2::new(left.right(), s.y)],
                mark,
            );
        }
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
                ui.label(format!(
                    "{} has unsaved changes.",
                    page_name(self.file.as_deref(), self.untitled)
                ));
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
                    // Quit asks again for the next tab with changes.
                    self.request(p, ctx);
                }
            }
            Some(1) => {
                self.confirm = None;
                match p {
                    Pending::Quit => {
                        // Discard this tab's changes, then ask about the next.
                        self.doc = self.saved_doc.clone();
                        self.request(p, ctx);
                    }
                    Pending::Close => self.perform(p, ctx),
                }
            }
            Some(2) => self.confirm = None,
            _ => {}
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = page_name(self.file.as_deref(), self.untitled);
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

    /// The hovered object, or the single selected one: kind, id, where.
    pub(crate) fn object_text(&self) -> Option<String> {
        let o = self
            .hover
            .or(match (self.selection.len(), self.selection.first()) {
                (1, Some(o)) => Some(*o),
                _ => None,
            })?;
        if !doc_ops::exists(&self.doc, o) {
            return None;
        }
        let id = scene::ids(&self.doc).of(&self.doc, o);
        let group = doc_ops::group_of(&self.doc, o).map_or(String::new(), |g| {
            format!("  [group {}]", self.doc.groups[g].id)
        });
        Some(format!(
            "{} {id}: {}{group}",
            inspect::kind(o),
            inspect::summary(&self.doc, o)
        ))
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
        if ctx.input(|i| i.viewport().close_requested())
            && self.any_dirty().is_some()
            && !self.allow_close
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Pending::Quit, &ctx);
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        self.open_paths(&dropped);
        // egui moves keyboard focus between widgets with Tab and the arrow
        // keys; while no widget has focus those keys belong to the canvas.
        if ctx.memory(|m| m.focused().is_none()) {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        self.handle_keys(&ctx);
        self.sync_scene();

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("commands").show(ui, |ui| self.commands_bar(ui));
        egui::Panel::top("tool-controls").show(ui, |ui| self.tool_controls(ui));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("palettes").show(ui, |ui| self.palettes(ui));
        egui::Panel::left("toolbox")
            .resizable(false)
            .exact_size(52.0)
            .show(ui, |ui| self.toolbox(ui));
        egui::Panel::right("dialogs")
            .resizable(true)
            .default_size(320.0)
            .min_size(240.0)
            .show(ui, |ui| self.dialogs(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                self.tab_bar(ui);
                self.canvas(ui);
            });

        self.confirm_dialog(&ctx);
        self.shortcuts_window(&ctx);
        self.about_window(&ctx);
        self.latex_window(&ctx);

        let interacting =
            ctx.input(|i| i.pointer.any_down()) || ctx.memory(|m| m.focused().is_some());
        if !interacting {
            self.history.settle(&self.doc);
        }
        self.sync_scene();
        self.update_title(&ctx);
    }
}

/// Name shown for a diagram: its file name, or "Untitled N".
fn page_name(file: Option<&Path>, untitled: usize) -> String {
    match file.and_then(|p| p.file_name()) {
        Some(n) => n.to_string_lossy().into_owned(),
        None if untitled > 1 => format!("Untitled {untitled}"),
        None => "Untitled".into(),
    }
}

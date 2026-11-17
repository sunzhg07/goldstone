//! The key-binding table. The editor dispatches keyboard input from this
//! table, and both the in-app shortcut list and the manual are generated
//! from it, so they cannot disagree.
//!
//! Bindings are written once with `CMD` for the platform's command key:
//! Cmd on macOS, Ctrl on Linux and Windows (egui's `Modifiers::COMMAND`).
//! Labels are formatted for the platform the editor runs on (`⇧⌘Z` on macOS,
//! `Ctrl+Shift+Z` elsewhere); a few bindings exist on one platform only,
//! following its conventions.

use egui::{Key, Modifiers};

/// Operating-system family, which decides key conventions and labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    /// Linux, Windows and the BSDs.
    Other,
}

impl Os {
    /// The platform this binary was built for.
    pub const fn current() -> Os {
        if cfg!(target_os = "macos") {
            Os::Mac
        } else {
            Os::Other
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Os::Mac => "macOS",
            Os::Other => "Linux / Windows",
        }
    }

    /// The command modifier as written in labels.
    pub fn cmd(self) -> &'static str {
        match self {
            Os::Mac => "⌘",
            Os::Other => "Ctrl",
        }
    }

    pub fn alt(self) -> &'static str {
        match self {
            Os::Mac => "⌥",
            Os::Other => "Alt",
        }
    }

    pub fn shift(self) -> &'static str {
        match self {
            Os::Mac => "⇧",
            Os::Other => "Shift",
        }
    }

    /// Replaces `{cmd}`, `{alt}` and `{shift}` in a description.
    pub fn fill(self, s: &str) -> String {
        let join = |m: &str| match self {
            Os::Mac => m.to_string(),
            Os::Other => format!("{m}+"),
        };
        s.replace("{cmd}", &join(self.cmd()))
            .replace("{alt}", &join(self.alt()))
            .replace("{shift}", &join(self.shift()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToolSelect,
    ToolEdit,
    ToolEdge,
    ToolArc,
    ToolVertex,
    ToolCross,
    ToolText,
    ToolRect,
    ToolEllipse,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    PasteInPlace,
    Duplicate,
    Delete,
    SelectAll,
    Deselect,
    SelectNext,
    SelectPrev,
    SelectSame(Same),
    Group,
    Ungroup,
    FlipH,
    FlipV,
    /// Rotate by this many degrees (counterclockwise).
    Rotate(i16),
    Nudge {
        dx: i8,
        dy: i8,
        step: Step,
    },
    Align(AlignOp),
    Bool(goldstone_core::shapes::BoolOp),
    ObjectToPath,
    ZoomIn,
    ZoomOut,
    Zoom1,
    ZoomSelection,
    ZoomDrawing,
    ZoomPage,
    ToggleGrid,
    ToggleRulers,
    ToggleIds,
    New,
    Open,
    CloseTab,
    NextTab,
    PrevTab,
    Save,
    SaveAs,
    Export,
    Quit,
    ShowFillStroke,
    ShowObjects,
    ShowAlign,
    ShowSource,
    ShowCode,
    FromLatex,
    FunctionPlot,
    Shortcuts,
}

/// Criteria for Edit ▸ Select Same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Same {
    FillAndStroke,
    StrokeColor,
    StrokeStyle,
    Class,
    Kind,
}

/// Align & Distribute operations (relative to the selection's bounding box).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignOp {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
    DistributeX,
    DistributeY,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Normal,
    Large,
    Pixel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trigger {
    Key(Modifiers, Key),
    /// A typed character, for keys egui has no `Key` for (e.g. `#`).
    Text(&'static str),
    /// The platform's copy / cut / paste shortcut, which egui delivers as
    /// events rather than key presses.
    Copy,
    Cut,
    Paste,
}

/// Which platforms a binding exists on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Only {
    All,
    Mac,
    NotMac,
}

impl Only {
    pub fn includes(self, os: Os) -> bool {
        match self {
            Only::All => true,
            Only::Mac => os == Os::Mac,
            Only::NotMac => os != Os::Mac,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub trigger: Trigger,
    pub action: Action,
    pub desc: &'static str,
    pub group: &'static str,
    /// Whether Inkscape uses the same binding for the same action.
    pub inkscape: bool,
    pub only: Only,
    /// Shown in the shortcut list (siblings such as the four arrow keys
    /// are listed once).
    pub listed: bool,
    /// Replaces the key name in labels (e.g. "Arrow" for the arrow keys).
    pub key_label: Option<&'static str>,
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::SHIFT;
const ALT: Modifiers = Modifiers::ALT;
const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};
const CTRL: Modifiers = Modifiers::CTRL;
const CTRL_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::CTRL
};
const CMD_ALT: Modifiers = Modifiers {
    alt: true,
    ..Modifiers::COMMAND
};

const fn b(
    mods: Modifiers,
    key: Key,
    action: Action,
    desc: &'static str,
    group: &'static str,
) -> Binding {
    Binding {
        trigger: Trigger::Key(mods, key),
        action,
        desc,
        group,
        inkscape: true,
        only: Only::All,
        listed: true,
        key_label: None,
    }
}

const fn special(
    trigger: Trigger,
    action: Action,
    desc: &'static str,
    group: &'static str,
) -> Binding {
    Binding {
        trigger,
        action,
        desc,
        group,
        inkscape: true,
        only: Only::All,
        listed: true,
        key_label: None,
    }
}

/// Not an Inkscape binding.
const fn own(mut x: Binding) -> Binding {
    x.inkscape = false;
    x
}

/// Not listed separately (a sibling of a listed binding).
const fn quiet(mut x: Binding) -> Binding {
    x.listed = false;
    x
}

const fn only(mut x: Binding, o: Only) -> Binding {
    x.only = o;
    x
}

const fn label(mut x: Binding, l: &'static str) -> Binding {
    x.key_label = Some(l);
    x
}

/// The four arrow keys with `mods`, listed once as "Arrow".
const fn arrows(mods: Modifiers, step: Step, desc: &'static str) -> [Binding; 4] {
    [
        label(
            b(
                mods,
                Key::ArrowLeft,
                Action::Nudge {
                    dx: -1,
                    dy: 0,
                    step,
                },
                desc,
                "Transform",
            ),
            "Arrow",
        ),
        quiet(b(
            mods,
            Key::ArrowRight,
            Action::Nudge { dx: 1, dy: 0, step },
            desc,
            "Transform",
        )),
        quiet(b(
            mods,
            Key::ArrowUp,
            Action::Nudge { dx: 0, dy: 1, step },
            desc,
            "Transform",
        )),
        quiet(b(
            mods,
            Key::ArrowDown,
            Action::Nudge {
                dx: 0,
                dy: -1,
                step,
            },
            desc,
            "Transform",
        )),
    ]
}

const TOOLS: &str = "Tools";
const EDIT: &str = "Editing";
const OBJ: &str = "Objects and paths";
const XF: &str = "Transform";
const VIEW: &str = "View";
const FILE: &str = "Dialogs and file";

const LARGE: [Binding; 4] = arrows(SHIFT, Step::Large, "Move selection by 20 pt");
const PIXEL: [Binding; 4] = arrows(ALT, Step::Pixel, "Move selection by one screen pixel");
const NORMAL: [Binding; 4] = arrows(NONE, Step::Normal, "Move selection by 2 pt");

/// Keyboard bindings. More specific modifier combinations come first.
pub const BINDINGS: &[Binding] = &[
    // Tools
    b(NONE, Key::S, Action::ToolSelect, "Select tool", TOOLS),
    b(NONE, Key::F1, Action::ToolSelect, "Select tool", TOOLS),
    b(
        NONE,
        Key::N,
        Action::ToolEdit,
        "Edit tool (bend, control points, loop size, arrow position)",
        TOOLS,
    ),
    b(
        NONE,
        Key::B,
        Action::ToolEdge,
        "Line tool (straight lines between vertices)",
        TOOLS,
    ),
    own(b(
        NONE,
        Key::A,
        Action::ToolArc,
        "Arc tool (click start, click end, click to set the bulge; same vertex twice: loop)",
        TOOLS,
    )),
    own(b(NONE, Key::P, Action::ToolVertex, "Vertex tool", TOOLS)),
    own(b(
        NONE,
        Key::X,
        Action::ToolCross,
        "Cross tool (one-body operator ---X)",
        TOOLS,
    )),
    b(
        NONE,
        Key::T,
        Action::ToolText,
        "Text tool (LaTeX labels)",
        TOOLS,
    ),
    b(NONE, Key::R, Action::ToolRect, "Rectangle tool", TOOLS),
    b(NONE, Key::E, Action::ToolEllipse, "Ellipse tool", TOOLS),
    // Editing
    b(CMD_SHIFT, Key::Z, Action::Redo, "Redo", EDIT),
    only(b(CMD, Key::Y, Action::Redo, "Redo", EDIT), Only::NotMac),
    b(CMD, Key::Z, Action::Undo, "Undo", EDIT),
    special(
        Trigger::Copy,
        Action::Copy,
        "Copy (also puts the objects' .gold text on the clipboard)",
        EDIT,
    ),
    special(Trigger::Cut, Action::Cut, "Cut", EDIT),
    special(
        Trigger::Paste,
        Action::Paste,
        "Paste (Goldstone objects or .gold text) next to the original",
        EDIT,
    ),
    b(
        CMD_ALT,
        Key::V,
        Action::PasteInPlace,
        "Paste in place",
        EDIT,
    ),
    b(CMD, Key::D, Action::Duplicate, "Duplicate", EDIT),
    b(
        NONE,
        Key::Delete,
        Action::Delete,
        "Delete selection (and lines attached to deleted vertices)",
        EDIT,
    ),
    b(
        NONE,
        Key::Backspace,
        Action::Delete,
        "Delete selection",
        EDIT,
    ),
    b(CMD, Key::A, Action::SelectAll, "Select all", EDIT),
    b(
        NONE,
        Key::Escape,
        Action::Deselect,
        "Deselect, or cancel the line being drawn",
        EDIT,
    ),
    b(
        SHIFT,
        Key::Tab,
        Action::SelectPrev,
        "Select previous object",
        EDIT,
    ),
    b(
        NONE,
        Key::Tab,
        Action::SelectNext,
        "Select next object",
        EDIT,
    ),
    // Objects and paths
    b(CMD_SHIFT, Key::G, Action::Ungroup, "Ungroup", OBJ),
    b(CMD, Key::G, Action::Group, "Group", OBJ),
    b(
        CMD,
        Key::Plus,
        Action::Bool(goldstone_core::shapes::BoolOp::Union),
        "Union of shapes",
        OBJ,
    ),
    quiet(b(
        CMD,
        Key::Equals,
        Action::Bool(goldstone_core::shapes::BoolOp::Union),
        "Union of shapes",
        OBJ,
    )),
    b(
        CMD,
        Key::Minus,
        Action::Bool(goldstone_core::shapes::BoolOp::Difference),
        "Difference (bottom shape minus the others)",
        OBJ,
    ),
    b(
        CMD_SHIFT,
        Key::Num8,
        Action::Bool(goldstone_core::shapes::BoolOp::Intersection),
        "Intersection of shapes (Inkscape's Ctrl+*)",
        OBJ,
    ),
    b(
        CMD_SHIFT,
        Key::Num6,
        Action::Bool(goldstone_core::shapes::BoolOp::Exclusion),
        "Exclusion of shapes (Inkscape's Ctrl+^)",
        OBJ,
    ),
    b(
        CMD_SHIFT,
        Key::C,
        Action::ObjectToPath,
        "Object to path",
        OBJ,
    ),
    // Transform
    b(
        NONE,
        Key::H,
        Action::FlipH,
        "Flip selection horizontally",
        XF,
    ),
    b(NONE, Key::V, Action::FlipV, "Flip selection vertically", XF),
    b(
        CMD,
        Key::OpenBracket,
        Action::Rotate(90),
        "Rotate selection 90° counterclockwise",
        XF,
    ),
    b(
        CMD,
        Key::CloseBracket,
        Action::Rotate(-90),
        "Rotate selection 90° clockwise",
        XF,
    ),
    b(
        NONE,
        Key::OpenBracket,
        Action::Rotate(15),
        "Rotate selection 15° counterclockwise",
        XF,
    ),
    b(
        NONE,
        Key::CloseBracket,
        Action::Rotate(-15),
        "Rotate selection 15° clockwise",
        XF,
    ),
    LARGE[0],
    LARGE[1],
    LARGE[2],
    LARGE[3],
    PIXEL[0],
    PIXEL[1],
    PIXEL[2],
    PIXEL[3],
    NORMAL[0],
    NORMAL[1],
    NORMAL[2],
    NORMAL[3],
    // View
    b(NONE, Key::Plus, Action::ZoomIn, "Zoom in", VIEW),
    b(NONE, Key::Equals, Action::ZoomIn, "Zoom in", VIEW),
    b(NONE, Key::Minus, Action::ZoomOut, "Zoom out", VIEW),
    b(NONE, Key::Num1, Action::Zoom1, "Zoom 1:1", VIEW),
    b(
        NONE,
        Key::Num3,
        Action::ZoomSelection,
        "Zoom to selection",
        VIEW,
    ),
    b(
        NONE,
        Key::Num4,
        Action::ZoomDrawing,
        "Zoom to drawing",
        VIEW,
    ),
    b(
        NONE,
        Key::Num5,
        Action::ZoomPage,
        "Zoom to page (the export area)",
        VIEW,
    ),
    special(
        Trigger::Text("#"),
        Action::ToggleGrid,
        "Toggle grid (snapping to the grid follows its visibility)",
        VIEW,
    ),
    b(CMD, Key::R, Action::ToggleRulers, "Toggle rulers", VIEW),
    own(b(
        NONE,
        Key::I,
        Action::ToggleIds,
        "Show object ids and coordinates on the canvas",
        VIEW,
    )),
    // Dialogs and file
    b(
        CMD_SHIFT,
        Key::F,
        Action::ShowFillStroke,
        "Fill & Stroke (object properties)",
        FILE,
    ),
    b(CMD_SHIFT, Key::L, Action::ShowObjects, "Objects", FILE),
    b(
        CMD_SHIFT,
        Key::A,
        Action::ShowAlign,
        "Align & Distribute",
        FILE,
    ),
    b(
        CMD_SHIFT,
        Key::X,
        Action::ShowSource,
        "Source (the .gold text; counterpart of Inkscape's XML editor)",
        FILE,
    ),
    b(CMD_SHIFT, Key::S, Action::SaveAs, "Save as", FILE),
    b(
        CMD_SHIFT,
        Key::E,
        Action::Export,
        "Export (PDF, SVG, PNG, TikZ)",
        FILE,
    ),
    b(CMD, Key::N, Action::New, "New", FILE),
    b(
        CMD,
        Key::O,
        Action::Open,
        "Open (.gold, or an SVG made by Goldstone)",
        FILE,
    ),
    b(CMD, Key::S, Action::Save, "Save", FILE),
    own(b(
        CMD,
        Key::W,
        Action::CloseTab,
        "Close the diagram (tab)",
        FILE,
    )),
    // Next / previous tab: Ctrl+Tab everywhere (on macOS the Control key).
    own(only(
        b(CTRL_SHIFT, Key::Tab, Action::PrevTab, "Previous tab", FILE),
        Only::Mac,
    )),
    own(only(
        b(CTRL, Key::Tab, Action::NextTab, "Next tab", FILE),
        Only::Mac,
    )),
    own(only(
        b(CMD_SHIFT, Key::Tab, Action::PrevTab, "Previous tab", FILE),
        Only::NotMac,
    )),
    own(only(
        b(CMD, Key::Tab, Action::NextTab, "Next tab", FILE),
        Only::NotMac,
    )),
    own(b(CMD, Key::PageDown, Action::NextTab, "Next tab", FILE)),
    own(b(CMD, Key::PageUp, Action::PrevTab, "Previous tab", FILE)),
    own(b(
        CMD_SHIFT,
        Key::T,
        Action::FromLatex,
        "Diagram from LaTeX: draw a tensor expression such as E2 = V_ijab T_abij",
        FILE,
    )),
    b(CMD, Key::Q, Action::Quit, "Quit", FILE),
    own(special(
        Trigger::Text("?"),
        Action::Shortcuts,
        "Show keyboard shortcuts",
        FILE,
    )),
];

fn key_name(k: Key) -> &'static str {
    match k {
        Key::Escape => "Esc",
        Key::Minus => "-",
        Key::Plus => "+",
        Key::Equals => "=",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        Key::Num1 => "1",
        Key::Num3 => "3",
        Key::Num4 => "4",
        Key::Num5 => "5",
        Key::Num6 => "6",
        Key::Num8 => "8",
        _ => k.name(),
    }
}

/// How a binding's keys are written on `os`.
pub fn keys_label(bnd: &Binding, os: Os) -> String {
    let (mods, key) = match bnd.trigger {
        Trigger::Key(m, k) => (m, bnd.key_label.unwrap_or(key_name(k))),
        Trigger::Text(t) => return t.to_string(),
        Trigger::Copy => (CMD, "C"),
        Trigger::Cut => (CMD, "X"),
        Trigger::Paste => (CMD, "V"),
    };
    let key = match (os, key) {
        (Os::Mac, "Backspace") => "⌫",
        (Os::Mac, "Delete") => "⌦",
        (Os::Mac, "Tab") => "⇥",
        _ => key,
    };
    let mut parts: Vec<&str> = Vec::new();
    // macOS order is ⌃⌥⇧⌘; elsewhere Ctrl+Alt+Shift.
    if mods.ctrl && os == Os::Mac {
        parts.push("⌃");
    }
    if mods.command && os == Os::Other {
        parts.push("Ctrl");
    }
    if mods.alt {
        parts.push(os.alt());
    }
    if mods.shift {
        parts.push(os.shift());
    }
    if mods.command && os == Os::Mac {
        parts.push("⌘");
    }
    parts.push(key);
    match os {
        Os::Mac if parts.len() > 1 && key.chars().count() > 1 => {
            // "⇧ Arrow" reads better than "⇧Arrow".
            let (k, m) = parts.split_last().expect("non-empty");
            format!("{} {k}", m.concat())
        }
        Os::Mac => parts.concat(),
        Os::Other => parts.join("+"),
    }
}

/// The label of the first listed binding for `action` on `os`.
pub fn shortcut(action: Action, os: Os) -> String {
    BINDINGS
        .iter()
        .find(|b| b.action == action && b.only.includes(os))
        .map_or(String::new(), |b| keys_label(b, os))
}

/// Whether a key event with modifiers `m` fires a binding `mods`+`key` on `os`.
pub fn matches(mods: Modifiers, key: Key, m: Modifiers, os: Os) -> bool {
    // Shift is part of typing "+" on most layouts.
    let shift_ok = m.shift == mods.shift || matches!(key, Key::Plus | Key::Equals | Key::Minus);
    // On macOS the Control key is separate from Command.
    let ctrl_ok = os != Os::Mac || m.ctrl == mods.ctrl;
    m.command == mods.command && m.alt == mods.alt && shift_ok && ctrl_ok
}

/// Mouse interactions, for the manual. `{cmd}`, `{alt}`, `{shift}` are
/// written per platform. (Handled in the canvas code.)
pub const MOUSE: &[(&str, &str, &str, bool)] = &[
    (
        "Any tool",
        "Right-click on the canvas",
        "Menu: commands for the selection (the clicked object is selected first), tools, tool options, view",
        true,
    ),
    (
        "Toolbox",
        "Right-click a tool",
        "That tool's options (class, stroke, arrow, double line, vertex kind, grid)",
        false,
    ),
    (
        "Tabs",
        "Click / middle-click / right-click a tab",
        "Switch to it / close it / menu",
        false,
    ),
    (
        "Select tool",
        "Click",
        "Select only this object (a whole group if it is in one)",
        true,
    ),
    (
        "Select tool",
        "{shift}click",
        "Add or remove this object (toggle)",
        true,
    ),
    (
        "Select tool",
        "{cmd}click",
        "Select one object inside a group",
        true,
    ),
    (
        "Select tool",
        "Drag an object",
        "Move the selection (lines attached to unselected vertices stretch)",
        true,
    ),
    (
        "Select tool",
        "Drag on empty canvas",
        "Rubber-band select (objects fully inside)",
        true,
    ),
    (
        "Select tool",
        "{shift}drag on empty canvas",
        "Rubber band: add to the selection",
        true,
    ),
    (
        "Select tool",
        "{cmd}{shift}drag on empty canvas",
        "Rubber band: remove from the selection",
        false,
    ),
    (
        "Select tool",
        "{alt}drag",
        "Touch select: select every object the red path crosses",
        true,
    ),
    (
        "Select tool",
        "{shift}{alt}drag",
        "Touch select: add to the selection",
        true,
    ),
    (
        "Any tool",
        "{cmd} while dragging",
        "Move only horizontally or vertically",
        true,
    ),
    (
        "Any tool",
        "{shift} while dragging or placing",
        "Do not snap",
        true,
    ),
    (
        "Line tool",
        "Click, click (or drag)",
        "Straight line; ends snap to vertices, or create invisible vertices",
        true,
    ),
    (
        "Line, Arc tool",
        "Esc or right click",
        "Cancel the line being drawn",
        true,
    ),
    (
        "Arc tool",
        "Click start, click end, click",
        "Arc; the last click sets the bulge ({cmd}: bend in 15° steps)",
        false,
    ),
    (
        "Arc tool",
        "Click, then drag at the end point",
        "Arc through the point where the mouse is released",
        false,
    ),
    (
        "Arc tool",
        "Click the same vertex twice, then click",
        "Loop toward the pointer, sized by its distance",
        false,
    ),
    (
        "Edit tool",
        "Drag the round handle",
        "Bend a line into an arc / move a Bézier control point / resize a loop",
        true,
    ),
    (
        "Edit tool",
        "Drag the diamond handle",
        "Move the arrow along the line",
        false,
    ),
    ("Cross tool", "Click", "Place a cross vertex", false),
    (
        "Cross tool",
        "Drag from a vertex",
        "Create a dashed line ending in a cross (---X)",
        false,
    ),
    (
        "Rectangle, Ellipse",
        "Drag",
        "Draw the shape; {cmd} makes it a square / circle",
        true,
    ),
    (
        "View",
        "Scroll / {shift}scroll",
        "Pan vertically / horizontally",
        true,
    ),
    (
        "View",
        "{cmd}scroll, or pinch",
        "Zoom around the pointer",
        true,
    ),
    ("View", "Middle-button drag, or Space+drag", "Pan", true),
];

/// The shortcut section of the manual, in Markdown, with a column per
/// platform.
pub fn manual_markdown() -> String {
    let mut out = String::new();
    let mut group = "";
    for bnd in BINDINGS.iter().filter(|b| b.listed) {
        if bnd.group != group {
            group = bnd.group;
            out.push_str(&format!(
                "\n### {group}\n\n| Linux / Windows | macOS | Action | Inkscape |\n|---|---|---|---|\n"
            ));
        }
        let col = |os: Os| {
            if bnd.only.includes(os) {
                format!("`{}`", keys_label(bnd, os))
            } else {
                "—".into()
            }
        };
        let ink = if bnd.inkscape {
            "same"
        } else {
            "Goldstone only"
        };
        out.push_str(&format!(
            "| {} | {} | {} | {ink} |\n",
            col(Os::Other),
            col(Os::Mac),
            bnd.desc
        ));
    }
    out.push_str(
        "\n### Mouse\n\n| Where | Linux / Windows | macOS | Action | Inkscape |\n|---|---|---|---|---|\n",
    );
    for (ctx, input, desc, ink) in MOUSE {
        let ink = if *ink { "same" } else { "Goldstone only" };
        out.push_str(&format!(
            "| {ctx} | {} | {} | {} | {ink} |\n",
            Os::Other.fill(input),
            Os::Mac.fill(input),
            Os::Other.fill(desc),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manual's shortcut section must match the table.
    #[test]
    fn manual_is_up_to_date() {
        let manual = include_str!("../../../docs/MANUAL.md");
        let start =
            manual.find("<!-- keys:start -->").expect("start marker") + "<!-- keys:start -->".len();
        let end = manual.find("<!-- keys:end -->").expect("end marker");
        assert_eq!(
            manual[start..end].trim(),
            manual_markdown().trim(),
            "docs/MANUAL.md is out of date; regenerate with `goldstone keys`"
        );
    }

    #[test]
    fn no_duplicate_triggers() {
        for os in [Os::Mac, Os::Other] {
            let mut seen = Vec::new();
            for x in BINDINGS.iter().filter(|b| b.only.includes(os)) {
                let k = format!("{:?}", x.trigger);
                assert!(!seen.contains(&k), "duplicate binding {k} on {os:?}");
                seen.push(k);
            }
        }
    }

    #[test]
    fn labels_follow_the_platform() {
        let redo = &BINDINGS[BINDINGS
            .iter()
            .position(|b| b.action == Action::Redo)
            .unwrap()];
        assert_eq!(keys_label(redo, Os::Mac), "⇧⌘Z");
        assert_eq!(keys_label(redo, Os::Other), "Ctrl+Shift+Z");
        assert_eq!(shortcut(Action::Copy, Os::Mac), "⌘C");
        assert_eq!(shortcut(Action::Copy, Os::Other), "Ctrl+C");
        assert_eq!(
            shortcut(
                Action::Nudge {
                    dx: -1,
                    dy: 0,
                    step: Step::Large
                },
                Os::Mac
            ),
            "⇧ Arrow"
        );
        assert_eq!(shortcut(Action::PasteInPlace, Os::Other), "Ctrl+Alt+V");
        assert_eq!(shortcut(Action::PasteInPlace, Os::Mac), "⌥⌘V");
        assert_eq!(Os::Mac.fill("{cmd}{shift}drag"), "⌘⇧drag");
        assert_eq!(Os::Other.fill("{cmd}{shift}drag"), "Ctrl+Shift+drag");
    }

    #[test]
    fn macos_control_is_not_command() {
        let ctrl_z = Modifiers {
            ctrl: true,
            ..Default::default()
        };
        let cmd_z = Modifiers {
            mac_cmd: true,
            command: true,
            ..Default::default()
        };
        assert!(!matches(CMD, Key::Z, ctrl_z, Os::Mac));
        assert!(matches(CMD, Key::Z, cmd_z, Os::Mac));
        let linux_ctrl_z = Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        assert!(matches(CMD, Key::Z, linux_ctrl_z, Os::Other));
    }
}

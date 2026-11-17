//! The key-binding table. The editor dispatches keyboard input from this
//! table, and the manual's shortcut section is generated from it, so the two
//! cannot disagree.

use egui::{Key, Modifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToolSelect,
    ToolEdit,
    ToolEdge,
    ToolVertex,
    ToolCross,
    ToolText,
    Undo,
    Redo,
    Delete,
    SelectAll,
    Deselect,
    FlipH,
    FlipV,
    Nudge { dx: i8, dy: i8, step: Step },
    ZoomIn,
    ZoomOut,
    Zoom1,
    ZoomSelection,
    ZoomDrawing,
    ZoomPage,
    ToggleGrid,
    New,
    Open,
    Save,
    SaveAs,
    Export,
    ShowFillStroke,
    ShowObjects,
    ShowSource,
    Shortcuts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Normal,
    Large,
    Pixel,
}

#[derive(Debug, Clone, Copy)]
pub enum Trigger {
    Key(Modifiers, Key),
    /// A typed character, for keys egui has no `Key` for (e.g. `#`).
    Text(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct Binding {
    pub trigger: Trigger,
    pub action: Action,
    /// How the keys are written in the manual (Ctrl means Cmd on macOS).
    pub keys: &'static str,
    pub desc: &'static str,
    pub group: &'static str,
    /// Whether Inkscape uses the same binding for the same action.
    pub inkscape: bool,
}

const NONE: Modifiers = Modifiers::NONE;
const SHIFT: Modifiers = Modifiers::SHIFT;
const ALT: Modifiers = Modifiers::ALT;
const CMD: Modifiers = Modifiers::COMMAND;
const CMD_SHIFT: Modifiers = Modifiers {
    shift: true,
    ..Modifiers::COMMAND
};

const fn b(
    mods: Modifiers,
    key: Key,
    action: Action,
    keys: &'static str,
    desc: &'static str,
    group: &'static str,
) -> Binding {
    Binding {
        trigger: Trigger::Key(mods, key),
        action,
        keys,
        desc,
        group,
        inkscape: true,
    }
}

const fn own(mut x: Binding) -> Binding {
    x.inkscape = false;
    x
}

const fn nudge(dx: i8, dy: i8, step: Step) -> Action {
    Action::Nudge { dx, dy, step }
}

/// Keyboard bindings. More specific modifier combinations come first,
/// because egui ignores extra Shift/Alt when matching.
pub const BINDINGS: &[Binding] = &[
    // Tools
    b(
        NONE,
        Key::S,
        Action::ToolSelect,
        "S",
        "Select tool",
        "Tools",
    ),
    b(
        NONE,
        Key::F1,
        Action::ToolSelect,
        "F1",
        "Select tool",
        "Tools",
    ),
    b(
        NONE,
        Key::N,
        Action::ToolEdit,
        "N",
        "Edit tool (bend, control points, loop size, arrow position)",
        "Tools",
    ),
    b(
        NONE,
        Key::B,
        Action::ToolEdge,
        "B",
        "Edge tool (draw lines between vertices)",
        "Tools",
    ),
    own(b(
        NONE,
        Key::P,
        Action::ToolVertex,
        "P",
        "Vertex tool",
        "Tools",
    )),
    own(b(
        NONE,
        Key::X,
        Action::ToolCross,
        "X",
        "Cross tool (one-body operator ---X)",
        "Tools",
    )),
    b(
        NONE,
        Key::T,
        Action::ToolText,
        "T",
        "Text tool (LaTeX labels)",
        "Tools",
    ),
    // Editing
    b(
        CMD_SHIFT,
        Key::Z,
        Action::Redo,
        "Ctrl+Shift+Z",
        "Redo",
        "Editing",
    ),
    b(CMD, Key::Y, Action::Redo, "Ctrl+Y", "Redo", "Editing"),
    b(CMD, Key::Z, Action::Undo, "Ctrl+Z", "Undo", "Editing"),
    b(
        NONE,
        Key::Delete,
        Action::Delete,
        "Delete",
        "Delete selection (and lines attached to deleted vertices)",
        "Editing",
    ),
    b(
        NONE,
        Key::Backspace,
        Action::Delete,
        "Backspace",
        "Delete selection",
        "Editing",
    ),
    b(
        CMD,
        Key::A,
        Action::SelectAll,
        "Ctrl+A",
        "Select all",
        "Editing",
    ),
    b(
        NONE,
        Key::Escape,
        Action::Deselect,
        "Esc",
        "Deselect, or cancel the line being drawn",
        "Editing",
    ),
    b(
        NONE,
        Key::H,
        Action::FlipH,
        "H",
        "Flip selection horizontally",
        "Editing",
    ),
    b(
        NONE,
        Key::V,
        Action::FlipV,
        "V",
        "Flip selection vertically",
        "Editing",
    ),
    b(
        SHIFT,
        Key::ArrowLeft,
        nudge(-1, 0, Step::Large),
        "Shift+Arrow",
        "Move selection by 20 pt",
        "Editing",
    ),
    b(
        SHIFT,
        Key::ArrowRight,
        nudge(1, 0, Step::Large),
        "",
        "",
        "Editing",
    ),
    b(
        SHIFT,
        Key::ArrowUp,
        nudge(0, 1, Step::Large),
        "",
        "",
        "Editing",
    ),
    b(
        SHIFT,
        Key::ArrowDown,
        nudge(0, -1, Step::Large),
        "",
        "",
        "Editing",
    ),
    b(
        ALT,
        Key::ArrowLeft,
        nudge(-1, 0, Step::Pixel),
        "Alt+Arrow",
        "Move selection by one screen pixel",
        "Editing",
    ),
    b(
        ALT,
        Key::ArrowRight,
        nudge(1, 0, Step::Pixel),
        "",
        "",
        "Editing",
    ),
    b(
        ALT,
        Key::ArrowUp,
        nudge(0, 1, Step::Pixel),
        "",
        "",
        "Editing",
    ),
    b(
        ALT,
        Key::ArrowDown,
        nudge(0, -1, Step::Pixel),
        "",
        "",
        "Editing",
    ),
    b(
        NONE,
        Key::ArrowLeft,
        nudge(-1, 0, Step::Normal),
        "Arrow keys",
        "Move selection by 2 pt",
        "Editing",
    ),
    b(
        NONE,
        Key::ArrowRight,
        nudge(1, 0, Step::Normal),
        "",
        "",
        "Editing",
    ),
    b(
        NONE,
        Key::ArrowUp,
        nudge(0, 1, Step::Normal),
        "",
        "",
        "Editing",
    ),
    b(
        NONE,
        Key::ArrowDown,
        nudge(0, -1, Step::Normal),
        "",
        "",
        "Editing",
    ),
    // View
    b(NONE, Key::Plus, Action::ZoomIn, "+", "Zoom in", "View"),
    b(NONE, Key::Equals, Action::ZoomIn, "=", "Zoom in", "View"),
    b(NONE, Key::Minus, Action::ZoomOut, "-", "Zoom out", "View"),
    b(NONE, Key::Num1, Action::Zoom1, "1", "Zoom 1:1", "View"),
    b(
        NONE,
        Key::Num3,
        Action::ZoomSelection,
        "3",
        "Zoom to selection",
        "View",
    ),
    b(
        NONE,
        Key::Num4,
        Action::ZoomDrawing,
        "4",
        "Zoom to drawing",
        "View",
    ),
    b(
        NONE,
        Key::Num5,
        Action::ZoomPage,
        "5",
        "Zoom to page (the export area)",
        "View",
    ),
    Binding {
        trigger: Trigger::Text("#"),
        action: Action::ToggleGrid,
        keys: "#",
        desc: "Toggle grid (snapping to the grid follows its visibility)",
        group: "View",
        inkscape: true,
    },
    // Dialogs and file
    b(
        CMD_SHIFT,
        Key::F,
        Action::ShowFillStroke,
        "Ctrl+Shift+F",
        "Fill & Stroke (object properties)",
        "Dialogs and file",
    ),
    b(
        CMD_SHIFT,
        Key::L,
        Action::ShowObjects,
        "Ctrl+Shift+L",
        "Objects",
        "Dialogs and file",
    ),
    b(
        CMD_SHIFT,
        Key::X,
        Action::ShowSource,
        "Ctrl+Shift+X",
        "Source (the .gold text; counterpart of Inkscape's XML editor)",
        "Dialogs and file",
    ),
    b(
        CMD_SHIFT,
        Key::S,
        Action::SaveAs,
        "Ctrl+Shift+S",
        "Save as",
        "Dialogs and file",
    ),
    b(
        CMD_SHIFT,
        Key::E,
        Action::Export,
        "Ctrl+Shift+E",
        "Export (PDF, SVG, PNG, TikZ)",
        "Dialogs and file",
    ),
    b(
        CMD,
        Key::N,
        Action::New,
        "Ctrl+N",
        "New",
        "Dialogs and file",
    ),
    b(
        CMD,
        Key::O,
        Action::Open,
        "Ctrl+O",
        "Open (.gold, or an SVG made by Goldstone)",
        "Dialogs and file",
    ),
    b(
        CMD,
        Key::S,
        Action::Save,
        "Ctrl+S",
        "Save",
        "Dialogs and file",
    ),
    Binding {
        trigger: Trigger::Text("?"),
        action: Action::Shortcuts,
        keys: "?",
        desc: "Show keyboard shortcuts",
        group: "Dialogs and file",
        inkscape: false,
    },
];

/// Mouse interactions, for the manual. (Handled in the canvas code.)
pub const MOUSE: &[(&str, &str, &str, bool)] = &[
    ("Select tool", "Click", "Select only this object", true),
    (
        "Select tool",
        "Shift+click",
        "Add or remove this object (toggle)",
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
        "Shift+drag on empty canvas",
        "Rubber band: add to the selection",
        true,
    ),
    (
        "Select tool",
        "Ctrl+Shift+drag on empty canvas",
        "Rubber band: remove from the selection",
        false,
    ),
    (
        "Any tool",
        "Ctrl while dragging",
        "Move only horizontally or vertically",
        true,
    ),
    (
        "Any tool",
        "Shift while dragging or placing",
        "Do not snap",
        true,
    ),
    (
        "Edge tool",
        "Click, click",
        "Straight line; ends snap to vertices, or create invisible vertices",
        true,
    ),
    (
        "Edge tool",
        "Click, then drag at the second point",
        "Arc through the point where the mouse is released",
        false,
    ),
    ("Edge tool", "Click the same vertex twice", "Loop", false),
    (
        "Edge tool",
        "Esc or right click",
        "Cancel the line being drawn",
        true,
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
        "View",
        "Scroll / Shift+scroll",
        "Pan vertically / horizontally",
        true,
    ),
    (
        "View",
        "Ctrl+scroll, or pinch",
        "Zoom around the pointer",
        true,
    ),
    ("View", "Middle-button drag, or Space+drag", "Pan", true),
];

/// The shortcut section of the manual, in Markdown.
pub fn manual_markdown() -> String {
    let mut out = String::new();
    let mut group = "";
    for bnd in BINDINGS.iter().filter(|b| !b.keys.is_empty()) {
        if bnd.group != group {
            group = bnd.group;
            out.push_str(&format!(
                "\n### {group}\n\n| Keys | Action | Inkscape |\n|---|---|---|\n"
            ));
        }
        let ink = if bnd.inkscape {
            "same"
        } else {
            "Goldstone only"
        };
        out.push_str(&format!("| `{}` | {} | {ink} |\n", bnd.keys, bnd.desc));
    }
    out.push_str("\n### Mouse\n\n| Where | Input | Action | Inkscape |\n|---|---|---|---|\n");
    for (ctx, input, desc, ink) in MOUSE {
        let ink = if *ink { "same" } else { "Goldstone only" };
        out.push_str(&format!("| {ctx} | {input} | {desc} | {ink} |\n"));
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
        let mut seen = Vec::new();
        for x in BINDINGS {
            let k = format!("{:?}", x.trigger);
            assert!(!seen.contains(&k), "duplicate binding {k}");
            seen.push(k);
        }
    }
}

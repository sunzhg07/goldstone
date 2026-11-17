//! Drives the editor with simulated mouse and keyboard input.

use egui::{Event, Key, Modifiers, PointerButton, Pos2};
use egui_kittest::Harness;
use goldstone_app::app::GoldstoneApp;
use goldstone_core::model::{Stroke, VertexKind};

fn harness() -> Harness<'static, GoldstoneApp> {
    let mut h = Harness::builder()
        .with_size([1280.0, 820.0])
        .build_eframe(|cc| GoldstoneApp::new(cc, None));
    h.run_steps(4);
    h
}

fn pos(h: &Harness<'_, GoldstoneApp>, x: f64, y: f64) -> Pos2 {
    h.state().screen_pos(x, y)
}

fn button(h: &mut Harness<'_, GoldstoneApp>, p: Pos2, pressed: bool, modifiers: Modifiers) {
    h.event(Event::PointerButton {
        pos: p,
        button: PointerButton::Primary,
        pressed,
        modifiers,
    });
    h.step();
}

fn click(h: &mut Harness<'_, GoldstoneApp>, x: f64, y: f64) {
    let p = pos(h, x, y);
    h.event(Event::PointerMoved(p));
    h.step();
    button(h, p, true, Modifiers::NONE);
    button(h, p, false, Modifiers::NONE);
    h.run_steps(2);
}

fn drag(h: &mut Harness<'_, GoldstoneApp>, from: (f64, f64), to: (f64, f64)) {
    let (a, b) = (pos(h, from.0, from.1), pos(h, to.0, to.1));
    h.event(Event::PointerMoved(a));
    h.step();
    button(h, a, true, Modifiers::NONE);
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        h.event(Event::PointerMoved(a + (b - a) * t));
        h.step();
    }
    button(h, b, false, Modifiers::NONE);
    h.run_steps(2);
}

#[test]
fn draw_snap_undo_cross_move_delete() {
    let mut h = harness();
    assert!(h.state().document().edges.is_empty());

    // Edge tool: two clicks make a line with two invisible end vertices.
    h.key_press(Key::B);
    h.run_steps(2);
    click(&mut h, 0.0, 0.0);
    click(&mut h, 0.0, 40.0);
    let d = h.state().document();
    assert_eq!(d.edges.len(), 1, "one edge");
    assert_eq!(d.vertices.len(), 2, "two end vertices");
    assert_eq!(d.vertices[1].at, [0.0, 40.0]);

    // A second line ending on the existing vertex reuses it (snapping).
    click(&mut h, 40.0, 40.0);
    click(&mut h, 0.5, 39.5);
    let d = h.state().document();
    assert_eq!(d.edges.len(), 2);
    assert_eq!(d.vertices.len(), 3, "end snapped onto the existing vertex");
    assert_eq!(d.edges[1].to, d.vertices[1].id);

    // Undo removes the second line and its new vertex.
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run_steps(2);
    assert_eq!(h.state().document().edges.len(), 1);
    assert_eq!(h.state().document().vertices.len(), 2);

    // Cross tool: drag from a vertex draws ---X.
    h.key_press(Key::X);
    h.run_steps(2);
    drag(&mut h, (0.0, 40.0), (40.0, 60.0));
    let d = h.state().document();
    assert_eq!(d.edges.len(), 2);
    let x = d.vertices.last().unwrap();
    assert_eq!(x.kind, Some(VertexKind::Cross));
    assert_eq!(x.at, [40.0, 60.0]);
    assert_eq!(d.edges[1].stroke, Some(Stroke::Dashed));

    // Select tool: dragging the bottom vertex moves it; its line follows.
    h.key_press(Key::S);
    h.run_steps(2);
    drag(&mut h, (0.0, 0.0), (10.0, -10.0));
    let d = h.state().document();
    assert_eq!(d.vertices[0].at, [10.0, -10.0]);
    assert_eq!(d.edges[0].from, d.vertices[0].id);

    // Select all, delete.
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(2);
    assert!(h.state().selected().len() >= 5);
    h.key_press(Key::Delete);
    h.run_steps(2);
    let d = h.state().document();
    assert!(d.vertices.is_empty() && d.edges.is_empty());
}

#[test]
fn rubber_band_and_loop() {
    let mut h = harness();
    h.key_press(Key::B);
    h.run_steps(2);
    click(&mut h, 0.0, 0.0);
    click(&mut h, 30.0, 0.0);
    // The Line tool does not make loops.
    click(&mut h, 30.0, 0.0);
    click(&mut h, 30.0, 0.0);
    assert_eq!(h.state().document().edges.len(), 1);
    h.key_press(Key::Escape);
    // Arc tool: the same vertex twice, then a click, makes a loop.
    h.key_press(Key::A);
    h.run_steps(2);
    click(&mut h, 30.0, 0.0);
    click(&mut h, 30.0, 0.0);
    click(&mut h, 30.0, 20.0);
    let d = h.state().document();
    assert_eq!(d.edges.len(), 2);
    assert_eq!(d.edges[1].from, d.edges[1].to, "loop");
    let l = d.edges[1].loop_.as_ref().unwrap();
    assert_eq!(
        (l.radius, l.angle),
        (Some(10.0), Some(90.0)),
        "toward and sized by the pointer"
    );

    // Rubber band around the left vertex only.
    h.key_press(Key::S);
    h.run_steps(2);
    drag(&mut h, (-10.0, -10.0), (10.0, 10.0));
    assert_eq!(
        h.state().selected(),
        vec![goldstone_core::scene::ObjRef::Vertex(0)]
    );
}

#[test]
fn edit_tool_bends_a_line() {
    let mut h = harness();
    h.key_press(Key::B);
    h.run_steps(2);
    click(&mut h, 0.0, 0.0);
    click(&mut h, 40.0, 0.0);
    h.key_press(Key::N);
    h.run_steps(2);
    // The bend handle sits at the middle of the (selected) line.
    drag(&mut h, (20.0, 0.0), (20.0, 20.0));
    assert_eq!(
        h.state().document().edges[0].bend,
        Some(90.0),
        "semicircle through (20, 20)"
    );
}

fn drag_with(h: &mut Harness<'_, GoldstoneApp>, from: (f64, f64), to: (f64, f64), m: Modifiers) {
    h.event(Event::ModifiersChanged(m));
    let (a, b) = (pos(h, from.0, from.1), pos(h, to.0, to.1));
    h.event(Event::PointerMoved(a));
    h.step();
    button(h, a, true, m);
    for i in 1..=8 {
        let t = i as f32 / 8.0;
        h.event(Event::PointerMoved(a + (b - a) * t));
        h.step();
    }
    button(h, b, false, m);
    h.event(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

/// Two lines; returns the harness with the Select tool active.
fn two_lines() -> Harness<'static, GoldstoneApp> {
    let mut h = harness();
    h.key_press(Key::B);
    h.run_steps(2);
    click(&mut h, 0.0, 0.0);
    click(&mut h, 40.0, 0.0);
    click(&mut h, 0.0, 30.0);
    click(&mut h, 40.0, 30.0);
    h.key_press(Key::S);
    h.run_steps(2);
    assert_eq!(h.state().document().edges.len(), 2);
    h
}

#[test]
fn group_then_click_selects_the_group() {
    let mut h = two_lines();
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::G);
    h.run_steps(2);
    assert_eq!(h.state().document().groups.len(), 1);
    h.key_press(Key::Escape);
    h.run_steps(2);
    click(&mut h, 20.0, 0.0);
    assert_eq!(
        h.state().selected().len(),
        6,
        "whole group: 4 vertices, 2 edges"
    );
    // Ungroup, then a click selects one line only.
    h.key_press_modifiers(
        Modifiers {
            shift: true,
            ..Modifiers::COMMAND
        },
        Key::G,
    );
    h.run_steps(2);
    assert!(h.state().document().groups.is_empty());
    click(&mut h, 20.0, 0.0);
    assert_eq!(h.state().selected().len(), 1);
}

#[test]
fn copy_paste_and_duplicate() {
    let mut h = two_lines();
    click(&mut h, 20.0, 0.0);
    h.event(Event::Copy);
    h.run_steps(2);
    let text = h.state().clipboard_text().expect("copied").to_string();
    assert!(text.contains("[[edge]]"), "{text}");
    // Paste at the pointer.
    h.event(Event::PointerMoved(pos(&h, 20.0, 60.0)));
    h.step();
    h.event(Event::Paste(text));
    h.run_steps(2);
    let d = h.state().document();
    assert_eq!(d.edges.len(), 3);
    assert_eq!(d.vertices.len(), 6, "pasted edge brings its two vertices");
    let ys: Vec<f64> = d.vertices[4..].iter().map(|v| v.at[1]).collect();
    assert_eq!(ys, vec![60.0, 60.0], "centered on the pointer");
    h.key_press_modifiers(Modifiers::COMMAND, Key::D);
    h.run_steps(2);
    assert_eq!(h.state().document().edges.len(), 4);
}

#[test]
fn rectangles_union_into_a_path() {
    let mut h = harness();
    h.key_press(Key::R);
    h.run_steps(2);
    drag(&mut h, (0.0, 0.0), (40.0, 20.0));
    drag(&mut h, (20.0, 10.0), (60.0, 30.0));
    assert_eq!(h.state().document().shapes.len(), 2);
    h.key_press(Key::S);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::Plus);
    h.run_steps(2);
    let d = h.state().document();
    assert_eq!(d.shapes.len(), 1);
    assert_eq!(d.shapes[0].kind, goldstone_core::model::ShapeKind::Path);
    assert!(goldstone_core::scene::build(d).is_ok());
}

#[test]
fn alt_drag_touch_selects() {
    let mut h = two_lines();
    // A vertical stroke crossing both lines in the middle.
    drag_with(&mut h, (20.0, -10.0), (20.0, 40.0), Modifiers::ALT);
    let sel = h.state().selected();
    assert_eq!(
        sel,
        vec![
            goldstone_core::scene::ObjRef::Edge(0),
            goldstone_core::scene::ObjRef::Edge(1)
        ]
    );
}

#[test]
fn tab_cycles_and_rotate() {
    let mut h = two_lines();
    h.key_press(Key::Tab);
    h.run_steps(2);
    assert_eq!(
        h.state().selected(),
        vec![goldstone_core::scene::ObjRef::Vertex(0)]
    );
    h.key_press(Key::Tab);
    h.run_steps(2);
    assert_eq!(
        h.state().selected(),
        vec![goldstone_core::scene::ObjRef::Vertex(1)]
    );
    // Rotate the first line 90° about its center.
    click(&mut h, 20.0, 0.0);
    h.key_press_modifiers(Modifiers::COMMAND, Key::OpenBracket);
    h.run_steps(2);
    let d = h.state().document();
    assert_eq!(d.vertices[0].at, [20.0, -20.0]);
    assert_eq!(d.vertices[1].at, [20.0, 20.0]);
}

#[test]
fn arc_tool_three_clicks() {
    let mut h = harness();
    h.key_press(Key::A);
    h.run_steps(2);
    click(&mut h, 0.0, 0.0);
    click(&mut h, 40.0, 0.0);
    assert!(h.state().document().edges.is_empty(), "waits for the bulge");
    click(&mut h, 20.0, 20.0);
    let d = h.state().document();
    assert_eq!(d.edges.len(), 1);
    assert_eq!(d.edges[0].bend, Some(90.0), "semicircle through (20, 20)");
    // The Line tool draws straight lines only, even when dragging.
    h.key_press(Key::B);
    h.run_steps(2);
    click(&mut h, 0.0, 30.0);
    drag(&mut h, (40.0, 30.0), (40.0, 40.0));
    let d = h.state().document();
    assert_eq!(d.edges.len(), 2);
    assert_eq!(d.edges[1].bend, None);
}

#[test]
fn diagram_from_latex_dialog() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness();
    h.key_press_modifiers(
        Modifiers {
            shift: true,
            ..Modifiers::COMMAND
        },
        Key::T,
    );
    h.run_steps(3);
    // The default expression is the MBPT2 energy.
    h.get_by_label("Insert").click();
    h.run_steps(3);
    let d = h.state().document();
    assert_eq!(d.vertices.len(), 4);
    assert_eq!(d.edges.len(), 6, "4 lines, 1 interaction, 1 amplitude bar");
    assert_eq!(h.state().selected().len(), 11);
}

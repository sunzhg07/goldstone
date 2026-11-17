//! Renders the editor offscreen to PNG files for visual inspection.
//! Run with: GOLDSTONE_SHOTS=/some/dir cargo test -p goldstone-app --test screenshots -- --ignored

use std::path::PathBuf;

use egui::{Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use goldstone_app::app::GoldstoneApp;

fn example(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(name)
}

fn shoot(h: &mut Harness<'_, GoldstoneApp>, name: &str) {
    let Ok(dir) = std::env::var("GOLDSTONE_SHOTS") else {
        return;
    };
    h.run_steps(4);
    let img = h.render().expect("render");
    img.save(PathBuf::from(dir).join(format!("{name}.png")))
        .expect("save");
}

fn harness(file: &str) -> Harness<'static, GoldstoneApp> {
    let path = example(file);
    let mut h = Harness::builder()
        .with_size([1280.0, 820.0])
        .wgpu()
        .build_eframe(move |cc| GoldstoneApp::new(cc, Some(path)));
    h.run_steps(6);
    h
}

#[test]
#[ignore = "renders with wgpu; run explicitly"]
fn editor_screenshots() {
    let mut h = harness("gallery.gold");
    shoot(&mut h, "01-gallery");

    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    shoot(&mut h, "02-select-all");

    let mut h = harness("mbpt2.gold");
    h.get_by_label("Objects").click();
    h.run_steps(2);
    h.get_by_label("a → c  [particle]  $a$").click();
    h.run_steps(2);
    h.key_press(Key::N);
    shoot(&mut h, "03-edit-tool-objects");

    h.get_by_label("Fill & Stroke").click();
    shoot(&mut h, "04-fill-stroke");

    h.get_by_label("Source").click();
    shoot(&mut h, "05-source");

    h.get_by_label("Help").click();
    h.run_steps(2);
    h.get_by_label_contains("Keyboard and Mouse").click();
    shoot(&mut h, "06-shortcuts");
}

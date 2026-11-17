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
    h.get_by_label_contains("e3  a (0, 0) → c").click();
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

    // Ids and coordinates on the canvas; multi-edit with mixed values.
    let mut h = harness("mbpt2.gold");
    h.key_press(Key::I);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    shoot(&mut h, "07-ids-multi-edit");

    h.get_by_label("Align").click();
    shoot(&mut h, "08-align");

    h.get_by_label("Path").click();
    shoot(&mut h, "09-path-menu");

    let mut h = harness("mbpt2.gold");
    h.key_press_modifiers(
        Modifiers {
            shift: true,
            ..Modifiers::COMMAND
        },
        Key::T,
    );
    shoot(&mut h, "11-latex-dialog");
    h.get_by_label("Insert").click();
    shoot(&mut h, "12-latex-inserted");
}

/// All icons on one sheet, at 4× (light theme tint).
#[test]
#[ignore = "writes a PNG; run explicitly"]
fn icon_sheet() {
    let Ok(dir) = std::env::var("GOLDSTONE_SHOTS") else {
        return;
    };
    let mut labels = goldstone_export::Labels::new();
    let px = 96u32;
    let cols = 9u32;
    let n = goldstone_app::icons::SOURCES.len() as u32;
    let rows = n.div_ceil(cols);
    let (w, h) = (cols * (px + 8), rows * (px + 8));
    let mut sheet = image::RgbaImage::from_pixel(w, h, image::Rgba([248, 248, 248, 255]));
    for (k, (name, _)) in goldstone_app::icons::SOURCES.iter().enumerate() {
        let mut p = goldstone_app::icons::render(name, px, &mut labels).unwrap();
        goldstone_app::icons::tint(&mut p, [30, 30, 30], [248, 248, 248]);
        let (x0, y0) = (
            (k as u32 % cols) * (px + 8) + 4,
            (k as u32 / cols) * (px + 8) + 4,
        );
        for y in 0..px {
            for x in 0..px {
                let i = ((y * px + x) * 4) as usize;
                let a = p[i + 3] as u32;
                let bg = sheet.get_pixel(x0 + x, y0 + y).0;
                let c = |k: usize| (p[i + k] as u32 + bg[k] as u32 * (255 - a) / 255) as u8;
                sheet.put_pixel(x0 + x, y0 + y, image::Rgba([c(0), c(1), c(2), 255]));
            }
        }
    }
    sheet.save(PathBuf::from(dir).join("10-icons.png")).unwrap();
}

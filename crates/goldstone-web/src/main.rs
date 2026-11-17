//! Goldstone in the browser: the same editor as the desktop program,
//! compiled to WebAssembly and drawn with WebGL.
//!
//!     cd crates/goldstone-web && trunk serve      # http://127.0.0.1:8080
//!     trunk build --release --public-url ./       # the site, in dist/

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    goldstone_app::i18n::set_lang(goldstone_app::i18n::initial());
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("a web page");
        let canvas = document
            .get_element_by_id("goldstone")
            .expect("the page has <canvas id=goldstone>")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("a canvas");
        let options = eframe::WebOptions {
            renderer: eframe::Renderer::Glow,
            ..Default::default()
        };
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(|cc| Ok(Box::new(goldstone_app::app::GoldstoneApp::new(cc, None)))),
            )
            .await;
        if let Some(loading) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => loading.remove(),
                Err(e) => loading
                    .set_inner_html(&format!("Goldstone could not start in this browser: {e:?}")),
            }
        }
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "goldstone-web is the browser version. Build it with trunk: \
         cd crates/goldstone-web && trunk serve"
    );
}

//! What differs between the desktop program and the web version: where
//! files and settings come from and go to. On the desktop these are files;
//! in the browser, files are picked or dropped (and arrive later, read
//! asynchronously), saved files are downloads, and settings live in the
//! browser's local storage.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// Files that arrived (picked, dropped or fetched) and wait to be opened:
/// (name, contents).
#[derive(Clone, Default)]
pub struct Inbox(Rc<RefCell<Vec<Arrival>>>);

/// A file that arrived: its name and contents.
pub type Arrival = (PathBuf, Result<Vec<u8>, String>);

impl Inbox {
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub fn push(&self, name: PathBuf, bytes: Result<Vec<u8>, String>) {
        self.0.borrow_mut().push((name, bytes));
    }

    pub fn take(&self) -> Vec<Arrival> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

/// Whether this is the web version.
pub const WEB: bool = cfg!(target_arch = "wasm32");

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use wasm_bindgen::JsCast;

    fn window() -> Option<web_sys::Window> {
        web_sys::window()
    }

    /// Lets the browser save `bytes` as a download named `name`.
    pub fn download(name: &str, bytes: &[u8], mime: &str) -> Result<(), String> {
        let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
        let array = js_sys::Uint8Array::from(bytes);
        let parts = js_sys::Array::of1(&array.buffer());
        let opts = web_sys::BlobPropertyBag::new();
        opts.set_type(mime);
        let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &opts)
            .map_err(err)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
        let document = window().and_then(|w| w.document()).ok_or("no document")?;
        let a: web_sys::HtmlAnchorElement = document
            .create_element("a")
            .map_err(err)?
            .dyn_into()
            .map_err(|_| "no anchor")?;
        a.set_href(&url);
        a.set_download(name);
        a.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok(())
    }

    /// Asks the browser for files to open; they arrive in `inbox`.
    pub fn pick_files(inbox: Inbox, ctx: egui::Context) {
        wasm_bindgen_futures::spawn_local(async move {
            let Some(files) = rfd::AsyncFileDialog::new()
                .add_filter("Goldstone diagram", &["gold", "svg"])
                .pick_files()
                .await
            else {
                return;
            };
            for f in files {
                let bytes = f.read().await;
                inbox.push(PathBuf::from(f.file_name()), Ok(bytes));
            }
            ctx.request_repaint();
        });
    }

    /// Reads files dropped on the page into `inbox`.
    pub fn read_dropped(
        files: Vec<std::sync::Arc<dyn egui::DroppedFile + Send + Sync>>,
        inbox: Inbox,
        ctx: egui::Context,
    ) {
        wasm_bindgen_futures::spawn_local(async move {
            for f in files {
                let name = f.path().to_path_buf();
                let bytes = f.bytes_async().await;
                inbox.push(name, bytes);
            }
            ctx.request_repaint();
        });
    }

    /// Fetches a file of the web site (e.g. a font), bytes to `done`.
    pub fn fetch(url: &str, done: impl FnOnce(Result<Vec<u8>, String>) + 'static) {
        let url = url.to_string();
        wasm_bindgen_futures::spawn_local(async move {
            let r = async {
                let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
                let w = window().ok_or("no window")?;
                let resp: web_sys::Response =
                    wasm_bindgen_futures::JsFuture::from(w.fetch_with_str(&url))
                        .await
                        .map_err(err)?
                        .dyn_into()
                        .map_err(|_| "no response")?;
                if !resp.ok() {
                    return Err(format!("{url}: HTTP {}", resp.status()));
                }
                let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().map_err(err)?)
                    .await
                    .map_err(err)?;
                Ok(js_sys::Uint8Array::new(&buf).to_vec())
            }
            .await;
            done(r);
        });
    }

    fn storage() -> Option<web_sys::Storage> {
        window()?.local_storage().ok()?
    }

    pub fn get(key: &str) -> Option<String> {
        storage()?.get_item(key).ok()?
    }

    pub fn set(key: &str, value: &str) {
        if let Some(s) = storage() {
            let _ = s.set_item(key, value);
        }
    }

    /// The browser's language, e.g. "zh-CN".
    pub fn language() -> Option<String> {
        window()?.navigator().language()
    }

    /// Whether the browser runs on a Mac (for ⌘ in shortcut labels).
    pub fn is_mac() -> bool {
        window()
            .and_then(|w| w.navigator().user_agent().ok())
            .is_some_and(|ua| ua.contains("Mac"))
    }

    /// Makes closing or reloading the page ask first (`on`), or not.
    pub fn guard_unload(on: bool) {
        use wasm_bindgen::closure::Closure;
        thread_local! {
            static HANDLER: Closure<dyn FnMut(web_sys::BeforeUnloadEvent)> =
                Closure::new(|e: web_sys::BeforeUnloadEvent| {
                    e.prevent_default();
                    e.set_return_value("unsaved");
                });
        }
        if let Some(w) = window() {
            if on {
                HANDLER.with(|h| w.set_onbeforeunload(Some(h.as_ref().unchecked_ref())));
            } else {
                w.set_onbeforeunload(None);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use web::*;

/// Settings value (desktop: none here; see the settings file).
#[cfg(not(target_arch = "wasm32"))]
pub fn get(_key: &str) -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn set(_key: &str, _value: &str) {}

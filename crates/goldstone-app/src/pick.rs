//! Picking a color anywhere on the screen, with the system's own picker:
//! the color sampler (magnifier) on macOS, the desktop portal's PickColor
//! on Linux (GNOME, KDE, …), and the pixel under the next click on Windows.

use std::sync::mpsc::{Receiver, Sender, channel};

/// A picked color, or why there is none.
pub type Picked = Result<[u8; 3], String>;

#[derive(Default)]
pub struct Picker {
    rx: Option<Receiver<Picked>>,
    /// Apply the color to the fill (else the stroke).
    pub fill: bool,
    #[cfg(target_os = "macos")]
    sampler: Option<objc2::rc::Retained<objc2_app_kit::NSColorSampler>>,
}

impl Picker {
    pub fn active(&self) -> bool {
        self.rx.is_some()
    }

    /// Starts picking; the result arrives through [`Picker::poll`].
    pub fn start(&mut self, ctx: &egui::Context, fill: bool) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        self.fill = fill;
        self.start_platform(ctx.clone(), tx);
    }

    pub fn poll(&mut self) -> Option<Picked> {
        let r = self.rx.as_ref()?.try_recv().ok()?;
        self.rx = None;
        #[cfg(target_os = "macos")]
        {
            self.sampler = None;
        }
        Some(r)
    }

    #[cfg(target_os = "macos")]
    fn start_platform(&mut self, ctx: egui::Context, tx: Sender<Picked>) {
        use block2::RcBlock;
        use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace};
        let sampler = NSColorSampler::new();
        let block = RcBlock::new(move |color: *mut NSColor| {
            // Called on the main thread when the user clicks or cancels.
            let r = match unsafe { color.as_ref() } {
                None => Err("cancelled".to_string()),
                Some(c) => match c.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace()) {
                    Some(c) => {
                        let ch = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
                        Ok([
                            ch(c.redComponent()),
                            ch(c.greenComponent()),
                            ch(c.blueComponent()),
                        ])
                    }
                    None => Err("the color has no sRGB value".to_string()),
                },
            };
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        unsafe { sampler.showSamplerWithSelectionHandler(&block) };
        self.sampler = Some(sampler);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn start_platform(&mut self, ctx: egui::Context, tx: Sender<Picked>) {
        std::thread::spawn(move || {
            let _ = tx.send(portal_pick());
            ctx.request_repaint();
        });
    }

    #[cfg(windows)]
    fn start_platform(&mut self, ctx: egui::Context, tx: Sender<Picked>) {
        std::thread::spawn(move || {
            let _ = tx.send(windows_pick());
            ctx.request_repaint();
        });
    }

    #[cfg(not(any(unix, windows)))]
    fn start_platform(&mut self, _ctx: egui::Context, tx: Sender<Picked>) {
        let _ = tx.send(Err("not available on this system".into()));
    }
}

/// org.freedesktop.portal.Screenshot.PickColor.
#[cfg(all(unix, not(target_os = "macos")))]
fn portal_pick() -> Picked {
    use std::collections::HashMap;
    use zbus::blocking::{Connection, Proxy};
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Structure, Value};
    let err = |e: zbus::Error| format!("desktop portal: {e}");
    let conn = Connection::session().map_err(err)?;
    let token = format!("goldstone{}", std::process::id());
    let sender = conn
        .unique_name()
        .ok_or("desktop portal: no bus name")?
        .trim_start_matches(':')
        .replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let request = Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(err)?;
    // Listen before asking, so the answer cannot be missed.
    let mut answers = request.receive_signal("Response").map_err(err)?;
    let portal = Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
    )
    .map_err(err)?;
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    let _: OwnedObjectPath = portal.call("PickColor", &("", options)).map_err(err)?;
    let msg = answers.next().ok_or("desktop portal: no answer")?;
    let (code, results): (u32, HashMap<String, OwnedValue>) =
        msg.body().deserialize().map_err(err)?;
    if code != 0 {
        return Err("cancelled".into());
    }
    let color = results.get("color").ok_or("desktop portal: no color")?;
    let s: Structure = color
        .try_clone()
        .map_err(|e| e.to_string())?
        .try_into()
        .map_err(|e: zbus::zvariant::Error| e.to_string())?;
    let ch: Vec<u8> = s
        .fields()
        .iter()
        .map(|f| match f {
            Value::F64(v) => (v.clamp(0.0, 1.0) * 255.0).round() as u8,
            _ => 0,
        })
        .collect();
    match ch[..] {
        [r, g, b] => Ok([r, g, b]),
        _ => Err("desktop portal: unexpected color".into()),
    }
}

/// Waits for the next left click anywhere and reads the pixel under it
/// (Esc cancels).
#[cfg(windows)]
fn windows_pick() -> Picked {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::Graphics::Gdi::{CLR_INVALID, GetDC, GetPixel, ReleaseDC};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_ESCAPE, VK_LBUTTON,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let down = |k: u16| unsafe { GetAsyncKeyState(k as i32) } as u16 & 0x8000 != 0;
    let wait = || std::thread::sleep(std::time::Duration::from_millis(15));
    // The click on the Pick button itself is still held: wait for release.
    while down(VK_LBUTTON) {
        wait();
    }
    loop {
        if down(VK_ESCAPE) {
            return Err("cancelled".into());
        }
        if down(VK_LBUTTON) {
            break;
        }
        wait();
    }
    let mut p = POINT { x: 0, y: 0 };
    unsafe {
        GetCursorPos(&mut p);
        let dc = GetDC(std::ptr::null_mut());
        let c = GetPixel(dc, p.x, p.y);
        ReleaseDC(std::ptr::null_mut(), dc);
        if c == CLR_INVALID {
            return Err("cannot read the screen there".into());
        }
        Ok([
            (c & 0xff) as u8,
            ((c >> 8) & 0xff) as u8,
            ((c >> 16) & 0xff) as u8,
        ])
    }
}

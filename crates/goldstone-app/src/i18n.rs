//! Interface language: English or Simplified Chinese. Strings are written
//! in English in the code and looked up in the Chinese table when Chinese
//! is chosen (View ▸ Language). Chinese text needs a CJK font: the
//! system's own is loaded (PingFang / Hiragino on macOS, Microsoft YaHei on
//! Windows, Noto Sans CJK or WenQuanYi on Linux).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Zh,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Zh];

    /// The language's own name.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Zh => "简体中文",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Zh => "zh-CN",
        }
    }

    pub fn from_code(c: &str) -> Option<Lang> {
        let c = c.trim().to_ascii_lowercase();
        if c.starts_with("zh") {
            Some(Lang::Zh)
        } else if c.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn lang() -> Lang {
    if LANG.load(Ordering::Relaxed) == 1 {
        Lang::Zh
    } else {
        Lang::En
    }
}

pub fn set_lang(l: Lang) {
    LANG.store(matches!(l, Lang::Zh) as u8, Ordering::Relaxed);
}

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| crate::i18n_zh::ZH.iter().copied().collect())
}

/// Translates a fixed English string.
pub fn t(s: &str) -> &str {
    if lang() == Lang::En {
        return s;
    }
    table().get(s).copied().unwrap_or(s)
}

/// Translates a string that may contain variable parts: an exact entry,
/// else a template entry whose `{}` match any text ("Opened {}").
pub fn tr(s: &str) -> String {
    if lang() == Lang::En {
        return s.to_string();
    }
    if let Some(z) = table().get(s) {
        return z.to_string();
    }
    for (en, zh) in crate::i18n_zh::TEMPLATES {
        if let Some(parts) = match_template(en, s) {
            let mut out = String::new();
            let mut it = parts.into_iter();
            let mut segs = zh.split("{}").peekable();
            while let Some(seg) = segs.next() {
                out.push_str(seg);
                if segs.peek().is_some() {
                    out.push_str(&tr(&it.next().unwrap_or_default()));
                }
            }
            return out;
        }
    }
    s.to_string()
}

/// The texts that `{}` in `template` stand for in `s`, if it matches.
fn match_template(template: &str, s: &str) -> Option<Vec<String>> {
    let segs: Vec<&str> = template.split("{}").collect();
    if segs.len() < 2 {
        return None;
    }
    let mut rest = s.strip_prefix(segs[0])?;
    let mut out = Vec::new();
    for (k, seg) in segs[1..].iter().enumerate() {
        let last = k == segs.len() - 2;
        if last {
            let v = rest.strip_suffix(seg)?;
            out.push(v.to_string());
            rest = "";
        } else {
            let p = if seg.is_empty() {
                return None;
            } else {
                rest.find(seg)?
            };
            out.push(rest[..p].to_string());
            rest = &rest[p + seg.len()..];
        }
    }
    rest.is_empty().then_some(out)
}

// ---- settings and fonts ----------------------------------------------------

#[allow(dead_code)]
fn settings_file() -> Option<PathBuf> {
    Some(
        crate::templates_ui::user_dir()?
            .parent()?
            .join("settings.toml"),
    )
}

/// The saved language, else the system's.
pub fn initial() -> Lang {
    #[cfg(target_arch = "wasm32")]
    {
        return crate::platform::get("goldstone.language")
            .and_then(|l| Lang::from_code(&l))
            .or_else(|| crate::platform::language().and_then(|l| Lang::from_code(&l)))
            .unwrap_or(Lang::En);
    }
    #[allow(unreachable_code)]
    if let Some(f) = settings_file()
        && let Ok(text) = std::fs::read_to_string(f)
        && let Ok(v) = text.parse::<toml::Table>()
        && let Some(l) = v
            .get("language")
            .and_then(|l| l.as_str())
            .and_then(Lang::from_code)
    {
        return l;
    }
    system().unwrap_or(Lang::En)
}

fn system() -> Option<Lang> {
    for k in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(v) = std::env::var_os(k) {
            return Lang::from_code(&v.to_string_lossy());
        }
    }
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("defaults")
            .args(["read", "-g", "AppleLanguages"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout);
        let first = s
            .split(['(', ',', '"', '\n', ' '])
            .find(|w| !w.trim().is_empty())?;
        return Lang::from_code(first);
    }
    None
}

pub fn save(l: Lang) {
    if crate::platform::WEB {
        crate::platform::set("goldstone.language", l.code());
        return;
    }
    if let Some(f) = settings_file() {
        let mut v: toml::Table = std::fs::read_to_string(&f)
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or_default();
        v.insert("language".into(), toml::Value::String(l.code().into()));
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(&f, toml::to_string(&v).unwrap_or_default());
    }
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    /// Web: the Chinese font, fetched from the site when first needed.
    static WEB_CJK: std::cell::RefCell<Option<Vec<u8>>> = const { std::cell::RefCell::new(None) };
    static WEB_CJK_FETCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Web: where the site keeps the Chinese font (OFL, see THIRD_PARTY_LICENSES).
#[cfg(target_arch = "wasm32")]
const WEB_CJK_URL: &str = "fonts/NotoSansSC-Regular.otf";

/// Web: the Chinese font, if fetched.
#[cfg(target_arch = "wasm32")]
pub fn cjk_font() -> Option<(Vec<u8>, u32)> {
    WEB_CJK.with(|c| c.borrow().clone()).map(|b| (b, 0))
}

/// A system font with Chinese characters: (bytes, index in a collection).
#[cfg(not(target_arch = "wasm32"))]
pub fn cjk_font() -> Option<(Vec<u8>, u32)> {
    let mut candidates: Vec<PathBuf> = [
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        "/System/Library/Fonts/STHeiti Medium.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf",
        "C:\\Windows\\Fonts\\simsun.ttc",
        // Linux
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/wqy-microhei/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        "/usr/share/fonts/truetype/arphic/uming.ttc",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    // Linux: ask fontconfig as well.
    if cfg!(all(unix, not(target_os = "macos")))
        && let Ok(out) = std::process::Command::new("fc-match")
            .args(["-f", "%{file}", ":lang=zh-cn"])
            .output()
    {
        let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !p.is_empty() {
            candidates.insert(0, PathBuf::from(p));
        }
    }
    candidates
        .into_iter()
        .find_map(|p| std::fs::read(&p).ok())
        .map(|b| (b, 0))
}

/// Sets the interface fonts: egui's, Goldstone's symbol font, and (for
/// Chinese) a system CJK font. Returns false when Chinese was asked for but
/// no CJK font was found.
pub fn install_fonts(ctx: &egui::Context, chinese: bool) -> bool {
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, data: egui::FontData| {
        fonts
            .font_data
            .insert(name.into(), std::sync::Arc::new(data));
        for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(fam).or_default().push(name.into());
        }
    };
    if let Some(data) = goldstone_export::labels::symbol_font() {
        add("symbols", egui::FontData::from_static(data));
    }
    let mut ok = true;
    #[cfg(target_arch = "wasm32")]
    if chinese && cjk_font().is_none() && !WEB_CJK_FETCHING.with(|f| f.replace(true)) {
        // Fetch the font, then set the fonts again.
        let ctx = ctx.clone();
        crate::platform::fetch(WEB_CJK_URL, move |r| {
            WEB_CJK_FETCHING.with(|f| f.set(false));
            if let Ok(bytes) = r {
                WEB_CJK.with(|c| *c.borrow_mut() = Some(bytes));
                if lang() == Lang::Zh {
                    install_fonts(&ctx, true);
                }
            }
            ctx.request_repaint();
        });
    }
    if chinese {
        match cjk_font() {
            Some((bytes, index)) => {
                let mut d = egui::FontData::from_owned(bytes);
                d.index = index;
                add("cjk", d);
            }
            None => ok = crate::platform::WEB,
        }
    }
    ctx.set_fonts(fonts);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_match() {
        assert_eq!(
            match_template("Opened {}", "Opened /a/b.gold"),
            Some(vec!["/a/b.gold".into()])
        );
        assert_eq!(
            match_template("Grouped {} objects as {}", "Grouped 3 objects as g1"),
            Some(vec!["3".into(), "g1".into()])
        );
        assert_eq!(match_template("Opened {}", "Saved x"), None);
    }

    /// Every `t("…")` in the interface code has a Chinese entry.
    #[test]
    fn every_ui_string_is_translated() {
        let sources = [
            include_str!("panels.rs"),
            include_str!("menus.rs"),
            include_str!("plot_dialog.rs"),
            include_str!("templates_ui.rs"),
            include_str!("update.rs"),
            include_str!("app.rs"),
        ];
        let table: std::collections::HashSet<&str> =
            crate::i18n_zh::ZH.iter().map(|(en, _)| *en).collect();
        let mut missing = Vec::new();
        for src in sources {
            let mut rest = src;
            while let Some(k) = rest.find("t(\"") {
                let ok_before =
                    k == 0 || !rest[..k].ends_with(|c: char| c.is_alphanumeric() || c == '_');
                rest = &rest[k + 3..];
                let mut lit = String::new();
                let mut chars = rest.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some('n') => lit.push('\n'),
                            Some(e) => lit.push(e),
                            None => break,
                        },
                        '"' => break,
                        c => lit.push(c),
                    }
                }
                if ok_before
                    && lit.chars().any(|c| c.is_ascii_alphabetic())
                    && !table.contains(lit.as_str())
                {
                    missing.push(lit);
                }
            }
        }
        missing.sort();
        missing.dedup();
        assert!(missing.is_empty(), "not translated: {missing:#?}");
    }

    #[test]
    fn table_has_no_duplicates_or_empty_entries() {
        let mut seen = std::collections::HashSet::new();
        for (en, zh) in crate::i18n_zh::ZH {
            assert!(seen.insert(*en), "duplicate entry {en:?}");
            assert!(!zh.trim().is_empty(), "empty translation of {en:?}");
        }
        for (en, zh) in crate::i18n_zh::TEMPLATES {
            assert_eq!(en.matches("{}").count(), zh.matches("{}").count(), "{en:?}");
        }
    }
}

//! Help ▸ Check for Updates: asks GitHub for the latest release and
//! installs it in place of the running program. Network access and
//! unpacking use the system's own `curl`, `tar` and (macOS) `hdiutil`, so
//! Goldstone carries no TLS or archive code of its own.
//!
//! How the update is installed depends on how Goldstone was installed:
//! the macOS app bundle is replaced from the new `.dmg`; an AppImage is
//! replaced by the new AppImage; a program unpacked from a `.tar.gz` or
//! `.zip` is replaced by the new one; a Debian package (in `/usr`) is
//! downloaded for the user to install with `apt`.

use crate::i18n::t;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};

pub const REPO: &str = "sunzhg07/goldstone";

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub tag: String,
    pub notes: String,
    pub page: String,
    /// (file name, download URL)
    pub assets: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub enum Status {
    Idle,
    Checking,
    UpToDate(String),
    Available(Release),
    Installing(String),
    /// Installed in place; restart to use it. The path is what to start.
    Installed(String, PathBuf),
    /// Downloaded; the user finishes the installation (message, file).
    Downloaded(String, PathBuf),
    Failed(String),
}

pub struct Updater {
    pub open: bool,
    pub status: Status,
    rx: Option<Receiver<Status>>,
}

impl Default for Updater {
    fn default() -> Self {
        Self {
            open: false,
            status: Status::Idle,
            rx: None,
        }
    }
}

/// Version numbers of a tag or version string: "v0.1" → [0, 1].
pub fn version(s: &str) -> Vec<u64> {
    s.trim_start_matches(['v', 'V'])
        .split(['.', '-', '+'])
        .map_while(|p| p.parse().ok())
        .collect()
}

/// Whether version `a` is newer than `b` (missing parts count as 0).
pub fn newer(a: &[u64], b: &[u64]) -> bool {
    let n = a.len().max(b.len());
    let get = |v: &[u64], i: usize| v.get(i).copied().unwrap_or(0);
    for i in 0..n {
        if get(a, i) != get(b, i) {
            return get(a, i) > get(b, i);
        }
    }
    false
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Reads GitHub's `releases/latest` answer.
pub fn parse_release(json: &str) -> Result<Release, String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("unexpected answer from GitHub: {e}"))?;
    let s = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let tag = s("tag_name");
    if tag.is_empty() {
        return Err(match v.get("message").and_then(|m| m.as_str()) {
            Some(m) => format!("GitHub: {m}"),
            None => "no release found".into(),
        });
    }
    let assets = v
        .get("assets")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| {
                    Some((
                        x.get("name")?.as_str()?.to_string(),
                        x.get("browser_download_url")?.as_str()?.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Release {
        tag,
        notes: s("body"),
        page: s("html_url"),
        assets,
    })
}

/// How this copy of Goldstone was installed.
#[derive(Debug, Clone, PartialEq)]
pub enum Install {
    /// macOS app bundle (the `.app` directory).
    MacApp(PathBuf),
    /// Linux AppImage file.
    AppImage(PathBuf),
    /// Linux system package (`/usr/bin`).
    Deb,
    /// A plain executable (from a `.tar.gz` or `.zip`).
    Exe(PathBuf),
}

pub fn install_kind() -> Option<Install> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    if cfg!(target_os = "macos")
        && let Some(app) = exe
            .ancestors()
            .find(|p| p.extension().is_some_and(|e| e == "app"))
    {
        return Some(Install::MacApp(app.to_path_buf()));
    }
    if cfg!(target_os = "linux") {
        if let Some(a) = std::env::var_os("APPIMAGE") {
            return Some(Install::AppImage(PathBuf::from(a)));
        }
        if exe.starts_with("/usr") {
            return Some(Install::Deb);
        }
    }
    Some(Install::Exe(exe))
}

/// The release file for an installation kind on this system.
pub fn asset_for<'a>(r: &'a Release, kind: &Install) -> Option<&'a (String, String)> {
    let suffix = match kind {
        Install::MacApp(_) => "-macos-universal.dmg",
        Install::AppImage(_) => ".AppImage",
        Install::Deb => "_amd64.deb",
        Install::Exe(_) if cfg!(target_os = "macos") => "-macos-universal.tar.gz",
        Install::Exe(_) if cfg!(windows) => "-windows-x86_64.zip",
        Install::Exe(_) => "-linux-x86_64.tar.gz",
    };
    r.assets.iter().find(|(n, _)| n.ends_with(suffix))
}

fn run(cmd: &mut Command) -> Result<String, String> {
    let name = format!("{:?}", cmd.get_program());
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {name}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(format!("{name} failed: {}", err.trim()))
    }
}

fn curl() -> Command {
    let mut c = Command::new("curl");
    c.args(["-fsSL", "--retry", "2", "-A", "goldstone-updater"]);
    c
}

fn check() -> Result<Release, String> {
    let json = run(curl()
        .arg("-H")
        .arg("Accept: application/vnd.github+json")
        .arg(format!(
            "https://api.github.com/repos/{REPO}/releases/latest"
        )))?;
    parse_release(&json)
}

/// Finds a file named `name` under `dir`.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = find(&p, name) {
                return Some(f);
            }
        } else if p.file_name().is_some_and(|n| n == name) {
            return Some(p);
        }
    }
    None
}

/// Puts `new` in place of `target` (which may be running): the old file
/// is moved aside first, which every system allows for a running program.
fn replace_file(new: &Path, target: &Path) -> Result<(), String> {
    let staged = target.with_extension("update-new");
    std::fs::copy(new, &staged).map_err(|e| format!("{}: {e}", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755));
    }
    let old = old_path(target);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(target, &old).map_err(|e| format!("{}: {e}", target.display()))?;
    if let Err(e) = std::fs::rename(&staged, target) {
        let _ = std::fs::rename(&old, target);
        return Err(format!("{}: {e}", target.display()));
    }
    if !cfg!(windows) {
        let _ = std::fs::remove_file(&old);
    }
    Ok(())
}

/// Where the replaced program is kept until the next start (Windows cannot
/// delete a running program).
fn old_path(exe: &Path) -> PathBuf {
    let name = exe
        .file_name()
        .map_or("goldstone".into(), |n| n.to_string_lossy().into_owned());
    exe.with_file_name(format!("{name}.old"))
}

/// Deletes the program replaced by the last update.
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
    }
}

fn install(r: &Release, tx: &Sender<Status>, ctx: &egui::Context) -> Result<Status, String> {
    let step = |s: String| {
        let _ = tx.send(Status::Installing(s));
        ctx.request_repaint();
    };
    let kind = install_kind().ok_or("cannot tell where Goldstone is installed")?;
    let (name, url) = asset_for(r, &kind)
        .ok_or_else(|| format!("release {} has no package for this system", r.tag))?;
    let dir = std::env::temp_dir().join(format!("goldstone-update-{}", r.tag));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(name);
    step(format!("Downloading {name}…"));
    run(curl().arg("-o").arg(&file).arg(url))?;
    match kind {
        Install::MacApp(app) => {
            step("Opening the disk image…".into());
            let mnt = dir.join("mnt");
            run(Command::new("hdiutil")
                .args(["attach", "-nobrowse", "-readonly", "-mountpoint"])
                .arg(&mnt)
                .arg(&file))?;
            let result = (|| {
                let src = std::fs::read_dir(&mnt)
                    .map_err(|e| e.to_string())?
                    .flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().is_some_and(|e| e == "app"))
                    .ok_or("no app in the disk image")?;
                step("Replacing the app…".into());
                let parent = app.parent().ok_or("app has no folder")?;
                let new = parent.join(".Goldstone-update.app");
                let old = parent.join(".Goldstone-old.app");
                let _ = std::fs::remove_dir_all(&new);
                let _ = std::fs::remove_dir_all(&old);
                run(Command::new("ditto").arg(&src).arg(&new))?;
                std::fs::rename(&app, &old).map_err(|e| format!("{}: {e}", app.display()))?;
                if let Err(e) = std::fs::rename(&new, &app) {
                    let _ = std::fs::rename(&old, &app);
                    return Err(format!("{}: {e}", app.display()));
                }
                let _ = std::fs::remove_dir_all(&old);
                Ok(())
            })();
            let _ = run(Command::new("hdiutil").arg("detach").arg(&mnt));
            result?;
            Ok(Status::Installed(
                format!("Goldstone {} is installed.", r.tag),
                app,
            ))
        }
        Install::AppImage(path) => {
            step("Replacing the AppImage…".into());
            replace_file(&file, &path)?;
            Ok(Status::Installed(
                format!("Goldstone {} is installed.", r.tag),
                path,
            ))
        }
        Install::Deb => {
            let target = std::env::var_os("HOME")
                .map(|h| PathBuf::from(h).join("Downloads"))
                .filter(|d| d.is_dir())
                .unwrap_or(dir.clone())
                .join(name);
            std::fs::copy(&file, &target).map_err(|e| format!("{}: {e}", target.display()))?;
            Ok(Status::Downloaded(
                format!(
                    "Goldstone was installed as a system package. Install the new one with:\n\nsudo apt install {}",
                    target.display()
                ),
                target,
            ))
        }
        Install::Exe(exe) => {
            step("Unpacking…".into());
            let out = dir.join("unpacked");
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            // tar reads .tar.gz, and on Windows (bsdtar) also .zip.
            run(Command::new("tar")
                .arg(if cfg!(windows) { "-xf" } else { "-xzf" })
                .arg(&file)
                .arg("-C")
                .arg(&out))?;
            let bin = if cfg!(windows) {
                "goldstone.exe"
            } else {
                "goldstone"
            };
            let new = find(&out, bin).ok_or_else(|| format!("no {bin} in {name}"))?;
            step("Replacing the program…".into());
            replace_file(&new, &exe)?;
            Ok(Status::Installed(
                format!("Goldstone {} is installed.", r.tag),
                exe,
            ))
        }
    }
}

/// Starts the installed program (with these files) after an update.
pub fn relaunch(path: &Path, files: &[PathBuf]) -> Result<(), String> {
    let r = if path.extension().is_some_and(|e| e == "app") {
        Command::new("open")
            .arg("-n")
            .arg(path)
            .arg("--args")
            .args(files)
            .spawn()
    } else {
        Command::new(path).args(files).spawn()
    };
    r.map(|_| ())
        .map_err(|e| format!("cannot start {}: {e}", path.display()))
}

/// Opens a web page or file with the system's handler.
pub fn open_external(target: &str) {
    let _ = if cfg!(target_os = "macos") {
        Command::new("open").arg(target).spawn()
    } else if cfg!(windows) {
        Command::new("cmd")
            .args(["/C", "start", "", target])
            .spawn()
    } else {
        Command::new("xdg-open").arg(target).spawn()
    };
}

impl Updater {
    fn spawn(
        &mut self,
        ctx: &egui::Context,
        job: impl FnOnce(&Sender<Status>, &egui::Context) -> Status + Send + 'static,
    ) {
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let s = job(&tx, &ctx);
            let _ = tx.send(s);
            ctx.request_repaint();
        });
    }

    pub fn check(&mut self, ctx: &egui::Context) {
        self.status = Status::Checking;
        self.spawn(ctx, |_, _| match check() {
            Ok(r) if newer(&version(&r.tag), &version(current())) => Status::Available(r),
            Ok(r) => Status::UpToDate(r.tag),
            Err(e) => Status::Failed(e),
        });
    }

    pub fn install(&mut self, ctx: &egui::Context, r: Release) {
        self.status = Status::Installing("Starting…".into());
        self.spawn(ctx, move |tx, ctx| {
            install(&r, tx, ctx).unwrap_or_else(Status::Failed)
        });
    }

    /// Takes the latest news from the background job.
    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(s) = rx.try_recv() {
                self.status = s;
            }
            if !matches!(self.status, Status::Checking | Status::Installing(_)) {
                self.rx = None;
            }
        }
    }
}

impl crate::app::GoldstoneApp {
    /// Help ▸ Check for Updates window.
    pub(crate) fn update_window(&mut self, ctx: &egui::Context) {
        if !self.updater.open {
            return;
        }
        self.updater.poll();
        let mut open = true;
        let page = format!("https://github.com/{REPO}/releases/latest");
        let unsaved = self.has_unsaved();
        let mut restart: Option<PathBuf> = None;
        egui::Window::new(t("Check for Updates"))
            .open(&mut open)
            .collapsible(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                ui.label(crate::i18n::tr(&format!(
                    "This is Goldstone {}.",
                    current()
                )));
                ui.add_space(4.0);
                match self.updater.status.clone() {
                    Status::Idle | Status::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t("Asking GitHub for the latest release…"));
                        });
                    }
                    Status::UpToDate(tag) => {
                        ui.label(
                            egui::RichText::new(crate::i18n::tr(&format!(
                                "✓ This is the latest version ({tag})."
                            )))
                            .color(egui::Color32::from_rgb(40, 150, 60)),
                        );
                    }
                    Status::Available(r) => {
                        ui.label(
                            egui::RichText::new(crate::i18n::tr(&format!(
                                "Goldstone {} is available.",
                                r.tag
                            )))
                            .strong(),
                        );
                        if !r.notes.trim().is_empty() {
                            egui::ScrollArea::vertical()
                                .max_height(180.0)
                                .show(ui, |ui| {
                                    ui.label(r.notes.trim());
                                });
                        }
                        ui.horizontal(|ui| {
                            let can = install_kind().is_some_and(|k| asset_for(&r, &k).is_some());
                            if ui
                                .add_enabled(
                                    can,
                                    egui::Button::new(crate::i18n::tr(&format!(
                                        "Install {}",
                                        r.tag
                                    ))),
                                )
                                .clicked()
                            {
                                self.updater.install(ctx, r.clone());
                            }
                            if ui.button(t("Release Page")).clicked() {
                                open_external(&r.page);
                            }
                        });
                    }
                    Status::Installing(step) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(step);
                        });
                    }
                    Status::Installed(msg, path) => {
                        ui.label(
                            egui::RichText::new(crate::i18n::tr(&format!("✓ {msg}")))
                                .color(egui::Color32::from_rgb(40, 150, 60)),
                        );
                        if unsaved {
                            ui.label(t("Save your diagrams, then restart to use it."));
                        }
                        if ui
                            .add_enabled(!unsaved, egui::Button::new(t("Restart Now")))
                            .clicked()
                        {
                            restart = Some(path);
                        }
                    }
                    Status::Downloaded(msg, file) => {
                        ui.add(
                            egui::Label::new(egui::RichText::new(msg).monospace()).selectable(true),
                        );
                        ui.horizontal(|ui| {
                            if ui.button(t("Open the Package")).clicked() {
                                open_external(&file.to_string_lossy());
                            }
                            if let Some(dir) = file.parent()
                                && ui.button(t("Show Folder")).clicked()
                            {
                                open_external(&dir.to_string_lossy());
                            }
                        });
                    }
                    Status::Failed(e) => {
                        ui.label(
                            egui::RichText::new(e).color(egui::Color32::from_rgb(190, 30, 30)),
                        );
                        ui.horizontal(|ui| {
                            if ui.button(t("Try Again")).clicked() {
                                self.updater.check(ctx);
                            }
                            if ui.button(t("Release Page")).clicked() {
                                open_external(&page);
                            }
                        });
                    }
                }
            });
        if let Some(path) = restart {
            match relaunch(&path, &self.open_files()) {
                Ok(()) => self.quit_now(ctx),
                Err(e) => self.updater.status = Status::Failed(e),
            }
        }
        self.updater.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(version("v0.1"), vec![0, 1]);
        assert_eq!(version("0.1.0"), vec![0, 1, 0]);
        assert_eq!(version("v1.2.3-beta"), vec![1, 2, 3]);
        assert!(!newer(&version("v0.1"), &version("0.1.0")));
        assert!(newer(&version("v0.2"), &version("0.1.0")));
        assert!(newer(&version("v0.1.1"), &version("0.1.0")));
        assert!(!newer(&version("v0.0.9"), &version("0.1.0")));
        assert!(newer(&version("v1.0"), &version("0.9.9")));
    }

    #[test]
    fn reads_a_release_and_picks_the_package() {
        let json = r#"{"tag_name": "v0.2", "html_url": "https://github.com/x/y/releases/tag/v0.2", "body": "news",
          "assets": [
            {"name": "goldstone-v0.2-macos-universal.dmg", "browser_download_url": "https://e/dmg"},
            {"name": "goldstone-v0.2-macos-universal.tar.gz", "browser_download_url": "https://e/mtgz"},
            {"name": "goldstone-v0.2-linux-x86_64.tar.gz", "browser_download_url": "https://e/ltgz"},
            {"name": "goldstone-v0.2-x86_64.AppImage", "browser_download_url": "https://e/appimage"},
            {"name": "goldstone_0.2_amd64.deb", "browser_download_url": "https://e/deb"},
            {"name": "goldstone-v0.2-windows-x86_64.zip", "browser_download_url": "https://e/zip"}
          ]}"#;
        let r = parse_release(json).unwrap();
        assert_eq!(r.tag, "v0.2");
        assert_eq!(r.assets.len(), 6);
        let url = |k: Install| asset_for(&r, &k).map(|a| a.1.as_str());
        assert_eq!(
            url(Install::MacApp("/A/Goldstone.app".into())),
            Some("https://e/dmg")
        );
        assert_eq!(
            url(Install::AppImage("/x".into())),
            Some("https://e/appimage")
        );
        assert_eq!(url(Install::Deb), Some("https://e/deb"));
        let exe = url(Install::Exe("/x".into())).unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(exe, "https://e/mtgz");
        } else if cfg!(windows) {
            assert_eq!(exe, "https://e/zip");
        } else {
            assert_eq!(exe, "https://e/ltgz");
        }
        assert!(
            parse_release(r#"{"message": "Not Found"}"#)
                .unwrap_err()
                .contains("Not Found")
        );
    }

    #[test]
    fn replaces_a_running_style_file() {
        let dir = std::env::temp_dir().join(format!("goldstone-upd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (target, new) = (dir.join("goldstone"), dir.join("new"));
        std::fs::write(&target, "old").unwrap();
        std::fs::write(&new, "new").unwrap();
        replace_file(&new, &target).unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! Native "open file" / "save file" windows.
//!
//! The window runs on its own thread so the synth keeps drawing (and the host keeps running)
//! while it is open; the editor polls the result every frame.
//! - Windows / macOS: the system dialog (rfd).
//! - Linux: zenity (GNOME and most desktops) or kdialog (KDE), whichever is installed; plug-in
//!   windows cannot use the GTK / Qt dialogs of the host directly.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// What the dialog gave: files, nothing (cancelled), or why there is no dialog.
pub type PickResult = Result<Vec<PathBuf>, String>;

/// A running dialog. `poll` returns the result once, when the window has closed.
pub struct Pending {
    slot: Arc<Mutex<Option<PickResult>>>,
}

impl Pending {
    pub fn poll(&self) -> Option<PickResult> {
        self.slot.lock().ok().and_then(|mut g| g.take())
    }
}

/// A file type filter: (name, extensions without the dot).
pub type Filter = (&'static str, &'static [&'static str]);

fn spawn(f: impl FnOnce() -> PickResult + Send + 'static) -> Pending {
    let slot: Arc<Mutex<Option<PickResult>>> = Arc::new(Mutex::new(None));
    let s2 = slot.clone();
    std::thread::spawn(move || {
        let r = f();
        if let Ok(mut g) = s2.lock() {
            *g = Some(r);
        }
    });
    Pending { slot }
}

/// Opens one file (or several with `multi`).
pub fn open(title: &str, filter: Filter, multi: bool) -> Pending {
    let title = title.to_string();
    spawn(move || open_blocking(&title, filter, multi))
}

/// Asks where to save, suggesting `name` (with extension `ext`).
pub fn save(title: &str, name: &str, ext: &'static str) -> Pending {
    let (title, name) = (title.to_string(), name.to_string());
    spawn(move || save_blocking(&title, &name, ext))
}

#[cfg(target_os = "linux")]
fn run(cmd: &str, args: &[String]) -> Option<PickResult> {
    // None: the program is not installed
    let out = std::process::Command::new(cmd).args(args).output().ok()?;
    if !out.status.success() {
        // cancelled (exit code 1), or the program failed
        return Some(Ok(Vec::new()));
    }
    let txt = String::from_utf8_lossy(&out.stdout);
    Some(Ok(txt.lines().map(str::trim).filter(|l| !l.is_empty()).map(PathBuf::from).collect()))
}

#[cfg(target_os = "linux")]
const NO_DIALOG: &str = "no file window on this system: install zenity (or kdialog), or drop the file onto the synth";

#[cfg(target_os = "linux")]
fn open_blocking(title: &str, filter: Filter, multi: bool) -> PickResult {
    let globs: Vec<String> = filter.1.iter().map(|e| format!("*.{e} *.{}", e.to_uppercase())).collect();
    let mut z = vec!["--file-selection".to_string(), format!("--title={title}"), format!("--file-filter={} | {}", filter.0, globs.join(" ")), "--file-filter=All files | *".to_string()];
    if multi {
        z.push("--multiple".into());
        z.push("--separator=\n".into());
    }
    if let Some(r) = run("zenity", &z) {
        return r;
    }
    let mut kd = vec!["--title".to_string(), title.to_string(), "--getopenfilename".to_string(), dirs_home(), format!("{}|{}", globs.join(" "), filter.0)];
    if multi {
        kd.insert(2, "--multiple".into());
        kd.insert(3, "--separate-output".into());
    }
    run("kdialog", &kd).unwrap_or_else(|| Err(NO_DIALOG.into()))
}

#[cfg(target_os = "linux")]
fn save_blocking(title: &str, name: &str, ext: &str) -> PickResult {
    let z = vec!["--file-selection".to_string(), "--save".into(), "--confirm-overwrite".into(), format!("--title={title}"), format!("--filename={}/{name}", dirs_home())];
    let r = match run("zenity", &z) {
        Some(r) => r,
        None => run("kdialog", &["--title".to_string(), title.to_string(), "--getsavefilename".to_string(), format!("{}/{name}", dirs_home()), format!("*.{ext}")]).unwrap_or_else(|| Err(NO_DIALOG.into())),
    };
    r.map(|v| v.into_iter().map(|p| with_ext(p, ext)).collect())
}

#[cfg(target_os = "linux")]
fn dirs_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| "/".into())
}

#[cfg(not(target_os = "linux"))]
fn open_blocking(title: &str, filter: Filter, multi: bool) -> PickResult {
    let d = rfd::FileDialog::new().set_title(title).add_filter(filter.0, filter.1);
    Ok(if multi { d.pick_files().unwrap_or_default() } else { d.pick_file().into_iter().collect() })
}

#[cfg(not(target_os = "linux"))]
fn save_blocking(title: &str, name: &str, ext: &'static str) -> PickResult {
    let d = rfd::FileDialog::new().set_title(title).set_file_name(name).add_filter(ext, &[ext]);
    Ok(d.save_file().into_iter().map(|p| with_ext(p, ext)).collect())
}

/// Adds the extension when the name typed in the dialog has none.
fn with_ext(p: PathBuf, ext: &str) -> PathBuf {
    if p.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
        p
    } else {
        let mut s = p.into_os_string();
        s.push(format!(".{ext}"));
        PathBuf::from(s)
    }
}

pub const PICTURES: Filter = ("Pictures (PNG, JPG)", &["png", "jpg", "jpeg"]);
pub const THEMES: Filter = ("SyntiTyzer themes", &["theme"]);
pub const WAVS: Filter = ("WAV audio", &["wav"]);
pub const PRESETS: Filter = ("SyntiTyzer presets", &["nyp"]);
pub const SFZ: Filter = ("Multisamples (SFZ)", &["sfz"]);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_is_added_once() {
        assert_eq!(with_ext(PathBuf::from("/a/b"), "theme"), PathBuf::from("/a/b.theme"));
        assert_eq!(with_ext(PathBuf::from("/a/b.THEME"), "theme"), PathBuf::from("/a/b.THEME"));
        assert_eq!(with_ext(PathBuf::from("/a/b.png"), "theme"), PathBuf::from("/a/b.png.theme"));
    }
}

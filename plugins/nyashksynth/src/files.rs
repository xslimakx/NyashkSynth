//! Folders on disk: presets, wavetables, noise samples. Everything is plain files, so
//! dropping a file into the folder is all it takes to make it appear in the synth.

use std::path::{Path, PathBuf};

pub fn base_dir() -> Option<PathBuf> {
    let base: Option<PathBuf> = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    };
    base.map(|b| b.join("NyashkSynth"))
}

pub fn presets_dir() -> Option<PathBuf> {
    base_dir().map(|b| b.join("Presets"))
}
pub fn tables_dir() -> Option<PathBuf> {
    base_dir().map(|b| b.join("Tables"))
}
pub fn noises_dir() -> Option<PathBuf> {
    base_dir().map(|b| b.join("Noises"))
}

/// Create the folders (and a short README in each) if they do not exist yet.
pub fn ensure_dirs() {
    let items: [(Option<PathBuf>, &str); 3] = [
        (presets_dir(), "Put .nyp preset files here. Sub-folders become categories (Bass, Lead, Pad...)."),
        (tables_dir(), "Put wavetable .wav files here (2048-sample frames, or any single cycle). Sub-folders become categories."),
        (noises_dir(), "Put noise / sample .wav files here. They appear in the NOISE source list."),
    ];
    for (d, txt) in items {
        if let Some(d) = d {
            if !d.exists() {
                let _ = std::fs::create_dir_all(&d);
                let _ = std::fs::write(d.join("README.txt"), txt);
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub cat: String,
    pub path: PathBuf,
}

/// Recursively list files with one of the extensions. The category is the relative folder.
pub fn scan(root: &Path, exts: &[&str]) -> Vec<Entry> {
    let mut out = Vec::new();
    walk(root, root, exts, 0, &mut out);
    out.sort_by(|a, b| (a.cat.to_lowercase(), a.name.to_lowercase()).cmp(&(b.cat.to_lowercase(), b.name.to_lowercase())));
    out
}

fn walk(root: &Path, dir: &Path, exts: &[&str], depth: usize, out: &mut Vec<Entry>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 4 {
                walk(root, &p, exts, depth + 1, out);
            }
        } else if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
            if exts.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                let name = p.file_stem().and_then(|x| x.to_str()).unwrap_or("?").to_string();
                let rel = p.parent().and_then(|pp| pp.strip_prefix(root).ok()).map(|r| r.to_string_lossy().replace('\\', "/")).unwrap_or_default();
                out.push(Entry { name, cat: if rel.is_empty() { "User".to_string() } else { rel }, path: p });
            }
        }
    }
}

pub fn clean_name(name: &str) -> String {
    let s: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' || c == '(' || c == ')' { c } else { '_' }).collect();
    s.trim().trim_matches('.').to_string()
}

/// Open a folder in the system file manager.
pub fn open_folder(p: &Path) {
    let _ = std::fs::create_dir_all(p);
    let cmd = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(cmd).arg(p).spawn();
}

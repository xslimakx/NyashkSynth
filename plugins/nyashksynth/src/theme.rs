//! Colour themes and the editor layout.
//!
//! A theme is a small text file (`key = #rrggbb` lines) in the `Themes` folder, so people can make
//! their own, share them and edit them in any text editor. The editor also has a theme page with
//! a colour picker for every entry. The base palette is black / grey / white; every category of
//! module (oscillators, filter, envelopes, LFOs, effects ...) has its own colour, used for its
//! values, curves and active buttons.
//!
//! The chosen theme, the UI scale and the module order are global (not part of a preset): they
//! live in `ui.cfg` next to the presets.

use nih_plug_egui::egui::Color32;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Colour slots of a theme.
pub const N_COL: usize = 20;
pub mod k {
    pub const BG: usize = 0;
    pub const PANEL: usize = 1;
    pub const PANEL2: usize = 2;
    pub const LINE: usize = 3;
    pub const TEXT: usize = 4;
    pub const DIM: usize = 5;
    pub const MID: usize = 6;
    pub const ACCENT: usize = 7;
    pub const MODC: usize = 8;
    // categories
    pub const OSC: usize = 9;
    pub const SUB: usize = 10;
    pub const NOISE: usize = 11;
    pub const FILTER: usize = 12;
    pub const ENV: usize = 13;
    pub const LFO: usize = 14;
    pub const FX: usize = 15;
    pub const VOICE: usize = 16;
    pub const MACRO: usize = 17;
    pub const MATRIX: usize = 18;
    pub const SEQ: usize = 19;
}

/// (file key, label on the theme page, tooltip)
pub const SLOTS: [(&str, &str, &str); N_COL] = [
    ("background", "BACKGROUND", "Window background"),
    ("panel", "PANEL", "Module background"),
    ("panel2", "PANEL 2", "Knob bodies, buttons, raised parts"),
    ("line", "LINES", "Borders and grid lines"),
    ("text", "TEXT", "Bright text and the brightest grey"),
    ("dim", "DIM TEXT", "Labels"),
    ("mid", "MID GREY", "Inactive symbols"),
    ("accent", "ACCENT", "Header, presets and everything without a category (and every module when COLOUR BY CATEGORY is off)"),
    ("modulation", "MODULATION", "Modulation rings, source chips, live dots"),
    ("osc", "OSCILLATORS", "OSC A / B / C"),
    ("sub", "SUB", "Sub oscillator"),
    ("noise", "NOISE", "Noise oscillator"),
    ("filter", "FILTER", "Filters"),
    ("env", "ENVELOPES", "ENV 1-4"),
    ("lfo", "LFOS", "LFO 1-8"),
    ("fx", "EFFECTS", "The FX page"),
    ("voice", "VOICING", "Voicing, mixer, global"),
    ("macro", "MACROS", "Macro knobs"),
    ("matrix", "MATRIX", "Modulation matrix"),
    ("seq", "ARP / SEQ", "Arpeggiator and clip sequencer"),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pal {
    pub c: [Color32; N_COL],
    /// corner roundness, 0 = square, 1 = normal, 2 = very round
    pub round: f32,
    /// false: every module uses ACCENT
    pub by_cat: bool,
    /// opacity of the module panels (below 1 the background picture shows through)
    pub alpha: f32,
    /// darkening of the background picture, 0 = as it is
    pub dim: f32,
    /// softness (blur) of the background picture
    pub soft: f32,
    /// strength of the per-module pictures
    pub mod_img: f32,
}

/// A picture of a theme, already scaled down to a sensible size (RGBA, not premultiplied).
#[derive(Clone, Debug, Default)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
    /// the stored file (PNG / JPEG) as base64, made once, so saving a theme never re-encodes
    pub b64: String,
}
impl PartialEq for Img {
    fn eq(&self, o: &Img) -> bool {
        self.w == o.w && self.h == o.h && self.rgba == o.rgba
    }
}
impl Img {
    pub fn new(w: usize, h: usize, rgba: Vec<u8>) -> Img {
        Img { w, h, rgba, b64: String::new() }
    }
    /// base64 of the picture file for the theme text
    pub fn stored(&self) -> Option<String> {
        if !self.b64.is_empty() {
            return Some(self.b64.clone());
        }
        encode_png(self).map(|b| b64_encode(&b))
    }
}

/// A picture file prepared for a theme: scaled down if it is too big; the original file is
/// kept as it is when it already fits (a 200 kB JPEG stays a 200 kB JPEG).
pub fn prepare(bytes: &[u8], mw: usize, mh: usize) -> Result<Img, String> {
    let img = decode_bytes(bytes)?;
    let fitted = fit(&img, mw, mh);
    let b64 = if fitted.w == img.w && fitted.h == img.h { b64_encode(bytes) } else { encode_png(&fitted).map(|b| b64_encode(&b)).ok_or("could not store the picture")? };
    Ok(Img { b64, ..fitted })
}

/// Picture slots: 0 = the window background, otherwise the category of a module (k::OSC ..).
pub const IMG_SLOTS: [usize; 12] = [k::BG, k::OSC, k::SUB, k::NOISE, k::FILTER, k::ENV, k::LFO, k::FX, k::VOICE, k::MACRO, k::MATRIX, k::SEQ];
pub fn img_label(slot: usize) -> &'static str {
    if slot == k::BG {
        "WINDOW"
    } else {
        SLOTS[slot].1
    }
}
pub type Imgs = Vec<(usize, Arc<Img>)>;

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub name: String,
    pub pal: Pal,
    pub imgs: Imgs,
    /// built-in (cannot be overwritten or deleted)
    pub builtin: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

fn greys(bg: Color32, panel: Color32, panel2: Color32, line: Color32, text: Color32, dim: Color32, mid: Color32) -> [Color32; 7] {
    [bg, panel, panel2, line, text, dim, mid]
}

fn make(g: [Color32; 7], accent: Color32, modc: Color32, cats: [Color32; 11], round: f32, by_cat: bool) -> Pal {
    let mut c = [Color32::BLACK; N_COL];
    c[..7].copy_from_slice(&g);
    c[k::ACCENT] = accent;
    c[k::MODC] = modc;
    c[9..].copy_from_slice(&cats);
    Pal { c, round, by_cat, alpha: 1.0, dim: 0.55, soft: 0.25, mod_img: 0.3 }
}

pub fn builtins() -> Vec<Theme> {
    let studio_g = greys(rgb(12, 12, 14), rgb(21, 21, 24), rgb(32, 32, 36), rgb(47, 47, 52), rgb(236, 236, 240), rgb(134, 134, 142), rgb(112, 112, 120));
    // one colour per category, all of similar brightness so none shouts
    let studio_c = [
        rgb(92, 172, 255),  // osc: blue
        rgb(86, 214, 196),  // sub: teal
        rgb(176, 184, 204), // noise: silver
        rgb(255, 170, 72),  // filter: amber
        rgb(118, 224, 136), // env: green
        rgb(246, 212, 84),  // lfo: yellow
        rgb(255, 112, 168), // fx: pink
        rgb(108, 206, 255), // voice: sky
        rgb(196, 150, 255), // macro: lilac
        rgb(170, 140, 255), // matrix: violet
        rgb(255, 120, 100), // seq: coral
    ];
    let mono_g = greys(rgb(8, 8, 8), rgb(18, 18, 18), rgb(30, 30, 30), rgb(46, 46, 46), rgb(240, 240, 240), rgb(138, 138, 138), rgb(110, 110, 110));
    let white = rgb(236, 236, 236);
    let coral_g = greys(rgb(11, 13, 19), rgb(19, 22, 30), rgb(29, 33, 44), rgb(42, 47, 61), rgb(232, 236, 244), rgb(124, 132, 150), rgb(112, 120, 138));
    let coral = rgb(255, 128, 92);
    let ocean_g = greys(rgb(9, 14, 22), rgb(15, 23, 36), rgb(24, 35, 52), rgb(36, 52, 74), rgb(226, 238, 250), rgb(120, 142, 170), rgb(100, 122, 150));
    let ocean_c = [
        rgb(70, 170, 255),
        rgb(60, 210, 220),
        rgb(150, 180, 220),
        rgb(90, 230, 210),
        rgb(120, 200, 255),
        rgb(140, 160, 255),
        rgb(190, 140, 255),
        rgb(100, 220, 255),
        rgb(160, 200, 255),
        rgb(170, 140, 255),
        rgb(80, 240, 200),
    ];
    let neon_g = greys(rgb(10, 9, 14), rgb(19, 17, 26), rgb(30, 27, 40), rgb(48, 43, 62), rgb(240, 236, 250), rgb(140, 132, 160), rgb(116, 108, 136));
    let neon_c = [
        rgb(0, 229, 255),
        rgb(0, 255, 170),
        rgb(200, 200, 255),
        rgb(255, 60, 172),
        rgb(160, 255, 60),
        rgb(255, 230, 0),
        rgb(255, 90, 240),
        rgb(90, 200, 255),
        rgb(255, 140, 40),
        rgb(180, 120, 255),
        rgb(255, 80, 80),
    ];
    vec![
        Theme { name: "Studio".into(), pal: make(studio_g, rgb(236, 236, 240), rgb(170, 140, 255), studio_c, 1.0, true), imgs: Vec::new(), builtin: true },
        Theme { name: "Mono".into(), pal: make(mono_g, white, rgb(190, 190, 190), [white; 11], 0.6, true), imgs: Vec::new(), builtin: true },
        Theme { name: "Coral".into(), pal: make(coral_g, coral, rgb(170, 140, 255), [coral; 11], 1.0, false), imgs: Vec::new(), builtin: true },
        Theme { name: "Ocean".into(), pal: make(ocean_g, rgb(120, 200, 255), rgb(160, 130, 255), ocean_c, 1.2, true), imgs: Vec::new(), builtin: true },
        Theme { name: "Neon".into(), pal: make(neon_g, rgb(0, 229, 255), rgb(255, 60, 172), neon_c, 0.4, true), imgs: Vec::new(), builtin: true },
    ]
}

impl Default for Pal {
    fn default() -> Self {
        builtins()[0].pal
    }
}

// ------------------------------------------------------------------ text format

pub fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

pub fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let v = |i: usize| u8::from_str_radix(s.get(i..i + 2)?, 16).ok();
    match s.len() {
        6 => Some(rgb(v(0)?, v(2)?, v(4)?)),
        3 => {
            let d = |i: usize| u8::from_str_radix(s.get(i..i + 1)?, 16).ok().map(|x| x * 17);
            Some(rgb(d(0)?, d(1)?, d(2)?))
        }
        _ => None,
    }
}

pub fn to_text(pal: &Pal) -> String {
    let mut t = String::from("# SyntiTyzer theme: one `key = #rrggbb` per line. Missing keys keep the Studio colour.\n");
    for (i, (key, _, tip)) in SLOTS.iter().enumerate() {
        t.push_str(&format!("{key:<11} = {}   # {tip}\n", hex(pal.c[i])));
    }
    t.push_str(&format!("roundness   = {:.2}   # 0 = square corners, 1 = normal, 2 = very round\n", pal.round));
    t.push_str(&format!("by_category = {}   # 1 = every module type has its own colour, 0 = ACCENT everywhere\n", pal.by_cat as u8));
    t.push_str(&format!("panel_alpha = {:.2}   # opacity of the modules over the background picture\n", pal.alpha));
    t.push_str(&format!("bg_darken   = {:.2}   # 0 = picture as it is, 1 = black\n", pal.dim));
    t.push_str(&format!("bg_soft     = {:.2}   # blur of the background picture\n", pal.soft));
    t.push_str(&format!("module_img  = {:.2}   # strength of the per-module pictures\n", pal.mod_img));
    t
}

/// Theme text with its pictures inside (PNG, base64), so one file carries everything.
pub fn to_text_full(pal: &Pal, imgs: &Imgs) -> String {
    let mut t = to_text(pal);
    for (slot, img) in imgs {
        if let Some(b) = img.stored() {
            t.push_str(&format!("\n[image {}]\n", SLOTS[*slot].0));
            for ch in b.as_bytes().chunks(100) {
                t.push_str(std::str::from_utf8(ch).unwrap_or(""));
                t.push('\n');
            }
            t.push_str("[end]\n");
        }
    }
    t
}

/// Colours and pictures of a full theme text.
pub fn from_text_full(txt: &str) -> (Pal, Imgs) {
    let mut imgs: Imgs = Vec::new();
    let mut plain = String::new();
    let mut cur: Option<(usize, String)> = None;
    for line in txt.lines() {
        let l = line.trim();
        if let Some((slot, mut data)) = cur.take() {
            if l == "[end]" {
                if let Some(img) = b64_decode(&data).and_then(|b| decode_bytes(&b).ok()).map(|i| {
                    // a shared file could hold a huge picture: keep it to the usual size
                    let (mw, mh) = if slot == k::BG { MAX_BG } else { MAX_MOD };
                    if i.w > mw || i.h > mh { fit(&i, mw, mh) } else { Img { b64: data.clone(), ..i } }
                }) {
                    imgs.retain(|(s, _)| *s != slot);
                    imgs.push((slot, Arc::new(img)));
                }
            } else {
                data.push_str(l);
                cur = Some((slot, data));
            }
            continue;
        }
        if let Some(key) = l.strip_prefix("[image ").and_then(|r| r.strip_suffix(']')) {
            if let Some(i) = SLOTS.iter().position(|(k, _, _)| *k == key.trim()) {
                if IMG_SLOTS.contains(&i) {
                    cur = Some((i, String::new()));
                    continue;
                }
            }
            // unknown picture: skip its data
            cur = Some((usize::MAX, String::new()));
            continue;
        }
        plain.push_str(line);
        plain.push('\n');
    }
    imgs.retain(|(s, _)| *s != usize::MAX);
    (from_text(&plain), imgs)
}

pub fn from_text(txt: &str) -> Pal {
    let mut pal = Pal::default();
    for line in txt.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        // `#` starts a comment except right after `=` (the colour itself)
        let Some((key, rest)) = line.split_once('=') else { continue };
        let key = key.trim().to_ascii_lowercase();
        let val = rest.trim();
        let val = if let Some(stripped) = val.strip_prefix('#') { format!("#{}", stripped.split_whitespace().next().unwrap_or("")) } else { val.split_whitespace().next().unwrap_or("").to_string() };
        if let Some(i) = SLOTS.iter().position(|(k, _, _)| *k == key) {
            if let Some(c) = parse_hex(&val) {
                pal.c[i] = c;
            }
        } else if key == "roundness" {
            if let Ok(v) = val.parse::<f32>() {
                pal.round = v.clamp(0.0, 2.0);
            }
        } else if key == "by_category" {
            pal.by_cat = val != "0" && !val.eq_ignore_ascii_case("false");
        } else if let (Some(slot), Ok(v)) = (
            match key.as_str() {
                "panel_alpha" => Some(&mut pal.alpha),
                "bg_darken" => Some(&mut pal.dim),
                "bg_soft" => Some(&mut pal.soft),
                "module_img" => Some(&mut pal.mod_img),
                _ => None,
            },
            val.parse::<f32>(),
        ) {
            *slot = v.clamp(0.0, 1.0);
        }
    }
    pal
}

// ------------------------------------------------------------------ files

pub fn themes_dir() -> Option<PathBuf> {
    crate::files::base_dir().map(|b| b.join("Themes"))
}

fn cfg_path() -> Option<PathBuf> {
    crate::files::base_dir().map(|b| b.join("ui.cfg"))
}

/// Built-ins first, then the user's `.theme` files (a user file with a built-in name is skipped).
pub fn list() -> Vec<Theme> {
    let mut v = builtins();
    if let Some(d) = themes_dir() {
        let mut user: Vec<Theme> = std::fs::read_dir(&d)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()).map(|x| x.eq_ignore_ascii_case("theme")) != Some(true) {
                    return None;
                }
                let name = p.file_stem()?.to_string_lossy().to_string();
                let txt = std::fs::read_to_string(&p).ok()?;
                let (pal, imgs) = from_text_full(&txt);
                Some(Theme { name, pal, imgs, builtin: false })
            })
            .filter(|t| !v.iter().any(|b| b.name.eq_ignore_ascii_case(&t.name)))
            .collect();
        user.sort_by_key(|t| t.name.to_lowercase());
        v.extend(user);
    }
    v
}

pub fn safe_name(name: &str) -> String {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || " -_".contains(*c)).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "My theme".to_string()
    } else {
        s.chars().take(40).collect()
    }
}

pub fn save(name: &str, pal: &Pal, imgs: &Imgs) -> Result<String, String> {
    let name = safe_name(name);
    if builtins().iter().any(|b| b.name.eq_ignore_ascii_case(&name)) {
        return Err(format!("\"{name}\" is a built-in theme: choose another name"));
    }
    let d = themes_dir().ok_or("no settings folder")?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    std::fs::write(d.join(format!("{name}.theme")), to_text_full(pal, imgs)).map_err(|e| e.to_string())?;
    Ok(name)
}

pub fn delete(name: &str) -> bool {
    themes_dir().map(|d| std::fs::remove_file(d.join(format!("{}.theme", safe_name(name)))).is_ok()).unwrap_or(false)
}

// ------------------------------------------------------------------ layout

/// Modules of the OSC page that can be moved. The top row and the bottom row each keep their own
/// set; dragging one module onto another (in LAYOUT mode) swaps them.
pub const TOP_MODS: [&str; 5] = ["SUB / NOISE", "OSC A", "OSC B", "OSC C", "FILTER"];
pub const BOT_MODS: [&str; 3] = ["ENVELOPES", "LFOS", "VOICING"];
pub const TOP_W: [f32; 5] = [128.0, 307.0, 307.0, 307.0, 357.0];
pub const BOT_W: [f32; 3] = [470.0, 620.0, 328.0];

#[derive(Clone, Debug, PartialEq)]
pub struct UiCfg {
    pub theme: String,
    pub pal: Pal,
    pub imgs: Imgs,
    pub top: [usize; 5],
    pub bot: [usize; 3],
    /// keyboard and wheels row at the bottom
    pub keys: bool,
    /// initial window width for new instances (0 = default)
    pub width: u32,
}

impl Default for UiCfg {
    fn default() -> Self {
        UiCfg { theme: "Studio".into(), pal: Pal::default(), imgs: Vec::new(), top: [0, 1, 2, 3, 4], bot: [0, 1, 2], keys: true, width: 0 }
    }
}

fn parse_order<const N: usize>(s: &str) -> Option<[usize; N]> {
    let v: Vec<usize> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if v.len() != N {
        return None;
    }
    let mut seen = [false; N];
    for &i in &v {
        if i >= N || seen[i] {
            return None;
        }
        seen[i] = true;
    }
    let mut a = [0; N];
    a.copy_from_slice(&v);
    Some(a)
}

impl UiCfg {
    pub fn to_text(&self) -> String {
        let j = |a: &[usize]| a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        format!(
            "# SyntiTyzer editor settings\ntheme = {}\ntop = {}\nbottom = {}\nkeyboard = {}\nwidth = {}\n[colours]\n{}",
            self.theme,
            j(&self.top),
            j(&self.bot),
            self.keys as u8,
            self.width,
            to_text_full(&self.pal, &self.imgs)
        )
    }

    pub fn from_text(txt: &str) -> UiCfg {
        let mut c = UiCfg::default();
        let (head, cols) = txt.split_once("[colours]").unwrap_or((txt, ""));
        for line in head.lines() {
            let Some((key, val)) = line.split_once('=') else { continue };
            let val = val.trim();
            match key.trim() {
                "theme" => c.theme = val.to_string(),
                "top" => c.top = parse_order(val).unwrap_or(c.top),
                "bottom" => c.bot = parse_order(val).unwrap_or(c.bot),
                "keyboard" => c.keys = val != "0",
                "width" => c.width = val.parse().unwrap_or(0),
                _ => {}
            }
        }
        if cols.trim().is_empty() {
            if let Some(t) = list().into_iter().find(|t| t.name == c.theme) {
                c.pal = t.pal;
                c.imgs = t.imgs;
            }
        } else {
            (c.pal, c.imgs) = from_text_full(cols);
        }
        c
    }

    pub fn load() -> UiCfg {
        cfg_path().and_then(|p| std::fs::read_to_string(p).ok()).map(|t| UiCfg::from_text(&t)).unwrap_or_default()
    }

    pub fn store(&self) {
        if let Some(p) = cfg_path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            let _ = std::fs::write(p, self.to_text());
        }
    }
}

// ------------------------------------------------------------------ pictures

/// Biggest stored size: the window picture, and the per-module pictures.
pub const MAX_BG: (usize, usize) = (1920, 1200);
pub const MAX_MOD: (usize, usize) = (720, 480);

pub fn decode_file(path: &Path) -> Result<Img, String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    decode_bytes(&b)
}

pub fn is_image(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("png" | "jpg" | "jpeg"))
}

pub fn decode_bytes(b: &[u8]) -> Result<Img, String> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        let mut dec = png::Decoder::new(std::io::Cursor::new(b));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut rd = dec.read_info().map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; rd.output_buffer_size()];
        let info = rd.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let (w, h) = (info.width as usize, info.height as usize);
        let px = &buf[..info.buffer_size()];
        let rgba: Vec<u8> = match info.color_type {
            png::ColorType::Rgba => px.to_vec(),
            png::ColorType::Rgb => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            png::ColorType::GrayscaleAlpha => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
            png::ColorType::Indexed => return Err("indexed PNG".into()),
        };
        if rgba.len() != w * h * 4 {
            return Err("broken PNG".into());
        }
        Ok(Img::new(w, h, rgba))
    } else if b.starts_with(&[0xFF, 0xD8]) {
        let mut dec = jpeg_decoder::Decoder::new(std::io::Cursor::new(b));
        let px = dec.decode().map_err(|e| e.to_string())?;
        let info = dec.info().ok_or("broken JPEG")?;
        let (w, h) = (info.width as usize, info.height as usize);
        let rgba: Vec<u8> = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            jpeg_decoder::PixelFormat::L8 => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
            jpeg_decoder::PixelFormat::L16 => px.chunks(2).flat_map(|c| [c[0], c[0], c[0], 255]).collect(),
            jpeg_decoder::PixelFormat::CMYK32 => px
                .chunks(4)
                .flat_map(|c| {
                    let kk = 255 - c[3] as u32;
                    let f = |x: u8| ((255 - x as u32) * kk / 255) as u8;
                    [f(c[0]), f(c[1]), f(c[2]), 255]
                })
                .collect(),
        };
        if rgba.len() != w * h * 4 {
            return Err("broken JPEG".into());
        }
        Ok(Img::new(w, h, rgba))
    } else {
        Err("not a PNG or JPEG picture".into())
    }
}

/// Area-average downscale so the picture fits inside (mw, mh); never upscales.
pub fn fit(img: &Img, mw: usize, mh: usize) -> Img {
    let k = (mw as f32 / img.w as f32).min(mh as f32 / img.h as f32);
    if k >= 1.0 || img.w == 0 || img.h == 0 {
        return img.clone();
    }
    scale_to(img, ((img.w as f32 * k).round() as usize).max(1), ((img.h as f32 * k).round() as usize).max(1))
}

pub fn scale_to(img: &Img, nw: usize, nh: usize) -> Img {
    let mut out = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        let y0 = y * img.h / nh;
        let y1 = ((y + 1) * img.h / nh).max(y0 + 1).min(img.h);
        for x in 0..nw {
            let x0 = x * img.w / nw;
            let x1 = ((x + 1) * img.w / nw).max(x0 + 1).min(img.w);
            let mut acc = [0u32; 4];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let i = (yy * img.w + xx) * 4;
                    for c in 0..4 {
                        acc[c] += img.rgba[i + c] as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            let o = (y * nw + x) * 4;
            for c in 0..4 {
                out[o + c] = (acc[c] / n) as u8;
            }
        }
    }
    Img::new(nw, nh, out)
}

/// Blurred version for the window background: a smaller copy (the GPU's linear filter does the
/// rest) followed by a light box blur, so `soft` 1 is a soft colour wash.
pub fn softened(img: &Img, soft: f32) -> Img {
    if soft <= 0.01 {
        return img.clone();
    }
    let div = 1.0 + soft * 23.0;
    let small = scale_to(img, ((img.w as f32 / div) as usize).max(4), ((img.h as f32 / div) as usize).max(4));
    let mut a = small.clone();
    for _ in 0..2 {
        let src = a.rgba.clone();
        for y in 0..a.h {
            for x in 0..a.w {
                let mut acc = [0u32; 4];
                let mut n = 0;
                for (dx, dy) in [(0i32, 0i32), (-1, 0), (1, 0), (0, -1), (0, 1)] {
                    let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                    if xx >= 0 && yy >= 0 && (xx as usize) < a.w && (yy as usize) < a.h {
                        let i = (yy as usize * a.w + xx as usize) * 4;
                        for c in 0..4 {
                            acc[c] += src[i + c] as u32;
                        }
                        n += 1;
                    }
                }
                let o = (y * a.w + x) * 4;
                for c in 0..4 {
                    a.rgba[o + c] = (acc[c] / n) as u8;
                }
            }
        }
    }
    a
}

pub fn encode_png(img: &Img) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.w as u32, img.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Default);
        let mut w = enc.write_header().ok()?;
        w.write_image_data(&img.rgba).ok()?;
    }
    Some(out)
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len().div_ceil(3) * 4);
    for ch in b.chunks(3) {
        let n = (ch[0] as u32) << 16 | (*ch.get(1).unwrap_or(&0) as u32) << 8 | *ch.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= ch.len() {
                s.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        } as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xFF) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_text_round_trips() {
        for t in builtins() {
            let back = from_text(&to_text(&t.pal));
            assert_eq!(back, t.pal, "{}", t.name);
        }
    }

    #[test]
    fn partial_and_bad_theme_files_are_safe() {
        let p = from_text("filter = #ff0000\nosc=#0f0 # short\nbogus = 1\nline = nothing\nroundness = 9\n");
        assert_eq!(p.c[k::FILTER], rgb(255, 0, 0));
        assert_eq!(p.c[k::OSC], rgb(0, 255, 0));
        assert_eq!(p.c[k::LINE], Pal::default().c[k::LINE]);
        assert_eq!(p.round, 2.0);
    }

    #[test]
    fn base64_round_trips() {
        for n in 0..40usize {
            let v: Vec<u8> = (0..n).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&v)).unwrap(), v);
        }
    }

    #[test]
    fn pictures_travel_inside_the_theme_file() {
        let img = Img::new(3, 2, (0..24).map(|i| (i * 10) as u8).collect());
        let mut pal = builtins()[3].pal;
        pal.alpha = 0.6;
        let imgs: Imgs = vec![(k::BG, Arc::new(img.clone())), (k::FILTER, Arc::new(img.clone()))];
        let txt = to_text_full(&pal, &imgs);
        let (p2, i2) = from_text_full(&txt);
        assert_eq!(p2, pal);
        assert_eq!(i2.len(), 2);
        assert_eq!(*i2[0].1, img);
        assert_eq!(i2[1].0, k::FILTER);
        // a damaged picture is dropped, the colours still load
        let bad = txt.replacen("[image filter]\n", "[image filter]\n!!!!\n", 1);
        let (p3, i3) = from_text_full(&bad);
        assert_eq!(p3, pal);
        assert_eq!(i3.len(), 1);
    }

    #[test]
    fn downscale_keeps_aspect_and_colour() {
        let img = Img::new(400, 200, [200u8, 100, 50, 255].repeat(400 * 200));
        let f = fit(&img, 100, 100);
        assert_eq!((f.w, f.h), (100, 50));
        assert_eq!(&f.rgba[..4], &[200, 100, 50, 255]);
        let s = softened(&img, 1.0);
        assert!(s.w < 40 && &s.rgba[..4] == &[200, 100, 50, 255]);
    }

    #[test]
    fn cfg_round_trips_and_rejects_bad_orders() {
        let mut c = UiCfg::default();
        c.top = [4, 1, 0, 3, 2];
        c.bot = [2, 0, 1];
        c.keys = false;
        c.width = 1600;
        c.pal.c[k::ENV] = rgb(1, 2, 3);
        assert_eq!(UiCfg::from_text(&c.to_text()), c);
        let bad = UiCfg::from_text("top = 0,0,1,2,3\nbottom = 1,2\n");
        assert_eq!(bad.top, [0, 1, 2, 3, 4]);
        assert_eq!(bad.bot, [0, 1, 2]);
    }
}

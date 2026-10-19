//! NyashkSynth GUI: black / white / grey, same look as NyashkTune.

use nih_plug::prelude::*;
use nih_plug_egui::{
    create_egui_editor,
    egui::{self, epaint::PathStroke, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2},
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::browse::{self, BAct, BItem, BState, FAV_CAT};
use crate::engine::{lfo_eval_c, warp_phase};
use crate::files;
use crate::fxui;
use crate::params::*;
use crate::presets::{self, PresetRef};
use crate::sample::{LfoCurve, NSample, LFO_POINTS};
use crate::shared::Shared;
use crate::ui::*;
use crate::wavedit::WavEdit;
use crate::wavetable::{self, Table, NAMES};
use crate::Meters;

pub const BASE_W: f32 = 1458.0;
pub const BASE_H: f32 = 902.0;

#[derive(PartialEq, Clone, Copy)]
pub enum Overlay {
    None,
    Presets,
    Tables(usize),
    Noises,
    Edit,
}

pub enum TblSrc {
    Builtin(usize),
    File(PathBuf),
}

pub struct Gui {
    auto_loaded: bool,
    pub tab: usize,
    pub fx_sel: usize,
    pub fx_bus: usize,
    pub filt_sel: usize,
    pub clip_len: u8,
    pub smp_target: Option<usize>,
    pub multi_target: usize,
    pub ms_sel: usize,
    pub ms_dirty: Option<usize>,
    osc_clip: Option<(Vec<f32>, String, String)>,
    lfo_grid: [u8; 8],
    lfo_pre: [u8; 8],
    smp_dev: bool,
    mode_dev: bool,
    ftype_dev: bool,
    arp_dev: bool,
    undo: Vec<Vec<f32>>,
    redo: Vec<Vec<f32>>,
    snap: Vec<f32>,
    prev_cur: Vec<f32>,
    pub learn_on: bool,
    pub learn_target: Option<nih_plug::params::internals::ParamPtr>,
    pub last_cc_seen: u32,
    pub pmap: Vec<(String, nih_plug::params::internals::ParamPtr)>,
    pub fx_drag: Option<usize>,
    pub mat_page: usize,
    pub lfo_page: usize,
    pub overlay: Overlay,
    presets: Vec<PresetRef>,
    favs: Vec<String>,
    hidden_n: usize,
    pub spec: Vec<f32>,
    pub spec_fs: f32,
    meta: presets::Meta,
    tag_edit: String,
    tag_for: String,
    pb: BState,
    save_name: String,
    save_cat: String,
    last_scan: f64,
    tables: Vec<(BItem, TblSrc)>,
    tbs: BState,
    frame_size: usize,
    noises: Vec<files::Entry>,
    nbs: BState,
    edit: Option<WavEdit>,
    pub copy_menu: bool,
    pub status: String,
    pub status_until: f64,
    auto: Option<(usize, f64, f32, String)>,
}

// ---------------------------------------------------------------- data helpers

pub fn osc(p: &SynthParams, o: usize) -> &OscParams {
    match o {
        0 => &p.a,
        1 => &p.b,
        _ => &p.c,
    }
}

pub const OSC_NAMES: [&str; 3] = ["A", "B", "C"];

fn scan_tables() -> Vec<(BItem, TblSrc)> {
    let mut v: Vec<(BItem, TblSrc)> = NAMES.iter().enumerate().map(|(i, n)| (BItem { name: n.to_string(), cat: "Factory".into(), fav: false, user: false, rating: 0, tags: vec![] }, TblSrc::Builtin(i))).collect();
    if let Some(d) = files::tables_dir() {
        for e in files::scan(&d, &["wav"]) {
            v.push((BItem { name: e.name, cat: e.cat, fav: false, user: true, rating: 0, tags: vec![] }, TblSrc::File(e.path)));
        }
    }
    v
}

fn cur_table(shared: &Shared, p: &SynthParams, o: usize) -> Arc<Table> {
    shared.waves[o].get().unwrap_or_else(|| wavetable::tables().t[osc(p, o).table.value().to_index()].clone())
}

fn wave_name(p: &SynthParams, o: usize) -> String {
    let s = osc(p, o).wave.lock().map(|g| g.clone()).unwrap_or_default();
    if s.is_empty() {
        NAMES[osc(p, o).table.value().to_index()].to_string()
    } else {
        wavetable::encoded_name(&s)
    }
}

fn use_builtin(setter: &ParamSetter, p: &SynthParams, shared: &Shared, o: usize, idx: usize) {
    let op = osc(p, o);
    set_e(setter, &op.table, <TableId as Enum>::from_index(idx));
    if let Ok(mut g) = op.wave.lock() {
        g.clear();
    }
    shared.waves[o].set(None);
}

fn use_custom(p: &SynthParams, shared: &Shared, o: usize, name: &str, t: Table) {
    let s = wavetable::encode_table(name, &t);
    if let Ok(mut g) = osc(p, o).wave.lock() {
        *g = s;
    }
    shared.waves[o].set(Some(Arc::new(t)));
}

fn load_table(setter: &ParamSetter, p: &SynthParams, shared: &Shared, o: usize, src: &TblSrc, frame_size: usize) -> Result<String, String> {
    match src {
        TblSrc::Builtin(i) => {
            use_builtin(setter, p, shared, o, *i);
            Ok(NAMES[*i].to_string())
        }
        TblSrc::File(path) => {
            let w = crate::wav::read(path)?;
            let t = wavetable::from_audio(&w.mono, w.clm, frame_size).ok_or("file too short")?;
            let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("wave").to_string();
            use_custom(p, shared, o, &name, t);
            Ok(name)
        }
    }
}

fn load_multi(setter: &ParamSetter, p: &SynthParams, shared: &Shared, o: usize, path: &std::path::Path) -> Result<String, String> {
    let m = crate::sample::MultiS::from_sfz(path)?;
    let name = format!("{} ({} regions)", m.name, m.regions.len());
    if let Ok(mut g) = osc(p, o).msmp.lock() {
        *g = m.encode();
    }
    shared.ms[o].set(Some(Arc::new(m)));
    let op = osc(p, o);
    if op.mode.value() != OscMode::Multisample {
        set_e(setter, &op.mode, OscMode::Multisample);
    }
    set_f(setter, &op.s_start, 0.0);
    set_f(setter, &op.s_end, 1.0);
    Ok(name)
}

fn load_ir(p: &SynthParams, shared: &Shared, path: &std::path::Path) -> Result<String, String> {
    let s = NSample::from_file(path)?;
    let name = s.name.clone();
    if let Ok(mut g) = p.ir.lock() {
        *g = s.encode();
    }
    shared.ir.set(Some(Arc::new(shared.build_ir(&s))));
    Ok(name)
}

fn load_noise(setter: &ParamSetter, p: &SynthParams, shared: &Shared, path: &std::path::Path) -> Result<String, String> {
    let s = NSample::from_file(path)?;
    let name = s.name.clone();
    if let Ok(mut g) = p.noise_sample.lock() {
        *g = s.encode();
    }
    shared.noise.set(Some(Arc::new(s)));
    set_e(setter, &p.noise_type, NoiseType::Sample);
    Ok(name)
}

fn paste_osc(setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, o: usize) {
    let Some(clip) = st.osc_clip.clone() else { return };
    apply_osc_clip(setter, p, shared, o, clip);
}

fn apply_osc_clip(setter: &ParamSetter, p: &SynthParams, shared: &Shared, o: usize, clip: (Vec<f32>, String, String)) {
    let (vals, w, smms) = clip;
    let (sm, ms) = smms.split_once('\u{1}').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((smms.clone(), String::new()));
    let op = osc(p, o);
    let map = op.param_map();
    if map.len() != vals.len() {
        return;
    }
    for ((_, ptr, _), v) in map.iter().zip(vals.iter()) {
        // SAFETY: valid pointers into `params`; GUI thread only.
        unsafe {
            if ptr.unmodulated_normalized_value() != *v {
                setter.raw_context.raw_begin_set_parameter(*ptr);
                setter.raw_context.raw_set_parameter_normalized(*ptr, *v);
                setter.raw_context.raw_end_set_parameter(*ptr);
            }
        }
    }
    if let Ok(mut g) = op.wave.lock() {
        *g = w.clone();
    }
    if let Ok(mut g) = op.smp.lock() {
        *g = sm.clone();
    }
    if let Ok(mut g) = op.msmp.lock() {
        *g = ms.clone();
    }
    shared.ms[o].set(if ms.is_empty() { None } else { crate::sample::MultiS::decode(&ms).map(Arc::new) });
    shared.smp[o].set(if sm.is_empty() { None } else { NSample::decode(&sm).map(Arc::new) });
    shared.waves[o].set(if w.is_empty() { None } else { wavetable::decode_table(&w).map(|(_, t)| Arc::new(t)) });
    shared.version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn load_osc_sample(setter: &ParamSetter, p: &SynthParams, shared: &Shared, o: usize, path: &std::path::Path) -> Result<String, String> {
    let s = NSample::from_file(path)?;
    let name = s.name.clone();
    if let Ok(mut g) = osc(p, o).smp.lock() {
        *g = s.encode();
    }
    shared.smp[o].set(Some(Arc::new(s)));
    let op = osc(p, o);
    if !matches!(op.mode.value(), OscMode::Sample | OscMode::Granular) {
        set_e(setter, &op.mode, OscMode::Sample);
    }
    set_f(setter, &op.s_start, 0.0);
    set_f(setter, &op.s_end, 1.0);
    Ok(name)
}

fn assign_mod(setter: &ParamSetter, p: &SynthParams, src_idx: usize, dst_idx: usize) -> String {
    let src = <ModSrc as Enum>::from_index(src_idx);
    let dst = <ModDst as Enum>::from_index(dst_idx);
    // already routed? just make it audible
    for s in p.slot.iter() {
        if s.src.value() == src && s.dst.value() == dst {
            if s.amount.value().abs() < 0.01 {
                set_f(setter, &s.amount, 0.5);
            }
            if !s.on.value() {
                set_b(setter, &s.on, true);
            }
            return format!("{} -> {}", ModSrc::variants()[src_idx], ModDst::variants()[dst_idx]);
        }
    }
    for s in p.slot.iter() {
        if s.src.value() == ModSrc::None && s.dst.value() == ModDst::None {
            set_e(setter, &s.src, src);
            set_e(setter, &s.dst, dst);
            set_f(setter, &s.amount, 0.5);
            set_e(setter, &s.aux, ModSrc::None);
            set_f(setter, &s.curve, 0.0);
            set_b(setter, &s.uni, false);
            set_b(setter, &s.on, true);
            return format!("{} -> {}", ModSrc::variants()[src_idx], ModDst::variants()[dst_idx]);
        }
    }
    "modulation matrix is full (16 slots)".to_string()
}

impl Gui {
    fn new() -> Gui {
        files::ensure_dirs();
        Gui {
            auto_loaded: false,
            tab: std::env::var("NYASHK_TAB").ok().and_then(|s| s.parse().ok()).unwrap_or(0),
            fx_sel: std::env::var("NYASHK_FX").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(11),
            fx_bus: 0,
            filt_sel: 0,
            clip_len: 1,
            smp_target: None,
            multi_target: 0,
            ms_sel: 0,
            ms_dirty: None,
            osc_clip: None,
            lfo_grid: [0; 8],
            lfo_pre: [0; 8],
            smp_dev: false,
            mode_dev: false,
            ftype_dev: false,
            arp_dev: false,
            undo: Vec::new(),
            redo: Vec::new(),
            snap: Vec::new(),
            prev_cur: Vec::new(),
            learn_on: false,
            learn_target: None,
            last_cc_seen: 0,
            pmap: Vec::new(),
            fx_drag: None,
            mat_page: 0,
            lfo_page: 0,
            overlay: match std::env::var("NYASHK_OVERLAY").ok().as_deref() {
                Some("presets") => Overlay::Presets,
                Some("tables") => Overlay::Tables(0),
                Some("noises") => Overlay::Noises,
                Some("edit") => Overlay::Edit,
                _ => Overlay::None,
            },
            presets: presets::list_all(),
            favs: presets::load_favs(),
            hidden_n: presets::load_hidden().len(),
            spec: Vec::new(),
            spec_fs: 48000.0,
            meta: presets::load_meta(),
            tag_edit: String::new(),
            tag_for: String::new(),
            pb: BState::default(),
            save_name: std::env::var("NYASHK_SAVENAME").unwrap_or_default(),
            save_cat: String::new(),
            last_scan: 0.0,
            tables: scan_tables(),
            tbs: BState::default(),
            frame_size: 0,
            noises: files::noises_dir().map(|d| files::scan(&d, &["wav", "sfz"])).unwrap_or_default(),
            nbs: BState::default(),
            edit: None,
            status: String::new(),
            copy_menu: false,
            status_until: 0.0,
            auto: if std::env::var("NYASHK_AUTOTEST").is_ok() { Some((0, -1.0, 0.0, String::new())) } else { None },
        }
    }

    fn say(&mut self, now: f64, t: impl Into<String>) {
        self.status = t.into();
        self.status_until = now + 4.0;
    }

    pub fn open_ir(&mut self) {
        self.rescan();
        self.smp_target = Some(99);
        self.overlay = Overlay::Noises;
    }

    fn rescan(&mut self) {
        self.presets = presets::list_all();
        self.tables = scan_tables();
        self.noises = files::noises_dir().map(|d| files::scan(&d, &["wav", "sfz"])).unwrap_or_default();
    }

    fn open_edit(&mut self, shared: &Shared, p: &SynthParams, o: usize) {
        let t = (*cur_table(shared, p, o)).clone();
        self.edit = Some(WavEdit::new(o, t, wave_name(p, o), osc(p, o).pos.value()));
        self.overlay = Overlay::Edit;
    }

    fn close_overlay(&mut self, shared: &Shared, p: &SynthParams, now: f64) {
        if let Some(e) = self.edit.as_mut() {
            e.flush(shared, p, now, true);
        }
        self.edit = None;
        self.overlay = Overlay::None;
    }
}

pub fn create(params: Arc<SynthParams>, meters: Arc<Meters>, shared: Arc<Shared>) -> Option<Box<dyn Editor>> {
    let editor_state = params.editor_state.clone();
    let size_state = editor_state.clone();
    // build the built-in wavetables once, off the GUI thread's first frame where possible
    std::thread::spawn(|| {
        let _ = wavetable::tables();
    });
    let params2 = params.clone();
    create_egui_editor(
        editor_state,
        Gui::new(),
        |ctx, _| {
            let mut v = egui::Visuals::dark();
            v.panel_fill = BG;
            v.window_fill = PANEL;
            v.extreme_bg_color = PANEL;
            v.override_text_color = Some(WHITE);
            v.selection.bg_fill = Color32::from_rgb(64, 64, 64);
            v.selection.stroke = Stroke::new(1.0_f32, WHITE);
            ctx.set_visuals(v);
            install_fonts(ctx);
        },
        move |ctx, setter, st| {
            ctx.request_repaint_after(Duration::from_millis(40));
            let p = &*params;
            meters.gui_beat.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            begin_frame(&params2, meters.live_sources());
            learn_begin(p, st);
            update_spectrum(st, &meters);
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BG)).show(ctx, |ui| {
                let full = ui.max_rect();
                let o_real = full.min;
                // everything below the header sits one page bar lower
                let o = Pos2::new(o_real.x, o_real.y + PAGE_SHIFT);
                let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(o.x + x, o.y + y), Vec2::new(w, h));
                let painter = ui.painter().clone();
                let now = ui.input(|i| i.time);
                if st.overlay == Overlay::Edit && st.edit.is_none() {
                    st.open_edit(&shared, p, 0);
                }

                if let Ok(name) = std::env::var("NYASHK_PRESET") {
                    // developer aid for screenshots: load a preset once, by name
                    if !st.auto_loaded {
                        if let Some(rf) = st.presets.iter().find(|q| q.name == name).cloned() {
                            st.auto_loaded = true;
                            let _ = presets::load(setter, p, &shared, &rf);
                        }
                    }
                }
                if let Ok(path) = std::env::var("NYASHK_SMP") {
                    // developer aid for screenshots: load a .wav as the sample of OSC A once
                    if !st.smp_dev {
                        st.smp_dev = true;
                        let _ = load_osc_sample(setter, p, &shared, 0, std::path::Path::new(&path));
                        if std::env::var("NYASHK_GRAN").is_ok() {
                            set_e(setter, &p.a.mode, OscMode::Granular);
                        }
                    }
                }
                if let Ok(v) = std::env::var("NYASHK_MODE") {
                    // developer aid for screenshots: osc A mode by index
                    if !st.mode_dev {
                        st.mode_dev = true;
                        if let Ok(i) = v.parse::<usize>() {
                            set_e(setter, &p.a.mode, OscMode::from_index(i.min(4)));
                        }
                        if let Ok(sfz) = std::env::var("NYASHK_SFZ") {
                            let _ = load_multi(setter, p, &shared, 0, std::path::Path::new(&sfz));
                        }
                    }
                }
                if let Ok(v) = std::env::var("NYASHK_FTYPE") {
                    if !st.ftype_dev {
                        st.ftype_dev = true;
                        if let Ok(i) = v.parse::<usize>() {
                            set_e(setter, &p.f1.fl_type, FilterType::from_index(i));
                        }
                    }
                }
                if std::env::var("NYASHK_ARPPAT").is_ok() && !st.arp_dev {
                    st.arp_dev = true;
                    set_e(setter, &p.arp_mode, ArpMode::Pattern);
                }
                if std::env::var("NYASHK_CLIPDEMO").is_ok() && !st.arp_dev {
                    st.arp_dev = true;
                    if let Ok(mut g) = p.clips.lock() {
                        *g = "2|0,2,0,100;2,1,7,80;4,2,12,100;6,1,5,70;8,3,3,100;11,1,10,60;12,2,-5,90;16,2,0,100;18,2,4,80;20,4,7,100;26,2,12,70;28,3,-12,100|0,30,60,90,100,80,40,0,-30,-60,-90,-100,-70,-30,0,0,0,30,60,90,100,80,40,0,-30,-60,-90,-100,-70,-30,0,0".to_string();
                    }
                    set_b(setter, &p.clip_on, true);
                }
                if let Some(n) = std::env::var("NYASHK_PLAY").ok().and_then(|v| v.parse::<usize>().ok()) {
                    // developer aid for screenshots: hold an on-screen key
                    if let Some(k) = meters.gui_keys.get(n) {
                        k.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                autotest(setter, p, &shared, st, &meters, now);
                header(ui, setter, p, &shared, st, &meters, o_real, now);
                page_bar(ui, p, st, o_real);

                // ================================================= pages
                let page_panel = |title: &str, r: Rect| -> Rect {
                    painter.rect_filled(r, CornerRadius::same(6), PANEL);
                    painter.rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
                    painter.text(Pos2::new(r.left() + 14.0, r.top() + 16.0), Align2::LEFT_CENTER, title, mono(11.0), WHITE);
                    Rect::from_min_size(Pos2::new(r.left() + 10.0, r.top() + 40.0), Vec2::new(r.width() - 20.0, r.height() - 46.0))
                };
                match st.tab {
                    0 => {
                        osc_panel(ui, setter, p, &shared, st, rect(14.0, 62.0, 350.0, 346.0), 0);
                        osc_panel(ui, setter, p, &shared, st, rect(372.0, 62.0, 350.0, 346.0), 1);
                        osc_panel(ui, setter, p, &shared, st, rect(730.0, 62.0, 350.0, 346.0), 2);
                        filter_panel(ui, setter, p, st, rect(1088.0, 62.0, 356.0, 346.0));
                        sub_noise_panel(ui, setter, p, &shared, st, rect(14.0, 416.0, 350.0, 300.0), now);
                        let c = page_panel("ENVELOPES", rect(372.0, 416.0, 1072.0, 300.0));
                        env_tab(ui, setter, p, st, c, now);
                    }
                    1 => {
                        let c = page_panel("LFO 1-4", rect(14.0, 62.0, 1430.0, 322.0));
                        lfo_tab(ui, setter, p, &shared, st, c, 0);
                        let c = page_panel("LFO 5-8", rect(14.0, 394.0, 1430.0, 322.0));
                        lfo_tab(ui, setter, p, &shared, st, c, 1);
                    }
                    2 => {
                        let c = page_panel("MODULATION MATRIX  (all routes. Add one here, or drag a chip from the bottom strip onto a knob on SOUND / FX / SETTINGS)", rect(14.0, 62.0, 1430.0, 654.0));
                        matrix_tab(ui, setter, p, st, c);
                    }
                    3 => {
                        let c = page_panel("EFFECTS", rect(14.0, 62.0, 1430.0, 654.0));
                        fxui::fx_tab(ui, setter, p, st, c);
                        let sr = Rect::from_min_max(Pos2::new(c.left(), c.top() + 94.0 + fxui::flow_offset(st.fx_sel)), Pos2::new(c.right(), c.bottom() - 6.0));
                        painter.rect_filled(sr, CornerRadius::same(4), Color32::from_rgb(8, 8, 8));
                        painter.rect_stroke(sr, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
                        let inner = sr.shrink2(Vec2::new(40.0, 14.0));
                        let sp = painter.with_clip_rect(sr);
                        for db in [-60.0f32, -40.0, -20.0, 0.0] {
                            let y = inner.bottom() - inner.height() * (db + 80.0) / 80.0;
                            sp.line_segment([Pos2::new(inner.left(), y), Pos2::new(inner.right(), y)], Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 24)));
                            sp.text(Pos2::new(inner.left() - 6.0, y), Align2::RIGHT_CENTER, format!("{db:.0}"), mono(8.5), DIM);
                        }
                        for (f, t) in [(50.0f32, "50"), (100.0, "100"), (200.0, "200"), (500.0, "500"), (1000.0, "1k"), (2000.0, "2k"), (5000.0, "5k"), (10000.0, "10k")] {
                            let x = inner.left() + inner.width() * (f / 20.0).ln() / 1000.0f32.ln();
                            sp.line_segment([Pos2::new(x, inner.top()), Pos2::new(x, inner.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 24)));
                            sp.text(Pos2::new(x, inner.bottom() + 8.0), Align2::CENTER_CENTER, t, mono(8.5), DIM);
                        }
                        draw_spectrum(&sp, inner, &st.spec, st.spec_fs, -80.0, Color32::from_rgb(200, 200, 200));
                        sp.text(Pos2::new(sr.right() - 10.0, sr.top() + 10.0), Align2::RIGHT_CENTER, "OUTPUT SPECTRUM", mono(9.5), DIM);
                    }
                    4 => {
                        let c = page_panel("ARPEGGIATOR", rect(14.0, 62.0, 1430.0, 322.0));
                        arp_tab(ui, setter, p, &meters, c);
                        let c = page_panel("CLIP SEQUENCER", rect(14.0, 394.0, 1430.0, 322.0));
                        clip_tab(ui, setter, p, &meters, st, c);
                    }
                    _ => {
                        let c = page_panel("SETTINGS", rect(14.0, 62.0, 1430.0, 654.0));
                        settings_tab(ui, setter, p, &meters, st, c);
                    }
                }

                // ================================================= bottom strip: sources, macros, voice
                strip(ui, setter, p, st, rect(14.0, 724.0, 1430.0, 76.0), now);
                if p.show_wheels.value() {
                    wheels_panel(ui, setter, p, &meters, rect(14.0, 806.0, 118.0, 54.0));
                    keyboard(ui, &meters, rect(138.0, 808.0, 1306.0, 50.0));
                } else {
                    keyboard(ui, &meters, rect(14.0, 808.0, 1430.0, 50.0));
                }

                // ================================================= overlays
                learn_end(p, st, &meters, now);
                undo_track(ui, p, st);
                let (kz, ky, shift) = ui.input(|i| (i.modifiers.command && i.key_pressed(egui::Key::Z), i.modifiers.command && i.key_pressed(egui::Key::Y), i.modifiers.shift));
                let undo_ok = st.overlay == Overlay::None && ui.ctx().memory(|m| m.focused().is_none());
                if !undo_ok {
                    // wavetable editor / text fields have their own undo
                } else if kz && !shift {
                    undo_redo(setter, st, true);
                } else if ky || (kz && shift) {
                    undo_redo(setter, st, false);
                }
                overlays(ui, setter, p, &shared, st, o, now);
                if let Some(mo) = st.ms_dirty {
                    // region edits are written into the saved state even when the page was switched meanwhile
                    if !ui.input(|i| i.pointer.any_down()) {
                        if let Some(cur) = shared.ms[mo].get() {
                            if let Ok(mut g) = osc(p, mo).msmp.lock() {
                                *g = cur.encode();
                            }
                        }
                        st.ms_dirty = None;
                    }
                }
                drop_files(ui, setter, p, &shared, st, o, now);

                // resize grip (keeps proportions)
                let grip = Rect::from_min_size(full.max - Vec2::splat(13.0), Vec2::splat(13.0));
                let gr_resp = ui.interact(grip, egui::Id::new("synth_resize_grip"), Sense::drag()).on_hover_text("Drag to resize");
                if gr_resp.dragged() {
                    if let Some(pos) = gr_resp.interact_pointer_pos() {
                        let ppp = ctx.pixels_per_point();
                        let w = ((pos.x - full.min.x + 11.0) * ppp).clamp(800.0, 2600.0);
                        size_state.set_requested_size((w.round() as u32, (w * BASE_H / BASE_W).round() as u32));
                    }
                }
                let gc = if gr_resp.hovered() || gr_resp.dragged() { WHITE } else { DIM };
                for k in [6.0f32, 11.0, 16.0] {
                    painter.line_segment([Pos2::new(full.max.x - 3.0, full.max.y - k), Pos2::new(full.max.x - k, full.max.y - 3.0)], Stroke::new(1.0_f32, gc));
                }
            });
        },
    )
}

const PAGE_SHIFT: f32 = 32.0;

fn page_bar(ui: &mut Ui, p: &SynthParams, st: &mut Gui, o: Pos2) {
    let painter = ui.painter().clone();
    let names = ["SOUND", "MOD", "MATRIX", "FX", "SEQ", "SETTINGS"];
    let tips = ["Oscillators, filter, sub / noise and envelopes", "LFO 1-8", "Modulation matrix (all 16 routes)", "Effects chain, two buses, routing", "Arpeggiator and clip sequencer", "Tuning, MIDI, bindings"];
    for (i, n) in names.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(o.x + 14.0 + 98.0 * i as f32, o.y + 56.0), Vec2::new(94.0, 26.0));
        if pill_tip(ui, r, n, st.tab == i, tips[i]) {
            if st.tab != i {
                // typed-value boxes that were open must not follow us to the other page
                ui.data_mut(|d| d.remove_by_type::<String>());
                st.copy_menu = false;
            }
            st.tab = i;
        }
        let lit = (i == 4 && (p.arp_on.value() || p.clip_on.value())) || (i == 3 && st.tab != 3 && false);
        if lit && st.tab != i {
            painter.circle_filled(Pos2::new(r.right() - 8.0, r.top() + 7.0), 3.0, WHITE);
        }
    }
    painter.text(Pos2::new(o.x + 14.0 + 98.0 * 6.0 + 10.0, o.y + 69.0), Align2::LEFT_CENTER, "drag LFO / ENV / MACRO chips onto any knob", mono(9.0), DIM);
}

// ===================================================================== header

fn header(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, meters: &Meters, o: Pos2, now: f64) {
    let painter = ui.painter().clone();
    let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(o.x + x, o.y + y), Vec2::new(w, h));
    let time = ui.input(|i| i.time);
    painter.text(Pos2::new(o.x + 18.0, o.y + 30.0), Align2::LEFT_CENTER, "Nyashk", mono(19.0), WHITE);
    painter.text(Pos2::new(o.x + 18.0 + 6.0 * 11.4, o.y + 30.0), Align2::LEFT_CENTER, "Synth", mono(19.0), DIM);
    if ((time * 2.0) as i64) % 2 == 0 {
        painter.text(Pos2::new(o.x + 18.0 + 11.0 * 11.4, o.y + 30.0), Align2::LEFT_CENTER, "_", mono(19.0), WHITE);
    }
    painter.hline(o.x + 14.0..=o.x + BASE_W - 14.0, o.y + 54.0, Stroke::new(1.0_f32, LINE));

    // ---- preset selector
    let cur_name = p.preset_name.lock().map(|g| g.clone()).unwrap_or_default();
    let cur_idx = st.presets.iter().position(|x| x.name == cur_name);
    let n = st.presets.len().max(1);
    let mut pick: Option<usize> = None;
    if pill_tip(ui, rect(262.0, 14.0, 28.0, 26.0), "<", false, "Previous preset") {
        pick = Some(cur_idx.map(|i| (i + n - 1) % n).unwrap_or(0));
    }
    let nb = rect(296.0, 14.0, 260.0, 26.0);
    let cat = cur_idx.map(|i| st.presets[i].cat.clone()).unwrap_or_default();
    let label = if cat.is_empty() { cur_name.clone() } else { format!("{}  /  {}", cat, cur_name) };
    let label: String = if label.chars().count() > 34 { label.chars().take(33).collect::<String>() + "~" } else { label };
    if pill_tip(ui, nb, &label, false, "Open the preset browser (categories, search, favorites)") {
        if st.overlay == Overlay::Edit {
            st.close_overlay(shared, p, now);
        }
        st.overlay = if st.overlay == Overlay::Presets { Overlay::None } else { Overlay::Presets };
        st.rescan();
    }
    if pill_tip(ui, rect(562.0, 14.0, 28.0, 26.0), ">", false, "Next preset") {
        pick = Some(cur_idx.map(|i| (i + 1) % n).unwrap_or(0));
    }
    let dir_txt = files::presets_dir().map(|d| d.display().to_string()).unwrap_or_default();
    if pill_tip(ui, rect(598.0, 14.0, 54.0, 26.0), "SAVE", false, &format!("Save the current sound as a preset.\nPresets are plain files in:\n{dir_txt}")) {
        if st.overlay == Overlay::Edit {
            st.close_overlay(shared, p, now);
        }
        st.overlay = Overlay::Presets;
        st.rescan();
        if st.save_name.is_empty() {
            st.save_name = cur_name.clone();
        }
        if st.save_cat.is_empty() {
            st.save_cat = if cat.is_empty() { "User".to_string() } else { cat.clone() };
        }
    }
    if let Some(i) = pick {
        if st.overlay == Overlay::Edit {
            st.close_overlay(shared, p, now);
        }
        if let Some(r) = st.presets.get(i).cloned() {
            if let Err(e) = presets::load(setter, p, shared, &r) {
                st.say(now, e);
            }
        }
    }
    if now < st.status_until {
        let stxt: String = if st.status.chars().count() > 70 { st.status.chars().take(69).collect::<String>() + "~" } else { st.status.clone() };
        let g = painter.layout_no_wrap(stxt, mono(11.0), WHITE);
        let rc = Rect::from_center_size(Pos2::new(o.x + 550.0, o.y + 46.0), g.size() + Vec2::new(16.0, 6.0));
        painter.rect_filled(rc, CornerRadius::same(5), Color32::BLACK);
        painter.rect_stroke(rc, CornerRadius::same(5), Stroke::new(1.0_f32, WHITE), egui::StrokeKind::Inside);
        painter.galley(rc.center() - g.size() * 0.5, g, WHITE);
    } else {
        painter.text(Pos2::new(o.x + 664.0, o.y + 27.0), Align2::LEFT_CENTER, "PRESET", mono(10.0), DIM);
    }

    // output meter + master
    let vc = meters.voices();
    painter.text(Pos2::new(o.x + 796.0, o.y + 27.0), Align2::RIGHT_CENTER, format!("{vc:>2} voices"), mono(10.5), DIM);
    {
        let pr = rect(716.0, 15.0, 84.0, 24.0);
        let pid = egui::Id::new("panic_click");
        let presp = ui.interact(pr, pid, Sense::click()).on_hover_text("Click = PANIC: release every note (use it if a note ever sticks)");
        if presp.clicked() {
            meters.panic.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let mr = rect(806.0, 9.0, 146.0, 38.0);
    painter.rect_filled(mr, CornerRadius::same(4), Color32::from_rgb(5, 5, 5));
    painter.rect_stroke(mr, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    {
        let snap = meters.scope_snapshot();
        let start = (0..400).find(|&i| snap[i] <= 0.0 && snap[i + 1] > 0.0).unwrap_or(0);
        let view = &snap[start..start + 560];
        let pk = view.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let gain = 1.0 / pk.max(0.2);
        let inner = mr.shrink2(Vec2::new(3.0, 5.0));
        painter.line_segment([Pos2::new(inner.left(), inner.center().y), Pos2::new(inner.right(), inner.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(26, 26, 26)));
        let n = 140;
        let pts: Vec<Pos2> = (0..n)
            .map(|i| {
                let v = view[i * view.len() / n] * gain;
                Pos2::new(inner.left() + inner.width() * i as f32 / (n - 1) as f32, inner.center().y - v * inner.height() * 0.5)
            })
            .collect();
        painter.add(egui::Shape::line(pts, PathStroke::new(1.3_f32, if pk > 0.002 { WHITE } else { Color32::from_rgb(60, 60, 60) })));
        let db = 20.0 * (meters.peak() + 1e-6).log10();
        let t = ((db + 48.0) / 48.0).clamp(0.0, 1.0);
        painter.rect_filled(Rect::from_min_size(Pos2::new(mr.left() + 2.0, mr.bottom() - 4.0), Vec2::new((mr.width() - 4.0) * t, 2.0)), CornerRadius::same(1), if db > -1.0 { WHITE } else { Color32::from_rgb(150, 150, 150) });
    }
    // MIDI learn
    if pill_tip(ui, rect(968.0, 14.0, 92.0, 26.0), "MIDI LEARN", st.learn_on, "Bind a MIDI controller to any knob: switch this on, click a knob, then move a knob / fader on your keyboard.\nRight-click a bound knob to unbind it.") {
        st.learn_on = !st.learn_on;
        st.learn_target = None;
    }
    if pill_tip(ui, rect(1064.0, 14.0, 52.0, 26.0), "CLEAR", false, "Remove every MIDI binding") {
        if let Ok(mut g) = p.cc_map.lock() {
            g.clear();
        }
        crate::rebuild_cc_map(p, meters);
    }
    if pill_tip(ui, rect(1190.0, 14.0, 44.0, 26.0), "UNDO", false, "Undo the last parameter change (Ctrl+Z). Wavetable edits are not part of this history.") {
        undo_redo(setter, st, true);
    }
    if pill_tip(ui, rect(1238.0, 14.0, 44.0, 26.0), "REDO", false, "Redo (Ctrl+Y / Ctrl+Shift+Z)") {
        undo_redo(setter, st, false);
    }
    {
        let lc = meters.last_cc.load(std::sync::atomic::Ordering::Relaxed);
        let txt = if st.learn_on {
            if st.learn_target.is_some() { "move a CC".to_string() } else { "click knob".to_string() }
        } else if lc != 0 {
            format!("last CC {}", lc & 255)
        } else {
            String::new()
        };
        painter.text(Pos2::new(o.x + 1124.0, o.y + 27.0), Align2::LEFT_CENTER, txt, mono(10.0), if st.learn_on { WHITE } else { DIM });
    }
    painter.text(Pos2::new(o.x + 1296.0, o.y + 26.0), Align2::LEFT_CENTER, "MASTER", mono(10.0), DIM);
    h_slider(ui, setter, &p.master, rect(1359.0, 19.0, 78.0, 14.0), "master_slider", false, "Master volume");
    {
        // type a value: click the number (or drag it), Enter to confirm
        let mut v = p.master.value();
        let dv = egui::DragValue::new(&mut v).range(-60.0..=6.0).speed(0.1).fixed_decimals(1).suffix(" dB");
        let resp = ui.put(rect(1352.0, 35.0, 92.0, 16.0), dv).on_hover_text("Master volume in dB: click and type a value");
        if resp.drag_started() || resp.gained_focus() {
            setter.begin_set_parameter(&p.master);
        }
        if resp.changed() {
            setter.set_parameter(&p.master, v);
        }
        if resp.drag_stopped() || resp.lost_focus() {
            setter.end_set_parameter(&p.master);
        }
    }
}

// ===================================================================== overlays

/// Drag and drop from a file manager: .wav -> wavetable (or noise over the SUB/NOISE panel), .nyp -> preset.
fn drop_files(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, o: Pos2, now: f64) {
    let area_a = Rect::from_min_size(Pos2::new(o.x + 14.0, o.y + 62.0), Vec2::new(350.0, 346.0));
    let area_b = Rect::from_min_size(Pos2::new(o.x + 372.0, o.y + 62.0), Vec2::new(350.0, 346.0));
    let area_c = Rect::from_min_size(Pos2::new(o.x + 730.0, o.y + 62.0), Vec2::new(350.0, 346.0));
    let area_n = Rect::from_min_size(Pos2::new(o.x + 14.0, o.y + 416.0), Vec2::new(350.0, 290.0));
    if ui.input(|i| !i.raw.hovered_files.is_empty()) {
        let pos = ui.input(|i| i.pointer.hover_pos());
        let painter = ui.painter().clone();
        for (r, label) in [(area_a, "DROP .wav: WAVETABLE A"), (area_b, "DROP .wav: WAVETABLE B"), (area_c, "DROP .wav: WAVETABLE C"), (area_n, "DROP .wav: NOISE SAMPLE")] {
            let hot = pos.map_or(false, |q| r.contains(q));
            painter.rect_stroke(r, CornerRadius::same(6), Stroke::new(if hot { 2.0_f32 } else { 1.0 }, if hot { WHITE } else { MID }), StrokeKind::Inside);
            painter.text(r.center_top() + Vec2::new(0.0, 14.0), Align2::CENTER_CENTER, label, mono(10.0), if hot { WHITE } else { DIM });
        }
        painter.text(Pos2::new(o.x + 560.0, o.y + 560.0), Align2::CENTER_CENTER, "Drop a .nyp preset anywhere", mono(11.0), DIM);
    }
    let dropped: Vec<PathBuf> = ui.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
    if dropped.is_empty() {
        return;
    }
    let pos = ui.input(|i| i.pointer.hover_pos()).unwrap_or(area_a.center());
    st.say(now, format!("dropped {} file(s)", dropped.len()));
    let mut msgs: Vec<String> = Vec::new();
    for path in dropped {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("file").to_string();
        if ext == "nyp" {
            // keep a copy in the preset folder, then load it
            let mut dest = path.clone();
            if let Some(dir) = files::presets_dir() {
                let _ = std::fs::create_dir_all(&dir);
                let mut d = dir.join(format!("{}.nyp", files::clean_name(&stem)));
                let mut n = 2;
                while d.exists() && d != path {
                    d = dir.join(format!("{} {n}.nyp", files::clean_name(&stem)));
                    n += 1;
                }
                if d == path || std::fs::copy(&path, &d).is_ok() {
                    dest = d;
                }
            }
            st.rescan();
            let rf = PresetRef { name: stem.clone(), cat: "User".into(), factory: None, path: Some(dest) };
            match presets::load(setter, p, shared, &rf) {
                Ok(()) => {
                    st.rescan();
                    msgs.push(format!("preset {stem} loaded and added to your presets"));
                }
                Err(e) => msgs.push(format!("preset failed: {e}")),
            }
        } else if ext == "wav" && matches!(st.overlay, Overlay::Noises) && st.smp_target.is_some() {
            // the browser was opened for an IR / oscillator sample / SFZ: the file goes there, not to the noise source
            if let Some(dir) = files::noises_dir() {
                let _ = std::fs::create_dir_all(&dir);
                let d = dir.join(path.file_name().unwrap_or_default());
                if !d.exists() {
                    let _ = std::fs::copy(&path, &d);
                }
            }
            match st.smp_target {
                Some(98) => msgs.push(format!("{stem}.wav copied to the Noises folder (now pick the .sfz that uses it)")),
                Some(99) => match load_ir(p, shared, &path) {
                    Ok(n) => msgs.push(format!("impulse response {n} loaded")),
                    Err(e) => msgs.push(format!("impulse response failed: {e}")),
                },
                Some(o) => match load_osc_sample(setter, p, shared, o.min(2), &path) {
                    Ok(n) => msgs.push(format!("sample {n} loaded into OSC {}", OSC_NAMES[o.min(2)])),
                    Err(e) => msgs.push(format!("sample failed: {e}")),
                },
                None => {}
            }
        } else if ext == "wav" {
            let to_noise = area_n.contains(pos) && !matches!(st.overlay, Overlay::Tables(_)) || matches!(st.overlay, Overlay::Noises);
            if to_noise {
                if let Some(dir) = files::noises_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    let d = dir.join(path.file_name().unwrap_or_default());
                    if !d.exists() {
                        let _ = std::fs::copy(&path, &d);
                    }
                }
                match load_noise(setter, p, shared, &path) {
                    Ok(n) => msgs.push(format!("noise sample {n} loaded")),
                    Err(e) => msgs.push(format!("noise failed: {e}")),
                }
            } else if let Some(so) = [(area_a, 0usize), (area_b, 1), (area_c, 2)].iter().find(|(a, i)| a.contains(pos) && !matches!(osc(p, *i).mode.value(), OscMode::Wavetable | OscMode::Spectral)).map(|(_, i)| *i) {
                if let Some(dir) = files::noises_dir() {
                    let _ = std::fs::create_dir_all(&dir);
                    let d = dir.join(path.file_name().unwrap_or_default());
                    if !d.exists() {
                        let _ = std::fs::copy(&path, &d);
                    }
                }
                match load_osc_sample(setter, p, shared, so, &path) {
                    Ok(n) => msgs.push(format!("sample {n} loaded into OSC {}", OSC_NAMES[so])),
                    Err(e) => msgs.push(format!("sample failed: {e}")),
                }
            } else {
                // inside a wavetable browser the file goes to that oscillator, otherwise to the one under the cursor
                let o_i = match st.overlay {
                    Overlay::Tables(i) => i.min(2),
                    _ => if area_c.contains(pos) { 2 } else { usize::from(area_b.contains(pos)) },
                };
                if let Some(dir) = files::tables_dir().map(|d| d.join("Dropped")) {
                    let _ = std::fs::create_dir_all(&dir);
                    let d = dir.join(path.file_name().unwrap_or_default());
                    if !d.exists() {
                        let _ = std::fs::copy(&path, &d);
                    }
                }
                match load_table(setter, p, shared, o_i, &TblSrc::File(path.clone()), st.frame_size) {
                    Ok(n) => msgs.push(format!("wavetable {n} loaded into OSC {}", OSC_NAMES[o_i])),
                    Err(e) => msgs.push(format!("wavetable failed: {e}")),
                }
            }
            st.rescan();
        } else {
            msgs.push(format!("{stem}: only .wav and .nyp files can be dropped"));
        }
    }
    if let Some(m) = msgs.last() {
        st.say(now, m.clone());
    }
}

fn overlays(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, o: Pos2, now: f64) {
    let area = Rect::from_min_size(Pos2::new(o.x + 14.0, o.y + 62.0), Vec2::new(1430.0, 738.0));
    if st.overlay == Overlay::None {
        return;
    }
    // swallow clicks so nothing below reacts
    ui.interact(area, egui::Id::new("overlay_block"), Sense::click_and_drag());
    match st.overlay {
        Overlay::Presets => preset_overlay(ui, setter, p, shared, st, area, now),
        Overlay::Tables(oi) => table_overlay(ui, setter, p, shared, st, area, oi, now),
        Overlay::Noises => noise_overlay(ui, setter, p, shared, st, area, now),
        Overlay::Edit => {
            let mut close = false;
            let mut status = String::new();
            if let Some(e) = st.edit.as_mut() {
                close = e.show(ui, setter, p, shared, area, now, &mut status);
            } else {
                close = true;
            }
            if !status.is_empty() {
                if status.starts_with("saved wavetable") {
                    st.rescan();
                }
                st.say(now, status);
            }
            if close {
                st.close_overlay(shared, p, now);
            }
        }
        Overlay::None => {}
    }
}

fn preset_overlay(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, area: Rect, now: f64) {
    if now - st.last_scan > 2.5 {
        st.presets = presets::list_all();
        st.last_scan = now;
    }
    let cur_name = p.preset_name.lock().map(|g| g.clone()).unwrap_or_default();
    let items: Vec<BItem> = st
        .presets
        .iter()
        .map(|r| {
            let (rating, tags) = st.meta.get(&r.key()).cloned().unwrap_or((0, vec![]));
            BItem { name: r.name.clone(), cat: r.cat.clone(), fav: st.favs.contains(&r.key()), user: r.is_user(), rating, tags }
        })
        .collect();
    let sel = st.presets.iter().position(|r| r.name == cur_name);
    let act = browse::browser(ui, area, "PRESETS", &items, sel, &mut st.pb, true, "presets");
    // footer: save, delete, open folder
    let f = browse::footer(area);
    let y = f.top() + 4.0;
    ui.painter().text(Pos2::new(f.left(), y + 12.0), Align2::LEFT_CENTER, "SAVE AS", mono(10.0), DIM);
    let nr = Rect::from_min_size(Pos2::new(f.left() + 62.0, y), Vec2::new(250.0, 24.0));
    let te = egui::TextEdit::singleline(&mut st.save_name).font(mono(12.0)).hint_text("preset name").desired_width(nr.width()).margin(egui::Margin::symmetric(6, 4));
    ui.put(nr, te);
    ui.painter().text(Pos2::new(nr.right() + 14.0, y + 12.0), Align2::LEFT_CENTER, "CATEGORY", mono(10.0), DIM);
    let cr = Rect::from_min_size(Pos2::new(nr.right() + 84.0, y), Vec2::new(170.0, 24.0));
    let te = egui::TextEdit::singleline(&mut st.save_cat).font(mono(12.0)).hint_text("Bass, Lead, Pad...").desired_width(cr.width()).margin(egui::Margin::symmetric(6, 4));
    ui.put(cr, te);
    if pill_tip(ui, Rect::from_min_size(Pos2::new(cr.right() + 10.0, y), Vec2::new(70.0, 24.0)), "SAVE", true, "Write the preset file into its category folder") {
        match presets::save_user(p, &st.save_name, &st.save_cat) {
            Ok(path) => {
                if let Ok(mut g) = p.preset_name.lock() {
                    *g = files::clean_name(&st.save_name);
                }
                st.rescan();
                st.say(now, format!("saved {}", path.file_name().and_then(|x| x.to_str()).unwrap_or("")));
            }
            Err(e) => st.say(now, format!("not saved: {e}")),
        }
    }
    let selr = sel.and_then(|i| st.presets.get(i)).cloned();
    if let Some(r) = selr.as_ref().filter(|r| r.is_user()) {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(cr.right() + 88.0, y), Vec2::new(64.0, 24.0)), "DELETE", false, "Delete the selected user preset file") {
            presets::delete_user(r);
            st.rescan();
            st.say(now, format!("deleted {}", r.name));
        }
    }
    if let Some(r) = selr.as_ref().filter(|r| !r.is_user()) {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(cr.right() + 88.0, y), Vec2::new(64.0, 24.0)), "HIDE", false, "Factory presets are built into the plugin and cannot be deleted from disk. HIDE removes this one from the browser (RESTORE brings every hidden preset back).") {
            let mut h = presets::load_hidden();
            h.push(r.key());
            presets::save_hidden(&h);
            st.hidden_n = h.len();
            st.rescan();
            st.say(now, format!("hidden {}", r.name));
        }
    }
    let nhid = st.hidden_n;
    if nhid > 0 && pill_tip(ui, Rect::from_min_size(Pos2::new(cr.right() + 158.0, y), Vec2::new(104.0, 24.0)), &format!("RESTORE {nhid}"), false, "Show all hidden factory presets again") {
        presets::save_hidden(&[]);
        st.hidden_n = 0;
        st.rescan();
        st.say(now, format!("{nhid} presets restored"));
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 340.0, y), Vec2::new(122.0, 24.0)), "EXPORT PACK", false, "Bundle all your own presets into one .nypack file (Packs folder) that you can share") {
        match presets::export_pack("NyashkSynth User Pack", None) {
            Ok((path, n)) => st.say(now, format!("{n} presets exported to {}", path.display())),
            Err(e) => st.say(now, e),
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 470.0, y), Vec2::new(124.0, 24.0)), "IMPORT PACKS", false, "Unpack every .nypack from the Packs folder into your presets (nothing is overwritten)") {
        match presets::import_packs() {
            Ok(n) => {
                st.presets = presets::list_all();
                st.say(now, format!("{n} presets imported"));
            }
            Err(e) => st.say(now, e),
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 210.0, y), Vec2::new(116.0, 24.0)), "OPEN FOLDER", false, "Open the preset folder in your file manager. Drop .nyp files (or whole category folders) in there.") {
        if let Some(d) = files::presets_dir() {
            files::open_folder(&d);
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 88.0, y), Vec2::new(88.0, 24.0)), "RESCAN", false, "Look for new files now") {
        st.rescan();
        st.say(now, "folders rescanned");
    }
    ui.painter().text(Pos2::new(f.left(), y + 40.0), Align2::LEFT_CENTER, "Click: load. Up / Down: step. Stars: rating (left), favorite (right).", mono(9.5), DIM);
    if let Some(r) = selr.as_ref() {
        let key = r.key();
        if st.tag_for != key {
            st.tag_for = key.clone();
            st.tag_edit = st.meta.get(&key).map(|m| m.1.join(", ")).unwrap_or_default();
        }
        ui.painter().text(Pos2::new(f.left() + 560.0, y + 40.0), Align2::LEFT_CENTER, "TAGS", mono(10.0), DIM);
        let tr = Rect::from_min_size(Pos2::new(f.left() + 600.0, y + 28.0), Vec2::new(300.0, 24.0));
        let te = egui::TextEdit::singleline(&mut st.tag_edit).font(mono(12.0)).hint_text("bass, dark, wobble").desired_width(tr.width()).margin(egui::Margin::symmetric(6, 4));
        ui.put(tr, te);
        if pill_tip(ui, Rect::from_min_size(Pos2::new(tr.right() + 8.0, y + 28.0), Vec2::new(64.0, 24.0)), "SET", false, "Store these comma separated tags for the selected preset") {
            let tags: Vec<String> = st.tag_edit.split(',').map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
            let e = st.meta.entry(key).or_insert((0, vec![]));
            e.1 = tags;
            presets::save_meta(&st.meta);
            st.say(now, "tags saved");
        }
    }
    match act {
        BAct::Pick(i) => {
            if let Some(r) = st.presets.get(i).cloned() {
                if let Err(e) = presets::load(setter, p, shared, &r) {
                    st.say(now, e);
                }
                st.save_name = r.name.clone();
                st.save_cat = r.cat.clone();
            }
        }
        BAct::Fav(i) => {
            if let Some(r) = st.presets.get(i) {
                let k = r.key();
                if let Some(pos) = st.favs.iter().position(|x| *x == k) {
                    st.favs.remove(pos);
                } else {
                    st.favs.push(k);
                }
                presets::save_favs(&st.favs);
            }
        }
        BAct::Rate(i, v) => {
            if let Some(r) = st.presets.get(i) {
                let k = r.key();
                let e = st.meta.entry(k).or_insert((0, vec![]));
                e.0 = v;
                presets::save_meta(&st.meta);
            }
        }
        BAct::Close => st.overlay = Overlay::None,
        BAct::None => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn table_overlay(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, area: Rect, oi: usize, now: f64) {
    let items: Vec<BItem> = st.tables.iter().map(|(b, _)| BItem { name: b.name.clone(), cat: b.cat.clone(), fav: false, user: b.user, rating: 0, tags: vec![] }).collect();
    let cur = wave_name(p, oi);
    let sel = st.tables.iter().position(|(b, _)| b.name == cur);
    let act = browse::browser(ui, area, &format!("WAVETABLES  -  OSC {}", OSC_NAMES[oi.min(2)]), &items, sel, &mut st.tbs, false, "tables");
    let f = browse::footer(area);
    let y = f.top() + 4.0;
    ui.painter().text(Pos2::new(f.left(), y + 12.0), Align2::LEFT_CENTER, "IMPORT FRAME SIZE", mono(10.0), DIM);
    for (i, (label, v)) in [("AUTO", 0usize), ("256", 256), ("512", 512), ("1024", 1024), ("2048", 2048)].iter().enumerate() {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(f.left() + 142.0 + 62.0 * i as f32, y), Vec2::new(58.0, 24.0)), label, st.frame_size == *v, "How .wav files are cut into frames. AUTO reads the file header, or uses 2048 samples.") {
            st.frame_size = *v;
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 210.0, y), Vec2::new(116.0, 24.0)), "OPEN FOLDER", false, "Open the wavetable folder. Drop .wav files in there.") {
        if let Some(d) = files::tables_dir() {
            files::open_folder(&d);
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 88.0, y), Vec2::new(88.0, 24.0)), "RESCAN", false, "Look for new files now") {
        st.rescan();
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.left() + 470.0, y), Vec2::new(90.0, 24.0)), "EDIT", false, "Edit the loaded wavetable") {
        st.open_edit(shared, p, oi);
    }
    ui.painter().text(Pos2::new(f.left(), y + 40.0), Align2::LEFT_CENTER, "Click a wavetable to load it. Serum wavetables (.wav, 2048 samples per frame) work as they are.", mono(9.5), DIM);
    match act {
        BAct::Pick(i) => {
            if let Some((_, src)) = st.tables.get(i) {
                match load_table(setter, p, shared, oi, src, st.frame_size) {
                    Ok(n) => st.say(now, format!("loaded {n}")),
                    Err(e) => st.say(now, format!("could not load: {e}")),
                }
            }
        }
        BAct::Close => st.overlay = Overlay::None,
        _ => {}
    }
}

fn noise_overlay(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, area: Rect, now: f64) {
    let want_sfz = st.smp_target == Some(98);
    let list: Vec<files::Entry> = st.noises.iter().filter(|e| e.path.extension().and_then(|x| x.to_str()).map_or(false, |x| x.eq_ignore_ascii_case("sfz")) == want_sfz).cloned().collect();
    let items: Vec<BItem> = list.iter().map(|e| BItem { name: e.name.clone(), cat: e.cat.clone(), fav: false, user: true, rating: 0, tags: vec![] }).collect();
    let cur = p.noise_sample.lock().map(|g| crate::sample::encoded_name(&g)).unwrap_or_default();
    let sel = list.iter().position(|e| e.name == cur);
    let act = browse::browser(ui, area, if want_sfz { "MULTISAMPLES (.sfz)" } else { "NOISE / SAMPLES" }, &items, sel, &mut st.nbs, false, "noises");
    let f = browse::footer(area);
    let y = f.top() + 4.0;
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 210.0, y), Vec2::new(116.0, 24.0)), "OPEN FOLDER", false, "Open the noise folder. Drop .wav files in there.") {
        if let Some(d) = files::noises_dir() {
            files::open_folder(&d);
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 88.0, y), Vec2::new(88.0, 24.0)), "RESCAN", false, "Look for new files now") {
        st.rescan();
    }
    ui.painter().text(Pos2::new(f.left(), y + 12.0), Align2::LEFT_CENTER, if st.smp_target == Some(98) { "SFZ instruments: put the .sfz and its .wav files in this folder (sub-folders are fine)." } else { "Any .wav file works (first 4 seconds are used). Pitch / key / loop are set in the NOISE section." }, mono(9.5), DIM);
    match act {
        BAct::Pick(i) => {
            if let Some(e) = list.get(i).cloned() {
                let res = match st.smp_target {
                    Some(98) => load_multi(setter, p, shared, st.multi_target, &e.path),
                    Some(99) => load_ir(p, shared, &e.path),
                    Some(o) => load_osc_sample(setter, p, shared, o, &e.path),
                    None => load_noise(setter, p, shared, &e.path),
                };
                match res {
                    Ok(n) => st.say(now, format!("loaded {n}")),
                    Err(e) => st.say(now, format!("could not load: {e}")),
                }
            }
        }
        BAct::Close => {
            st.overlay = Overlay::None;
            st.smp_target = None;
        }
        _ => {}
    }
}
// ===================================================================== widgets

pub fn panel(ui: &Ui, r: Rect, num: &str, title: &str) {
    ui.painter().rect_filled(r, CornerRadius::same(6), PANEL);
    ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    section_title(ui, Pos2::new(r.left() + 12.0, r.top() + 9.0), num, title);
}

pub fn cell(x: f32, y: f32, w: f32) -> Rect {
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 64.0))
}

/// Thin horizontal slider (bipolar fills from the centre).
pub fn h_slider(ui: &mut Ui, setter: &ParamSetter, p: &FloatParam, r: Rect, id: &str, bipolar: bool, tip: &str) {
    let resp = ui.interact(r, egui::Id::new(id), Sense::click_and_drag()).on_hover_text(tip);
    let norm = p.unmodulated_normalized_value();
    let learning = crate::ui::learn_widget(ui, p.as_ptr(), &resp, r);
    if learning {
        // MIDI learn mode: a click picks the parameter instead of changing it
    } else if resp.drag_started() {
        setter.begin_set_parameter(p);
    }
    if !learning && (resp.dragged() || resp.clicked()) {
        if let Some(pos) = resp.interact_pointer_pos() {
            let v = ((pos.x - r.left()) / r.width()).clamp(0.0, 1.0);
            let v = if (v - 0.5).abs() < 0.015 && bipolar { 0.5 } else { v };
            if resp.dragged() {
                setter.set_parameter_normalized(p, v);
            } else {
                set_norm(setter, p, v);
            }
        }
    }
    if !learning && resp.drag_stopped() {
        setter.end_set_parameter(p);
    }
    if !learning && resp.double_clicked() {
        set_norm(setter, p, p.default_normalized_value());
    }
    let _ = norm;
    let norm = p.unmodulated_normalized_value();
    let pt = ui.painter();
    let ty = r.center().y;
    pt.rect_filled(Rect::from_center_size(r.center(), Vec2::new(r.width(), 4.0)), CornerRadius::same(2), LINE);
    let from: f32 = if bipolar { 0.5 } else { 0.0 };
    let (a, b) = (from.min(norm), from.max(norm));
    pt.rect_filled(Rect::from_min_max(Pos2::new(r.left() + r.width() * a, ty - 2.0), Pos2::new(r.left() + r.width() * b, ty + 2.0)), CornerRadius::same(2), WHITE);
    if bipolar {
        pt.line_segment([Pos2::new(r.center().x, ty - 6.0), Pos2::new(r.center().x, ty + 6.0)], Stroke::new(1.0_f32, DIM));
    }
    pt.circle_filled(Pos2::new(r.left() + r.width() * norm, ty), 5.0, WHITE);
}

pub fn on_pill(ui: &mut Ui, setter: &ParamSetter, p: &BoolParam, r: Rect, tip: &str) {
    if pill_tip(ui, r, if p.value() { "ON" } else { "OFF" }, p.value(), tip) {
        set_b(setter, p, !p.value());
    }
}

/// Fill the area under a polyline with a mesh (egui cannot fill concave polygons).
pub fn fill_under(painter: &egui::Painter, pts: &[Pos2], bottom: f32, color: Color32) {
    let mut mesh = egui::epaint::Mesh::default();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let i = mesh.vertices.len() as u32;
        let v = |p: Pos2| egui::epaint::Vertex { pos: p, uv: egui::epaint::WHITE_UV, color };
        mesh.vertices.push(v(a));
        mesh.vertices.push(v(b));
        mesh.vertices.push(v(Pos2::new(b.x, bottom)));
        mesh.vertices.push(v(Pos2::new(a.x, bottom)));
        mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    }
    painter.add(egui::Shape::mesh(mesh));
}

pub fn dark_box(ui: &Ui, r: Rect) {
    ui.painter().rect_filled(r, CornerRadius::same(4), Color32::from_rgb(5, 5, 5));
    ui.painter().rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
}

// ===================================================================== oscillator panel

fn osc_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, r: Rect, o: usize) {
    let op = osc(p, o);
    let name = OSC_NAMES[o];
    panel(ui, r, ["01", "02", "03"][o], &format!("OSC {name}"));
    on_pill(ui, setter, &op.on, Rect::from_min_size(Pos2::new(r.right() - 56.0, r.top() + 6.0), Vec2::new(44.0, 22.0)), "Oscillator on/off");
    if pill_tip(ui, Rect::from_min_size(Pos2::new(r.left() + 92.0, r.top() + 6.0), Vec2::new(18.0, 22.0)), "C", false, "COPY: every setting of this oscillator (wavetable / sample included)") {
        let vals: Vec<f32> = op.param_map().iter().map(|(_, ptr, _)| unsafe { ptr.unmodulated_normalized_value() }).collect();
        let w = op.wave.lock().map(|g| g.clone()).unwrap_or_default();
        let sm = op.smp.lock().map(|g| g.clone()).unwrap_or_default();
        let ms = op.msmp.lock().map(|g| g.clone()).unwrap_or_default();
        st.osc_clip = Some((vals, w, format!("{sm}\u{1}{ms}")));
    }
    if st.osc_clip.is_some() && pill_tip(ui, Rect::from_min_size(Pos2::new(r.left() + 112.0, r.top() + 6.0), Vec2::new(18.0, 22.0)), "P", false, "PASTE the copied oscillator settings here") {
        paste_osc(setter, p, shared, st, o);
    }
    let sm = matches!(op.mode.value(), OscMode::Sample | OscMode::Granular | OscMode::Multisample);
    enum_combo(ui, setter, &op.mode, &format!("oscmode{o}"), Rect::from_min_size(Pos2::new(r.right() - 214.0, r.top() + 6.0), Vec2::new(154.0, 22.0)));
    let disp = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 34.0), Vec2::new(196.0, 100.0));
    if op.mode.value() == OscMode::Multisample {
        multi_panel(ui, setter, p, shared, st, r, o, disp);
        return osc_knobs(ui, setter, p, r, o, true);
    }
    if sm {
        sample_panel(ui, setter, p, shared, st, r, o, disp);
        return osc_knobs(ui, setter, p, r, o, true);
    }
    // wavetable display (click = editor, drag = scan)
    let tbl = cur_table(shared, p, o);
    let wname = wave_name(p, o);
    if wave_display(ui, setter, disp, op, &tbl, &wname, &format!("wavedisp{o}")) {
        st.open_edit(shared, p, o);
    }
    // table selector
    let rx = r.left() + 218.0;
    let ty = r.top() + 34.0;
    let list_len = st.tables.len().max(1);
    let cur_i = st.tables.iter().position(|(b, _)| b.name == wname);
    let mut step: Option<usize> = None;
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rx, ty), Vec2::new(22.0, 24.0)), "<", false, "Previous wavetable") {
        step = Some(cur_i.map(|i| (i + list_len - 1) % list_len).unwrap_or(0));
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rx + 24.0, ty), Vec2::new(74.0, 24.0)), "BROWSE", false, "Open the wavetable browser (factory tables + your .wav files)") {
        st.rescan();
        st.overlay = Overlay::Tables(o);
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rx + 100.0, ty), Vec2::new(22.0, 24.0)), ">", false, "Next wavetable") {
        step = Some(cur_i.map(|i| (i + 1) % list_len).unwrap_or(0));
    }
    if let Some(i) = step {
        if let Some((_, src)) = st.tables.get(i) {
            if let Err(e) = load_table(setter, p, shared, o, src, st.frame_size) {
                let now = ui.input(|i| i.time);
                st.say(now, format!("could not load: {e}"));
            }
        }
    }
    enum_combo(ui, setter, &op.warp_mode, &format!("warpm{o}"), Rect::from_min_size(Pos2::new(rx, ty + 28.0), Vec2::new(122.0, 24.0)));
    enum_combo(ui, setter, &op.warp2_mode, &format!("warpm2{o}"), Rect::from_min_size(Pos2::new(rx, ty + 56.0), Vec2::new(122.0, 24.0)));
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rx, ty + 84.0), Vec2::new(122.0, 20.0)), "EDIT WAVE", false, "Open the wavetable editor (also: click the wave display)") {
        st.open_edit(shared, p, o);
    }
    osc_knobs(ui, setter, p, r, o, false);
}

fn osc_knobs(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, r: Rect, o: usize, sm: bool) {
    let op = osc(p, o);
    let (pos_d, wrp_d, lvl_d, pan_d, bl_d, pit_d) = match o { 0 => (ModDst::APos, ModDst::AWarp, ModDst::ALevel, ModDst::APan, ModDst::ABlend, ModDst::APitch), 1 => (ModDst::BPos, ModDst::BWarp, ModDst::BLevel, ModDst::BPan, ModDst::BBlend, ModDst::BPitch), _ => (ModDst::CPos, ModDst::CWarp, ModDst::CLevel, ModDst::CPan, ModDst::CBlend, ModDst::CPitch) };
    let w = ((r.width() - 24.0) / 5.0).floor();
    let x0 = r.left() + 12.0 + ((r.width() - 24.0) - w * 5.0) * 0.5;
    let y0 = r.top() + 142.0;
    let dy = 66.0;
    mdst(pos_d);
    if sm && op.mode.value() == OscMode::Granular {
        knob_s(ui, setter, &op.s_start, "POS", cell(x0, y0, w), "Where in the sample the grains are taken from (modulatable: scan through it)", false);
        mdst(wrp_d);
        knob_s(ui, setter, &op.g_size, "SIZE", cell(x0 + w, y0, w), "Grain length (modulatable)", false);
        mdst([ModDst::AWarp2, ModDst::BWarp2, ModDst::CWarp2][o]);
        knob_s(ui, setter, &op.g_dens, "DENSITY", cell(x0 + w * 2.0, y0, w), "Grains per second, up to 256 (modulatable)", false);
    } else if sm {
        knob_s(ui, setter, &op.s_start, "START", cell(x0, y0, w), "Sample start point (modulatable)", false);
        mdst(wrp_d);
        knob_s(ui, setter, &op.s_end, "END", cell(x0 + w, y0, w), "Sample end / loop end point (modulatable)", false);
        mdst([ModDst::AWarp2, ModDst::BWarp2, ModDst::CWarp2][o]);
        knob_s(ui, setter, &op.s_xfade, "XFADE", cell(x0 + w * 2.0, y0, w), "Loop crossfade length (Loop mode)", false);
    } else if op.mode.value() == OscMode::Spectral {
        knob_s(ui, setter, &op.pos, "POS", cell(x0, y0, w), "Which wavetable frame the partials are taken from (also drag on the display)", false);
        knob_s(ui, setter, &op.sp_tilt, "TILT", cell(x0 + w, y0, w), "Spectral tilt: negative = darker (fewer highs), positive = brighter", true);
        knob_s(ui, setter, &op.sp_shift, "SHIFT", cell(x0 + w * 2.0, y0, w), "Stretches the partials apart: from harmonic to bell-like / metallic", false);
    } else {
    knob_s(ui, setter, &op.pos, "POS", cell(x0, y0, w), "Wavetable position (also drag on the display)", false);
    mdst(wrp_d);
    knob_s(ui, setter, &op.warp, "WARP", cell(x0 + w, y0, w), "Warp 1 amount", false);
    mdst([ModDst::AWarp2, ModDst::BWarp2, ModDst::CWarp2][o]);
    knob_s(ui, setter, &op.warp2, "WARP 2", cell(x0 + w * 2.0, y0, w), "Warp 2 amount (applied after warp 1)", false);
    }
    mdst(lvl_d);
    knob_s(ui, setter, &op.level, "LEVEL", cell(x0 + w * 3.0, y0, w), "Oscillator level", false);
    mdst(pan_d);
    knob_s(ui, setter, &op.pan, "PAN", cell(x0 + w * 4.0, y0, w), "Stereo balance", true);
    let y1 = y0 + dy;
    knob_s(ui, setter, &op.oct, "OCT", cell(x0, y1, w), "Octave", false);
    match op.tune_mode.value() {
        OscTune::Semitone => { knob_s(ui, setter, &op.semi, "SEMI", cell(x0 + w, y1, w), "Semitones (click the label to switch to HARM / RATIO tuning)", false); }
        OscTune::Harmonic => { knob_s(ui, setter, &op.harm, "HARM", cell(x0 + w, y1, w), "Harmonic number: 1 = the played pitch, 2 = an octave up, 3 = octave + fifth ... (click the label to switch)", false); }
        OscTune::Step => { knob_s(ui, setter, &op.semi, "STEP", cell(x0 + w, y1, w), "Moves the oscillator by whole steps of the KEY LOCK scale (SETTINGS page), e.g. +2 = a diatonic third up. Click the label to switch.", false); }
        OscTune::Ratio => { knob_s(ui, setter, &op.ratio, "RATIO", cell(x0 + w, y1, w), "Frequency ratio to the played pitch, e.g. 1.5 = a fifth up (click the label to switch)", false); }
    }
    {
        let lr = Rect::from_min_size(Pos2::new(x0 + w + 8.0, y1 + 46.0), Vec2::new(w - 16.0, 16.0));
        let resp = ui.interact(lr, egui::Id::new(("tunemode", o)), Sense::click()).on_hover_text("Click: switch between SEMI (semitones), HARM (harmonic number) and RATIO (free frequency ratio)");
        if resp.hovered() {
            ui.painter().rect_stroke(lr, CornerRadius::same(3), Stroke::new(1.0_f32, MID), StrokeKind::Inside);
        }
        if resp.clicked() {
            let next = match op.tune_mode.value() {
                OscTune::Semitone => OscTune::Harmonic,
                OscTune::Harmonic => OscTune::Ratio,
                OscTune::Ratio => OscTune::Step,
                OscTune::Step => OscTune::Semitone,
            };
            set_e(setter, &op.tune_mode, next);
        }
    }
    mdst(pit_d);
    knob_s(ui, setter, &op.fine, "FINE", cell(x0 + w * 2.0, y1, w), "Fine tune in cents (modulation: +-24 semitones)", true);
    knob_s(ui, setter, &op.uni, "UNISON", cell(x0 + w * 3.0, y1, w), "Number of unison voices", false);
    mdst(ModDst::Detune);
    knob_s(ui, setter, &op.detune, "DETUNE", cell(x0 + w * 4.0, y1, w), "Unison detune", false);
    let y2 = y1 + dy;
    mdst(bl_d);
    knob_s(ui, setter, &op.blend, "BLEND", cell(x0, y2, w), "Level of the outer unison voices (low = centre voice louder)", false);
    knob_s(ui, setter, &op.spread, "SPREAD", cell(x0 + w, y2, w), "Stereo width of the unison voices", false);
    if sm {
        knob_s(ui, setter, &op.s_root, "ROOT", cell(x0 + w * 2.0, y2, w), "Semitone offset of the sample's own pitch: the sample plays at its original speed on middle C + this", true);
        if op.mode.value() == OscMode::Granular {
            knob_s(ui, setter, &op.g_spray, "SPRAY", cell(x0 + w * 3.0, y2, w), "Random spread of the grain positions around POS", false);
        }
    } else if op.mode.value() == OscMode::Spectral {
        knob_s(ui, setter, &op.sp_eo, "EVEN/ODD", cell(x0 + w * 2.0, y2, w), "Negative = fewer odd partials (hollow), positive = fewer even partials (square-like)", true);
        ui.painter().text(Pos2::new(x0 + w * 3.0 + w * 0.5, y2 + 30.0), Align2::CENTER_CENTER, "1 voice,\n64 partials", mono(8.5), DIM);
    } else {
        knob_s(ui, setter, &op.phase, "PHASE", cell(x0 + w * 2.0, y2, w), "Start phase", false);
        knob_s(ui, setter, &op.rand, "RAND", cell(x0 + w * 3.0, y2, w), "Random start phase", false);
    }
    // same rows as the value boxes (box centre y2+43) and labels (y2+57) of the knobs next to it
    enum_combo_sz(ui, setter, &op.uni_mode, &format!("unimode{o}"), Rect::from_min_size(Pos2::new(x0 + w * 4.0 + 1.0, y2 + 32.0), Vec2::new(w + 3.0, 22.0)), 10.5);
    ui.painter().text(Pos2::new(x0 + w * 4.0 + w * 0.5, y2 + 57.0), Align2::CENTER_CENTER, "UNI MODE", mono(9.5), DIM);
}

/// Multisample mode: key map of the loaded instrument, LOAD SFZ button, loop mode.
fn multi_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, r: Rect, o: usize, disp: Rect) {
    let op = osc(p, o);
    dark_box(ui, disp);
    let painter = ui.painter().with_clip_rect(disp.shrink(1.0));
    let m = shared.ms[o].get();
    let kx = |k: f32| disp.left() + 4.0 + (disp.width() - 8.0) * (k / 127.0);
    match &m {
        Some(m) => {
            for (i, rg) in m.regions.iter().enumerate() {
                let row = i % 6;
                let y = disp.top() + 24.0 + 11.0 * row as f32;
                painter.rect_filled(Rect::from_min_max(Pos2::new(kx(rg.lo as f32), y), Pos2::new(kx(rg.hi as f32 + 1.0).max(kx(rg.lo as f32) + 2.0), y + 8.0)), CornerRadius::same(1), Color32::from_rgb(70, 70, 70));
                painter.rect_filled(Rect::from_center_size(Pos2::new(kx(rg.root as f32 + 0.5), y + 4.0), Vec2::new(3.0, 8.0)), CornerRadius::ZERO, WHITE);
            }
            let nm = format!("{} ({} regions)", m.name, m.regions.len());
            let shown: String = if nm.chars().count() > 26 { nm.chars().take(25).collect::<String>() + "~" } else { nm };
            painter.text(Pos2::new(disp.left() + 5.0, disp.top() + 8.0), Align2::LEFT_CENTER, shown, mono(9.5), DIM);
        }
        None => {
            painter.text(disp.center(), Align2::CENTER_CENTER, "no instrument - LOAD SFZ", mono(10.0), DIM);
        }
    }
    let rx = r.left() + 218.0;
    let ty = r.top() + 34.0;
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rx, ty), Vec2::new(122.0, 24.0)), "LOAD SFZ", false, "Choose an .sfz instrument (put the .sfz and its .wav files in the Noises folder)") {
        st.rescan();
        st.smp_target = Some(98);
        st.multi_target = o;
        st.overlay = Overlay::Noises;
    }
    enum_combo(ui, setter, &op.s_loop, &format!("msloop{o}"), Rect::from_min_size(Pos2::new(rx, ty + 28.0), Vec2::new(122.0, 24.0)));
    // ---- mapping editor: click a bar, then edit its key range / root / velocity layer
    let Some(m) = m else {
        ui.painter().text(Pos2::new(rx + 61.0, ty + 70.0), Align2::CENTER_CENTER, "Every key plays the\nrecording of its range\n(bars = key ranges,\nwhite = root key)", mono(8.5), DIM);
        return;
    };
    let resp = ui.interact(disp, egui::Id::new(("msdisp", o)), Sense::click()).on_hover_text("Click a bar to select a region, then edit its key range, root and velocity layer on the right.");
    if resp.clicked() {
        if let Some(pt) = resp.interact_pointer_pos() {
            for (i, rg) in m.regions.iter().enumerate() {
                let y = disp.top() + 24.0 + 11.0 * (i % 6) as f32;
                let x0 = kx(rg.lo as f32);
                let x1 = kx(rg.hi as f32 + 1.0).max(x0 + 2.0);
                if pt.x >= x0 - 2.0 && pt.x <= x1 + 2.0 && pt.y >= y - 2.0 && pt.y <= y + 10.0 {
                    st.ms_sel = i;
                }
            }
        }
    }
    let sel = st.ms_sel.min(m.regions.len() - 1);
    {
        let rg = &m.regions[sel];
        let y = disp.top() + 24.0 + 11.0 * (sel % 6) as f32;
        ui.painter().with_clip_rect(disp).rect_stroke(Rect::from_min_max(Pos2::new(kx(rg.lo as f32) - 1.0, y - 1.0), Pos2::new(kx(rg.hi as f32 + 1.0).max(kx(rg.lo as f32) + 2.0) + 1.0, y + 9.0)), CornerRadius::same(1), Stroke::new(1.0_f32, WHITE), StrokeKind::Outside);
    }
    let mut vals = [m.regions[sel].lo, m.regions[sel].hi, m.regions[sel].root, m.regions[sel].lv, m.regions[sel].hv];
    let before = vals;
    let cap = |ui: &mut Ui, x: f32, y: f32, t: &str| {
        ui.painter().text(Pos2::new(x + 19.0, y), Align2::CENTER_CENTER, t, mono(8.0), DIM);
    };
    cap(ui, rx, ty + 57.0, "LO");
    cap(ui, rx + 40.0, ty + 57.0, "HI");
    cap(ui, rx + 80.0, ty + 57.0, "ROOT");
    let mut dv = |ui: &mut Ui, x: f32, y: f32, w: f32, v: &mut u8, lo: u8, hi: u8| {
        ui.put(Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, 16.0)), egui::DragValue::new(v).range(lo..=hi).speed(0.25));
    };
    dv(ui, rx, ty + 62.0, 38.0, &mut vals[0], 0, 127);
    dv(ui, rx + 40.0, ty + 62.0, 38.0, &mut vals[1], 0, 127);
    dv(ui, rx + 80.0, ty + 62.0, 38.0, &mut vals[2], 0, 127);
    cap(ui, rx, ty + 84.0, "VEL LO");
    cap(ui, rx + 40.0, ty + 84.0, "VEL HI");
    cap(ui, rx + 80.0, ty + 84.0, "REGION");
    dv(ui, rx, ty + 90.0, 38.0, &mut vals[3], 1, 127);
    dv(ui, rx + 40.0, ty + 90.0, 38.0, &mut vals[4], 1, 127);
    ui.painter().text(Pos2::new(rx + 99.0, ty + 98.0), Align2::CENTER_CENTER, format!("{}/{}", sel + 1, m.regions.len()), mono(9.0), WHITE);
    if vals != before {
        let mut regs = m.regions.clone();
        let rg = &mut regs[sel];
        rg.lo = vals[0].min(vals[1]);
        rg.hi = vals[1].max(vals[0]);
        rg.root = vals[2];
        rg.lv = vals[3].min(vals[4]);
        rg.hv = vals[4].max(vals[3]);
        shared.ms[o].set(Some(Arc::new(crate::sample::MultiS { name: m.name.clone(), regions: regs })));
        shared.version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        st.ms_dirty = Some(o);
    }
    if st.ms_dirty == Some(o) && !ui.input(|i| i.pointer.any_down()) {
        if let Some(cur) = shared.ms[o].get() {
            if let Ok(mut g) = op.msmp.lock() {
                *g = cur.encode();
            }
        }
        st.ms_dirty = None;
    }
}

/// Sample-mode left side: waveform with draggable start / end, load button, loop mode.
fn sample_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, r: Rect, o: usize, disp: Rect) {
    let op = osc(p, o);
    let data = shared.smp[o].get();
    let resp = ui.interact(disp, egui::Id::new(("smpdisp", o)), Sense::click_and_drag()).on_hover_text("Drag the markers to set the start and end of the sample.\nDrop a .wav here to load it.");
    let gran = op.mode.value() == OscMode::Granular;
    let (s0, s1) = if gran { (op.s_start.value(), op.s_start.value()) } else { (op.s_start.value(), op.s_end.value()) };
    let key = egui::Id::new(("smpdrag", o));
    if resp.drag_started() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let x = (pt.x - disp.left()) / disp.width();
            let which = if gran || (x - s0).abs() <= (x - s1).abs() { 0u8 } else { 1u8 };
            ui.data_mut(|d| d.insert_temp(key, which));
        }
    }
    if resp.dragged() {
        if let (Some(pt), Some(which)) = (resp.interact_pointer_pos(), ui.data(|d| d.get_temp::<u8>(key))) {
            let x = ((pt.x - disp.left()) / disp.width()).clamp(0.0, 1.0);
            if which == 0 {
                set_f(setter, &op.s_start, if gran { x } else { x.min(s1 - 0.005) });
            } else {
                set_f(setter, &op.s_end, x.max(s0 + 0.005));
            }
        }
    }
    dark_box(ui, disp);
    let painter = ui.painter().with_clip_rect(disp.shrink(1.0));
    painter.line_segment([Pos2::new(disp.left(), disp.center().y), Pos2::new(disp.right(), disp.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
    let xa = disp.left() + 2.0 + (disp.width() - 4.0) * s0;
    let xb = disp.left() + 2.0 + (disp.width() - 4.0) * s1;
    if gran {
        let sp = op.g_spray.value() * 0.5;
        let (ga, gb) = (disp.left() + 2.0 + (disp.width() - 4.0) * (s0 - sp).max(0.0), disp.left() + 2.0 + (disp.width() - 4.0) * (s0 + sp).min(1.0));
        painter.rect_filled(Rect::from_min_max(Pos2::new(ga, disp.top()), Pos2::new(gb, disp.bottom())), CornerRadius::ZERO, Color32::from_rgb(26, 26, 26));
    } else {
        painter.rect_filled(Rect::from_min_max(Pos2::new(xa, disp.top()), Pos2::new(xb, disp.bottom())), CornerRadius::ZERO, Color32::from_rgb(20, 20, 20));
    }
    let name = match &data {
        Some(sm) => {
            let cols = (disp.width() - 4.0) as usize;
            let n = sm.data.len();
            let amp = disp.height() * 0.42;
            for c in 0..cols {
                let a = c * n / cols;
                let b = ((c + 1) * n / cols).max(a + 1).min(n);
                let step = ((b - a) / 48).max(1);
                let (mut lo, mut hi) = (0.0f32, 0.0f32);
                let mut i = a;
                while i < b {
                    lo = lo.min(sm.data[i]);
                    hi = hi.max(sm.data[i]);
                    i += step;
                }
                let x = disp.left() + 2.0 + c as f32;
                let inside = gran || (x >= xa && x <= xb);
                painter.line_segment([Pos2::new(x, disp.center().y - hi * amp), Pos2::new(x, disp.center().y - lo * amp - 1.0)], Stroke::new(1.0_f32, if inside { WHITE } else { Color32::from_rgb(70, 70, 70) }));
            }
            sm.name.clone()
        }
        None => {
            painter.text(disp.center(), Align2::CENTER_CENTER, "no sample - press LOAD", mono(10.0), DIM);
            String::new()
        }
    };
    for x in (if gran { vec![xa] } else { vec![xa, xb] }) {
        painter.line_segment([Pos2::new(x, disp.top()), Pos2::new(x, disp.bottom())], Stroke::new(1.5_f32, WHITE));
    }
    let shown: String = if name.chars().count() > 26 { name.chars().take(25).collect::<String>() + "~" } else { name };
    painter.text(Pos2::new(disp.left() + 5.0, disp.top() + 8.0), Align2::LEFT_CENTER, shown, mono(9.5), DIM);
    let rx = r.left() + 218.0;
    let ty = r.top() + 34.0;
    if pill_tip(ui, Rect::from_min_size(Pos2::new(rx, ty), Vec2::new(122.0, 24.0)), "LOAD SAMPLE", false, "Choose a .wav file (the Noises folder is used as the sample library)") {
        st.rescan();
        st.smp_target = Some(o);
        st.overlay = Overlay::Noises;
    }
    if gran {
        ui.painter().text(Pos2::new(rx + 61.0, ty + 56.0), Align2::CENTER_CENTER, "Grains play at the\nkey pitch. Modulate\nPOS to scan the\nsample.", mono(8.5), DIM);
    } else {
        enum_combo(ui, setter, &op.s_loop, &format!("smploop{o}"), Rect::from_min_size(Pos2::new(rx, ty + 28.0), Vec2::new(122.0, 24.0)));
        ui.painter().text(Pos2::new(rx + 61.0, ty + 70.0), Align2::CENTER_CENTER, "Plays at its original\npitch on middle C", mono(8.5), DIM);
    }
}

/// Returns true when the display was clicked (open the editor).
fn wave_display(ui: &mut Ui, setter: &ParamSetter, r: Rect, op: &OscParams, tb: &Table, name: &str, id: &str) -> bool {
    let resp = ui.interact(r, egui::Id::new(id), Sense::click_and_drag()).on_hover_text("Click: open the wavetable editor.\nDrag left / right: scan through the wavetable.");
    let pos_p = &op.pos;
    if resp.drag_started() {
        setter.begin_set_parameter(pos_p);
    }
    if resp.dragged() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let v = ((pt.x - r.left()) / r.width()).clamp(0.0, 1.0);
            setter.set_parameter_normalized(pos_p, v);
        }
    }
    if resp.drag_stopped() {
        setter.end_set_parameter(pos_p);
    }
    let clicked = resp.clicked();
    dark_box(ui, r);
    let painter = ui.painter().with_clip_rect(r.shrink(1.0));
    for i in 1..4 {
        let x = r.left() + r.width() * i as f32 / 4.0;
        painter.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)));
    }
    painter.line_segment([Pos2::new(r.left(), r.center().y), Pos2::new(r.right(), r.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
    let pos = op.pos.value();
    let mode = op.warp_mode.value();
    let amt = op.warp.value();
    let mode2 = op.warp2_mode.value();
    let amt2 = op.warp2.value();
    let n = 220usize;
    let amp = r.height() * 0.40;
    let curve = |frame_pos: f32, warp: bool| -> Vec<Pos2> {
        (0..=n)
            .map(|i| {
                let ph = (i as f32 / n as f32).min(0.9999);
                let mut w = ph;
                if warp && mode != WarpMode::Off {
                    w = warp_phase(mode, amt, w, 0.0);
                }
                if warp && mode2 != WarpMode::Off {
                    w = warp_phase(mode2, amt2, w, 0.0);
                }
                let s = tb.sample(0, frame_pos, w);
                Pos2::new(r.left() + 2.0 + (r.width() - 4.0) * i as f32 / n as f32, r.center().y - s * amp)
            })
            .collect()
    };
    for d in [-0.16f32, -0.08, 0.08, 0.16] {
        let fp = pos + d;
        if (0.0..=1.0).contains(&fp) {
            painter.add(egui::Shape::line(curve(fp, false), PathStroke::new(1.0_f32, Color32::from_rgb(42, 42, 42))));
        }
    }
    painter.add(egui::Shape::line(curve(pos, true), PathStroke::new(1.6_f32, WHITE)));
    let sy = r.bottom() - 5.0;
    painter.line_segment([Pos2::new(r.left() + 4.0, sy), Pos2::new(r.right() - 4.0, sy)], Stroke::new(1.0_f32, LINE));
    let px = r.left() + 4.0 + (r.width() - 8.0) * pos;
    painter.circle_filled(Pos2::new(px, sy), 2.5, WHITE);
    let shown: String = if name.chars().count() > 26 { name.chars().take(25).collect::<String>() + "~" } else { name.to_string() };
    painter.text(Pos2::new(r.left() + 5.0, r.top() + 8.0), Align2::LEFT_CENTER, shown, mono(9.5), DIM);
    painter.text(Pos2::new(r.right() - 5.0, r.top() + 8.0), Align2::RIGHT_CENTER, format!("{} fr", tb.frames()), mono(9.0), Color32::from_rgb(90, 90, 90));
    clicked
}

// ===================================================================== filter panel

#[derive(Clone, Copy)]
struct Cx(f32, f32);
impl Cx {
    fn mul(self, o: Cx) -> Cx {
        Cx(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn add(self, o: Cx) -> Cx {
        Cx(self.0 + o.0, self.1 + o.1)
    }
    fn div(self, o: Cx) -> Cx {
        let d = (o.0 * o.0 + o.1 * o.1).max(1e-12);
        Cx((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d)
    }
    fn abs(self) -> f32 {
        (self.0 * self.0 + self.1 * self.1).sqrt()
    }
}

/// Magnitude of the custom (point) filter at `f`.
pub fn custom_response(f: &FilterParams, fr: f32) -> f32 {
    let mut m = 1.0;
    for b in f.cb.iter().filter(|b| b.on.value()) {
        let cp = crate::engine::CbP { on: true, kind: b.kind.value(), freq: b.freq.value(), gain: b.gain.value(), q: b.q.value() };
        m *= crate::engine::cband_coef(&cp, 1.0, 48000.0).mag(fr, 48000.0);
    }
    m
}

pub fn filter_response(ty: FilterType, res: f32, fc: f32, f: f32) -> f32 {
    let fs = 48000.0f32;
    let f_norm = f / fc;
    let k1 = 0.05 + 1.95 * (1.0 - res).powi(2);
    let k2 = k1.max(0.9);
    let w2 = f_norm * f_norm;
    let stage = |k: f32, kind: u8| -> f32 {
        let den = ((1.0 - w2).powi(2) + (k * f_norm).powi(2)).sqrt().max(1e-6);
        match kind {
            0 => 1.0 / den,
            1 => w2 / den,
            2 => k * f_norm / den,
            _ => (1.0 - w2).abs() / den,
        }
    };
    use crate::fx::BqCoef;
    match ty {
        FilterType::Custom => 1.0,
        FilterType::Lp12 => stage(k1, 0),
        FilterType::Lp24 => stage(k1, 0) * stage(k2, 0),
        FilterType::Hp12 => stage(k1, 1),
        FilterType::Hp24 => stage(k1, 1) * stage(k2, 1),
        FilterType::Band => stage(k1, 2),
        FilterType::Notch => stage(k1, 3),
        FilterType::Lp6 => 1.0 / (1.0 + w2).sqrt(),
        FilterType::Hp6 => f_norm / (1.0 + w2).sqrt(),
        FilterType::Ladder => {
            let s = Cx(1.0, f_norm);
            let s2 = s.mul(s);
            let s4 = s2.mul(s2);
            let den = Cx(s4.0 + res * 3.95, s4.1);
            (Cx(1.0, 0.0).div(den).abs() * (1.0 + res * 3.95 * 0.3)).max(0.0)
        }
        FilterType::Peak => BqCoef::peaking(fc, 1.0, res * 18.0, fs).mag(f, fs),
        FilterType::LowShelf => BqCoef::low_shelf(fc, res * 18.0, fs).mag(f, fs),
        FilterType::HighShelf => BqCoef::high_shelf(fc, res * 18.0, fs).mag(f, fs),
        FilterType::CombPlus | FilterType::CombMinus => {
            let g = res.min(0.97) * 0.97 * if ty == FilterType::CombPlus { 1.0 } else { -1.0 };
            let c = (std::f32::consts::TAU * f / fc).cos();
            1.0 / (1.0 + g * g - 2.0 * g * c).max(1e-4).sqrt() / (1.0 + g.abs() * 0.6)
        }
        FilterType::PhaserF => {
            let w = std::f32::consts::TAU * f / fs;
            let z1 = Cx(w.cos(), -w.sin());
            let mut prod = Cx(1.0, 0.0);
            for i in 0..6 {
                let fi = (fc * 2f32.powf((i as f32 - 2.5) * 0.3)).clamp(20.0, 0.45 * fs);
                let t = (std::f32::consts::PI * fi / fs).tan();
                let a = (t - 1.0) / (t + 1.0);
                let num = Cx(a, 0.0).add(z1);
                let den = Cx(1.0, 0.0).add(Cx(a * z1.0, a * z1.1));
                prod = prod.mul(num.div(den));
            }
            Cx(0.5 * (1.0 + prod.0), 0.5 * prod.1).abs()
        }
        FilterType::Formant => {
            const V: [[f32; 3]; 5] = [[800.0, 1200.0, 2500.0], [400.0, 2000.0, 2600.0], [300.0, 2300.0, 3000.0], [500.0, 900.0, 2500.0], [350.0, 600.0, 2400.0]];
            let t = ((fc / 20.0).ln() / 1000f32.ln()).clamp(0.0, 0.9999) * 4.0;
            let i = t as usize;
            let fr = t - i as f32;
            let kq = 0.06 + (1.0 - res) * 0.5;
            let w = [1.0, 0.6, 0.35];
            let mut s = 0.0;
            for j in 0..3 {
                let fj = V[i][j] + (V[i + 1][j] - V[i][j]) * fr;
                let x = f / fj;
                s += w[j] * 3.0 * kq * x / ((1.0 - x * x).powi(2) + (kq * x).powi(2)).sqrt().max(1e-4);
            }
            s
        }
    }
}

/// Draw and edit the points of the custom filter on the graph.
fn custom_points(ui: &mut Ui, setter: &ParamSetter, fp: &FilterParams, fi: usize, d: Rect, xf: &dyn Fn(f32) -> f32, yd: &dyn Fn(f32) -> f32, db: (f32, f32)) {
    let sel_id = egui::Id::new(("cb_sel", fi));
    let drag_id = egui::Id::new(("cb_drag", fi));
    let mut sel = ui.data(|dd| dd.get_temp::<usize>(sel_id)).unwrap_or(0).min(CBANDS - 1);
    let has_gain = |k: BandKind| matches!(k, BandKind::Bell | BandKind::LowShelf | BandKind::HighShelf);
    let dot = |i: usize| -> Pos2 {
        let b = &fp.cb[i];
        let f = b.freq.value();
        let y = if has_gain(b.kind.value()) { yd(b.gain.value()) } else { yd(20.0 * custom_response(fp, f).max(1e-6).log10()) };
        Pos2::new(xf(f), y)
    };
    let hit = |pt: Pos2| -> Option<usize> {
        (0..CBANDS).filter(|&i| fp.cb[i].on.value()).map(|i| (i, dot(i).distance(pt))).filter(|&(_, dist)| dist < 12.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0)
    };
    let gr = ui.interact(d, egui::Id::new(("filter_graph_c", fi)), Sense::click_and_drag()).on_hover_text("Click: add a point | drag: frequency + gain | wheel: width | double-click / right-click: remove");
    let hover = gr.hover_pos();
    // wheel over a point = width
    if let Some(i) = hover.and_then(&hit) {
        let sc = ui.input(|i| i.smooth_scroll_delta.y);
        if sc != 0.0 {
            let q = (fp.cb[i].q.value() * (sc * 0.004).exp()).clamp(0.15, 16.0);
            setter.begin_set_parameter(&fp.cb[i].q);
            setter.set_parameter(&fp.cb[i].q, q);
            setter.end_set_parameter(&fp.cb[i].q);
            sel = i;
        }
    }
    if gr.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(i) = origin.and_then(&hit) {
            sel = i;
            ui.data_mut(|dd| dd.insert_temp(drag_id, Some(i)));
            let b = &fp.cb[i];
            setter.begin_set_parameter(&b.freq);
            setter.begin_set_parameter(&b.gain);
            setter.begin_set_parameter(&b.q);
        } else {
            ui.data_mut(|dd| dd.insert_temp(drag_id, None::<usize>));
        }
    }
    if gr.dragged() {
        if let (Some(Some(i)), Some(pt)) = (ui.data(|dd| dd.get_temp::<Option<usize>>(drag_id)), gr.interact_pointer_pos()) {
            let b = &fp.cb[i];
            let fx = ((pt.x - d.left()) / d.width()).clamp(0.0, 1.0);
            setter.set_parameter(&b.freq, 20.0 * (1000.0f32).powf(fx));
            if has_gain(b.kind.value()) {
                let g = db.0 + (db.1 - db.0) * ((d.bottom() - pt.y) / d.height()).clamp(0.0, 1.0);
                setter.set_parameter(&b.gain, g.clamp(-36.0, 36.0));
            } else {
                let dy = gr.drag_delta().y;
                setter.set_parameter(&b.q, (b.q.value() * (-dy * 0.02).exp()).clamp(0.15, 16.0));
            }
        }
    }
    if gr.drag_stopped() {
        if let Some(Some(i)) = ui.data(|dd| dd.get_temp::<Option<usize>>(drag_id)) {
            let b = &fp.cb[i];
            setter.end_set_parameter(&b.freq);
            setter.end_set_parameter(&b.gain);
            setter.end_set_parameter(&b.q);
        }
        ui.data_mut(|dd| dd.insert_temp(drag_id, None::<usize>));
    }
    let remove = gr.double_clicked() || gr.secondary_clicked();
    if remove {
        if let Some(i) = gr.interact_pointer_pos().and_then(&hit) {
            set_b(setter, &fp.cb[i].on, false);
        }
    } else if gr.clicked() {
        if let Some(pt) = gr.interact_pointer_pos() {
            if let Some(i) = hit(pt) {
                sel = i;
            } else if let Some(i) = (0..CBANDS).find(|&i| !fp.cb[i].on.value()) {
                let b = &fp.cb[i];
                let fx = ((pt.x - d.left()) / d.width()).clamp(0.0, 1.0);
                let g = db.0 + (db.1 - db.0) * ((d.bottom() - pt.y) / d.height()).clamp(0.0, 1.0);
                setter.begin_set_parameter(&b.freq);
                setter.set_parameter(&b.freq, 20.0 * (1000.0f32).powf(fx));
                setter.end_set_parameter(&b.freq);
                setter.begin_set_parameter(&b.gain);
                setter.set_parameter(&b.gain, g.clamp(-36.0, 36.0));
                setter.end_set_parameter(&b.gain);
                setter.begin_set_parameter(&b.q);
                setter.set_parameter(&b.q, 1.0);
                setter.end_set_parameter(&b.q);
                setter.begin_set_parameter(&b.kind);
                setter.set_parameter(&b.kind, BandKind::Bell);
                setter.end_set_parameter(&b.kind);
                set_b(setter, &b.on, true);
                sel = i;
            }
        }
    }
    ui.data_mut(|dd| dd.insert_temp(sel_id, sel));
    // drawing
    let painter = ui.painter().with_clip_rect(d.expand(6.0));
    let mut any = false;
    for i in 0..CBANDS {
        if !fp.cb[i].on.value() {
            continue;
        }
        any = true;
        let c = dot(i);
        let is_sel = i == sel;
        painter.line_segment([Pos2::new(c.x, d.top()), Pos2::new(c.x, d.bottom())], Stroke::new(1.0_f32, if is_sel { Color32::from_rgb(60, 60, 60) } else { Color32::from_rgb(30, 30, 30) }));
        if is_sel {
            painter.circle_filled(c, 6.0, WHITE);
        } else {
            painter.circle_filled(c, 5.0, Color32::BLACK);
            painter.circle_stroke(c, 5.0, Stroke::new(1.5_f32, WHITE));
        }
        painter.text(c + Vec2::new(0.0, -11.0), Align2::CENTER_CENTER, format!("{}", i + 1), mono(8.5), if is_sel { WHITE } else { DIM });
    }
    if !any {
        painter.text(d.center(), Align2::CENTER_CENTER, "CLICK HERE TO ADD POINTS", mono(10.0), DIM);
    } else {
        // type of the selected point
        let b = &fp.cb[sel];
        if b.on.value() {
            enum_combo(ui, setter, &b.kind, &format!("cb_kind{fi}"), Rect::from_min_size(Pos2::new(d.right() - 98.0, d.top() + 4.0), Vec2::new(94.0, 20.0)));
            if pill_tip(ui, Rect::from_min_size(Pos2::new(d.right() - 98.0, d.top() + 26.0), Vec2::new(94.0, 18.0)), "REMOVE", false, "Remove the selected point") {
                set_b(setter, &b.on, false);
            }
        }
    }
}

/// Height of the cutoff node in the filter graph: it sits on the curve when the filter has a resonance peak,
/// otherwise its height is simply the resonance amount.
fn node_y(ty: FilterType, res: f32, cut: f32, d: Rect) -> f32 {
    let db = |r: f32| 20.0 * filter_response(ty, r, cut, cut).max(1e-6).log10();
    let (dlo, dhi) = (-48.0f32, 30.0f32);
    if (db(1.0) - db(0.0)).abs() < 1.0 {
        return d.bottom() - 6.0 - (d.height() - 12.0) * res;
    }
    d.bottom() - d.height() * ((db(res).clamp(dlo, dhi) - dlo) / (dhi - dlo))
}

/// Inverse of `node_y`: resonance for a pointer height.
fn node_res(ty: FilterType, cut: f32, y: f32, d: Rect) -> f32 {
    let db = |r: f32| 20.0 * filter_response(ty, r, cut, cut).max(1e-6).log10();
    let (dlo, dhi) = (-48.0f32, 30.0f32);
    let (d0, d1) = (db(0.0), db(1.0));
    if (d1 - d0).abs() < 1.0 {
        return ((d.bottom() - 6.0 - y) / (d.height() - 12.0)).clamp(0.0, 1.0);
    }
    let target = dlo + (dhi - dlo) * ((d.bottom() - y) / d.height()).clamp(0.0, 1.0);
    // bisection (the peak grows with the resonance)
    let (mut a, mut b) = (0.0f32, 1.0f32);
    let up = d1 > d0;
    for _ in 0..14 {
        let m = 0.5 * (a + b);
        if (db(m) < target) == up { a = m } else { b = m }
    }
    0.5 * (a + b)
}

fn filter_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, r: Rect) {
    let fi = st.filt_sel.min(1);
    let fl = if fi == 0 { &p.f1 } else { &p.f2 };
    panel(ui, r, "04", "FILTER");
    for k in 0..2 {
        let on = if k == 0 { p.f1.fl_on.value() } else { p.f2.fl_on.value() };
        let lbl = format!("{}{}", k + 1, if on { "" } else { "\u{b7}" });
        if pill_tip(ui, Rect::from_min_size(Pos2::new(r.left() + 98.0 + 30.0 * k as f32, r.top() + 5.0), Vec2::new(28.0, 24.0)), &lbl, fi == k, "Edit filter 1 / filter 2") {
            st.filt_sel = k;
        }
    }
    on_pill(ui, setter, &fl.fl_on, Rect::from_min_size(Pos2::new(r.right() - 52.0, r.top() + 6.0), Vec2::new(40.0, 22.0)), "Filter on/off");
    if pill_tip(ui, Rect::from_min_size(Pos2::new(r.right() - 98.0, r.top() + 6.0), Vec2::new(42.0, 22.0)), "SER", p.fl_serial.value(), "Series: filter 1 feeds filter 2 (off = two parallel filters, sources are routed with the ROUTE buttons)") {
        set_b(setter, &p.fl_serial, !p.fl_serial.value());
    }
    enum_combo(ui, setter, &fl.fl_type, &format!("filter_type{fi}"), Rect::from_min_size(Pos2::new(r.left() + 162.0, r.top() + 5.0), Vec2::new(r.width() - 162.0 - 104.0, 24.0)));
    let d = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 36.0), Vec2::new(r.width() - 24.0, 150.0));
    let is_custom = fl.fl_type.value() == FilterType::Custom;
    // drag in the graph: left / right = cutoff, up / down = resonance
    if !is_custom {
        let gr = ui.interact(d, egui::Id::new(("filter_graph", fi)), Sense::click_and_drag()).on_hover_text("Drag: left / right = cutoff, up / down = resonance");
        let offk = egui::Id::new(("fnode_off", fi));
        if gr.drag_started() {
            setter.begin_set_parameter(&fl.cutoff);
            setter.begin_set_parameter(&fl.res);
            let (cutv, resv, tyv) = (fl.cutoff.value(), fl.res.unmodulated_normalized_value(), fl.fl_type.value());
            let nx = d.left() + d.width() * ((cutv / 20.0).ln() / (20000.0f32 / 20.0).ln());
            let node = Pos2::new(nx.clamp(d.left() + 7.0, d.right() - 7.0), node_y(tyv, resv, cutv, d));
            let off = gr.interact_pointer_pos().map_or(Vec2::ZERO, |pp| if (pp - node).length() < 16.0 { pp - node } else { Vec2::ZERO });
            ui.data_mut(|dd| dd.insert_temp(offk, off));
        }
        if gr.dragged() {
            if let Some(pt) = gr.interact_pointer_pos() {
                let pt = pt - ui.data(|dd| dd.get_temp::<Vec2>(offk)).unwrap_or(Vec2::ZERO);
                let fx = ((pt.x - d.left()) / d.width()).clamp(0.0, 1.0);
                let hz = 20.0 * (20000.0f32 / 20.0).powf(fx);
                setter.set_parameter(&fl.cutoff, hz);
                setter.set_parameter_normalized(&fl.res, node_res(fl.fl_type.value(), hz, pt.y, d));
            }
        }
        if gr.drag_stopped() {
            setter.end_set_parameter(&fl.cutoff);
            setter.end_set_parameter(&fl.res);
        }
    }
    dark_box(ui, d);
    let painter = ui.painter().with_clip_rect(d.shrink(1.0));
    draw_spectrum(&painter, d.shrink(1.0), &st.spec, st.spec_fs, -80.0, Color32::from_rgb(46, 46, 46));
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |f: f32| d.left() + d.width() * ((f / lo).ln() / (hi / lo).ln());
    for f in [100.0f32, 1000.0, 10000.0] {
        painter.line_segment([Pos2::new(xf(f), d.top()), Pos2::new(xf(f), d.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 24)));
    }
    let (dbmin, dbmax) = if is_custom { (-36.0f32, 36.0f32) } else { (-48.0f32, 30.0f32) };
    let yd = |db: f32| d.bottom() - d.height() * ((db.clamp(dbmin, dbmax) - dbmin) / (dbmax - dbmin));
    painter.line_segment([Pos2::new(d.left(), yd(0.0)), Pos2::new(d.right(), yd(0.0))], Stroke::new(1.0_f32, Color32::from_rgb(30, 30, 30)));
    let cut = fl.cutoff.value();
    let res = fl.res.value();
    let ty = fl.fl_type.value();
    let on = fl.fl_on.value();
    let n = 200;
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let f = lo * (hi / lo).powf(i as f32 / n as f32);
        let mag = if !on { 1.0 } else if is_custom { custom_response(fl, f) } else { filter_response(ty, res, cut, f) };
        pts.push(Pos2::new(xf(f), yd(20.0 * mag.max(1e-6).log10())));
    }
    fill_under(&painter, &pts, d.bottom(), Color32::from_rgb(30, 30, 30));
    painter.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, if on { WHITE } else { DIM })));
    if is_custom {
        custom_points(ui, setter, fl, fi, d, &xf, &yd, (dbmin, dbmax));
    } else {
        painter.line_segment([Pos2::new(xf(cut), d.top()), Pos2::new(xf(cut), d.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(70, 70, 70)));
        // draggable node: x = cutoff, y = resonance
        let nc = Pos2::new(xf(cut).clamp(d.left() + 7.0, d.right() - 7.0), node_y(ty, fl.res.unmodulated_normalized_value(), cut, d));
        // grows only while the pointer is really on the node (not while a knob below is being dragged upwards)
        let busy = ui.ctx().dragged_id().map_or(false, |id| id != egui::Id::new(("filter_graph", fi)));
        let hot = !busy && ui.input(|i| i.pointer.hover_pos()).map_or(false, |pp| (pp - nc).length() < 14.0);
        painter.circle_filled(nc, if hot { 7.0 } else { 5.5 }, Color32::BLACK);
        painter.circle_stroke(nc, if hot { 7.0 } else { 5.5 }, Stroke::new(1.6_f32, WHITE));
    }
    for (f, t) in [(100.0, "100"), (1000.0, "1k"), (10000.0, "10k")] {
        painter.text(Pos2::new(xf(f) + 2.0, d.bottom() - 7.0), Align2::LEFT_CENTER, t, mono(8.5), Color32::from_rgb(90, 90, 90));
    }
    let y = r.top() + 194.0;
    let w = 66.0;
    let x0 = r.left() + 10.0;
    if is_custom {
        let sel = ui.data(|dd| dd.get_temp::<usize>(egui::Id::new(("cb_sel", fi)))).unwrap_or(0).min(CBANDS - 1);
        let b = &fl.cb[sel];
        let w = 56.0;
        mdst(if fi == 0 { ModDst::Cutoff } else { ModDst::Cutoff2 });
        knob_s(ui, setter, &b.freq, "FREQ", cell(x0 - 2.0, y, w), "Frequency of the selected point. Drop an LFO / envelope here to move ALL points together", false);
        mdst(if fi == 0 { ModDst::F1PtGain } else { ModDst::F2PtGain });
        knob_s(ui, setter, &b.gain, "GAIN", cell(x0 - 2.0 + w, y, w), "Gain of the selected point (bell / shelves). Drop an LFO / envelope here to move the gain of ALL points", true);
        mdst(if fi == 0 { ModDst::F1PtQ } else { ModDst::F2PtQ });
        knob_s(ui, setter, &b.q, "WIDTH", cell(x0 - 2.0 + w * 2.0, y, w), "Q / width of the selected point (also mouse wheel over a point)", false);
        mdst(if fi == 0 { ModDst::Drive } else { ModDst::Drive2 });
        knob_s(ui, setter, &fl.drive, "DRIVE", cell(x0 - 2.0 + w * 3.0, y, w), "Pre-filter saturation", false);
        knob_s(ui, setter, &fl.keytrack, "KEY", cell(x0 - 2.0 + w * 4.0, y, w), "Key tracking: all points move with the played note", false);
        mdst(if fi == 0 { ModDst::FilterMix } else { ModDst::FilterMix2 });
        knob_s(ui, setter, &fl.fl_mix, "MIX", cell(x0 - 2.0 + w * 5.0, y, w), "Dry / filtered mix", false);
    } else {
        mdst(if fi == 0 { ModDst::Cutoff } else { ModDst::Cutoff2 });
        knob_s(ui, setter, &fl.cutoff, "CUTOFF", cell(x0, y, w), "Filter cutoff frequency", false);
        mdst(if fi == 0 { ModDst::Res } else { ModDst::Res2 });
        knob_s(ui, setter, &fl.res, "RES", cell(x0 + w, y, w), "Resonance (for Peak / shelves: boost in dB, comb: feedback)", false);
        mdst(if fi == 0 { ModDst::Drive } else { ModDst::Drive2 });
        knob_s(ui, setter, &fl.drive, "DRIVE", cell(x0 + w * 2.0, y, w), "Pre-filter saturation", false);
        knob_s(ui, setter, &fl.keytrack, "KEY", cell(x0 + w * 3.0, y, w), "Key tracking: cutoff follows the played note", false);
        mdst(if fi == 0 { ModDst::FilterMix } else { ModDst::FilterMix2 });
        knob_s(ui, setter, &fl.fl_mix, "MIX", cell(x0 + w * 4.0, y, w), "Dry / filtered mix", false);
    }
    let ry = r.top() + 274.0;
    ui.painter().text(Pos2::new(r.left() + 14.0, ry + 11.0), Align2::LEFT_CENTER, "ROUTE", mono(10.0), DIM);
    let items: [(&str, &BoolParam, &str); 5] = [
        ("A", &fl.fl_a, "Send oscillator A through this filter"),
        ("B", &fl.fl_b, "Send oscillator B through this filter"),
        ("C", &fl.fl_c, "Send oscillator C through this filter"),
        ("SUB", &fl.fl_sub, "Send the sub oscillator through this filter"),
        ("NOISE", &fl.fl_noise, "Send the noise through this filter"),
    ];
    let pitch = (r.width() - 12.0 - 62.0 + 4.0) / 5.0; // fills the row up to the graph's right edge
    for (i, (nm, bp, tip)) in items.iter().enumerate() {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(r.left() + 62.0 + pitch * i as f32, ry), Vec2::new(pitch - 4.0, 22.0)), nm, bp.value(), tip) {
            set_b(setter, bp, !bp.value());
        }
    }
    ui.painter().text(Pos2::new(r.left() + 14.0, r.top() + 322.0), Align2::LEFT_CENTER, if is_custom { "Click=add | drag | wheel=width | 2x click=del" } else { "Drag LFOs / envelopes onto CUTOFF to make it move" }, mono(9.5), DIM);
}

// ===================================================================== sub + noise

fn sub_noise_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, _shared: &Shared, st: &mut Gui, r: Rect, now: f64) {
    panel(ui, r, "05", "SUB / NOISE");
    let x = r.left();
    let y = r.top();
    ui.painter().text(Pos2::new(x + 14.0, y + 44.0), Align2::LEFT_CENTER, "SUB", mono(10.5), WHITE);
    on_pill(ui, setter, &p.sub_on, Rect::from_min_size(Pos2::new(x + 50.0, y + 33.0), Vec2::new(38.0, 22.0)), "Sub oscillator on/off");
    enum_combo_sz(ui, setter, &p.sub_wave, "sub_wave", Rect::from_min_size(Pos2::new(x + 94.0, y + 33.0), Vec2::new(62.0, 22.0)), 10.5);
    // same 5-column grid as the NOISE knobs below: SUB uses the right three columns
    let kw = ((r.width() - 24.0) / 5.0).floor();
    let k0 = r.left() + 12.0 + kw * 2.0;
    knob_s(ui, setter, &p.sub_oct, "OCT", cell(k0, y + 28.0, kw), "Sub octave offset", false);
    mdst(ModDst::SubLevel);
    knob_s(ui, setter, &p.sub_level, "LEVEL", cell(k0 + kw, y + 28.0, kw), "Sub level", false);
    knob_s(ui, setter, &p.sub_pan, "PAN", cell(k0 + kw * 2.0, y + 28.0, kw), "Sub stereo position", true);
    ui.painter().hline(x + 12.0..=r.right() - 12.0, y + 98.0, Stroke::new(1.0_f32, LINE));
    ui.painter().text(Pos2::new(x + 14.0, y + 116.0), Align2::LEFT_CENTER, "NOISE", mono(10.5), WHITE);
    enum_combo(ui, setter, &p.noise_type, "noise_type", Rect::from_min_size(Pos2::new(x + 70.0, y + 104.0), Vec2::new(130.0, 24.0)));
    let is_sample = p.noise_type.value() == NoiseType::Sample;
    let sname = p.noise_sample.lock().map(|g| crate::sample::encoded_name(&g)).unwrap_or_default();
    let label = if is_sample {
        if sname.is_empty() { "LOAD FILE...".to_string() } else { sname.chars().take(14).collect() }
    } else {
        "FILES...".to_string()
    };
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 208.0, y + 104.0), Vec2::new(128.0, 24.0)), &label, is_sample, "Choose a .wav file from the Noises folder as a noise / sample source") {
        st.rescan();
        st.smp_target = None;
        st.overlay = Overlay::Noises;
    }
    let ky = y + 134.0;
    let w = ((r.width() - 24.0) / 5.0).floor();
    let kx = x + 12.0;
    mdst(ModDst::NoiseLevel);
    knob_s(ui, setter, &p.noise_level, "LEVEL", cell(kx, ky, w), "Noise level", false);
    knob_s(ui, setter, &p.noise_pan, "PAN", cell(kx + w, ky, w), "Noise stereo position", true);
    mdst(ModDst::NoisePitch);
    knob_s(ui, setter, &p.noise_pitch, "PITCH", cell(kx + w * 2.0, ky, w), "Playback pitch of a loaded sample (semitones)", true);
    knob_s(ui, setter, &p.noise_phase, "START", cell(kx + w * 3.0, ky, w), "Where the sample starts", false);
    knob_s(ui, setter, &p.noise_rand, "RAND", cell(kx + w * 4.0, ky, w), "Random start position", false);
    let py = y + 206.0;
    if pill_tip(ui, Rect::from_min_size(Pos2::new(kx, py), Vec2::new(70.0, 22.0)), "KEY", p.noise_key.value(), "Sample pitch follows the keyboard") {
        set_b(setter, &p.noise_key, !p.noise_key.value());
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(kx + 76.0, py), Vec2::new(70.0, 22.0)), "LOOP", p.noise_loop.value(), "Loop the sample (off = play once)") {
        set_b(setter, &p.noise_loop, !p.noise_loop.value());
    }
    ui.painter().text(Pos2::new(kx + 160.0, py + 11.0), Align2::LEFT_CENTER, "KEY / LOOP are for samples", mono(9.0), DIM);
    // ---- tools
    ui.painter().hline(x + 12.0..=r.right() - 12.0, y + 244.0, Stroke::new(1.0_f32, LINE));
    ui.painter().text(Pos2::new(x + 14.0, y + 262.0), Align2::LEFT_CENTER, "TOOLS", mono(9.5), DIM);
    let by = y + 252.0;
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 62.0, by), Vec2::new(60.0, 22.0)), "INIT", false, "Reset to the Init sound") {
        if let Some(rf) = st.presets.iter().find(|q| q.name == "Init").cloned() {
            if let Err(e) = presets::load(setter, p, _shared, &rf) {
                st.say(now, e);
            }
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 126.0, by), Vec2::new(86.0, 22.0)), "RANDOM", false, "Make a new random sound (a starting point for ideas)") {
        let seed = (now * 1000.0) as u64 ^ 0x9E37_79B9_7F4A_7C15;
        randomize(setter, p, _shared, seed);
        st.say(now, "random sound".to_string());
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 216.0, by), Vec2::new(56.0, 22.0)), "COPY", st.copy_menu, "Copy one oscillator (with its wavetable) to another") {
        st.copy_menu = !st.copy_menu;
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 276.0, by), Vec2::new(r.width() - 12.0 - 276.0, 22.0)), "HYBRID", false, "Mix the current sound with a random preset: every section (oscillator, filter, LFO, effect...) comes from one or the other") {
        if st.presets.is_empty() {
            st.presets = presets::list_all();
        }
        let cur = p.preset_name.lock().map(|g| g.clone()).unwrap_or_default();
        let pool: Vec<presets::PresetRef> = st.presets.iter().filter(|q| q.name != cur && q.name != "Init").cloned().collect();
        if pool.is_empty() {
            st.say(now, "no presets to mix with".to_string());
        } else {
            let seed = (now * 1000.0) as u64 ^ 0xA076_1D64_78BD_642F;
            let pick = &pool[(seed >> 7) as usize % pool.len()];
            match presets::hybridize(setter, p, _shared, pick, seed) {
                Ok(()) => st.say(now, format!("hybrid with {}", pick.name)),
                Err(e) => st.say(now, e),
            }
        }
    }
    if st.copy_menu {
        let pairs = [(0usize, 1usize), (0, 2), (1, 0), (1, 2), (2, 0), (2, 1)];
        for (k, (f, t)) in pairs.iter().enumerate() {
            let rr = Rect::from_min_size(Pos2::new(x + 216.0 + 42.0 * (k % 3) as f32, by - 52.0 + 24.0 * (k / 3) as f32), Vec2::new(40.0, 22.0));
            if pill_tip(ui, rr, &format!("{}>{}", OSC_NAMES[*f], OSC_NAMES[*t]), false, "Copy the first oscillator onto the second one") {
                copy_osc(setter, p, _shared, *f, *t);
                st.say(now, format!("{} copied to {}", OSC_NAMES[*f], OSC_NAMES[*t]));
                st.copy_menu = false;
            }
        }
    }
}

// ===================================================================== ENV tab

/// Envelope display. Drag a segment up / down to bend its slope (double-click resets it).
fn env_curve(ui: &mut Ui, setter: &ParamSetter, e: &EnvParams, r: Rect, id: &str) {
    let seg = |t: f32| 0.05 + (t / 10000.0).powf(0.35);
    let wde = if e.delay.value() > 0.6 { seg(e.delay.value()) * 0.6 } else { 0.0 };
    let wa = seg(e.attack.value());
    let wh = if e.hold.value() > 0.6 { seg(e.hold.value()) * 0.6 } else { 0.0 };
    let wd = seg(e.decay.value());
    let wr = seg(e.release.value());
    let s = e.sustain.value();
    let wsus = 0.3;
    let tot = wde + wa + wh + wd + wsus + wr;
    let inner = r.shrink2(Vec2::new(6.0, 8.0));
    let x = |t: f32| inner.left() + inner.width() * t / tot;
    let y = |v: f32| inner.bottom() - inner.height() * v;
    let (ka, kd, kr) = (2f32.powf(e.atk_curve.value() * 2.0), 2f32.powf(e.dec_curve.value() * 2.0), 2f32.powf(e.rel_curve.value() * 2.0));
    let t_ah = wde + wa + wh;
    let t_d = t_ah + wd;
    let t_r = t_d + wsus;

    // ---- interaction
    let resp = ui.interact(r, egui::Id::new(id), Sense::click_and_drag()).on_hover_text("Drag the attack / decay / release part up or down to bend its slope.\nDouble-click a part to reset its slope.");
    let seg_at = |px: f32| -> usize {
        if px < x(t_ah) {
            0
        } else if px < x(t_r) {
            1
        } else {
            2
        }
    };
    let curves: [&FloatParam; 3] = [&e.atk_curve, &e.dec_curve, &e.rel_curve];
    let key = egui::Id::new((id, "seg"));
    if resp.drag_started() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let sg = seg_at(pt.x);
            ui.data_mut(|d| d.insert_temp(key, sg));
            setter.begin_set_parameter(curves[sg]);
        }
    }
    if resp.dragged() {
        if let Some(sg) = ui.data(|d| d.get_temp::<usize>(key)) {
            let dy = -resp.drag_delta().y;
            let cur = curves[sg].value();
            setter.set_parameter(curves[sg], (cur - dy / 40.0).clamp(-1.0, 1.0));
        }
    }
    if resp.drag_stopped() {
        if let Some(sg) = ui.data(|d| d.get_temp::<usize>(key)) {
            setter.end_set_parameter(curves[sg]);
        }
        ui.data_mut(|d| d.remove::<usize>(key));
    }
    if resp.double_clicked() {
        if let Some(pt) = resp.interact_pointer_pos() {
            reset(setter, curves[seg_at(pt.x)]);
        }
    }
    let active = if resp.dragged() { ui.data(|d| d.get_temp::<usize>(key)) } else if resp.hovered() { ui.input(|i| i.pointer.hover_pos()).map(|pt| seg_at(pt.x)) } else { None };

    // ---- drawing
    dark_box(ui, r);
    let painter = ui.painter().with_clip_rect(r.shrink(1.0));
    let mut pa: Vec<Pos2> = vec![Pos2::new(x(0.0), y(0.0)), Pos2::new(x(wde), y(0.0))];
    for i in 1..=24 {
        let f = i as f32 / 24.0;
        pa.push(Pos2::new(x(wde + wa * f), y(f.powf(ka))));
    }
    let mut ph: Vec<Pos2> = vec![Pos2::new(x(wde + wa), y(1.0)), Pos2::new(x(t_ah), y(1.0))];
    let mut pd: Vec<Pos2> = Vec::new();
    for i in 0..=24 {
        let f = i as f32 / 24.0;
        let u = (1.0 - f).powi(2);
        pd.push(Pos2::new(x(t_ah + wd * f), y(s + (1.0 - s) * u.powf(kd))));
    }
    let mut ps: Vec<Pos2> = vec![Pos2::new(x(t_d), y(s)), Pos2::new(x(t_r), y(s))];
    let mut pr: Vec<Pos2> = Vec::new();
    for i in 0..=24 {
        let f = i as f32 / 24.0;
        let u = (1.0 - f).powi(2);
        pr.push(Pos2::new(x(t_r + wr * f), y(s * u.powf(kr))));
    }
    let mut all: Vec<Pos2> = pa.clone();
    all.append(&mut ph.clone());
    all.extend(pd.iter().copied());
    all.append(&mut ps.clone());
    all.extend(pr.iter().copied());
    let mut dense = Vec::new();
    for w in all.windows(2) {
        let n = ((w[1].x - w[0].x).abs() / 1.5).ceil().max(1.0) as usize;
        for k in 0..n {
            let t = k as f32 / n as f32;
            dense.push(Pos2::new(w[0].x + (w[1].x - w[0].x) * t, w[0].y + (w[1].y - w[0].y) * t));
        }
    }
    fill_under(&painter, &dense, y(0.0), Color32::from_rgb(28, 28, 28));
    let base = Stroke::new(1.8_f32, WHITE);
    let hot = Stroke::new(2.8_f32, WHITE);
    let st_of = |i: usize| if active == Some(i) { hot } else { base };
    painter.add(egui::Shape::line(pa.clone(), PathStroke::new(st_of(0).width, WHITE)));
    ph.insert(0, *pa.last().unwrap());
    painter.add(egui::Shape::line(ph, PathStroke::new(1.8_f32, WHITE)));
    painter.add(egui::Shape::line(pd.clone(), PathStroke::new(st_of(1).width, WHITE)));
    ps.insert(0, *pd.last().unwrap());
    painter.add(egui::Shape::line(ps, PathStroke::new(1.8_f32, WHITE)));
    painter.add(egui::Shape::line(pr.clone(), PathStroke::new(st_of(2).width, WHITE)));
    // grab dots at the middle of each bendable part
    for (i, pts) in [&pa, &pd, &pr].iter().enumerate() {
        let m = pts[pts.len() / 2];
        let on = active == Some(i);
        painter.circle_filled(m, if on { 3.6 } else { 2.4 }, if on { WHITE } else { MID });
    }
}

fn env_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, c: Rect, now: f64) {
    let w = (c.width() - 30.0) / 4.0;
    let envs: [(&str, &EnvParams); 4] = [("ENV 1  (AMP)", &p.e1), ("ENV 2", &p.e2), ("ENV 3", &p.e3), ("ENV 4", &p.e4)];
    for (i, (name, e)) in envs.iter().enumerate() {
        let x = c.left() + (w + 10.0) * i as f32;
        let es = [ModSrc::Env1, ModSrc::Env2, ModSrc::Env3, ModSrc::Env4][i];
        source_chip(ui, setter, p, st, Rect::from_min_size(Pos2::new(x, c.top() - 4.0), Vec2::new(52.0, 20.0)), &format!("ENV {}", i + 1), es, "Drag this onto any knob", now);
        if i == 0 {
            ui.painter().text(Pos2::new(x + 58.0, c.top() + 6.0), Align2::LEFT_CENTER, "(AMP)", mono(9.5), DIM);
        }
        let _ = name;
        if pill_tip(ui, Rect::from_min_size(Pos2::new(x + w - 92.0, c.top() - 3.0), Vec2::new(92.0, 18.0)), "INV.LEGATO", e.relegato.value(), "Restart this envelope on every note even in Legato mode (Serum: Invert Legato)") {
            set_b(setter, &e.relegato, !e.relegato.value());
        }
        env_curve(ui, setter, e, Rect::from_min_size(Pos2::new(x, c.top() + 16.0), Vec2::new(w, 86.0)), &format!("envcurve{i}"));
        let kw = w / 3.0;
        let ky = c.top() + 108.0;
        knob_s(ui, setter, &e.delay, "DELAY", cell(x, ky, kw), "Time before the attack starts", false);
        knob_s(ui, setter, &e.attack, "ATTACK", cell(x + kw, ky, kw), "Attack time", false);
        knob_s(ui, setter, &e.hold, "HOLD", cell(x + kw * 2.0, ky, kw), "Time the level is held at the top", false);
        knob_s(ui, setter, &e.decay, "DECAY", cell(x, ky + 70.0, kw), "Decay time", false);
        knob_s(ui, setter, &e.sustain, "SUSTAIN", cell(x + kw, ky + 70.0, kw), "Sustain level", false);
        knob_s(ui, setter, &e.release, "RELEASE", cell(x + kw * 2.0, ky + 70.0, kw), "Release time", false);
    }
}

// ===================================================================== clip sequencer

fn clip_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, meters: &Meters, st: &mut Gui, c: Rect) {
    use crate::clip::*;
    let txt = p.clips.lock().map(|g| g.clone()).unwrap_or_default();
    let mut clips = decode_all(&txt);
    let sel = (p.clip_sel.value().clamp(1, 12) - 1) as usize;
    let mut changed = false;
    let on = p.clip_on.value();
    if pill_tip(ui, Rect::from_min_size(Pos2::new(c.left() + 4.0, c.top() - 2.0), Vec2::new(82.0, 24.0)), if on { "CLIP ON" } else { "CLIP OFF" }, on, "Clip sequencer: a held key plays the selected clip, transposed from that key (middle row = the key you hold). Replaces the arpeggiator while on.") {
        set_b(setter, &p.clip_on, !on);
    }
    for k in 0..NCLIPS {
        let has = !clips[k].notes.is_empty();
        let pr = Rect::from_min_size(Pos2::new(c.left() + 96.0 + 32.0 * k as f32, c.top() - 2.0), Vec2::new(30.0, 24.0));
        if pill_tip(ui, pr, &format!("{}", k + 1), k == sel, &format!("Clip slot {}{}", k + 1, if has { " (has notes)" } else { " (empty)" })) {
            set_i(setter, &p.clip_sel, k as i32 + 1);
        }
        if has && k != sel {
            ui.painter().circle_filled(Pos2::new(pr.right() - 5.0, pr.top() + 5.0), 2.0, WHITE);
        }
    }
    let bx = c.left() + 96.0 + 32.0 * 12.0 + 12.0;
    let bars = clips[sel].bars as usize;
    ui.painter().text(Pos2::new(bx, c.top() + 10.0), Align2::LEFT_CENTER, format!("BARS {bars}"), mono(10.0), WHITE);
    if pill_tip(ui, Rect::from_min_size(Pos2::new(bx + 58.0, c.top() - 1.0), Vec2::new(22.0, 22.0)), "-", false, "Shorter clip") && bars > 1 {
        clips[sel].bars -= 1;
        let max = clips[sel].steps();
        clips[sel].notes.retain(|n| (n.start as usize) < max);
        for n in clips[sel].notes.iter_mut() {
            n.len = n.len.min((max - n.start as usize).max(1) as u8);
        }
        changed = true;
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(bx + 82.0, c.top() - 1.0), Vec2::new(22.0, 22.0)), "+", false, "Longer clip (up to 4 bars)") && bars < MAX_BARS {
        clips[sel].bars += 1;
        changed = true;
    }
    ui.painter().text(Pos2::new(bx + 118.0, c.top() + 10.0), Align2::LEFT_CENTER, "STEP", mono(9.5), DIM);
    enum_combo(ui, setter, &p.clip_div, "clip_div", Rect::from_min_size(Pos2::new(bx + 152.0, c.top() - 2.0), Vec2::new(74.0, 24.0)));
    ui.painter().text(Pos2::new(bx + 244.0, c.top() + 10.0), Align2::LEFT_CENTER, "SWING", mono(9.5), DIM);
    h_slider(ui, setter, &p.clip_swing, Rect::from_min_size(Pos2::new(bx + 292.0, c.top() + 3.0), Vec2::new(80.0, 14.0)), "clip_swing", false, "Delays every second step of the clip (double-click to reset)");
    ui.painter().text(Pos2::new(bx + 380.0, c.top() + 10.0), Align2::LEFT_CENTER, format!("{:.0}%", p.clip_swing.value() * 100.0), mono(9.5), WHITE);
    if pill_tip(ui, Rect::from_min_size(Pos2::new(bx + 420.0, c.top() - 2.0), Vec2::new(88.0, 24.0)), "CAPTURE", false, "Turn the notes you just played on your keyboard into this clip. The first note you played becomes step 1 and the key the clip is transposed from.") {
        let ev: Vec<(bool, u8, f32, f64)> = meters.cap.lock().map(|g| g.clone()).unwrap_or_default();
        let now_b = f64::from_bits(meters.beat_now.load(std::sync::atomic::Ordering::Relaxed));
        let span = clips[sel].bars as f64 * 4.0 + 4.0;
        let recent: Vec<(bool, u8, f32, f64)> = ev.into_iter().filter(|e| now_b - e.3 <= span).collect();
        match clip_from_events(&recent, clips[sel].bars) {
            Some(cl) => {
                clips[sel].notes = cl.notes;
                changed = true;
            }
            None => {}
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(bx + 514.0, c.top() - 2.0), Vec2::new(70.0, 24.0)), "SYNC", p.clip_sync.value(), "Start the clip on the host's step grid, so it lines up with the song (needs a host that reports its position)") {
        set_b(setter, &p.clip_sync, !p.clip_sync.value());
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(c.right() - 70.0, c.top() - 2.0), Vec2::new(66.0, 24.0)), "CLEAR", false, "Delete every note of this clip") {
        clips[sel].notes.clear();
        changed = true;
    }
    // ---- piano roll
    let rows = (PITCH_MAX - PITCH_MIN + 1) as usize;
    let rh = ((c.height() - 36.0 - 50.0) / rows as f32).clamp(6.0, 13.0);
    let gx = c.left() + 34.0;
    let gy = c.top() + 30.0;
    let steps = clips[sel].steps();
    let cw = (c.right() - 4.0 - gx) / steps as f32;
    let grid = Rect::from_min_size(Pos2::new(gx, gy), Vec2::new(cw * steps as f32, rh * rows as f32));
    dark_box(ui, grid);
    {
        let pn = ui.painter();
        for r in 0..rows {
            let pitch = PITCH_MAX as i32 - r as i32;
            let y = gy + rh * r as f32;
            let black = matches!(pitch.rem_euclid(12), 1 | 3 | 6 | 8 | 10);
            if black {
                pn.rect_filled(Rect::from_min_size(Pos2::new(gx, y), Vec2::new(grid.width(), rh)), CornerRadius::ZERO, Color32::from_rgb(16, 16, 16));
            }
            if pitch == 0 {
                pn.rect_filled(Rect::from_min_size(Pos2::new(gx, y), Vec2::new(grid.width(), rh)), CornerRadius::ZERO, Color32::from_rgb(34, 34, 34));
                pn.text(Pos2::new(gx - 4.0, y + rh / 2.0), Align2::RIGHT_CENTER, "KEY", mono(7.5), WHITE);
            } else if pitch % 12 == 0 {
                pn.text(Pos2::new(gx - 4.0, y + rh / 2.0), Align2::RIGHT_CENTER, format!("{:+}", pitch), mono(7.5), DIM);
            }
        }
        for k in 0..=steps {
            let x = gx + cw * k as f32;
            let strong = k % 16 == 0;
            let beat = k % 4 == 0;
            pn.line_segment([Pos2::new(x, gy), Pos2::new(x, grid.bottom())], Stroke::new(1.0_f32, if strong { Color32::from_rgb(70, 70, 70) } else if beat { Color32::from_rgb(38, 38, 38) } else { Color32::from_rgb(22, 22, 22) }));
        }
    }
    {
        let pos = meters.clip_pos.load(std::sync::atomic::Ordering::Relaxed) as usize;
        if p.clip_on.value() && pos > 0 {
            let k = (pos - 1) % steps;
            let x = gx + cw * k as f32;
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(x, gy), Vec2::new(cw, grid.height())), CornerRadius::ZERO, Color32::from_rgba_unmultiplied(255, 255, 255, 26));
            ui.painter().line_segment([Pos2::new(x, gy), Pos2::new(x, grid.bottom())], Stroke::new(1.5_f32, WHITE));
            ui.ctx().request_repaint();
        }
    }
    // ---- automation lane (mod source LANE)
    {
        let lr = Rect::from_min_size(Pos2::new(gx, grid.bottom() + 6.0), Vec2::new(grid.width(), 40.0));
        dark_box(ui, lr);
        ui.painter().text(Pos2::new(gx - 4.0, lr.center().y), Align2::RIGHT_CENTER, "LANE", mono(7.5), DIM);
        ui.painter().line_segment([Pos2::new(lr.left(), lr.center().y), Pos2::new(lr.right(), lr.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(50, 50, 50)));
        for k in 0..steps {
            let v = clips[sel].lane.get(k).copied().unwrap_or(0);
            if v != 0 {
                let h = v as f32 / 100.0 * 18.0;
                let x = gx + cw * k as f32;
                let rr = if h > 0.0 { Rect::from_min_max(Pos2::new(x + 1.0, lr.center().y - h), Pos2::new(x + cw - 1.0, lr.center().y)) } else { Rect::from_min_max(Pos2::new(x + 1.0, lr.center().y), Pos2::new(x + cw - 1.0, lr.center().y - h)) };
                ui.painter().rect_filled(rr, CornerRadius::ZERO, WHITE);
            }
        }
        let lresp = ui.interact(lr, egui::Id::new("clip_lane"), Sense::click_and_drag()).on_hover_text("Automation lane: drag up/down on a step to set a value. Right-click a step resets it.\nDrag the LANE chip (bottom strip) onto any knob to send it to any knob. Plays together with the clip.");
        if let Some(pt) = lresp.interact_pointer_pos() {
            let k = ((pt.x - gx) / cw).floor() as i32;
            if k >= 0 && (k as usize) < steps {
                let k = k as usize;
                if clips[sel].lane.len() < MAX_STEPS {
                    clips[sel].lane.resize(MAX_STEPS, 0);
                }
                if lresp.secondary_clicked() || (lresp.dragged_by(egui::PointerButton::Secondary)) {
                    clips[sel].lane[k] = 0;
                    changed = true;
                } else if lresp.dragged_by(egui::PointerButton::Primary) || lresp.clicked_by(egui::PointerButton::Primary) {
                    let v = ((lr.center().y - pt.y) / 18.0 * 100.0).clamp(-100.0, 100.0);
                    let v = if v.abs() < 6.0 { 0 } else { v.round() as i8 };
                    if clips[sel].lane[k] != v {
                        clips[sel].lane[k] = v;
                        changed = true;
                    }
                }
            }
        }
    }
    let dkey = egui::Id::new("clip_drag");
    let resp = ui.interact(grid, egui::Id::new("clip_grid"), Sense::click_and_drag()).on_hover_text("Click: add a note (drag right = longer). Drag a note: move it. Right-click: delete.\nMouse wheel over a note: velocity.");
    let cell_of = |pt: Pos2| -> (i32, i32) { (((pt.x - gx) / cw).floor() as i32, ((pt.y - gy) / rh).floor() as i32) };
    let hit = |notes: &Vec<CNote>, stp: i32, row: i32| -> Option<usize> {
        let pitch = PITCH_MAX as i32 - row;
        notes.iter().position(|n| n.pitch as i32 == pitch && stp >= n.start as i32 && stp < n.start as i32 + n.len as i32)
    };
    if let Some(pt) = resp.interact_pointer_pos() {
        let (stp, row) = cell_of(pt);
        let in_grid = stp >= 0 && (stp as usize) < steps && row >= 0 && (row as usize) < rows;
        if resp.secondary_clicked() && in_grid {
            if let Some(i) = hit(&clips[sel].notes, stp, row) {
                clips[sel].notes.remove(i);
                changed = true;
            }
        }
        if resp.drag_started_by(egui::PointerButton::Primary) && in_grid {
            if let Some(i) = hit(&clips[sel].notes, stp, row) {
                let n = clips[sel].notes[i];
                ui.data_mut(|d| d.insert_temp(dkey, (i as i32, 1i32, stp - n.start as i32)));
                st.clip_len = n.len;
            } else if clips[sel].notes.len() < MAX_NOTES {
                let n = CNote { start: stp as u8, len: st.clip_len.max(1).min((clips[sel].steps() as i32 - stp).max(1) as u8), pitch: (PITCH_MAX as i32 - row) as i8, vel: 100 };
                clips[sel].notes.push(n);
                let i = clips[sel].notes.len() - 1;
                ui.data_mut(|d| d.insert_temp(dkey, (i as i32, 2i32, 0i32)));
                changed = true;
            }
        } else if resp.clicked_by(egui::PointerButton::Primary) && in_grid && hit(&clips[sel].notes, stp, row).is_none() && clips[sel].notes.len() < MAX_NOTES {
            let n = CNote { start: stp as u8, len: st.clip_len.max(1).min((clips[sel].steps() as i32 - stp).max(1) as u8), pitch: (PITCH_MAX as i32 - row) as i8, vel: 100 };
            clips[sel].notes.push(n);
            changed = true;
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some((i, mode, off)) = ui.data(|d| d.get_temp::<(i32, i32, i32)>(dkey)) {
                let i = i as usize;
                if i < clips[sel].notes.len() {
                    let n = &mut clips[sel].notes[i];
                    let stp_c = stp.clamp(0, steps as i32 - 1);
                    if mode == 1 {
                        let ns = (stp_c - off).clamp(0, (steps as i32 - n.len as i32).max(0));
                        let np = (PITCH_MAX as i32 - row).clamp(PITCH_MIN as i32, PITCH_MAX as i32) as i8;
                        if ns as u8 != n.start || np != n.pitch {
                            n.start = ns as u8;
                            n.pitch = np;
                            changed = true;
                        }
                    } else {
                        let nl = (stp_c - n.start as i32 + 1).clamp(1, steps as i32 - n.start as i32) as u8;
                        if nl != n.len {
                            n.len = nl;
                            st.clip_len = nl;
                            changed = true;
                        }
                    }
                }
            }
        }
        if resp.drag_stopped() {
            ui.data_mut(|d| d.remove::<(i32, i32, i32)>(dkey));
        }
        if resp.hovered() && in_grid {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                if let Some(i) = hit(&clips[sel].notes, stp, row) {
                    let v = clips[sel].notes[i].vel as i32 + (sc / 4.0) as i32;
                    clips[sel].notes[i].vel = v.clamp(1, 100) as u8;
                    changed = true;
                }
            }
        }
    }
    // notes + play head
    {
        let pn = ui.painter().with_clip_rect(grid);
        for n in clips[sel].notes.iter() {
            let r = (PITCH_MAX as i32 - n.pitch as i32) as f32;
            let rect = Rect::from_min_size(Pos2::new(gx + cw * n.start as f32 + 1.0, gy + rh * r + 1.0), Vec2::new(cw * n.len as f32 - 2.0, rh - 2.0));
            let g = (110.0 + 145.0 * n.vel as f32 / 100.0) as u8;
            pn.rect_filled(rect, CornerRadius::same(2), Color32::from_rgb(g, g, g));
        }
    }
    ui.painter().text(Pos2::new(c.left(), gy + rh * rows as f32 + 8.0), Align2::LEFT_CENTER, "", mono(8.0), DIM);
    if changed {
        if let Ok(mut g) = p.clips.lock() {
            *g = encode_all(&clips);
        }
    }
}

// ===================================================================== settings tab

fn settings_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, meters: &Meters, st: &mut Gui, c: Rect) {
    let mut head = |ui: &mut Ui, x: f32, t: &str| {
        ui.painter().text(Pos2::new(x, c.top() + 6.0), Align2::LEFT_CENTER, t, mono(10.5), WHITE);
    };
    // ---------------- tuning
    let x0 = c.left() + 4.0;
    head(ui, x0, "TUNING");
    knob_s(ui, setter, &p.a4, "A4 (Hz)", cell(x0, c.top() + 22.0, 74.0), "Reference pitch of A4: 440 Hz is standard, 432 Hz for \"Verdi tuning\"", false);
    knob_s(ui, setter, &p.tune, "FINE (ct)", cell(x0 + 78.0, c.top() + 22.0, 74.0), "Master fine tune in cents (same as TUNE in the bottom strip)", true);
    for (k, hz) in [432.0f32, 440.0, 442.0, 444.0].iter().enumerate() {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 164.0 + 48.0 * (k % 2) as f32, c.top() + 26.0 + 26.0 * (k / 2) as f32), Vec2::new(44.0, 22.0)), &format!("{hz:.0}"), (p.a4.value() - hz).abs() < 0.05, "Set the A4 reference") {
            set_f(setter, &p.a4, *hz);
        }
    }
    ui.painter().text(Pos2::new(x0, c.top() + 100.0), Align2::LEFT_CENTER, "SCALE", mono(9.5), DIM);
    enum_combo(ui, setter, &p.tuning, "tuning_scale", Rect::from_min_size(Pos2::new(x0 + 48.0, c.top() + 88.0), Vec2::new(170.0, 24.0)));
    ui.painter().text(Pos2::new(x0, c.top() + 128.0), Align2::LEFT_CENTER, "ROOT", mono(9.5), DIM);
    for (k, nm) in ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"].iter().enumerate() {
        if pill(ui, Rect::from_min_size(Pos2::new(x0 + 48.0 + 26.0 * k as f32, c.top() + 117.0), Vec2::new(24.0, 22.0)), nm, p.tun_root.value() == k as i32) {
            set_i(setter, &p.tun_root, k as i32);
        }
    }
    // deviation bars (cents vs equal temperament)
    let dev = p.tuning.value().cents();
    let br = Rect::from_min_size(Pos2::new(x0 + 48.0, c.top() + 146.0), Vec2::new(312.0, 70.0));
    dark_box(ui, br);
    for k in 0..12 {
        let bx = br.left() + 6.0 + 25.0 * k as f32;
        let v = (dev[k] / 25.0).clamp(-1.0, 1.0);
        let mid = br.center().y;
        let h = (br.height() * 0.42) * v;
        ui.painter().rect_filled(Rect::from_min_max(Pos2::new(bx, mid.min(mid - h)), Pos2::new(bx + 18.0, mid.max(mid - h) + 1.0)), CornerRadius::same(2), WHITE);
        ui.painter().text(Pos2::new(bx + 9.0, br.bottom() - 7.0), Align2::CENTER_CENTER, format!("{:+.0}", dev[k]), mono(7.5), DIM);
    }
    ui.painter().text(Pos2::new(x0, c.top() + 180.0), Align2::LEFT_CENTER, "cents vs.", mono(8.5), DIM);
    ui.painter().text(Pos2::new(x0, c.top() + 192.0), Align2::LEFT_CENTER, "12-TET", mono(8.5), DIM);
    // ---------------- MIDI / performance
    let x1 = c.left() + 400.0;
    head(ui, x1, "MIDI / PLAY");
    ui.painter().text(Pos2::new(x1, c.top() + 40.0), Align2::LEFT_CENTER, "CHANNEL", mono(9.5), DIM);
    spinner(ui, setter, &p.midi_ch, Rect::from_min_size(Pos2::new(x1 + 66.0, c.top() + 29.0), Vec2::new(54.0, 22.0)), "midi_ch", "MIDI channel to listen to (0 = all channels)");
    ui.painter().text(Pos2::new(x1 + 130.0, c.top() + 40.0), Align2::LEFT_CENTER, if p.midi_ch.value() == 0 { "OMNI" } else { "" }, mono(9.5), WHITE);
    knob_s(ui, setter, &p.vel_curve, "VEL CURVE", cell(x1, c.top() + 62.0, 78.0), "Velocity response: negative = you have to hit harder, positive = soft playing is already loud", true);
    knob_s(ui, setter, &p.bend, "BEND RANGE", cell(x1 + 80.0, c.top() + 62.0, 78.0), "Pitch-bend range in semitones", false);
    knob_s(ui, setter, &p.glide, "GLIDE", cell(x1 + 160.0, c.top() + 62.0, 78.0), "Portamento time", false);
    {
        let on = p.midi_out.value();
        if pill_tip(ui, Rect::from_min_size(Pos2::new(x1, c.top() + 132.0), Vec2::new(110.0, 24.0)), if on { "MIDI OUT ON" } else { "MIDI OUT OFF" }, on, "Send the notes made by the arpeggiator and the clip sequencer to the host as MIDI, so they can drive another instrument.") {
            set_b(setter, &p.midi_out, !on);
        }
    }
    {
        let on = p.show_wheels.value();
        if pill_tip(ui, Rect::from_min_size(Pos2::new(x1 + 120.0, c.top() + 132.0), Vec2::new(150.0, 24.0)), if on { "WHEELS: SHOWN" } else { "WHEELS: HIDDEN" }, on, "Show the on-screen OCT / SEMI spinners and pitch / mod wheels left of the keyboard. Off = the keyboard gets the full width (your MIDI keyboard has its own).") {
            set_b(setter, &p.show_wheels, !on);
        }
        ui.painter().text(Pos2::new(x1, c.top() + 208.0), Align2::LEFT_CENTER, "KEYBOARD", mono(9.5), DIM);
        ui.painter().text(Pos2::new(x1 + 74.0, c.top() + 208.0), Align2::LEFT_CENTER, "OCT", mono(9.5), DIM);
        spinner(ui, setter, &p.kb_octave, Rect::from_min_size(Pos2::new(x1 + 104.0, c.top() + 197.0), Vec2::new(54.0, 22.0)), "kb_oct_s", "Keyboard octave shift (affects MIDI input and the on-screen keys)");
        ui.painter().text(Pos2::new(x1 + 172.0, c.top() + 208.0), Align2::LEFT_CENTER, "SEMI", mono(9.5), DIM);
        spinner(ui, setter, &p.kb_transpose, Rect::from_min_size(Pos2::new(x1 + 206.0, c.top() + 197.0), Vec2::new(54.0, 22.0)), "kb_semi_s", "Transpose in semitones (affects MIDI input and the on-screen keys)");
    }
    // key lock
    let kt = c.top() + 250.0;
    ui.painter().text(Pos2::new(x0, kt - 14.0), Align2::LEFT_CENTER, "KEY LOCK", mono(10.5), WHITE);
    ui.painter().text(Pos2::new(x0, kt + 11.0), Align2::LEFT_CENTER, "SCALE", mono(9.5), DIM);
    enum_combo(ui, setter, &p.key_scale, "key_scale", Rect::from_min_size(Pos2::new(x0 + 48.0, kt), Vec2::new(170.0, 24.0)));
    ui.painter().text(Pos2::new(x0, kt + 40.0), Align2::LEFT_CENTER, "ROOT", mono(9.5), DIM);
    for (k, nm) in ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"].iter().enumerate() {
        if pill(ui, Rect::from_min_size(Pos2::new(x0 + 48.0 + 26.0 * k as f32, kt + 29.0), Vec2::new(24.0, 22.0)), nm, p.key_root.value() == k as i32) {
            set_i(setter, &p.key_root, k as i32);
        }
    }
    ui.painter().text(Pos2::new(x0, kt + 66.0), Align2::LEFT_TOP, "Key lock moves every incoming note to the nearest note of the scale,\nso wrong keys can not happen (also feeds the arpeggiator and clips).", mono(8.5), DIM);
    ui.painter().text(Pos2::new(x1, c.top() + 232.0), Align2::LEFT_TOP, "Transpose / octave: the OCT and SEMI spinners above.\nMod wheel = CC1, sustain = CC64; aftertouch and\npitch bend are always on.\nCtrl+Z / Ctrl+Y: undo / redo.\nRight-click a modulation ring: remove it.\nMIDI-learnable: every knob, slider and the OCT / SEMI\nspinners (switch on MIDI LEARN, click it, move a CC).", mono(8.5), DIM);
    // ---------------- MIDI bindings list
    let x2 = c.left() + 704.0;
    head(ui, x2, "MIDI BINDINGS (learned CCs)");
    let txt = p.cc_map.lock().map(|g| g.clone()).unwrap_or_default();
    let entries: Vec<(u8, String)> = txt.split(';').filter_map(|s| s.split_once('=').and_then(|(c, id)| c.trim().parse::<u8>().ok().map(|c| (c, id.trim().to_string())))).collect();
    if st.pmap.is_empty() {
        st.pmap = p.param_map().into_iter().map(|(id, ptr, _)| (id, ptr)).collect();
    }
    if entries.is_empty() {
        ui.painter().text(Pos2::new(x2, c.top() + 40.0), Align2::LEFT_TOP, "Nothing bound yet.\nSwitch on MIDI LEARN in the header, click a knob,\nthen move a control on your keyboard.", mono(9.0), DIM);
    }
    let mut remove: Option<String> = None;
    for (k, (cc, id)) in entries.iter().take(28).enumerate() {
        let (colx, row) = (x2 + 340.0 * (k / 14) as f32, k % 14);
        let y = c.top() + 24.0 + 21.0 * row as f32;
        let name = st.pmap.iter().find(|(pid, _)| pid == id).map(|(_, ptr)| unsafe { ptr.name().to_string() }).unwrap_or_else(|| id.clone());
        let name: String = if name.chars().count() > 30 { name.chars().take(29).collect::<String>() + "~" } else { name };
        ui.painter().text(Pos2::new(colx, y + 9.0), Align2::LEFT_CENTER, format!("CC {cc:<3} -> {name}"), mono(9.5), WHITE);
        if pill_tip(ui, Rect::from_min_size(Pos2::new(colx + 300.0, y), Vec2::new(22.0, 19.0)), "x", false, "Remove this binding") {
            remove = Some(id.clone());
        }
    }
    if entries.len() > 28 {
        ui.painter().text(Pos2::new(x2, c.top() + 24.0 + 21.0 * 14.0 + 8.0), Align2::LEFT_CENTER, format!("+ {} more (CLEAR in the header removes all)", entries.len() - 28), mono(9.0), DIM);
    }
    if let Some(id) = remove {
        let rest: Vec<String> = entries.iter().filter(|(_, i)| *i != id).map(|(c, i)| format!("{c}={i}")).collect();
        if let Ok(mut g) = p.cc_map.lock() {
            *g = rest.join(";");
        }
        crate::rebuild_cc_map(p, meters);
    }
}

// ===================================================================== LFO tab

const LFO_PRESETS: [&str; 10] = ["Sine", "Triangle", "Saw up", "Saw down", "Square", "Pulse 25%", "Stairs up", "Bounce", "Exp decay", "Random steps"];

fn lfo_preset(k: usize) -> LfoCurve {
    let mut pts = [0.0f32; LFO_POINTS];
    let n = LFO_POINTS as f32;
    let mut seed = 0x1234_5678u32;
    for (i, p) in pts.iter_mut().enumerate() {
        let t = i as f32 / n;
        *p = match k % LFO_PRESETS.len() {
            0 => (t * std::f32::consts::TAU).sin(),
            1 => 1.0 - 4.0 * (t - 0.5).abs(),
            2 => t * 2.0 - 1.0,
            3 => 1.0 - t * 2.0,
            4 => if t < 0.5 { 1.0 } else { -1.0 },
            5 => if t < 0.25 { 1.0 } else { -1.0 },
            6 => ((t * 8.0).floor() / 7.0) * 2.0 - 1.0,
            7 => (t * 3.0 * std::f32::consts::PI * 2.0).sin().abs() * 2.0 - 1.0,
            8 => (-t * 5.0).exp() * 2.0 - 1.0,
            _ => {
                if i % 8 == 0 {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                }
                ((seed >> 8) as f32 / 8388608.0) - 1.0
            }
        };
    }
    if k % LFO_PRESETS.len() == 9 {
        // hold each random value for 8 points
        let mut seed = 0x9E37_79B9u32;
        let mut cur = 0.0f32;
        for (i, p) in pts.iter_mut().enumerate() {
            if i % 8 == 0 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                cur = ((seed >> 9) as f32 / 4194304.0) - 1.0;
            }
            *p = cur;
        }
    }
    LfoCurve { pts }
}

fn lfo_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, c: Rect, page: usize) {
    let w = (c.width() - 30.0) / 4.0;
    for i in page * 4..page * 4 + 4 {
        let l = &p.lfo[i];
        let x = c.left() + (w + 10.0) * (i % 4) as f32;
        let lnow = ui.input(|i| i.time);
        source_chip(ui, setter, p, st, Rect::from_min_size(Pos2::new(x, c.top() - 1.0), Vec2::new(48.0, 24.0)), &format!("LFO {}", i + 1), lfo_src(i), "Drag this onto any knob", lnow);
        enum_combo(ui, setter, &l.shape, &format!("lfoshape{i}"), Rect::from_min_size(Pos2::new(x + 52.0, c.top() - 1.0), Vec2::new(w - 52.0, 24.0)));
        let d = Rect::from_min_size(Pos2::new(x, c.top() + 28.0), Vec2::new(w, 92.0));
        let shape = l.shape.value();
        let curve = shared.lfo[i].get();
        if shape == LfoShape::Custom {
            let resp = ui.interact(d, egui::Id::new(("lfodraw", i)), Sense::click_and_drag()).on_hover_text("Draw your own LFO shape");
            if resp.dragged() || resp.clicked() {
                if let Some(pt) = resp.interact_pointer_pos() {
                    let mut cv = LfoCurve { pts: curve.as_ref().map(|c| c.pts).unwrap_or_else(|| LfoCurve::default_curve().pts) };
                    let idx = (((pt.x - d.left()) / d.width()) * LFO_POINTS as f32).clamp(0.0, (LFO_POINTS - 1) as f32);
                    let v = ((d.center().y - pt.y) / (d.height() * 0.42)).clamp(-1.0, 1.0);
                    let gsteps = [0usize, 4, 8, 16, 32][st.lfo_grid[i] as usize % 5];
                    if gsteps > 0 {
                        // stair-step drawing: a whole grid cell gets one value
                        let cell_w = LFO_POINTS / gsteps;
                        let c0 = (idx as usize / cell_w) * cell_w;
                        for k in c0..(c0 + cell_w).min(LFO_POINTS) {
                            cv.pts[k] = v;
                        }
                    } else {
                        let last: Option<(f32, f32)> = ui.data(|dd| dd.get_temp(egui::Id::new(("lfolast", i))));
                        let (a, va) = if resp.drag_started() { (idx, v) } else { last.unwrap_or((idx, v)) };
                        let (lo, hi, vl, vh) = if a <= idx { (a, idx, va, v) } else { (idx, a, v, va) };
                        for k in (lo as usize)..=(hi as usize).min(LFO_POINTS - 1) {
                            let f = if hi - lo < 0.5 { 1.0 } else { (k as f32 - lo) / (hi - lo) };
                            cv.pts[k] = vl + (vh - vl) * f;
                        }
                        ui.data_mut(|dd| dd.insert_temp(egui::Id::new(("lfolast", i)), (idx, v)));
                    }
                    if let Ok(mut g) = l.draw.lock() {
                        *g = cv.encode();
                    }
                    shared.lfo[i].set(Some(Arc::new(cv)));
                }
            }
            if resp.drag_stopped() {
                ui.data_mut(|dd| dd.remove::<(f32, f32)>(egui::Id::new(("lfolast", i))));
            }
        }
        dark_box(ui, d);
        let painter = ui.painter().with_clip_rect(d.shrink(1.0));
        painter.line_segment([Pos2::new(d.left(), d.center().y), Pos2::new(d.right(), d.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
        let n = 160;
        let cv = curve.as_deref();
        let pts: Vec<Pos2> = (0..=n)
            .map(|k| {
                let ph = 2.0 * k as f64 / n as f64;
                let v = lfo_eval_c(shape, ph + 0.0001, 0x51ED + i as u32 * 977, cv);
                Pos2::new(d.left() + 4.0 + (d.width() - 8.0) * k as f32 / n as f32, d.center().y - v * (d.height() * 0.40))
            })
            .collect();
        painter.add(egui::Shape::line(pts, PathStroke::new(1.6_f32, WHITE)));
        if shape == LfoShape::Custom {
            let gl = ["GRID off", "GRID 4", "GRID 8", "GRID 16", "GRID 32"][st.lfo_grid[i] as usize % 5];
            if pill_tip(ui, Rect::from_min_size(Pos2::new(d.right() - 144.0, d.top() + 3.0), Vec2::new(66.0, 16.0)), gl, st.lfo_grid[i] != 0, "Grid: draw in steps (the shape becomes a step sequencer). Click to change the number of steps.") {
                st.lfo_grid[i] = (st.lfo_grid[i] + 1) % 5;
            }
            if pill_tip(ui, Rect::from_min_size(Pos2::new(d.right() - 74.0, d.top() + 3.0), Vec2::new(70.0, 16.0)), "SHAPES >", false, "Fill the drawing with a ready-made shape (click again for the next one)") {
                st.lfo_pre[i] = (st.lfo_pre[i] + 1) % LFO_PRESETS.len() as u8;
                let cv = lfo_preset(st.lfo_pre[i] as usize);
                if let Ok(mut g) = l.draw.lock() {
                    *g = cv.encode();
                }
                shared.lfo[i].set(Some(Arc::new(cv)));
            }
        }
        let ky = c.top() + 128.0;
        let kw = w / 3.0;
        if l.sync.value() {
            ui.painter().text(Pos2::new(x + kw * 0.5, ky + 7.0), Align2::CENTER_CENTER, "RATE", mono(9.5), DIM);
            enum_combo(ui, setter, &l.div, &format!("lfodiv{i}"), Rect::from_min_size(Pos2::new(x, ky + 18.0), Vec2::new(kw + 4.0, 24.0)));
        } else {
            knob_s(ui, setter, &l.rate, "RATE", cell(x, ky, kw), "LFO rate", false);
        }
        knob_s(ui, setter, &l.rise, "RISE", cell(x + kw, ky, kw), "Fade-in time (needs TRIG)", false);
        knob_s(ui, setter, &l.smooth, "SMOOTH", cell(x + kw * 2.0, ky, kw), "Smooths the LFO output", false);
        let ky2 = ky + 74.0;
        knob_s(ui, setter, &l.delay, "DELAY", cell(x, ky2, kw), "Wait this long after the note starts before the LFO begins (needs TRIG)", false);
        knob_s(ui, setter, &l.phase, "PHASE", cell(x + kw, ky2, kw), "Start phase of the LFO", false);
        let pw = kw - 2.0;
        let px = x + kw * 2.0 + 1.0;
        if pill_tip(ui, Rect::from_min_size(Pos2::new(px, ky2 + 4.0), Vec2::new(pw, 18.0)), "SYNC", l.sync.value(), "Lock the rate to the host tempo") {
            set_b(setter, &l.sync, !l.sync.value());
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(px, ky2 + 24.0), Vec2::new(pw, 18.0)), "TRIG", l.trig.value(), "Restart the LFO on every note (off = free running, shared by all notes)") {
            set_b(setter, &l.trig, !l.trig.value());
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(px, ky2 + 44.0), Vec2::new(pw, 18.0)), "1-SHOT", l.oneshot.value(), "Play a single cycle and hold the end value (needs TRIG)") {
            set_b(setter, &l.oneshot, !l.oneshot.value());
        }
    }
}

// ===================================================================== MATRIX tab

fn matrix_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, c: Rect) {
    let hy = c.top() + 4.0;
    let _ = &st;
    let colw = (c.width() - 24.0) / 2.0;
    for col in 0..2 {
        let cl = c.left() + (colw + 24.0) * col as f32;
        for (x, t) in [(30.0, "SOURCE"), (176.0, "DESTINATION"), (332.0, "AMOUNT"), (448.0, "CURVE"), (520.0, "UNI"), (556.0, "SCALED BY")] {
            ui.painter().text(Pos2::new(cl + x, hy), Align2::LEFT_CENTER, t, mono(9.5), DIM);
        }
    }
    let rh = ((c.height() - 24.0) / 8.0).min(64.0).floor();
    for r16 in 0..16 {
        let col = r16 / 8;
        let r = r16 % 8;
        let c = Rect::from_min_size(Pos2::new(c.left() + (colw + 24.0) * col as f32, c.top()), Vec2::new(colw, c.height()));
        let i = r16;
        let s = &p.slot[i];
        let y0 = c.top() + 14.0 + rh * r as f32;
        let y = y0 + ((rh - 26.0) * 0.5).floor();
        let active = s.src.value() != ModSrc::None && s.dst.value() != ModDst::None;
        ui.painter().rect_filled(Rect::from_min_size(Pos2::new(c.left() - 3.0, y0), Vec2::new(c.width() + 6.0, rh - 4.0)), CornerRadius::same(4), if active { Color32::from_rgb(22, 22, 22) } else { Color32::from_rgb(14, 14, 14) });
        if pill_tip(ui, Rect::from_min_size(Pos2::new(c.left(), y + 1.0), Vec2::new(24.0, 21.0)), &format!("{}", i + 1), s.on.value(), "Slot on / bypassed (click to toggle). Bypassed slots keep their settings but make no sound change.") {
            set_b(setter, &s.on, !s.on.value());
        }
        enum_combo(ui, setter, &s.src, &format!("msrc{i}"), Rect::from_min_size(Pos2::new(c.left() + 26.0, y), Vec2::new(116.0, 23.0)));
        ui.painter().text(Pos2::new(c.left() + 154.0, y + 12.0), Align2::CENTER_CENTER, "->", mono(11.0), if active { WHITE } else { DIM });
        enum_combo(ui, setter, &s.dst, &format!("mdst{i}"), Rect::from_min_size(Pos2::new(c.left() + 168.0, y), Vec2::new(150.0, 23.0)));
        h_slider(ui, setter, &s.amount, Rect::from_min_size(Pos2::new(c.left() + 328.0, y + 5.0), Vec2::new(70.0, 14.0)), &format!("mamt{i}"), true, "Modulation amount (double-click to reset)");
        ui.painter().text(Pos2::new(c.left() + 404.0, y + 12.0), Align2::LEFT_CENTER, format!("{:+.0}%", s.amount.value() * 100.0), mono(9.5), if active { WHITE } else { DIM });
        h_slider(ui, setter, &s.curve, Rect::from_min_size(Pos2::new(c.left() + 448.0, y + 5.0), Vec2::new(60.0, 14.0)), &format!("mcur{i}"), true, "Response curve: left = fast start, right = slow start (double-click to reset)");
        if pill_tip(ui, Rect::from_min_size(Pos2::new(c.left() + 518.0, y + 1.0), Vec2::new(30.0, 21.0)), "UNI", s.uni.value(), "Unipolar: the source only pushes one way (LFO 0..1 instead of -1..1)") {
            set_b(setter, &s.uni, !s.uni.value());
        }
        enum_combo(ui, setter, &s.aux, &format!("maux{i}"), Rect::from_min_size(Pos2::new(c.left() + 554.0, y), Vec2::new(78.0, 23.0)));
        if i > 0 && pill_tip(ui, Rect::from_min_size(Pos2::new(c.right() - 56.0, y + 1.0), Vec2::new(16.0, 21.0)), "^", false, "Move this slot up") {
            swap_slots(setter, p, i, i - 1);
        }
        if i + 1 < MOD_SLOTS && pill_tip(ui, Rect::from_min_size(Pos2::new(c.right() - 38.0, y + 1.0), Vec2::new(16.0, 21.0)), "v", false, "Move this slot down") {
            swap_slots(setter, p, i, i + 1);
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(c.right() - 22.0, y + 1.0), Vec2::new(22.0, 21.0)), "x", false, "Clear this slot") {
            set_e(setter, &s.src, ModSrc::None);
            set_e(setter, &s.dst, ModDst::None);
            set_f(setter, &s.amount, 0.0);
            set_e(setter, &s.aux, ModSrc::None);
            set_f(setter, &s.curve, 0.0);
            set_b(setter, &s.uni, false);
            set_b(setter, &s.on, true);
        }
    }
}

/// Swap two matrix rows (reorder).
fn swap_slots(setter: &ParamSetter, p: &SynthParams, a: usize, b: usize) {
    let (x, y) = (&p.slot[a], &p.slot[b]);
    let (xs, xd, xa, xx, xc, xu, xo) = (x.src.value(), x.dst.value(), x.amount.value(), x.aux.value(), x.curve.value(), x.uni.value(), x.on.value());
    let (ys, yd, ya, yx, yc, yu, yo) = (y.src.value(), y.dst.value(), y.amount.value(), y.aux.value(), y.curve.value(), y.uni.value(), y.on.value());
    set_e(setter, &x.src, ys);
    set_e(setter, &x.dst, yd);
    set_f(setter, &x.amount, ya);
    set_e(setter, &x.aux, yx);
    set_f(setter, &x.curve, yc);
    set_b(setter, &x.uni, yu);
    set_b(setter, &x.on, yo);
    set_e(setter, &y.src, xs);
    set_e(setter, &y.dst, xd);
    set_f(setter, &y.amount, xa);
    set_e(setter, &y.aux, xx);
    set_f(setter, &y.curve, xc);
    set_b(setter, &y.uni, xu);
    set_b(setter, &y.on, xo);
}

// ===================================================================== ARP tab

fn arp_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, meters: &Meters, c: Rect) {
    let x0 = c.left() + 4.0;
    let on = p.arp_on.value();
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x0, c.top() + 2.0), Vec2::new(70.0, 26.0)), if on { "ARP ON" } else { "ARP OFF" }, on, "Arpeggiator: held notes are played one after another, in time with the host tempo") {
        set_b(setter, &p.arp_on, !on);
    }
    ui.painter().text(Pos2::new(x0 + 84.0, c.top() + 15.0), Align2::LEFT_CENTER, "MODE", mono(10.0), DIM);
    enum_combo(ui, setter, &p.arp_mode, "arp_mode", Rect::from_min_size(Pos2::new(x0 + 126.0, c.top() + 2.0), Vec2::new(120.0, 26.0)));
    ui.painter().text(Pos2::new(x0 + 262.0, c.top() + 15.0), Align2::LEFT_CENTER, "RATE", mono(10.0), DIM);
    enum_combo(ui, setter, &p.arp_div, "arp_div", Rect::from_min_size(Pos2::new(x0 + 304.0, c.top() + 2.0), Vec2::new(86.0, 26.0)));
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 404.0, c.top() + 2.0), Vec2::new(70.0, 26.0)), "HOLD", p.arp_hold.value(), "Keep playing after the keys are released (latch). A new chord replaces the old one.") {
        set_b(setter, &p.arp_hold, !p.arp_hold.value());
    }
    let ky = c.top() + 44.0;
    let w = 78.0;
    knob_s(ui, setter, &p.arp_oct, "OCTAVES", cell(x0, ky, w), "How many octaves the pattern climbs", false);
    knob_s(ui, setter, &p.arp_gate, "GATE", cell(x0 + w, ky, w), "Note length relative to the step", false);
    knob_s(ui, setter, &p.arp_swing, "SWING", cell(x0 + w * 2.0, ky, w), "Delays every second step", false);
    if p.arp_mode.value() == ArpMode::Pattern {
        let bank = p.arp_pat.lock().map(|g| g.clone()).unwrap_or_default();
        let cur = (p.arp_sel.value().clamp(1, 12) - 1) as usize;
        ui.painter().text(Pos2::new(x0 + 490.0, c.top() + 15.0), Align2::LEFT_CENTER, "BANK", mono(10.0), DIM);
        for k in 0..12usize {
            let has = bank.split('#').nth(k).map_or(false, |t| !t.trim().is_empty());
            let pr = Rect::from_min_size(Pos2::new(x0 + 536.0 + 32.0 * k as f32, c.top() + 3.0), Vec2::new(30.0, 24.0));
            if pill_tip(ui, pr, &format!("{}", k + 1), k == cur, &format!("Pattern slot {}{}", k + 1, if has { " (edited)" } else { " (default)" })) {
                set_i(setter, &p.arp_sel, k as i32 + 1);
            }
            if has && k != cur {
                ui.painter().circle_filled(Pos2::new(pr.right() - 5.0, pr.top() + 5.0), 2.0, WHITE);
            }
        }
        pattern_editor(ui, p, Rect::from_min_size(Pos2::new(x0 + 270.0, c.top() + 36.0), Vec2::new(c.right() - x0 - 280.0, 210.0)));
        ui.painter().text(Pos2::new(x0, ky + 150.0), Align2::LEFT_TOP, "PATTERN: every step plays one of the held\nnotes (NOTE 1 = lowest, wraps round the\nchord) plus TRANSPOSE semitones.\nClick a step number: on / rest.\nRight-click a number: pattern ends there.", mono(9.0), DIM);
        return;
    }
    // pattern preview: the pitch of the notes that are currently held
    let pv = Rect::from_min_size(Pos2::new(x0 + 270.0, ky), Vec2::new(c.right() - x0 - 280.0, 120.0));
    dark_box(ui, pv);
    let pa = ui.painter().with_clip_rect(pv.shrink(1.0));
    let held: Vec<u8> = (0..128u8).filter(|n| meters.is_sounding(*n)).collect();
    let div = p.arp_div.value().beats();
    let steps = 16usize;
    for i in 0..steps {
        let x = pv.left() + 8.0 + (pv.width() - 16.0) * i as f32 / steps as f32;
        pa.line_segment([Pos2::new(x, pv.top() + 4.0), Pos2::new(x, pv.bottom() - 4.0)], Stroke::new(1.0_f32, Color32::from_rgb(if i % 4 == 0 { 34 } else { 20 }, if i % 4 == 0 { 34 } else { 20 }, if i % 4 == 0 { 34 } else { 20 })));
    }
    let mode = p.arp_mode.value();
    let octs = p.arp_oct.value().clamp(1, 4) as usize;
    let mut base: Vec<u8> = if held.is_empty() { vec![60, 64, 67] } else { held.clone() };
    base.sort_unstable();
    let mut seq: Vec<u8> = base.clone();
    for o in 1..octs {
        for n in base.iter() {
            seq.push((*n as usize + 12 * o).min(127) as u8);
        }
    }
    let n = seq.len();
    let (lo, hi) = (*seq.iter().min().unwrap_or(&48) as f32 - 2.0, *seq.iter().max().unwrap_or(&72) as f32 + 2.0);
    let sw = p.arp_swing.value() * 0.5;
    let mut tpos = 0.0f32;
    for i in 0..steps {
        let idx = match mode {
            ArpMode::Up | ArpMode::AsPlayed => i % n,
            ArpMode::Down => n - 1 - (i % n),
            ArpMode::UpDown | ArpMode::DownUp => {
                let period = (2 * n).saturating_sub(2).max(1);
                let k = i % period;
                let b = if k < n { k } else { period - k };
                if mode == ArpMode::UpDown { b } else { n - 1 - b }
            }
            ArpMode::Random => (i * 7 + 3) % n,
            ArpMode::Chord | ArpMode::Pattern => 0,
        };
        let len = if i % 2 == 0 { 1.0 + sw } else { 1.0 - sw };
        let x = pv.left() + 8.0 + (pv.width() - 16.0) * tpos / steps as f32;
        let wd = (pv.width() - 16.0) / steps as f32 * len * p.arp_gate.value();
        let notes: Vec<u8> = if mode == ArpMode::Chord { seq.clone() } else { vec![seq[idx]] };
        for nt in notes {
            let yy = pv.bottom() - 8.0 - (pv.height() - 16.0) * ((nt as f32 - lo) / (hi - lo));
            pa.rect_filled(Rect::from_min_size(Pos2::new(x, yy - 3.0), Vec2::new(wd.max(2.0), 6.0)), CornerRadius::same(2), if on { WHITE } else { DIM });
        }
        tpos += len;
    }
    ui.painter().text(Pos2::new(pv.left() + 8.0, pv.top() + 10.0), Align2::LEFT_CENTER, format!("PATTERN PREVIEW  (rate {} beat)", div), mono(9.0), DIM);
    ui.painter().text(Pos2::new(x0, ky + 130.0), Align2::LEFT_CENTER, "Hold a chord on your keyboard (or the keys below): the arpeggiator plays its notes in time with the host tempo.", mono(9.5), DIM);
    ui.painter().text(Pos2::new(x0, ky + 146.0), Align2::LEFT_CENTER, "Sustain pedal also works as a latch. Turn HOLD on to keep the pattern running without holding keys.", mono(9.5), DIM);
}

/// 16-step arpeggiator pattern: on/rest, velocity, which chord note, transposition.
fn pattern_editor(ui: &mut Ui, p: &SynthParams, r: Rect) {
    use crate::arp::{encode_pattern, parse_pattern, PAT_STEPS};
    let bank_txt = p.arp_pat.lock().map(|g| g.clone()).unwrap_or_default();
    let slot = (p.arp_sel.value().clamp(1, 12) - 1) as usize;
    let txt = bank_txt.split('#').nth(slot).unwrap_or("").to_string();
    let (mut pat, mut plen) = parse_pattern(&txt);
    let mut changed = false;
    dark_box(ui, r);
    let lab_w = 62.0;
    let cw = (r.width() - lab_w - 8.0) / PAT_STEPS as f32;
    let x0 = r.left() + lab_w;
    let rows = [("STEP", 24.0), ("VELOCITY", 70.0), ("NOTE", 34.0), ("TRANSPOSE", 34.0)];
    let mut ys = [0.0f32; 4];
    let mut y = r.top() + 6.0;
    for (i, (_, h)) in rows.iter().enumerate() {
        ys[i] = y;
        y += h + 8.0;
    }
    for (i, (name, h)) in rows.iter().enumerate() {
        ui.painter().text(Pos2::new(r.left() + 8.0, ys[i] + h / 2.0), Align2::LEFT_CENTER, *name, mono(8.5), DIM);
    }
    for k in 0..PAT_STEPS {
        let live = k < plen;
        let x = x0 + cw * k as f32;
        let col_on = |a: bool| if !live { Color32::from_rgb(44, 44, 44) } else if a { WHITE } else { MID };
        // step header: toggle / length
        let hr = Rect::from_min_size(Pos2::new(x + 2.0, ys[0]), Vec2::new(cw - 4.0, 24.0));
        let hresp = ui.interact(hr, egui::Id::new(("arp_step", k)), Sense::click()).on_hover_text("Click: step on / rest.  Right-click: the pattern ends at this step.");
        if hresp.clicked() {
            pat[k].on = !pat[k].on;
            changed = true;
        }
        if hresp.secondary_clicked() {
            plen = k + 1;
            changed = true;
        }
        let pn = ui.painter();
        pn.rect_filled(hr, CornerRadius::same(3), if live && pat[k].on { Color32::from_rgb(235, 235, 235) } else { Color32::from_rgb(18, 18, 18) });
        pn.rect_stroke(hr, CornerRadius::same(3), Stroke::new(1.0_f32, if live { LINE } else { Color32::from_rgb(30, 30, 30) }), StrokeKind::Inside);
        pn.text(hr.center(), Align2::CENTER_CENTER, format!("{}", k + 1), mono(10.0), if live && pat[k].on { Color32::BLACK } else { col_on(false) });
        // velocity bar
        let vr = Rect::from_min_size(Pos2::new(x + 2.0, ys[1]), Vec2::new(cw - 4.0, 70.0));
        let vresp = ui.interact(vr, egui::Id::new(("arp_vel", k)), Sense::click_and_drag());
        if vresp.dragged() || vresp.clicked() {
            if let Some(pt) = vresp.interact_pointer_pos() {
                pat[k].vel = ((vr.bottom() - pt.y) / vr.height()).clamp(0.02, 1.0);
                changed = true;
            }
        }
        let pn = ui.painter();
        pn.rect_filled(vr, CornerRadius::same(2), Color32::from_rgb(14, 14, 14));
        let bh = vr.height() * pat[k].vel;
        pn.rect_filled(Rect::from_min_max(Pos2::new(vr.left(), vr.bottom() - bh), vr.right_bottom()), CornerRadius::same(2), if live && pat[k].on { col_on(true) } else { Color32::from_rgb(50, 50, 50) });
        // note index + transpose as small draggable number cells
        for (row, is_note) in [(2usize, true), (3usize, false)] {
            let nr = Rect::from_min_size(Pos2::new(x + 2.0, ys[row]), Vec2::new(cw - 4.0, 34.0));
            let id = egui::Id::new(("arp_num", row, k));
            let nresp = ui.interact(nr, id, Sense::click_and_drag()).on_hover_text(if is_note { "Which held note (1 = lowest). Drag up / down, double-click = 1." } else { "Extra semitones. Drag up / down, double-click = 0." });
            let akey = egui::Id::new(("arp_acc", row, k));
            if nresp.dragged() {
                let acc = ui.data(|d| d.get_temp::<f32>(akey)).unwrap_or(0.0) - nresp.drag_delta().y / 6.0;
                let whole = acc.trunc();
                ui.data_mut(|d| d.insert_temp(akey, acc - whole));
                if whole != 0.0 {
                    if is_note {
                        pat[k].idx = (pat[k].idx as i32 + whole as i32).clamp(0, 15) as u8;
                    } else {
                        pat[k].tr = (pat[k].tr as i32 + whole as i32).clamp(-24, 24) as i8;
                    }
                    changed = true;
                }
            }
            if nresp.double_clicked() {
                if is_note { pat[k].idx = 0 } else { pat[k].tr = 0 }
                changed = true;
            }
            let pn = ui.painter();
            pn.rect_filled(nr, CornerRadius::same(3), Color32::from_rgb(10, 10, 10));
            pn.rect_stroke(nr, CornerRadius::same(3), Stroke::new(1.0_f32, if nresp.hovered() { WHITE } else { Color32::from_rgb(34, 34, 34) }), StrokeKind::Inside);
            let t = if is_note { format!("{}", pat[k].idx + 1) } else { format!("{:+}", pat[k].tr).replace("+0", "0") };
            pn.text(nr.center(), Align2::CENTER_CENTER, t, mono(11.0), col_on(true));
        }
    }
    // end-of-pattern marker
    let ex = x0 + cw * plen as f32;
    ui.painter().vline(ex, r.top() + 4.0..=r.bottom() - 4.0, Stroke::new(1.5_f32, MID));
    ui.painter().text(Pos2::new(r.right() - 8.0, r.bottom() - 10.0), Align2::RIGHT_CENTER, format!("LENGTH {plen}"), mono(9.0), DIM);
    if changed {
        if let Ok(mut g) = p.arp_pat.lock() {
            let mut parts: Vec<String> = g.split('#').map(|t| t.to_string()).collect();
            parts.resize(12, String::new());
            parts[slot] = encode_pattern(&pat, plen);
            *g = parts.join("#");
        }
    }
}

// ===================================================================== bottom strip

fn strip(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, r: Rect, now: f64) {
    ui.painter().rect_filled(r, CornerRadius::same(6), PANEL);
    ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    ui.painter().text(Pos2::new(r.left() + 12.0, r.top() + 10.0), Align2::LEFT_CENTER, "MOD SOURCES  (drag onto any knob)", mono(9.5), DIM);
    let chips: [(&str, ModSrc, &str); 32] = [
        ("ENV 1", ModSrc::Env1, "Amp envelope"),
        ("ENV 2", ModSrc::Env2, "Envelope 2"),
        ("ENV 3", ModSrc::Env3, "Envelope 3"),
        ("ENV 4", ModSrc::Env4, "Envelope 4"),
        ("LFO 1", ModSrc::Lfo1, "LFO 1"),
        ("LFO 2", ModSrc::Lfo2, "LFO 2"),
        ("LFO 3", ModSrc::Lfo3, "LFO 3"),
        ("LFO 4", ModSrc::Lfo4, "LFO 4"),
        ("LFO 5", ModSrc::Lfo5, "LFO 5"),
        ("LFO 6", ModSrc::Lfo6, "LFO 6"),
        ("LFO 7", ModSrc::Lfo7, "LFO 7"),
        ("LFO 8", ModSrc::Lfo8, "LFO 8"),
        ("BEND", ModSrc::Bend, "Pitch wheel"),
        ("MACRO1", ModSrc::Macro1, "Macro knob 1"),
        ("MACRO2", ModSrc::Macro2, "Macro knob 2"),
        ("MACRO3", ModSrc::Macro3, "Macro knob 3"),
        ("MACRO4", ModSrc::Macro4, "Macro knob 4"),
        ("MACRO5", ModSrc::Macro5, "Macro knob 5"),
        ("MACRO6", ModSrc::Macro6, "Macro knob 6"),
        ("MACRO7", ModSrc::Macro7, "Macro knob 7"),
        ("MACRO8", ModSrc::Macro8, "Macro knob 8"),
        ("OSC A", ModSrc::OscA, "Output of oscillator A (cross-modulation, FM from anything)"),
        ("OSC B", ModSrc::OscB, "Output of oscillator B"),
        ("OSC C", ModSrc::OscC, "Output of oscillator C"),
        ("LANE", ModSrc::ClipLane, "Automation lane of the active clip (SEQ tab)"),
        ("FILT 1", ModSrc::Filt1, "Output of filter 1 (resonance-following, self-modulation)"),
        ("FILT 2", ModSrc::Filt2, "Output of filter 2"),
        ("VEL", ModSrc::Vel, "Note velocity"),
        ("KEY", ModSrc::Key, "Note pitch"),
        ("AFTER", ModSrc::Aftertouch, "Channel aftertouch"),
        ("WHEEL", ModSrc::ModWheel, "Mod wheel (CC1)"),
        ("RANDOM", ModSrc::Random, "Random value per note"),
    ];
    let cw = 44.0;
    for (n, (label, src, tip)) in chips.iter().enumerate() {
        let (row, col) = (n / 16, n % 16);
        let rr = Rect::from_min_size(Pos2::new(r.left() + 12.0 + (cw + 3.0) * col as f32, r.top() + 22.0 + 26.0 * row as f32), Vec2::new(cw, 22.0));
        source_chip(ui, setter, p, st, rr, label, *src, tip, now);
    }
    // macros
    let mx = r.left() + 12.0 + (cw + 3.0) * 16.0 + 8.0;
    ui.painter().vline(mx - 4.0, r.top() + 8.0..=r.bottom() - 8.0, Stroke::new(1.0_f32, LINE));
    ui.painter().text(Pos2::new(mx + 4.0, r.top() + 10.0), Align2::LEFT_CENTER, "MACROS", mono(9.5), DIM);
    for (i, m) in p.macros().iter().enumerate() {
        knob_s(ui, setter, *m, &format!("M{}", i + 1), cell(mx + 42.0 * i as f32, r.top() + 14.0, 42.0), "Macro: assign it with the MACRO chips, then use this knob to move everything at once", false);
    }
    // voice
    let vx = mx + 42.0 * 8.0 + 10.0;
    ui.painter().vline(vx - 4.0, r.top() + 8.0..=r.bottom() - 8.0, Stroke::new(1.0_f32, LINE));
    ui.painter().text(Pos2::new(vx + 4.0, r.top() + 10.0), Align2::LEFT_CENTER, "VOICE", mono(9.5), DIM);
    enum_combo(ui, setter, &p.vmode, "vmode", Rect::from_min_size(Pos2::new(vx + 4.0, r.top() + 22.0), Vec2::new(76.0, 24.0)));
    let kw = 52.0;
    let k0 = vx + 84.0;
    knob_s(ui, setter, &p.glide, "GLIDE", cell(k0, r.top() + 14.0, kw), "Portamento time", false);
    knob_s(ui, setter, &p.bend, "BEND", cell(k0 + kw, r.top() + 14.0, kw), "Pitch-bend range in semitones", false);
    knob_s(ui, setter, &p.poly, "VOICES", cell(k0 + kw * 2.0, r.top() + 14.0, kw), "Maximum number of simultaneous notes", false);
    knob_s(ui, setter, &p.tune, "TUNE", cell(k0 + kw * 3.0, r.top() + 14.0, kw), "Master tune in cents", true);
}

#[allow(clippy::too_many_arguments)]
fn source_chip(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, rr: Rect, label: &str, src: ModSrc, tip: &str, now: f64) {
    let resp = ui.interact(rr, egui::Id::new(("chip", src as usize, rr.min.x as i32, rr.min.y as i32)), Sense::click_and_drag()).on_hover_text(format!("{tip}\nDrag onto a knob to modulate it"));
    let dragging = drag_src() == Some(src as usize);
    let used = p.slot.iter().any(|s| s.src.value() == src && s.dst.value() != ModDst::None);
    let painter = ui.painter();
    painter.rect_filled(rr, CornerRadius::same(6), if dragging { WHITE } else if resp.hovered() { PANEL2 } else { Color32::TRANSPARENT });
    painter.rect_stroke(rr, CornerRadius::same(6), Stroke::new(1.0_f32, if dragging { WHITE } else if used { Color32::from_rgb(190, 190, 190) } else { LINE }), StrokeKind::Inside);
    painter.text(rr.center(), Align2::CENTER_CENTER, label, mono(10.0), if dragging { Color32::BLACK } else if used { WHITE } else { Color32::from_rgb(170, 170, 170) });
    let uses = source_uses(src);
    if uses > 0 && !dragging {
        let bc = Pos2::new(rr.right() - 5.0, rr.top() + 2.0);
        painter.circle_filled(bc, 6.0, WHITE);
        painter.text(bc, Align2::CENTER_CENTER, uses.to_string(), mono(8.5), Color32::BLACK);
    }
    if resp.drag_started() {
        set_drag(Some(src as usize));
    }
    if resp.dragged() {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let lp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("mod_ghost")));
            let g = Rect::from_center_size(pos + Vec2::new(14.0, 14.0), Vec2::new(rr.width(), 22.0));
            lp.rect_filled(g, CornerRadius::same(6), WHITE);
            lp.text(g.center(), Align2::CENTER_CENTER, label, mono(10.0), Color32::BLACK);
        }
    }
    if resp.drag_stopped() {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            if let Some((_, dst)) = drop_targets().into_iter().find(|(r, _)| r.contains(pos)) {
                let msg = assign_mod(setter, p, src as usize, dst);
                if !msg.contains("full") {
                    crate::ui::flash(dst, ui.input(|i| i.time) + 2.5);
                }
                st.say(now, format!("{msg}  | knob ring = amount (drag it), right-click = remove, list: MATRIX tab"));
            }
        }
        set_drag(None);
    }
}


// ===================================================================== on-screen keyboard

// ===================================================================== undo / redo

fn read_all(st: &Gui) -> Vec<f32> {
    // SAFETY: the pointers come from `params`, which outlives the editor.
    st.pmap.iter().map(|(_, ptr)| unsafe { ptr.unmodulated_normalized_value() }).collect()
}

/// Called every frame: commits a history step once a change has settled and the mouse is up.
fn undo_track(ui: &mut Ui, p: &SynthParams, st: &mut Gui) {
    let _ = p;
    if st.pmap.is_empty() {
        return;
    }
    let cur = read_all(st);
    if st.snap.len() != cur.len() {
        st.snap = cur.clone();
        st.prev_cur = cur;
        return;
    }
    let held = ui.input(|i| i.pointer.any_down());
    let settled = cur == st.prev_cur;
    if settled && !held && cur != st.snap {
        let old = std::mem::replace(&mut st.snap, cur.clone());
        st.undo.push(old);
        if st.undo.len() > 200 {
            st.undo.remove(0);
        }
        st.redo.clear();
    }
    st.prev_cur = cur;
}

fn undo_redo(setter: &ParamSetter, st: &mut Gui, undo: bool) {
    let target = if undo { st.undo.pop() } else { st.redo.pop() };
    let Some(target) = target else { return };
    let now = read_all(st);
    if target.len() != st.pmap.len() {
        return;
    }
    if undo {
        st.redo.push(now);
    } else {
        st.undo.push(now);
    }
    for ((_, ptr), v) in st.pmap.iter().zip(target.iter()) {
        // SAFETY: valid pointers into `params`; the GUI thread is the only caller.
        unsafe {
            if ptr.unmodulated_normalized_value() != *v {
                setter.raw_context.raw_begin_set_parameter(*ptr);
                setter.raw_context.raw_set_parameter_normalized(*ptr, *v);
                setter.raw_context.raw_end_set_parameter(*ptr);
            }
        }
    }
    st.snap = target.clone();
    st.prev_cur = target;
}

// ===================================================================== spectrum

fn update_spectrum(st: &mut Gui, meters: &Meters) {
    use crate::spectrum::*;
    let snap = meters.spec_snapshot();
    let new = magnitudes_db(&snap);
    st.spec_fs = meters.fs.load(std::sync::atomic::Ordering::Relaxed).max(8000) as f32;
    if st.spec.len() != new.len() {
        st.spec = vec![-120.0; new.len()];
    }
    for (s, n) in st.spec.iter_mut().zip(new.iter()) {
        // rises at once, falls slowly
        *s = if *n > *s { *n } else { (*s - 2.5).max(*n).max(-120.0) };
    }
}

/// Filled log-frequency spectrum inside `r` (20 Hz .. 20 kHz, `floor`..0 dB).
pub fn draw_spectrum(painter: &egui::Painter, r: Rect, spec: &[f32], fs: f32, floor: f32, col: Color32) {
    if spec.is_empty() {
        return;
    }
    let w = r.width().floor() as usize;
    let bins = spec.len();
    let hz_per_bin = fs / (2.0 * bins as f32);
    for x in 0..w {
        let f0 = 20.0 * (1000.0f32).powf(x as f32 / w as f32);
        let f1 = 20.0 * (1000.0f32).powf((x + 1) as f32 / w as f32);
        let (b0, b1) = ((f0 / hz_per_bin) as usize, ((f1 / hz_per_bin).ceil() as usize).max((f0 / hz_per_bin) as usize + 1));
        let mut m = -120.0f32;
        if b1 - b0 <= 1 {
            // narrower than one bin: interpolate so the low end is not blocky
            let pos = 0.5 * (f0 + f1) / hz_per_bin;
            let i = (pos as usize).min(bins - 2);
            let fr = (pos - i as f32).clamp(0.0, 1.0);
            m = spec[i] + (spec[i + 1] - spec[i]) * fr;
        } else {
            for b in b0.min(bins - 1)..b1.min(bins) {
                m = m.max(spec[b]);
            }
        }
        let h = ((m - floor) / -floor).clamp(0.0, 1.0) * r.height();
        if h > 0.5 {
            let px = r.left() + x as f32 + 0.5;
            painter.line_segment([Pos2::new(px, r.bottom()), Pos2::new(px, r.bottom() - h)], Stroke::new(1.0_f32, col));
        }
    }
}

// ===================================================================== fonts

/// Extra fonts so names and notes can be typed in any language and with symbols: DejaVu Sans is built in
/// (Latin, Cyrillic, Greek, Hebrew, Arabic, many symbols and dingbats such as crosses and stars), and a CJK /
/// symbol font of the system is added when one exists.
fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontFamily};
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("dejavu".into(), std::sync::Arc::new(FontData::from_static(include_bytes!("fonts/DejaVuSans.ttf"))));
    let mut extra: Vec<String> = vec!["dejavu".into()];
    let sys = [
        ("C:\\Windows\\Fonts\\seguisym.ttf", "sys_sym"),
        ("C:\\Windows\\Fonts\\msyh.ttc", "sys_cjk1"),
        ("C:\\Windows\\Fonts\\YuGothM.ttc", "sys_cjk2"),
        ("C:\\Windows\\Fonts\\malgun.ttf", "sys_cjk3"),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", "sys_cjk4"),
        ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", "sys_cjk5"),
        ("/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc", "sys_cjk6"),
        ("/usr/share/fonts/TTF/NotoSansCJK-Regular.ttc", "sys_cjk7"),
        ("/System/Library/Fonts/PingFang.ttc", "sys_cjk8"),
        ("/System/Library/Fonts/Apple Symbols.ttf", "sys_sym2"),
    ];
    for (path, key) in sys {
        if let Ok(bytes) = std::fs::read(path) {
            if bytes.len() < 64 * 1024 * 1024 {
                fonts.font_data.insert(key.into(), std::sync::Arc::new(FontData::from_owned(bytes)));
                extra.push(key.into());
            }
        }
    }
    for fam in [FontFamily::Monospace, FontFamily::Proportional] {
        if let Some(list) = fonts.families.get_mut(&fam) {
            // after the primary font, before the emoji fall-backs
            let at = list.len().min(2);
            for (k, name) in extra.iter().enumerate() {
                list.insert(at + k, name.clone());
            }
        }
    }
    ctx.set_fonts(fonts);
}

// ===================================================================== MIDI learn

fn learn_begin(p: &SynthParams, st: &mut Gui) {
    if st.pmap.is_empty() {
        st.pmap = p.param_map().into_iter().map(|(id, ptr, _)| (id, ptr)).collect();
    }
    let txt = p.cc_map.lock().map(|g| g.clone()).unwrap_or_default();
    let mut bound = Vec::new();
    for part in txt.split(';') {
        if let Some((c, id)) = part.split_once('=') {
            if let (Ok(c), Some((_, ptr))) = (c.trim().parse::<u8>(), st.pmap.iter().find(|(pid, _)| pid == id.trim())) {
                bound.push((*ptr, c));
            }
        }
    }
    crate::ui::set_learn(st.learn_on, st.learn_target, bound);
}

fn learn_end(p: &SynthParams, st: &mut Gui, meters: &Meters, now: f64) {
    let (click, unbind) = crate::ui::take_learn_events();
    let id_of = |st: &Gui, ptr| st.pmap.iter().find(|(_, q)| *q == ptr).map(|(id, _)| id.clone());
    let write = |entries: Vec<(u8, String)>| {
        if let Ok(mut g) = p.cc_map.lock() {
            *g = entries.iter().map(|(c, id)| format!("{c}={id}")).collect::<Vec<_>>().join(";");
        }
        crate::rebuild_cc_map(p, meters);
    };
    let entries = |p: &SynthParams| -> Vec<(u8, String)> {
        let txt = p.cc_map.lock().map(|g| g.clone()).unwrap_or_default();
        txt.split(';').filter_map(|s| s.split_once('=').and_then(|(c, id)| c.trim().parse::<u8>().ok().map(|c| (c, id.trim().to_string())))).collect()
    };
    if let Some(ptr) = click {
        st.learn_target = Some(ptr);
    }
    if let Some(ptr) = unbind {
        if let Some(id) = id_of(st, ptr) {
            let mut e = entries(p);
            e.retain(|(_, i)| *i != id);
            write(e);
            st.say(now, "MIDI binding removed".to_string());
        }
    }
    let lc = meters.last_cc.load(std::sync::atomic::Ordering::Relaxed);
    if lc != st.last_cc_seen {
        st.last_cc_seen = lc;
        let cc = (lc & 255) as u8;
        if st.learn_on && cc != 64 && cc < 120 {
            if let Some(ptr) = st.learn_target.take() {
                if let Some(id) = id_of(st, ptr) {
                    let mut e = entries(p);
                    e.retain(|(_, i)| *i != id);
                    e.push((cc, id));
                    write(e);
                    st.say(now, format!("CC {cc} bound"));
                }
            }
        }
    }
}

// ===================================================================== transpose + wheels

/// Small number spinner: click the upper / lower half, drag, mouse wheel, double-click = reset.
fn spinner(ui: &mut Ui, setter: &ParamSetter, prm: &IntParam, r: Rect, id: &str, tip: &str) {
    let resp = ui.interact(r, egui::Id::new(id), Sense::click_and_drag()).on_hover_text(tip);
    let v = prm.value();
    let mut nv = v;
    let learning = crate::ui::learn_widget(ui, prm.as_ptr(), &resp, r);
    if learning {
        // MIDI learn mode: clicks pick the parameter instead of changing it
    } else if resp.clicked() {
        if let Some(pt) = resp.interact_pointer_pos() {
            nv = if pt.y < r.center().y { v + 1 } else { v - 1 };
        }
    }
    if !learning && resp.dragged() {
        let key = egui::Id::new(("spin_acc", id));
        let acc = ui.data(|d| d.get_temp::<f32>(key)).unwrap_or(0.0) - resp.drag_delta().y / 9.0;
        let whole = acc.trunc();
        ui.data_mut(|d| d.insert_temp(key, acc - whole));
        nv = v + whole as i32;
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<f32>(egui::Id::new(("spin_acc", id))));
    }
    if !learning && resp.hovered() {
        let sc = ui.input(|i| i.smooth_scroll_delta.y);
        if sc != 0.0 {
            let key = egui::Id::new(("spin_wh", id));
            let acc = ui.data(|d| d.get_temp::<f32>(key)).unwrap_or(0.0) + sc / 30.0;
            let whole = acc.trunc();
            ui.data_mut(|d| d.insert_temp(key, acc - whole));
            nv = v + whole as i32;
        }
    }
    if !learning && resp.double_clicked() {
        nv = 0;
    }
    if nv != v {
        setter.begin_set_parameter(prm);
        setter.set_parameter(prm, nv);
        setter.end_set_parameter(prm);
    }
    let pn = ui.painter();
    pn.rect_filled(r, CornerRadius::same(4), Color32::from_rgb(8, 8, 8));
    pn.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, if resp.hovered() { WHITE } else { LINE }), StrokeKind::Inside);
    pn.text(Pos2::new(r.left() + 16.0, r.center().y), Align2::CENTER_CENTER, format!("{:+}", prm.value()).replace("+0", "0"), mono(11.0), WHITE);
    let ax = r.right() - 8.0;
    pn.add(egui::Shape::convex_polygon(vec![Pos2::new(ax - 3.5, r.center().y - 2.0), Pos2::new(ax + 3.5, r.center().y - 2.0), Pos2::new(ax, r.center().y - 6.0)], DIM, Stroke::NONE));
    pn.add(egui::Shape::convex_polygon(vec![Pos2::new(ax - 3.5, r.center().y + 2.0), Pos2::new(ax + 3.5, r.center().y + 2.0), Pos2::new(ax, r.center().y + 6.0)], DIM, Stroke::NONE));
}

/// One vertical wheel. `pos` is the displayed position 0..1 (bottom..top); dragging writes 0..1 into `out`.
fn wheel(ui: &mut Ui, r: Rect, id: &str, pos: f32, out: &std::sync::atomic::AtomicU32, spring: Option<f32>, tip: &str) {
    let resp = ui.interact(r, egui::Id::new(id), Sense::click_and_drag()).on_hover_text(tip);
    if resp.dragged() || resp.drag_started() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let v = ((r.bottom() - 4.0 - pt.y) / (r.height() - 8.0)).clamp(0.0, 1.0);
            out.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
    if resp.drag_stopped() {
        if let Some(c) = spring {
            out.store(c.to_bits(), std::sync::atomic::Ordering::Relaxed);
        }
    }
    if resp.double_clicked() {
        out.store(spring.unwrap_or(0.0).to_bits(), std::sync::atomic::Ordering::Relaxed);
    }
    let pn = ui.painter();
    pn.rect_filled(r, CornerRadius::same(4), Color32::from_rgb(8, 8, 8));
    pn.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, if resp.hovered() || resp.dragged() { WHITE } else { LINE }), StrokeKind::Inside);
    // ribs
    for k in 0..9 {
        let y = r.top() + 6.0 + (r.height() - 12.0) * k as f32 / 8.0;
        pn.line_segment([Pos2::new(r.left() + 3.0, y), Pos2::new(r.right() - 3.0, y)], Stroke::new(1.0_f32, Color32::from_rgb(26, 26, 26)));
    }
    let y = r.bottom() - 4.0 - (r.height() - 8.0) * pos.clamp(0.0, 1.0);
    // level fill from the rest position (bottom for the mod wheel, centre for the pitch wheel) up to the handle
    let y0 = r.bottom() - 4.0 - (r.height() - 8.0) * spring.unwrap_or(0.0);
    let fill = Rect::from_x_y_ranges((r.left() + 3.0)..=(r.right() - 3.0), y.min(y0)..=y.max(y0));
    pn.rect_filled(fill, CornerRadius::same(1), Color32::from_rgb(70, 70, 70));
    pn.rect_filled(Rect::from_center_size(Pos2::new(r.center().x, y), Vec2::new(r.width() - 4.0, 6.0)), CornerRadius::same(2), WHITE);
}

fn wheels_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, meters: &Meters, r: Rect) {
    let lab = |ui: &mut Ui, x: f32, y: f32, t: &str| {
        ui.painter().text(Pos2::new(x, y), Align2::LEFT_CENTER, t, mono(8.5), DIM);
    };
    lab(ui, r.left(), r.top() + 11.0, "OCT");
    spinner(ui, setter, &p.kb_octave, Rect::from_min_size(Pos2::new(r.left() + 28.0, r.top() + 1.0), Vec2::new(44.0, 22.0)), "kb_oct", "Keyboard octave shift (click, drag or use the mouse wheel; double-click = 0)");
    lab(ui, r.left(), r.top() + 39.0, "SEMI");
    spinner(ui, setter, &p.kb_transpose, Rect::from_min_size(Pos2::new(r.left() + 28.0, r.top() + 29.0), Vec2::new(44.0, 22.0)), "kb_semi", "Transpose in semitones (affects MIDI input and the on-screen keys)");
    let bend = f32::from_bits(meters.bend_val.load(std::sync::atomic::Ordering::Relaxed));
    let modw = f32::from_bits(meters.mod_val.load(std::sync::atomic::Ordering::Relaxed));
    let wh = r.height() - 12.0;
    let semis = bend * p.bend.value() as f32;
    let bend_txt = if semis.abs() >= 9.95 { format!("{semis:+.0}") } else { format!("{semis:+.1}") };
    let mod_txt = format!("{}", (modw * 127.0).round() as i32);
    let br = Rect::from_min_size(Pos2::new(r.left() + 78.0, r.top()), Vec2::new(18.0, wh));
    let mr = Rect::from_min_size(Pos2::new(r.left() + 100.0, r.top()), Vec2::new(18.0, wh));
    wheel(ui, br, "wheel_bend", bend * 0.5 + 0.5, &meters.gui_bend, Some(0.5), &format!("Pitch wheel: {bend_txt} semitones (range {} st, set by BEND).\nFollows your keyboard's pitch bend, springs back. Double-click = centre.", p.bend.value()));
    wheel(ui, mr, "wheel_mod", modw, &meters.gui_mod, None, &format!("Mod wheel: {mod_txt} / 127.\nFollows CC 1 of your keyboard. It does nothing by itself: drag the WHEEL chip (bottom strip) onto any knob.\nDouble-click = 0."));
    let tcol = |active: bool| if active { WHITE } else { DIM };
    ui.painter().text(Pos2::new(br.center().x, r.bottom() - 4.0), Align2::CENTER_CENTER, &bend_txt, mono(8.0), tcol(semis.abs() > 0.05));
    ui.painter().text(Pos2::new(mr.center().x, r.bottom() - 4.0), Align2::CENTER_CENTER, &mod_txt, mono(8.0), tcol(modw > 0.004));
}

fn keyboard(ui: &mut Ui, meters: &Meters, r: Rect) {
    const FIRST: u8 = 36; // C2
    const LAST: u8 = 96; // C7
    let is_black = |n: u8| matches!(n % 12, 1 | 3 | 6 | 8 | 10);
    let whites: Vec<u8> = (FIRST..=LAST).filter(|n| !is_black(*n)).collect();
    let ww = r.width() / whites.len() as f32;
    let bh = r.height() * 0.62;
    let white_rect = |i: usize| Rect::from_min_size(Pos2::new(r.left() + ww * i as f32, r.top()), Vec2::new(ww, r.height()));
    let key_rect = |n: u8| -> Rect {
        if is_black(n) {
            let wi = whites.iter().position(|w| *w == n - 1).unwrap_or(0);
            Rect::from_min_size(Pos2::new(r.left() + ww * (wi as f32 + 1.0) - ww * 0.3, r.top()), Vec2::new(ww * 0.6, bh))
        } else {
            let wi = whites.iter().position(|w| *w == n).unwrap_or(0);
            white_rect(wi)
        }
    };
    let resp = ui.interact(r, egui::Id::new("onscreen_keys"), Sense::click_and_drag()).on_hover_text("Click / drag to play notes (C2 - C7)");
    let key_at = |pt: Pos2| -> Option<u8> {
        if !r.contains(pt) {
            return None;
        }
        (FIRST..=LAST).filter(|n| is_black(*n)).find(|n| key_rect(*n).contains(pt)).or_else(|| (FIRST..=LAST).filter(|n| !is_black(*n)).find(|n| key_rect(*n).contains(pt)))
    };
    let id = egui::Id::new("kb_cur");
    let cur: Option<u8> = ui.data(|d| d.get_temp(id));
    let down = ui.input(|i| i.pointer.primary_down() && i.pointer.has_pointer() && i.pointer.hover_pos().is_some());
    let want: Option<u8> = if down && (resp.dragged() || resp.is_pointer_button_down_on()) { ui.input(|i| i.pointer.latest_pos()).and_then(key_at) } else { None };
    if want != cur {
        if let Some(c) = cur {
            meters.gui_keys[c as usize].store(false, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some(w) = want {
            meters.gui_keys[w as usize].store(true, std::sync::atomic::Ordering::Relaxed);
        }
        ui.data_mut(|d| match want {
            Some(w) => d.insert_temp(id, w),
            None => d.remove::<u8>(id),
        });
    }
    let painter = ui.painter();
    painter.rect_filled(r.expand(2.0), CornerRadius::same(4), Color32::from_rgb(4, 4, 4));
    for (i, n) in whites.iter().enumerate() {
        let kr = white_rect(i).shrink2(Vec2::new(0.5, 0.0));
        let on = meters.is_sounding(*n) || meters.gui_keys[*n as usize].load(std::sync::atomic::Ordering::Relaxed);
        painter.rect_filled(kr, CornerRadius { nw: 0, ne: 0, sw: 3, se: 3 }, if on { Color32::from_rgb(120, 120, 120) } else { Color32::from_rgb(216, 216, 216) });
        if n % 12 == 0 {
            painter.text(Pos2::new(kr.center().x, kr.bottom() - 7.0), Align2::CENTER_CENTER, format!("C{}", *n as i32 / 12 - 1), mono(8.5), Color32::from_rgb(60, 60, 60));
        }
    }
    for n in (FIRST..=LAST).filter(|n| is_black(*n)) {
        let kr = key_rect(n);
        let on = meters.is_sounding(n) || meters.gui_keys[n as usize].load(std::sync::atomic::Ordering::Relaxed);
        painter.rect_filled(kr, CornerRadius { nw: 0, ne: 0, sw: 2, se: 2 }, if on { Color32::from_rgb(150, 150, 150) } else { Color32::from_rgb(14, 14, 14) });
        painter.rect_stroke(kr, CornerRadius { nw: 0, ne: 0, sw: 2, se: 2 }, Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 60)), StrokeKind::Inside);
    }
}


// ===================================================================== convenience tools

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
    fn idx(&mut self, n: usize) -> usize {
        ((self.next() * n as f32) as usize).min(n - 1)
    }
    fn chance(&mut self, p: f32) -> bool {
        self.next() < p
    }
}

fn rnd_enum<E: Enum + PartialEq + 'static>(setter: &ParamSetter, p: &EnumParam<E>, i: usize) {
    set_e(setter, p, E::from_index(i.min(E::variants().len() - 1)));
}

/// "Dice" button: a new, musical random patch.
fn randomize(setter: &ParamSetter, p: &SynthParams, shared: &Shared, seed: u64) {
    let mut r = Rng(seed | 1);
    for _ in 0..4 {
        r.next();
    }
    // start from a clean Init so nothing from the previous sound (arp, effects, mod routes...) lingers
    let master = p.master.value();
    presets::apply_values(setter, p, &[]);
    set_f(setter, &p.master, master);
    for o in 0..3 {
        let op = osc(p, o);
        let on = o == 0 || r.chance(if o == 1 { 0.55 } else { 0.3 });
        set_b(setter, &op.on, on);
        use_builtin(setter, p, shared, o, r.idx(TABLE_COUNT_UI));
        set_norm(setter, &op.pos, r.next());
        let wm = if r.chance(0.5) { 0 } else { 1 + r.idx(6) };
        rnd_enum(setter, &op.warp_mode, wm);
        set_norm(setter, &op.warp, r.range(0.0, 0.7));
        rnd_enum(setter, &op.warp2_mode, 0);
        set_norm(setter, &op.level, 0.7);
        let uni = if r.chance(0.5) { 1 } else { 2 + r.idx(6) } as f32;
        setter.begin_set_parameter(&op.uni);
        setter.set_parameter(&op.uni, uni as i32);
        setter.end_set_parameter(&op.uni);
        set_norm(setter, &op.detune, r.range(0.1, 0.5));
        setter.begin_set_parameter(&op.oct);
        setter.set_parameter(&op.oct, if o >= 1 && r.chance(0.4) { -1 } else { 0 });
        setter.end_set_parameter(&op.oct);
        let semis = [0, 0, 0, 0, 7, 12, 5, 3, -5];
        setter.begin_set_parameter(&op.semi);
        setter.set_parameter(&op.semi, if o >= 1 && r.chance(0.35) { semis[r.idx(semis.len())] } else { 0 });
        setter.end_set_parameter(&op.semi);
        set_norm(setter, &op.fine, r.range(0.45, 0.55));
        rnd_enum(setter, &op.uni_mode, r.idx(4));
        set_norm(setter, &op.spread, r.range(0.3, 1.0));
        set_norm(setter, &op.pan, if o >= 1 { r.range(0.4, 0.6) } else { 0.5 });
        if r.chance(0.3) {
            rnd_enum(setter, &op.warp2_mode, 1 + r.idx(6));
            set_norm(setter, &op.warp2, r.range(0.05, 0.6));
        }
    }
    set_b(setter, &p.sub_on, r.chance(0.3));
    set_b(setter, &p.f1.fl_on, true);
    rnd_enum(setter, &p.f1.fl_type, r.idx(6));
    set_norm(setter, &p.f1.cutoff, r.range(0.35, 0.9));
    set_norm(setter, &p.f1.res, r.range(0.05, 0.45));
    set_norm(setter, &p.f1.drive, r.range(0.0, 0.3));
    for e in [&p.e1, &p.e2] {
        set_norm(setter, &e.attack, r.range(0.0, 0.45) * r.range(0.3, 1.0));
        set_norm(setter, &e.decay, r.range(0.35, 0.8));
        set_norm(setter, &e.sustain, r.range(0.2, 1.0));
        set_norm(setter, &e.release, r.range(0.3, 0.75));
    }
    set_norm(setter, &p.e2.sustain, r.range(0.0, 0.5));
    // ---- envelope 3, macros, sub and noise get a say as well
    for e in [&p.e3, &p.e4] {
        set_norm(setter, &e.attack, r.range(0.0, 0.5) * r.range(0.2, 1.0));
        set_norm(setter, &e.decay, r.range(0.3, 0.85));
        set_norm(setter, &e.sustain, r.range(0.0, 0.9));
        set_norm(setter, &e.release, r.range(0.3, 0.8));
    }
    for m in p.macros() {
        set_norm(setter, m, r.range(0.0, 1.0));
    }
    set_norm(setter, &p.f1.keytrack, r.range(0.0, 0.8));
    rnd_enum(setter, &p.sub_wave, r.idx(4));
    set_norm(setter, &p.sub_level, r.range(0.2, 0.6));
    setter.begin_set_parameter(&p.sub_oct);
    setter.set_parameter(&p.sub_oct, if r.chance(0.7) { -1 } else { -2 });
    setter.end_set_parameter(&p.sub_oct);
    let noise = r.chance(0.2);
    set_norm(setter, &p.noise_level, if noise { r.range(0.05, 0.3) } else { 0.0 });
    rnd_enum(setter, &p.noise_type, r.idx(5));

    // ---- four LFOs: every shape (except the drawn one), speed, delay, fade-in and sync
    let shapes = [0usize, 1, 2, 3, 4, 5, 6, 8, 9];
    for l in p.lfo.iter() {
        rnd_enum(setter, &l.shape, shapes[r.idx(shapes.len())]);
        set_norm(setter, &l.rate, r.range(0.1, 0.85));
        let sync = r.chance(0.35);
        set_b(setter, &l.sync, sync);
        rnd_enum(setter, &l.div, r.idx(10));
        set_b(setter, &l.trig, r.chance(0.3));
        set_norm(setter, &l.rise, if r.chance(0.35) { r.range(0.1, 0.6) } else { 0.0 });
        set_norm(setter, &l.smooth, if r.chance(0.3) { r.range(0.1, 0.5) } else { 0.0 });
        set_norm(setter, &l.delay, if r.chance(0.2) { r.range(0.1, 0.4) } else { 0.0 });
        set_norm(setter, &l.phase, r.next());
    }

    // ---- modulation matrix: a random number of routes with random sources and targets
    for sl in p.slot.iter() {
        set_e(setter, &sl.src, ModSrc::None);
        set_e(setter, &sl.dst, ModDst::None);
        set_f(setter, &sl.amount, 0.0);
    }
    use ModDst as D;
    use ModSrc as S;
    let srcs = [S::Env1, S::Env2, S::Env3, S::Env4, S::Lfo1, S::Lfo2, S::Lfo3, S::Lfo4, S::Lfo5, S::Lfo6, S::Vel, S::Key, S::ModWheel, S::Random, S::Macro1, S::Macro2, S::Macro3, S::Macro4, S::Macro5, S::Macro6];
    let dsts = [
        D::APos, D::BPos, D::AWarp, D::BWarp, D::AWarp2, D::BWarp2, D::Cutoff, D::Cutoff, D::Res, D::Drive, D::Detune, D::APan, D::BPan, D::ABlend, D::BBlend, D::FilterMix, D::FxHyperMix, D::FxDistDrive,
        D::FxChorusMix, D::FxFlangerDepth, D::FxPhaserFreq, D::FxDelayMix, D::FxReverbMix, D::FxFilterCut, D::APitch, D::BPitch, D::CPos, D::CWarp,
    ];
    let routes = 3 + r.idx(8); // 3..10
    let mut used: Vec<(usize, usize)> = Vec::new();
    for k in 0..routes.min(p.slot.len()) {
        let (mut si, mut di) = (r.idx(srcs.len()), r.idx(dsts.len()));
        if k == 0 && r.chance(0.7) {
            si = if r.chance(0.5) { 1 } else { r.idx(3) }; // usually an envelope on the filter
            di = 6;
        }
        let pitch = matches!(dsts[di], D::APitch | D::BPitch);
        if used.contains(&(si, di)) {
            continue;
        }
        used.push((si, di));
        let sign = if r.chance(0.3) && !matches!(dsts[di], D::Cutoff | D::FxFilterCut) { -1.0 } else { 1.0 };
        let amt = if pitch { r.range(0.02, 0.12) } else { r.range(0.12, 0.65) };
        set_e(setter, &p.slot[k].src, srcs[si]);
        set_e(setter, &p.slot[k].dst, dsts[di]);
        set_f(setter, &p.slot[k].amount, sign * amt);
    }

    // the arpeggiator is part of the sound too: a dice roll must not inherit the previous one
    set_b(setter, &p.arp_on, r.chance(0.12));
    set_norm(setter, &p.glide, 0.0);

    // ---- effects: reset, then switch a random handful on with random amounts
    for b in [&p.fx.hy_on, &p.fx.ds_on, &p.fx.fg_on, &p.fx.ph_on, &p.fx.ch_on, &p.fx.dl_on, &p.fx.cp_on, &p.fx.rv_on] {
        set_b(setter, b, false);
    }
    set_b(setter, &p.fx.rv_on, r.chance(0.7));
    set_norm(setter, &p.fx.rv_mix, r.range(0.1, 0.45));
    set_b(setter, &p.fx.dl_on, r.chance(0.5));
    set_norm(setter, &p.fx.dl_mix, r.range(0.1, 0.4));
    set_b(setter, &p.fx.ch_on, r.chance(0.4));
    set_norm(setter, &p.fx.ch_mix, r.range(0.15, 0.5));
    set_b(setter, &p.fx.hy_on, r.chance(0.25));
    set_norm(setter, &p.fx.hy_mix, r.range(0.1, 0.5));
    set_b(setter, &p.fx.ds_on, r.chance(0.2));
    set_norm(setter, &p.fx.ds_mix, r.range(0.15, 0.5));
    set_b(setter, &p.fx.fg_on, r.chance(0.15));
    set_norm(setter, &p.fx.fg_mix, r.range(0.15, 0.5));
    set_b(setter, &p.fx.ph_on, r.chance(0.18));
    set_norm(setter, &p.fx.ph_mix, r.range(0.15, 0.5));
    set_b(setter, &p.fx.cp_on, r.chance(0.3));
    if let Ok(mut g) = p.preset_name.lock() {
        *g = "Random".to_string();
    }
}

const TABLE_COUNT_UI: usize = crate::wavetable::TABLE_COUNT;

/// Copies every setting of one oscillator to the other.
fn copy_osc(setter: &ParamSetter, p: &SynthParams, shared: &Shared, from: usize, to: usize) {
    // everything: mode, tuning, sample / multisample / granular / spectral settings, wavetable (the target keeps its ON state)
    let a = osc(p, from);
    let vals: Vec<f32> = a.param_map().iter().map(|(_, ptr, _)| unsafe { ptr.unmodulated_normalized_value() }).collect();
    let w = a.wave.lock().map(|g| g.clone()).unwrap_or_default();
    let sm = a.smp.lock().map(|g| g.clone()).unwrap_or_default();
    let ms = a.msmp.lock().map(|g| g.clone()).unwrap_or_default();
    let on = osc(p, to).on.value();
    apply_osc_clip(setter, p, shared, to, (vals, w, format!("{sm}\u{1}{ms}")));
    set_b(setter, &osc(p, to).on, on);
}


/// Developer aid (NYASHK_AUTOTEST=1): plays a chord on every preset and logs the loudness to /tmp/autotest.log.
fn autotest(setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, meters: &Meters, now: f64) {
    let filter = std::env::var("NYASHK_AUTOTEST").unwrap_or_default();
    if filter != "1" && !filter.is_empty() {
        if let Some((i, t0, _, log)) = st.auto.as_mut() {
            if *t0 < 0.0 {
                while *i < st.presets.len() && !st.presets[*i].name.to_lowercase().contains(&filter.to_lowercase()) {
                    *i += 1;
                }
                if *i >= st.presets.len() {
                    let _ = std::fs::write("/tmp/autotest.log", log.as_bytes());
                    std::process::exit(0);
                }
            }
        }
    }
    let Some((i, t0, peak, log)) = st.auto.as_mut() else { return };
    if *t0 < 0.0 {
        *t0 = now;
        let r = st.presets.get(*i).cloned();
        if let Some(r) = r {
            let _ = presets::load(setter, p, shared, &r);
        }
        return;
    }
    let t = now - *t0;
    if t > 0.35 && t < 1.9 {
        for n in [48usize, 55, 60] {
            meters.gui_keys[n].store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    if t > 0.4 && t < 1.8 {
        *peak = peak.max(meters.peak());
        if meters.voices() > 0 {
            *peak = peak.max(0.0001);
        }
    }
    if t > 1.9 {
        for n in [48usize, 55, 60] {
            meters.gui_keys[n].store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }
    if t > 2.8 {
        let name = st.presets.get(*i).map(|r| format!("{:<26} {:<7}", r.name, r.cat)).unwrap_or_default();
        log.push_str(&format!("{name} peak {:.3} voices {}\n", *peak, meters.voices()));
        *peak = 0.0;
        *i += 1;
        *t0 = -1.0;
        if *i >= st.presets.len() {
            let _ = std::fs::write("/tmp/autotest.log", log.as_bytes());
            std::process::exit(0);
        }
    }
}

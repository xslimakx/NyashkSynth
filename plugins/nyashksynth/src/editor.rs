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

pub const BASE_W: f32 = 1100.0;
pub const BASE_H: f32 = 870.0;

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
    pub tab: usize,
    pub fx_sel: usize,
    pub fx_drag: Option<usize>,
    pub mat_page: usize,
    pub overlay: Overlay,
    presets: Vec<PresetRef>,
    favs: Vec<String>,
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
    pub status: String,
    pub status_until: f64,
    auto: Option<(usize, f64, f32, String)>,
}

// ---------------------------------------------------------------- data helpers

fn osc(p: &SynthParams, o: usize) -> &OscParams {
    if o == 0 {
        &p.a
    } else {
        &p.b
    }
}

fn scan_tables() -> Vec<(BItem, TblSrc)> {
    let mut v: Vec<(BItem, TblSrc)> = NAMES.iter().enumerate().map(|(i, n)| (BItem { name: n.to_string(), cat: "Factory".into(), fav: false, user: false }, TblSrc::Builtin(i))).collect();
    if let Some(d) = files::tables_dir() {
        for e in files::scan(&d, &["wav"]) {
            v.push((BItem { name: e.name, cat: e.cat, fav: false, user: true }, TblSrc::File(e.path)));
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

fn assign_mod(setter: &ParamSetter, p: &SynthParams, src_idx: usize, dst_idx: usize) -> String {
    let src = <ModSrc as Enum>::from_index(src_idx);
    let dst = <ModDst as Enum>::from_index(dst_idx);
    // already routed? just make it audible
    for s in p.slot.iter() {
        if s.src.value() == src && s.dst.value() == dst {
            if s.amount.value().abs() < 0.01 {
                set_f(setter, &s.amount, 0.5);
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
            return format!("{} -> {}", ModSrc::variants()[src_idx], ModDst::variants()[dst_idx]);
        }
    }
    "modulation matrix is full (16 slots)".to_string()
}

impl Gui {
    fn new() -> Gui {
        files::ensure_dirs();
        Gui {
            tab: std::env::var("NYASHK_TAB").ok().and_then(|s| s.parse().ok()).unwrap_or(0),
            fx_sel: 0,
            fx_drag: None,
            mat_page: 0,
            overlay: match std::env::var("NYASHK_OVERLAY").ok().as_deref() {
                Some("presets") => Overlay::Presets,
                Some("tables") => Overlay::Tables(0),
                Some("noises") => Overlay::Noises,
                Some("edit") => Overlay::Edit,
                _ => Overlay::None,
            },
            presets: presets::list_all(),
            favs: presets::load_favs(),
            pb: BState::default(),
            save_name: String::new(),
            save_cat: String::new(),
            last_scan: 0.0,
            tables: scan_tables(),
            tbs: BState::default(),
            frame_size: 0,
            noises: files::noises_dir().map(|d| files::scan(&d, &["wav"])).unwrap_or_default(),
            nbs: BState::default(),
            edit: None,
            status: String::new(),
            status_until: 0.0,
            auto: if std::env::var("NYASHK_AUTOTEST").is_ok() { Some((0, -1.0, 0.0, String::new())) } else { None },
        }
    }

    fn say(&mut self, now: f64, t: impl Into<String>) {
        self.status = t.into();
        self.status_until = now + 4.0;
    }

    fn rescan(&mut self) {
        self.presets = presets::list_all();
        self.tables = scan_tables();
        self.noises = files::noises_dir().map(|d| files::scan(&d, &["wav"])).unwrap_or_default();
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
        },
        move |ctx, setter, st| {
            ctx.request_repaint_after(Duration::from_millis(40));
            let p = &*params;
            meters.gui_beat.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            begin_frame(&params2, meters.live_sources());
            egui::CentralPanel::default().frame(egui::Frame::NONE.fill(BG)).show(ctx, |ui| {
                let full = ui.max_rect();
                let o = full.min;
                let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(o.x + x, o.y + y), Vec2::new(w, h));
                let painter = ui.painter().clone();
                let now = ui.input(|i| i.time);
                if st.overlay == Overlay::Edit && st.edit.is_none() {
                    st.open_edit(&shared, p, 0);
                }

                autotest(setter, p, &shared, st, &meters, now);
                header(ui, setter, p, &shared, st, &meters, o, now);

                // ================================================= row 1: oscillators + filter
                osc_panel(ui, setter, p, &shared, st, rect(14.0, 62.0, 350.0, 346.0), 0);
                osc_panel(ui, setter, p, &shared, st, rect(372.0, 62.0, 350.0, 346.0), 1);
                filter_panel(ui, setter, p, rect(730.0, 62.0, 356.0, 346.0));

                // ================================================= row 2: sub/noise + tabs
                sub_noise_panel(ui, setter, p, &shared, st, rect(14.0, 416.0, 350.0, 300.0), now);
                let rp = rect(372.0, 416.0, 714.0, 300.0);
                painter.rect_filled(rp, CornerRadius::same(6), PANEL);
                painter.rect_stroke(rp, CornerRadius::same(6), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
                for (i, name) in ["ENV", "LFO", "MATRIX", "FX", "ARP"].iter().enumerate() {
                    if pill(ui, Rect::from_min_size(Pos2::new(rp.left() + 12.0 + 84.0 * i as f32, rp.top() + 8.0), Vec2::new(78.0, 24.0)), name, st.tab == i) {
                        st.tab = i;
                    }
                }
                let arp_on = p.arp_on.value();
                if arp_on && st.tab != 4 {
                    painter.circle_filled(Pos2::new(rp.left() + 12.0 + 84.0 * 4.0 + 66.0, rp.top() + 14.0), 3.0, WHITE);
                }
                let content = Rect::from_min_size(Pos2::new(rp.left() + 10.0, rp.top() + 40.0), Vec2::new(rp.width() - 20.0, rp.height() - 46.0));
                match st.tab {
                    0 => env_tab(ui, setter, p, content),
                    1 => lfo_tab(ui, setter, p, &shared, content),
                    2 => matrix_tab(ui, setter, p, st, content),
                    3 => fxui::fx_tab(ui, setter, p, st, content),
                    _ => arp_tab(ui, setter, p, &meters, content),
                }

                // ================================================= bottom strip: sources, macros, voice
                strip(ui, setter, p, st, rect(14.0, 724.0, 1072.0, 76.0), now);
                keyboard(ui, &meters, rect(14.0, 808.0, 1072.0, 50.0));

                // ================================================= overlays
                overlays(ui, setter, p, &shared, st, o, now);

                // resize grip (keeps proportions)
                let grip = Rect::from_min_size(full.max - Vec2::splat(22.0), Vec2::splat(22.0));
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
    if pill_tip(ui, nb, &label, false, "Open the preset browser (categories, search, favorites)") {
        st.overlay = if st.overlay == Overlay::Presets { Overlay::None } else { Overlay::Presets };
        st.rescan();
    }
    if pill_tip(ui, rect(562.0, 14.0, 28.0, 26.0), ">", false, "Next preset") {
        pick = Some(cur_idx.map(|i| (i + 1) % n).unwrap_or(0));
    }
    let dir_txt = files::presets_dir().map(|d| d.display().to_string()).unwrap_or_default();
    if pill_tip(ui, rect(598.0, 14.0, 54.0, 26.0), "SAVE", false, &format!("Save the current sound as a preset.\nPresets are plain files in:\n{dir_txt}")) {
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
        if let Some(r) = st.presets.get(i).cloned() {
            if let Err(e) = presets::load(setter, p, shared, &r) {
                st.say(now, e);
            }
        }
    }
    if now < st.status_until {
        let g = painter.layout_no_wrap(st.status.clone(), mono(11.0), WHITE);
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
    painter.text(Pos2::new(o.x + 962.0, o.y + 27.0), Align2::LEFT_CENTER, "MASTER", mono(10.0), DIM);
    h_slider(ui, setter, &p.master, rect(1022.0, 20.0, 64.0, 14.0), "master_slider", false, "Master volume");
}

// ===================================================================== overlays

fn overlays(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, o: Pos2, now: f64) {
    let area = Rect::from_min_size(Pos2::new(o.x + 14.0, o.y + 62.0), Vec2::new(1072.0, 738.0));
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
        .map(|r| BItem { name: r.name.clone(), cat: r.cat.clone(), fav: st.favs.contains(&r.key()), user: r.is_user() })
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
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 210.0, y), Vec2::new(116.0, 24.0)), "OPEN FOLDER", false, "Open the preset folder in your file manager. Drop .nyp files (or whole category folders) in there.") {
        if let Some(d) = files::presets_dir() {
            files::open_folder(&d);
        }
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(f.right() - 88.0, y), Vec2::new(88.0, 24.0)), "RESCAN", false, "Look for new files now") {
        st.rescan();
        st.say(now, "folders rescanned");
    }
    ui.painter().text(Pos2::new(f.left(), y + 40.0), Align2::LEFT_CENTER, "Click a preset to load it. Up / Down keys step through the list. The star marks favorites.", mono(9.5), DIM);
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
        BAct::Close => st.overlay = Overlay::None,
        BAct::None => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn table_overlay(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, st: &mut Gui, area: Rect, oi: usize, now: f64) {
    let items: Vec<BItem> = st.tables.iter().map(|(b, _)| BItem { name: b.name.clone(), cat: b.cat.clone(), fav: false, user: b.user }).collect();
    let cur = wave_name(p, oi);
    let sel = st.tables.iter().position(|(b, _)| b.name == cur);
    let act = browse::browser(ui, area, &format!("WAVETABLES  -  OSC {}", if oi == 0 { "A" } else { "B" }), &items, sel, &mut st.tbs, false, "tables");
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
    let items: Vec<BItem> = st.noises.iter().map(|e| BItem { name: e.name.clone(), cat: e.cat.clone(), fav: false, user: true }).collect();
    let cur = p.noise_sample.lock().map(|g| crate::sample::encoded_name(&g)).unwrap_or_default();
    let sel = st.noises.iter().position(|e| e.name == cur);
    let act = browse::browser(ui, area, "NOISE / SAMPLES", &items, sel, &mut st.nbs, false, "noises");
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
    ui.painter().text(Pos2::new(f.left(), y + 12.0), Align2::LEFT_CENTER, "Any .wav file works (first 4 seconds are used). Pitch / key / loop are set in the NOISE section.", mono(9.5), DIM);
    match act {
        BAct::Pick(i) => {
            if let Some(e) = st.noises.get(i).cloned() {
                match load_noise(setter, p, shared, &e.path) {
                    Ok(n) => st.say(now, format!("loaded {n}")),
                    Err(e) => st.say(now, format!("could not load: {e}")),
                }
            }
        }
        BAct::Close => st.overlay = Overlay::None,
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
    if resp.drag_started() {
        setter.begin_set_parameter(p);
    }
    if resp.dragged() || resp.clicked() {
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
    if resp.drag_stopped() {
        setter.end_set_parameter(p);
    }
    if resp.double_clicked() {
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
    let name = if o == 0 { "A" } else { "B" };
    panel(ui, r, if o == 0 { "01" } else { "02" }, &format!("OSC {name}"));
    on_pill(ui, setter, &op.on, Rect::from_min_size(Pos2::new(r.right() - 56.0, r.top() + 6.0), Vec2::new(44.0, 22.0)), "Oscillator on/off");
    // wavetable display (click = editor, drag = scan)
    let disp = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 34.0), Vec2::new(196.0, 100.0));
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
    let (pos_d, wrp_d, lvl_d, pan_d, bl_d, pit_d) = if o == 0 { (ModDst::APos, ModDst::AWarp, ModDst::ALevel, ModDst::APan, ModDst::ABlend, ModDst::APitch) } else { (ModDst::BPos, ModDst::BWarp, ModDst::BLevel, ModDst::BPan, ModDst::BBlend, ModDst::BPitch) };
    let w = 62.0;
    let x0 = r.left() + 12.0;
    let y0 = r.top() + 142.0;
    let dy = 66.0;
    mdst(pos_d);
    knob_s(ui, setter, &op.pos, "POS", cell(x0, y0, w), "Wavetable position (also drag on the display)", false);
    mdst(wrp_d);
    knob_s(ui, setter, &op.warp, "WARP", cell(x0 + w, y0, w), "Warp 1 amount", false);
    mdst(if o == 0 { ModDst::AWarp2 } else { ModDst::BWarp2 });
    knob_s(ui, setter, &op.warp2, "WARP 2", cell(x0 + w * 2.0, y0, w), "Warp 2 amount (applied after warp 1)", false);
    mdst(lvl_d);
    knob_s(ui, setter, &op.level, "LEVEL", cell(x0 + w * 3.0, y0, w), "Oscillator level", false);
    mdst(pan_d);
    knob_s(ui, setter, &op.pan, "PAN", cell(x0 + w * 4.0, y0, w), "Stereo balance", true);
    let y1 = y0 + dy;
    knob_s(ui, setter, &op.oct, "OCT", cell(x0, y1, w), "Octave", false);
    knob_s(ui, setter, &op.semi, "SEMI", cell(x0 + w, y1, w), "Semitones", false);
    mdst(pit_d);
    knob_s(ui, setter, &op.fine, "FINE", cell(x0 + w * 2.0, y1, w), "Fine tune in cents (modulation: +-24 semitones)", true);
    knob_s(ui, setter, &op.uni, "UNISON", cell(x0 + w * 3.0, y1, w), "Number of unison voices", false);
    mdst(ModDst::Detune);
    knob_s(ui, setter, &op.detune, "DETUNE", cell(x0 + w * 4.0, y1, w), "Unison detune", false);
    let y2 = y1 + dy;
    mdst(bl_d);
    knob_s(ui, setter, &op.blend, "BLEND", cell(x0, y2, w), "Level of the outer unison voices (low = centre voice louder)", false);
    knob_s(ui, setter, &op.spread, "SPREAD", cell(x0 + w, y2, w), "Stereo width of the unison voices", false);
    knob_s(ui, setter, &op.phase, "PHASE", cell(x0 + w * 2.0, y2, w), "Start phase", false);
    knob_s(ui, setter, &op.rand, "RAND", cell(x0 + w * 3.0, y2, w), "Random start phase", false);
    ui.painter().text(Pos2::new(x0 + w * 4.0 + w * 0.5, y2 + 6.0), Align2::CENTER_CENTER, "UNI MODE", mono(9.0), DIM);
    enum_combo(ui, setter, &op.uni_mode, &format!("unimode{o}"), Rect::from_min_size(Pos2::new(x0 + w * 4.0 - 2.0, y2 + 16.0), Vec2::new(w + 4.0, 24.0)));
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

fn filter_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, r: Rect) {
    panel(ui, r, "03", "FILTER");
    on_pill(ui, setter, &p.fl_on, Rect::from_min_size(Pos2::new(r.right() - 56.0, r.top() + 6.0), Vec2::new(44.0, 22.0)), "Filter on/off");
    enum_combo(ui, setter, &p.fl_type, "filter_type", Rect::from_min_size(Pos2::new(r.left() + 100.0, r.top() + 5.0), Vec2::new(140.0, 24.0)));
    let d = Rect::from_min_size(Pos2::new(r.left() + 12.0, r.top() + 36.0), Vec2::new(r.width() - 24.0, 150.0));
    // drag in the graph: left / right = cutoff, up / down = resonance
    {
        let gr = ui.interact(d, egui::Id::new("filter_graph"), Sense::click_and_drag()).on_hover_text("Drag: left / right = cutoff, up / down = resonance");
        if gr.drag_started() {
            setter.begin_set_parameter(&p.cutoff);
            setter.begin_set_parameter(&p.res);
        }
        if gr.dragged() {
            if let Some(pt) = gr.interact_pointer_pos() {
                let fx = ((pt.x - d.left()) / d.width()).clamp(0.0, 1.0);
                let hz = 20.0 * (20000.0f32 / 20.0).powf(fx);
                setter.set_parameter(&p.cutoff, hz);
                setter.set_parameter_normalized(&p.res, ((d.bottom() - pt.y) / d.height()).clamp(0.0, 1.0));
            }
        }
        if gr.drag_stopped() {
            setter.end_set_parameter(&p.cutoff);
            setter.end_set_parameter(&p.res);
        }
    }
    dark_box(ui, d);
    let painter = ui.painter().with_clip_rect(d.shrink(1.0));
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |f: f32| d.left() + d.width() * ((f / lo).ln() / (hi / lo).ln());
    for f in [100.0f32, 1000.0, 10000.0] {
        painter.line_segment([Pos2::new(xf(f), d.top()), Pos2::new(xf(f), d.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 24)));
    }
    let (dbmin, dbmax) = (-48.0f32, 30.0f32);
    let yd = |db: f32| d.bottom() - d.height() * ((db.clamp(dbmin, dbmax) - dbmin) / (dbmax - dbmin));
    painter.line_segment([Pos2::new(d.left(), yd(0.0)), Pos2::new(d.right(), yd(0.0))], Stroke::new(1.0_f32, Color32::from_rgb(30, 30, 30)));
    let cut = p.cutoff.value();
    let res = p.res.value();
    let ty = p.fl_type.value();
    let on = p.fl_on.value();
    let n = 200;
    let mut pts = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let f = lo * (hi / lo).powf(i as f32 / n as f32);
        let mag = if on { filter_response(ty, res, cut, f) } else { 1.0 };
        pts.push(Pos2::new(xf(f), yd(20.0 * mag.max(1e-6).log10())));
    }
    fill_under(&painter, &pts, d.bottom(), Color32::from_rgb(30, 30, 30));
    painter.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, if on { WHITE } else { DIM })));
    painter.line_segment([Pos2::new(xf(cut), d.top()), Pos2::new(xf(cut), d.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(70, 70, 70)));
    for (f, t) in [(100.0, "100"), (1000.0, "1k"), (10000.0, "10k")] {
        painter.text(Pos2::new(xf(f) + 2.0, d.bottom() - 7.0), Align2::LEFT_CENTER, t, mono(8.5), Color32::from_rgb(90, 90, 90));
    }
    let y = r.top() + 194.0;
    let w = 66.0;
    let x0 = r.left() + 10.0;
    mdst(ModDst::Cutoff);
    knob_s(ui, setter, &p.cutoff, "CUTOFF", cell(x0, y, w), "Filter cutoff frequency", false);
    mdst(ModDst::Res);
    knob_s(ui, setter, &p.res, "RES", cell(x0 + w, y, w), "Resonance (for Peak / shelves: boost in dB, comb: feedback)", false);
    mdst(ModDst::Drive);
    knob_s(ui, setter, &p.drive, "DRIVE", cell(x0 + w * 2.0, y, w), "Pre-filter saturation", false);
    knob_s(ui, setter, &p.keytrack, "KEY", cell(x0 + w * 3.0, y, w), "Key tracking: cutoff follows the played note", false);
    mdst(ModDst::FilterMix);
    knob_s(ui, setter, &p.fl_mix, "MIX", cell(x0 + w * 4.0, y, w), "Dry / filtered mix", false);
    let ry = r.top() + 274.0;
    ui.painter().text(Pos2::new(r.left() + 14.0, ry + 11.0), Align2::LEFT_CENTER, "ROUTE", mono(10.0), DIM);
    let items: [(&str, &BoolParam, &str); 4] = [
        ("OSC A", &p.fl_a, "Send oscillator A through the filter"),
        ("OSC B", &p.fl_b, "Send oscillator B through the filter"),
        ("SUB", &p.fl_sub, "Send the sub oscillator through the filter"),
        ("NOISE", &p.fl_noise, "Send the noise through the filter"),
    ];
    for (i, (nm, bp, tip)) in items.iter().enumerate() {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(r.left() + 62.0 + 72.0 * i as f32, ry), Vec2::new(66.0, 22.0)), nm, bp.value(), tip) {
            set_b(setter, bp, !bp.value());
        }
    }
    ui.painter().text(Pos2::new(r.left() + 14.0, r.top() + 322.0), Align2::LEFT_CENTER, "Drag LFOs / envelopes onto CUTOFF to make it move", mono(9.5), DIM);
}

// ===================================================================== sub + noise

fn sub_noise_panel(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, _shared: &Shared, st: &mut Gui, r: Rect, now: f64) {
    panel(ui, r, "04", "SUB / NOISE");
    let x = r.left();
    let y = r.top();
    ui.painter().text(Pos2::new(x + 14.0, y + 44.0), Align2::LEFT_CENTER, "SUB", mono(10.5), WHITE);
    on_pill(ui, setter, &p.sub_on, Rect::from_min_size(Pos2::new(x + 52.0, y + 33.0), Vec2::new(40.0, 22.0)), "Sub oscillator on/off");
    enum_combo(ui, setter, &p.sub_wave, "sub_wave", Rect::from_min_size(Pos2::new(x + 100.0, y + 32.0), Vec2::new(100.0, 24.0)));
    knob_s(ui, setter, &p.sub_oct, "OCT", cell(x + 206.0, y + 28.0, 46.0), "Sub octave offset", false);
    mdst(ModDst::SubLevel);
    knob_s(ui, setter, &p.sub_level, "LEVEL", cell(x + 250.0, y + 28.0, 46.0), "Sub level", false);
    knob_s(ui, setter, &p.sub_pan, "PAN", cell(x + 296.0, y + 28.0, 46.0), "Sub stereo position", true);
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
        st.overlay = Overlay::Noises;
    }
    let ky = y + 134.0;
    let w = 62.0;
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
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 216.0, by), Vec2::new(60.0, 22.0)), "A>B", false, "Copy oscillator A (with its wavetable) to oscillator B") {
        copy_osc(setter, p, _shared, 0, 1);
        st.say(now, "A copied to B".to_string());
    }
    if pill_tip(ui, Rect::from_min_size(Pos2::new(x + 280.0, by), Vec2::new(60.0, 22.0)), "B>A", false, "Copy oscillator B (with its wavetable) to oscillator A") {
        copy_osc(setter, p, _shared, 1, 0);
        st.say(now, "B copied to A".to_string());
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

fn env_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, c: Rect) {
    let w = (c.width() - 20.0) / 3.0;
    let envs: [(&str, &EnvParams); 3] = [("ENV 1  (AMP)", &p.e1), ("ENV 2", &p.e2), ("ENV 3", &p.e3)];
    for (i, (name, e)) in envs.iter().enumerate() {
        let x = c.left() + (w + 10.0) * i as f32;
        ui.painter().text(Pos2::new(x + 2.0, c.top() + 6.0), Align2::LEFT_CENTER, *name, mono(10.5), WHITE);
        ui.painter().text(Pos2::new(x + w - 2.0, c.top() + 6.0), Align2::RIGHT_CENTER, "drag curve to bend", mono(8.5), DIM);
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

// ===================================================================== LFO tab

fn lfo_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, c: Rect) {
    let w = (c.width() - 30.0) / 4.0;
    for i in 0..4 {
        let l = &p.lfo[i];
        let x = c.left() + (w + 10.0) * i as f32;
        ui.painter().text(Pos2::new(x + 2.0, c.top() + 8.0), Align2::LEFT_CENTER, format!("LFO {}", i + 1), mono(10.5), WHITE);
        enum_combo(ui, setter, &l.shape, &format!("lfoshape{i}"), Rect::from_min_size(Pos2::new(x + 52.0, c.top() - 1.0), Vec2::new(w - 52.0, 24.0)));
        let d = Rect::from_min_size(Pos2::new(x, c.top() + 28.0), Vec2::new(w, 66.0));
        let shape = l.shape.value();
        let curve = shared.lfo[i].get();
        if shape == LfoShape::Custom {
            let resp = ui.interact(d, egui::Id::new(("lfodraw", i)), Sense::click_and_drag()).on_hover_text("Draw your own LFO shape");
            if resp.dragged() || resp.clicked() {
                if let Some(pt) = resp.interact_pointer_pos() {
                    let mut cv = LfoCurve { pts: curve.as_ref().map(|c| c.pts).unwrap_or_else(|| LfoCurve::default_curve().pts) };
                    let idx = (((pt.x - d.left()) / d.width()) * LFO_POINTS as f32).clamp(0.0, (LFO_POINTS - 1) as f32);
                    let v = ((d.center().y - pt.y) / (d.height() * 0.42)).clamp(-1.0, 1.0);
                    let last: Option<(f32, f32)> = ui.data(|dd| dd.get_temp(egui::Id::new(("lfolast", i))));
                    let (a, va) = if resp.drag_started() { (idx, v) } else { last.unwrap_or((idx, v)) };
                    let (lo, hi, vl, vh) = if a <= idx { (a, idx, va, v) } else { (idx, a, v, va) };
                    for k in (lo as usize)..=(hi as usize).min(LFO_POINTS - 1) {
                        let f = if hi - lo < 0.5 { 1.0 } else { (k as f32 - lo) / (hi - lo) };
                        cv.pts[k] = vl + (vh - vl) * f;
                    }
                    ui.data_mut(|dd| dd.insert_temp(egui::Id::new(("lfolast", i)), (idx, v)));
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
        let ky = c.top() + 98.0;
        let kw = w / 3.0;
        if l.sync.value() {
            ui.painter().text(Pos2::new(x + kw * 0.5, ky + 7.0), Align2::CENTER_CENTER, "RATE", mono(9.5), DIM);
            enum_combo(ui, setter, &l.div, &format!("lfodiv{i}"), Rect::from_min_size(Pos2::new(x, ky + 18.0), Vec2::new(kw + 4.0, 24.0)));
        } else {
            knob_s(ui, setter, &l.rate, "RATE", cell(x, ky, kw), "LFO rate", false);
        }
        knob_s(ui, setter, &l.rise, "RISE", cell(x + kw, ky, kw), "Fade-in time (needs TRIG)", false);
        knob_s(ui, setter, &l.smooth, "SMOOTH", cell(x + kw * 2.0, ky, kw), "Smooths the LFO output", false);
        let ky2 = ky + 70.0;
        knob_s(ui, setter, &l.delay, "DELAY", cell(x, ky2, kw), "Wait this long after the note starts before the LFO begins (needs TRIG)", false);
        knob_s(ui, setter, &l.phase, "PHASE", cell(x + kw, ky2, kw), "Start phase of the LFO", false);
        let pw = kw - 2.0;
        let px = x + kw * 2.0 + 1.0;
        if pill_tip(ui, Rect::from_min_size(Pos2::new(px, ky2 + 2.0), Vec2::new(pw, 20.0)), "SYNC", l.sync.value(), "Lock the rate to the host tempo") {
            set_b(setter, &l.sync, !l.sync.value());
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(px, ky2 + 24.0), Vec2::new(pw, 20.0)), "TRIG", l.trig.value(), "Restart the LFO on every note (off = free running, shared by all notes)") {
            set_b(setter, &l.trig, !l.trig.value());
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(px, ky2 + 46.0), Vec2::new(pw, 20.0)), "1-SHOT", l.oneshot.value(), "Play a single cycle and hold the end value (needs TRIG)") {
            set_b(setter, &l.oneshot, !l.oneshot.value());
        }
    }
}

// ===================================================================== MATRIX tab

fn matrix_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, c: Rect) {
    let rh = 27.0;
    let hy = c.top() + 4.0;
    for (x, t) in [(30.0, "SOURCE"), (176.0, "DESTINATION"), (332.0, "AMOUNT"), (448.0, "CURVE"), (520.0, "UNI"), (556.0, "SCALED BY")] {
        ui.painter().text(Pos2::new(c.left() + x, hy), Align2::LEFT_CENTER, t, mono(9.5), DIM);
    }
    for (i, t) in ["1-8", "9-16"].iter().enumerate() {
        if pill(ui, Rect::from_min_size(Pos2::new(c.right() - 122.0 + 62.0 * i as f32, c.top() - 29.0), Vec2::new(58.0, 20.0)), t, st.mat_page == i) {
            st.mat_page = i;
        }
    }
    for r in 0..8 {
        let i = st.mat_page * 8 + r;
        let s = &p.slot[i];
        let y = c.top() + 14.0 + rh * r as f32;
        let active = s.src.value() != ModSrc::None && s.dst.value() != ModDst::None;
        if active {
            ui.painter().rect_filled(Rect::from_min_size(Pos2::new(c.left(), y - 1.0), Vec2::new(c.width(), rh - 2.0)), CornerRadius::same(4), Color32::from_rgb(16, 16, 16));
        }
        ui.painter().text(Pos2::new(c.left() + 6.0, y + 12.0), Align2::LEFT_CENTER, format!("{:>2}", i + 1), mono(10.5), if active { WHITE } else { DIM });
        enum_combo(ui, setter, &s.src, &format!("msrc{i}"), Rect::from_min_size(Pos2::new(c.left() + 26.0, y), Vec2::new(116.0, 23.0)));
        ui.painter().text(Pos2::new(c.left() + 154.0, y + 12.0), Align2::CENTER_CENTER, "->", mono(11.0), if active { WHITE } else { DIM });
        enum_combo(ui, setter, &s.dst, &format!("mdst{i}"), Rect::from_min_size(Pos2::new(c.left() + 168.0, y), Vec2::new(150.0, 23.0)));
        h_slider(ui, setter, &s.amount, Rect::from_min_size(Pos2::new(c.left() + 328.0, y + 5.0), Vec2::new(70.0, 14.0)), &format!("mamt{i}"), true, "Modulation amount (double-click to reset)");
        ui.painter().text(Pos2::new(c.left() + 404.0, y + 12.0), Align2::LEFT_CENTER, format!("{:+.0}%", s.amount.value() * 100.0), mono(9.5), if active { WHITE } else { DIM });
        h_slider(ui, setter, &s.curve, Rect::from_min_size(Pos2::new(c.left() + 448.0, y + 5.0), Vec2::new(60.0, 14.0)), &format!("mcur{i}"), true, "Response curve: left = fast start, right = slow start (double-click to reset)");
        if pill_tip(ui, Rect::from_min_size(Pos2::new(c.left() + 518.0, y + 1.0), Vec2::new(30.0, 21.0)), "UNI", s.uni.value(), "Unipolar: the source only pushes one way (LFO 0..1 instead of -1..1)") {
            set_b(setter, &s.uni, !s.uni.value());
        }
        enum_combo(ui, setter, &s.aux, &format!("maux{i}"), Rect::from_min_size(Pos2::new(c.left() + 554.0, y), Vec2::new(104.0, 23.0)));
        if pill_tip(ui, Rect::from_min_size(Pos2::new(c.right() - 22.0, y + 1.0), Vec2::new(22.0, 21.0)), "x", false, "Clear this slot") {
            set_e(setter, &s.src, ModSrc::None);
            set_e(setter, &s.dst, ModDst::None);
            set_f(setter, &s.amount, 0.0);
            set_e(setter, &s.aux, ModSrc::None);
            set_f(setter, &s.curve, 0.0);
            set_b(setter, &s.uni, false);
        }
    }
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
            ArpMode::Chord => 0,
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

// ===================================================================== bottom strip

fn strip(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, r: Rect, now: f64) {
    ui.painter().rect_filled(r, CornerRadius::same(6), PANEL);
    ui.painter().rect_stroke(r, CornerRadius::same(6), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
    ui.painter().text(Pos2::new(r.left() + 12.0, r.top() + 10.0), Align2::LEFT_CENTER, "MOD SOURCES  (drag onto any knob)", mono(9.5), DIM);
    let chips: [(&str, ModSrc, &str); 17] = [
        ("ENV 1", ModSrc::Env1, "Amp envelope"),
        ("ENV 2", ModSrc::Env2, "Envelope 2"),
        ("ENV 3", ModSrc::Env3, "Envelope 3"),
        ("LFO 1", ModSrc::Lfo1, "LFO 1"),
        ("LFO 2", ModSrc::Lfo2, "LFO 2"),
        ("LFO 3", ModSrc::Lfo3, "LFO 3"),
        ("LFO 4", ModSrc::Lfo4, "LFO 4"),
        ("BEND", ModSrc::Bend, "Pitch wheel"),
        ("WHEEL", ModSrc::ModWheel, "Mod wheel (CC1)"),
        ("MACRO1", ModSrc::Macro1, "Macro knob 1"),
        ("MACRO2", ModSrc::Macro2, "Macro knob 2"),
        ("MACRO3", ModSrc::Macro3, "Macro knob 3"),
        ("MACRO4", ModSrc::Macro4, "Macro knob 4"),
        ("VEL", ModSrc::Vel, "Note velocity"),
        ("KEY", ModSrc::Key, "Note pitch"),
        ("AFTER", ModSrc::Aftertouch, "Channel aftertouch"),
        ("RANDOM", ModSrc::Random, "Random value per note"),
    ];
    let cw = 58.0;
    for (n, (label, src, tip)) in chips.iter().enumerate() {
        let (row, col) = (n / 9, n % 9);
        let rr = Rect::from_min_size(Pos2::new(r.left() + 12.0 + (cw + 4.0) * col as f32, r.top() + 22.0 + 26.0 * row as f32), Vec2::new(cw, 22.0));
        source_chip(ui, setter, p, st, rr, label, *src, tip, now);
    }
    // macros
    let mx = r.left() + 12.0 + (cw + 4.0) * 9.0 + 8.0;
    ui.painter().vline(mx - 4.0, r.top() + 8.0..=r.bottom() - 8.0, Stroke::new(1.0_f32, LINE));
    ui.painter().text(Pos2::new(mx + 4.0, r.top() + 10.0), Align2::LEFT_CENTER, "MACROS", mono(9.5), DIM);
    for (i, m) in p.macros().iter().enumerate() {
        knob_s(ui, setter, *m, &format!("M{}", i + 1), cell(mx + 48.0 * i as f32, r.top() + 14.0, 48.0), "Macro: assign it with the MACRO chips, then use this knob to move everything at once", false);
    }
    // voice
    let vx = mx + 48.0 * 4.0 + 10.0;
    ui.painter().vline(vx - 4.0, r.top() + 8.0..=r.bottom() - 8.0, Stroke::new(1.0_f32, LINE));
    ui.painter().text(Pos2::new(vx + 4.0, r.top() + 10.0), Align2::LEFT_CENTER, "VOICE", mono(9.5), DIM);
    enum_combo(ui, setter, &p.vmode, "vmode", Rect::from_min_size(Pos2::new(vx + 4.0, r.top() + 22.0), Vec2::new(84.0, 24.0)));
    knob_s(ui, setter, &p.glide, "GLIDE", cell(vx + 92.0, r.top() + 14.0, 48.0), "Portamento time", false);
    knob_s(ui, setter, &p.bend, "BEND", cell(vx + 140.0, r.top() + 14.0, 44.0), "Pitch-bend range in semitones", false);
    knob_s(ui, setter, &p.poly, "VOICES", cell(vx + 184.0, r.top() + 14.0, 48.0), "Maximum number of simultaneous notes", false);
    knob_s(ui, setter, &p.tune, "TUNE", cell(vx + 230.0, r.top() + 14.0, 44.0), "Master tune in cents", true);
}

#[allow(clippy::too_many_arguments)]
fn source_chip(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, rr: Rect, label: &str, src: ModSrc, tip: &str, now: f64) {
    let resp = ui.interact(rr, egui::Id::new(("chip", src as usize)), Sense::click_and_drag()).on_hover_text(format!("{tip}\nDrag onto a knob to modulate it"));
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
    for o in 0..2 {
        let op = osc(p, o);
        let on = o == 0 || r.chance(0.55);
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
        setter.set_parameter(&op.oct, if o == 1 && r.chance(0.4) { -1 } else { 0 });
        setter.end_set_parameter(&op.oct);
    }
    set_b(setter, &p.sub_on, r.chance(0.3));
    set_b(setter, &p.fl_on, true);
    rnd_enum(setter, &p.fl_type, r.idx(6));
    set_norm(setter, &p.cutoff, r.range(0.35, 0.9));
    set_norm(setter, &p.res, r.range(0.05, 0.45));
    set_norm(setter, &p.drive, r.range(0.0, 0.3));
    for e in [&p.e1, &p.e2] {
        set_norm(setter, &e.attack, r.range(0.0, 0.45) * r.range(0.3, 1.0));
        set_norm(setter, &e.decay, r.range(0.35, 0.8));
        set_norm(setter, &e.sustain, r.range(0.2, 1.0));
        set_norm(setter, &e.release, r.range(0.3, 0.75));
    }
    set_norm(setter, &p.e2.sustain, r.range(0.0, 0.5));
    for sl in p.slot.iter() {
        set_e(setter, &sl.src, ModSrc::None);
        set_e(setter, &sl.dst, ModDst::None);
        set_f(setter, &sl.amount, 0.0);
    }
    set_e(setter, &p.slot[0].src, ModSrc::Env2);
    set_e(setter, &p.slot[0].dst, ModDst::Cutoff);
    set_f(setter, &p.slot[0].amount, r.range(0.2, 0.7));
    let dsts = [ModDst::APos, ModDst::Cutoff, ModDst::AWarp, ModDst::Detune];
    set_e(setter, &p.lfo[0].shape, <LfoShape as Enum>::from_index(r.idx(4)));
    set_norm(setter, &p.lfo[0].rate, r.range(0.2, 0.6));
    set_e(setter, &p.slot[1].src, ModSrc::Lfo1);
    set_e(setter, &p.slot[1].dst, dsts[r.idx(dsts.len())]);
    set_f(setter, &p.slot[1].amount, r.range(0.15, 0.5));
    set_b(setter, &p.rv_on, r.chance(0.7));
    set_norm(setter, &p.rv_mix, r.range(0.1, 0.4));
    set_b(setter, &p.dl_on, r.chance(0.5));
    set_norm(setter, &p.dl_mix, r.range(0.1, 0.35));
    set_b(setter, &p.ch_on, r.chance(0.4));
    set_b(setter, &p.hy_on, r.chance(0.25));
    set_b(setter, &p.ds_on, r.chance(0.2));
    if let Ok(mut g) = p.preset_name.lock() {
        *g = "Random".to_string();
    }
}

const TABLE_COUNT_UI: usize = crate::wavetable::TABLE_COUNT;

/// Copies every setting of one oscillator to the other.
fn copy_osc(setter: &ParamSetter, p: &SynthParams, shared: &Shared, from: usize, to: usize) {
    let (a, b) = (osc(p, from), osc(p, to));
    let tbl = cur_table(shared, p, from);
    let name = wave_name(p, from);
    if a.wave.lock().map(|g| g.is_empty()).unwrap_or(true) {
        use_builtin(setter, p, shared, to, a.table.value().to_index());
    } else {
        use_custom(p, shared, to, &name, (*tbl).clone());
    }
    set_f(setter, &b.pos, a.pos.value());
    set_e(setter, &b.warp_mode, a.warp_mode.value());
    set_f(setter, &b.warp, a.warp.value());
    set_e(setter, &b.warp2_mode, a.warp2_mode.value());
    set_f(setter, &b.warp2, a.warp2.value());
    set_f(setter, &b.level, a.level.value());
    set_f(setter, &b.pan, a.pan.value());
    set_f(setter, &b.fine, a.fine.value());
    set_f(setter, &b.detune, a.detune.value());
    set_f(setter, &b.blend, a.blend.value());
    set_f(setter, &b.spread, a.spread.value());
    set_f(setter, &b.phase, a.phase.value());
    set_f(setter, &b.rand, a.rand.value());
    set_e(setter, &b.uni_mode, a.uni_mode.value());
    for (ip, v) in [(&b.oct, a.oct.value()), (&b.semi, a.semi.value()), (&b.uni, a.uni.value())] {
        setter.begin_set_parameter(ip);
        setter.set_parameter(ip, v);
        setter.end_set_parameter(ip);
    }
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

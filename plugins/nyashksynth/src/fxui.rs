//! FX rack tab: a reorderable chain of 10 modules and a detail view for the selected one.
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, epaint::PathStroke, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::editor::{cell, dark_box, fill_under, Gui};
use crate::fx::{eq_response_db, BqCoef};
use crate::params::*;
use crate::ui::*;

fn on_param(b: &FxBank, m: usize) -> &BoolParam {
    match m {
        0 => &b.hy_on,
        1 => &b.ds_on,
        2 => &b.fg_on,
        3 => &b.ph_on,
        4 => &b.ch_on,
        5 => &b.dl_on,
        6 => &b.cp_on,
        7 => &b.rv_on,
        8 => &b.eq_on,
        9 => &b.ff_on,
        10 => &b.bd_on,
        _ => &b.ut_on,
    }
}

fn kn<P: Param>(ui: &mut Ui, setter: &ParamSetter, prm: &P, label: &str, x: f32, y: f32, tip: &str) {
    knob_s(ui, setter, prm, label, cell(x, y, 62.0), tip, false);
}

pub fn fx_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, c: Rect) {
    let bus = st.fx_bus.min(1);
    let b = p.bank(bus);
    let md = |d: ModDst| {
        mdst(if bus == 0 { d } else { d.bus2() });
    };
    let order: Vec<usize> = b.order_params().iter().map(|q| (q.value().clamp(0, 11)) as usize).collect();
    let cw = (c.width() - 11.0 * 4.0) / 12.0;
    let chip = |i: usize| Rect::from_min_size(Pos2::new(c.left() + (cw + 4.0) * i as f32, c.top()), Vec2::new(cw, 30.0));
    let mut clicked: Option<usize> = None;
    let mut dropped: Option<(usize, usize)> = None;
    for slot in 0..FX_SLOTS {
        let m = order[slot];
        let r = chip(slot);
        let resp = ui.interact(r, egui::Id::new(("fxchip", bus, slot)), Sense::click_and_drag()).on_hover_text("Click: edit. Drag onto another slot: reorder the chain. Dot: on/off");
        let on = on_param(b, m).value();
        let sel = st.fx_sel == m;
        let painter = ui.painter();
        let dragging = st.fx_drag == Some(slot);
        painter.rect_filled(r, CornerRadius::same(5), if sel { WHITE } else if resp.hovered() { PANEL2 } else { Color32::from_rgb(14, 14, 14) });
        painter.rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0_f32, if dragging { WHITE } else { LINE }), StrokeKind::Inside);
        let fg = if sel { Color32::BLACK } else if on { WHITE } else { DIM };
        painter.text(Pos2::new(r.center().x, r.center().y + 2.0), Align2::CENTER_CENTER, FX_NAMES[m], mono(8.5), fg);
        painter.circle_filled(Pos2::new(r.left() + 7.0, r.top() + 6.0), 3.0, if on { if sel { Color32::BLACK } else { WHITE } } else { Color32::from_rgb(50, 50, 50) });
        let dot = Rect::from_center_size(Pos2::new(r.left() + 7.0, r.top() + 6.0), Vec2::splat(12.0));
        if resp.drag_started() {
            st.fx_drag = Some(slot);
        }
        if resp.drag_stopped() {
            if let Some(from) = st.fx_drag.take() {
                if let Some(pt) = ui.input(|i| i.pointer.latest_pos()) {
                    if let Some(to) = (0..FX_SLOTS).find(|&j| chip(j).contains(pt)) {
                        if to != from {
                            dropped = Some((from, to));
                        }
                    }
                }
            }
        } else if resp.clicked() {
            if let Some(pt) = ui.input(|i| i.pointer.interact_pos()) {
                if dot.contains(pt) {
                    set_b(setter, on_param(b, m), !on);
                } else {
                    clicked = Some(m);
                }
            }
        }
        let _ = painter;
    }
    if let Some(m) = clicked {
        st.fx_sel = m;
    }
    if let Some((from, to)) = dropped {
        let mut o = order.clone();
        let v = o.remove(from);
        o.insert(to, v);
        for (q, val) in b.order_params().iter().zip(o.iter()) {
            setter.begin_set_parameter(*q);
            setter.set_parameter(*q, *val as i32);
            setter.end_set_parameter(*q);
        }
    }
    // dragging ghost
    if let Some(from) = st.fx_drag {
        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
            let lp = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("fx_ghost")));
            let g = Rect::from_center_size(pos + Vec2::new(12.0, 12.0), Vec2::new(cw, 22.0));
            lp.rect_filled(g, CornerRadius::same(5), WHITE);
            lp.text(g.center(), Align2::CENTER_CENTER, FX_NAMES[order[from]], mono(8.5), Color32::BLACK);
        }
    }

    // ---- detail
    let m = st.fx_sel.min(11);
    let d = Rect::from_min_max(Pos2::new(c.left(), c.top() + 40.0), c.right_bottom());
    ui.painter().text(Pos2::new(d.left() + 2.0, d.top() + 8.0), Align2::LEFT_CENTER, FX_NAMES[m], mono(11.5), WHITE);
    let pos_in_chain = order.iter().position(|&x| x == m).unwrap_or(0) + 1;
    ui.painter().text(Pos2::new(d.left() + 90.0, d.top() + 8.0), Align2::LEFT_CENTER, format!("slot {pos_in_chain}/12"), mono(9.0), DIM);
    // ---- busses and routing (right side of the header line)
    {
        let bx = d.right() - 526.0;
        for k in 0..2 {
            if pill_tip(ui, Rect::from_min_size(Pos2::new(bx + 62.0 * k as f32, d.top() - 3.0), Vec2::new(58.0, 22.0)), if k == 0 { "BUS 1" } else { "BUS 2" }, bus == k, "Switch between the two effect busses (each has the full set of modules)") {
                st.fx_bus = k;
            }
        }
        ui.painter().text(Pos2::new(bx + 134.0, d.top() + 8.0), Align2::LEFT_CENTER, "ROUTING", mono(9.0), DIM);
        enum_combo(ui, setter, &p.fx_route, "fx_route", Rect::from_min_size(Pos2::new(bx + 190.0, d.top() - 3.0), Vec2::new(150.0, 22.0)));
        match p.fx_route.value() {
            FxRoute::SplitLh => {
                knob_s(ui, setter, &p.fx_split, "SPLIT", cell(bx + 346.0, d.top() + 6.0, 52.0), "Crossover: lows go to bus 1, highs to bus 2", false);
            }
            FxRoute::SplitLmh => {
                knob_s(ui, setter, &p.fx_split, "LOW/MID", cell(bx + 346.0, d.top() + 6.0, 52.0), "Low / mid crossover. Lows go to bus 1", false);
                knob_s(ui, setter, &p.fx_split2, "MID/HI", cell(bx + 402.0, d.top() + 6.0, 52.0), "Mid / high crossover. Mids go to bus 2, highs stay dry", false);
            }
            _ => {}
        }
        knob_s(ui, setter, &p.fx2_level, "BUS 2", cell(bx + 458.0, d.top() + 6.0, 52.0), "Level of bus 2 in the mix", false);
    }
    let on_p = on_param(b, m);
    if pill_tip(ui, Rect::from_min_size(Pos2::new(d.left() + 160.0, d.top() - 3.0), Vec2::new(44.0, 22.0)), if on_p.value() { "ON" } else { "OFF" }, on_p.value(), "Module on/off") {
        set_b(setter, on_p, !on_p.value());
    }
    let x0 = d.left();
    let y0 = d.top() + 22.0;
    let w = 62.0;
    let y1 = y0 + 72.0;
    let sync_pill = |ui: &mut Ui, prm: &BoolParam, x: f32, y: f32| {
        if pill_tip(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(54.0, 22.0)), "SYNC", prm.value(), "Lock to host tempo") {
            set_b(setter, prm, !prm.value());
        }
    };
    let fp = crate::fx_patch_from_bank(b, 120.0);
    match m {
        0 => {
            kn(ui, setter, &b.hy_rate, "RATE", x0, y0, "Hyper chorus speed");
            kn(ui, setter, &b.hy_detune, "DETUNE", x0 + w, y0, "Depth of the voices' pitch wobble");
            kn(ui, setter, &b.hy_uni, "UNISON", x0 + w * 2.0, y0, "Number of chorus voices");
            kn(ui, setter, &b.hy_dim, "DIMENSION", x0 + w * 3.0, y0, "Dimension-style stereo widening");
            md(ModDst::FxHyperMix);
            kn(ui, setter, &b.hy_mix, "MIX", x0 + w * 4.0, y0, "Dry / wet");
            ui.painter().text(Pos2::new(x0 + 2.0, y1 + 10.0), Align2::LEFT_CENTER, "Hyper + Dimension: Serum-style thick stereo chorus.", mono(9.5), DIM);
        }
        1 => {
            enum_combo(ui, setter, &b.ds_type, "ds_type", Rect::from_min_size(Pos2::new(x0, y0 + 4.0), Vec2::new(150.0, 24.0)));
            md(ModDst::FxDistDrive);
            kn(ui, setter, &b.ds_drive, "DRIVE", x0, y0 + 36.0, "Distortion amount");
            kn(ui, setter, &b.ds_tone, "TONE", x0 + w, y0 + 36.0, "Tilt: dark to bright");
            md(ModDst::FxDistMix);
            kn(ui, setter, &b.ds_mix, "MIX", x0 + w * 2.0, y0 + 36.0, "Dry / wet");
            kn(ui, setter, &b.ds_bias, "DC BIAS", x0 + w * 3.0, y0 + 36.0, "Offset before the shaper: adds even harmonics / asymmetry");
            // transfer curve
            let g = Rect::from_min_size(Pos2::new(x0 + 340.0, y0), Vec2::new(150.0, 120.0));
            dark_box(ui, g);
            let pa = ui.painter().with_clip_rect(g.shrink(1.0));
            pa.line_segment([Pos2::new(g.left(), g.center().y), Pos2::new(g.right(), g.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
            pa.line_segment([Pos2::new(g.center().x, g.top()), Pos2::new(g.center().x, g.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(28, 28, 28)));
            let drv = 1.0 + fp.ds_drive * 14.0;
            let pts: Vec<Pos2> = (0..=120)
                .map(|i| {
                    let x = i as f32 / 60.0 - 1.0;
                    let v = x * drv;
                    let y = match fp.ds_type {
                        DistType::Hard => v.clamp(-1.0, 1.0),
                        DistType::Fold | DistType::SinFold => (v * std::f32::consts::FRAC_PI_2).sin(),
                        DistType::Crush | DistType::Downsample => (v * 6.0).round() / 6.0,
                        DistType::Rectify => v.abs().min(1.0) * 2.0 - 1.0,
                        DistType::Asym | DistType::Diode1 | DistType::Diode2 => if v > 0.0 { v.tanh() } else { (v * 0.5).tanh() * 1.4 },
                        _ => v.tanh(),
                    };
                    Pos2::new(g.left() + g.width() * i as f32 / 120.0, g.center().y - y.clamp(-1.1, 1.1) * g.height() * 0.42)
                })
                .collect();
            pa.add(egui::Shape::line(pts, PathStroke::new(1.6_f32, WHITE)));
        }
        2 => {
            if b.fg_sync.value() {
                enum_combo(ui, setter, &b.fg_div, "fg_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
            } else {
                kn(ui, setter, &b.fg_rate, "RATE", x0, y0, "Flanger LFO speed");
            }
            md(ModDst::FxFlangerDepth);
            kn(ui, setter, &b.fg_depth, "DEPTH", x0 + w, y0, "Sweep depth");
            kn(ui, setter, &b.fg_delay, "DELAY", x0 + w * 2.0, y0, "Base delay");
            kn(ui, setter, &b.fg_fb, "FEEDBACK", x0 + w * 3.0, y0, "Feedback");
            kn(ui, setter, &b.fg_phase, "PHASE", x0 + w * 4.0, y0, "Stereo phase of the LFO");
            kn(ui, setter, &b.fg_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet");
            sync_pill(ui, &b.fg_sync, x0, y0 + 74.0);
        }
        3 => {
            if b.ph_sync.value() {
                enum_combo(ui, setter, &b.ph_div, "ph_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
            } else {
                kn(ui, setter, &b.ph_rate, "RATE", x0, y0, "Phaser LFO speed");
            }
            kn(ui, setter, &b.ph_depth, "DEPTH", x0 + w, y0, "Sweep depth");
            md(ModDst::FxPhaserFreq);
            kn(ui, setter, &b.ph_freq, "FREQ", x0 + w * 2.0, y0, "Centre frequency");
            kn(ui, setter, &b.ph_fb, "FEEDBACK", x0 + w * 3.0, y0, "Resonance");
            kn(ui, setter, &b.ph_stages, "STAGES", x0 + w * 4.0, y0, "Number of all-pass stages");
            kn(ui, setter, &b.ph_phase, "PHASE", x0 + w * 5.0, y0, "Stereo phase of the LFO");
            kn(ui, setter, &b.ph_mix, "MIX", x0 + w * 6.0, y0, "Dry / wet");
            sync_pill(ui, &b.ph_sync, x0, y0 + 74.0);
        }
        4 => {
            if b.ch_sync.value() {
                enum_combo(ui, setter, &b.ch_div, "ch_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
            } else {
                kn(ui, setter, &b.ch_rate, "RATE", x0, y0, "Chorus LFO speed");
            }
            kn(ui, setter, &b.ch_depth, "DEPTH", x0 + w, y0, "Depth");
            kn(ui, setter, &b.ch_delay, "DELAY", x0 + w * 2.0, y0, "Base delay");
            kn(ui, setter, &b.ch_fb, "FEEDBACK", x0 + w * 3.0, y0, "Feedback");
            kn(ui, setter, &b.ch_filter, "FILTER", x0 + w * 4.0, y0, "Tone of the chorus signal");
            md(ModDst::FxChorusMix);
            kn(ui, setter, &b.ch_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet");
            sync_pill(ui, &b.ch_sync, x0, y0 + 74.0);
        }
        5 => {
            if b.dl_sync.value() {
                enum_combo(ui, setter, &b.dl_div, "dl_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
                if !b.dl_link.value() {
                    enum_combo(ui, setter, &b.dl_div_r, "dl_divr", Rect::from_min_size(Pos2::new(x0 + w, y0 + 18.0), Vec2::new(62.0, 24.0)));
                }
            } else {
                kn(ui, setter, &b.dl_time, "TIME L", x0, y0, "Delay time (left)");
                if !b.dl_link.value() {
                    kn(ui, setter, &b.dl_time_r, "TIME R", x0 + w, y0, "Delay time (right)");
                }
            }
            md(ModDst::FxDelayFb);
            kn(ui, setter, &b.dl_fb, "FEEDBACK", x0 + w * 2.0, y0, "Feedback");
            kn(ui, setter, &b.dl_lp, "LOW PASS", x0 + w * 3.0, y0, "Dampens repeats (high cut)");
            kn(ui, setter, &b.dl_hp, "HIGH PASS", x0 + w * 4.0, y0, "Removes lows from repeats");
            kn(ui, setter, &b.dl_width, "WIDTH", x0 + w * 5.0, y0, "Stereo width of the repeats");
            md(ModDst::FxDelayMix);
            kn(ui, setter, &b.dl_mix, "MIX", x0 + w * 6.0, y0, "Dry / wet");
            sync_pill(ui, &b.dl_sync, x0, y0 + 74.0);
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 60.0, y0 + 74.0), Vec2::new(54.0, 22.0)), "LINK", b.dl_link.value(), "Left and right use the same time") {
                set_b(setter, &b.dl_link, !b.dl_link.value());
            }
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 120.0, y0 + 74.0), Vec2::new(80.0, 22.0)), "PINGPONG", b.dl_pp.value(), "Repeats bounce between left and right") {
                set_b(setter, &b.dl_pp, !b.dl_pp.value());
            }
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 206.0, y0 + 74.0), Vec2::new(44.0, 22.0)), "HQ", b.dl_hq.value(), "High quality: cubic interpolation and smooth time changes (tape-like glides)") {
                set_b(setter, &b.dl_hq, !b.dl_hq.value());
            }
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 110.0), Vec2::new(d.width() - 4.0, d.top() + 260.0 - (y0 + 110.0)));
            delay_taps(ui, g, &fp);
        }
        6 => {
            kn(ui, setter, &b.cp_thresh, "THRESH", x0, y0, "Threshold in dB");
            kn(ui, setter, &b.cp_ratio, "RATIO", x0 + w, y0, "Compression ratio");
            kn(ui, setter, &b.cp_attack, "ATTACK", x0 + w * 2.0, y0, "Attack time");
            kn(ui, setter, &b.cp_release, "RELEASE", x0 + w * 3.0, y0, "Release time");
            kn(ui, setter, &b.cp_gain, "GAIN", x0 + w * 4.0, y0, "Make-up gain");
            kn(ui, setter, &b.cp_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet (parallel compression)");
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0, y0 + 74.0), Vec2::new(54.0, 22.0)), "MULTI", b.cp_multi.value(), "3-band multiband mode") {
                set_b(setter, &b.cp_multi, !b.cp_multi.value());
            }
            if b.cp_multi.value() {
                kn(ui, setter, &b.cp_f1, "LOW/MID", x0 + w * 6.0, y0, "Crossover 1");
                kn(ui, setter, &b.cp_f2, "MID/HI", x0 + w * 7.0, y0, "Crossover 2");
            }
            // static curve (input level -> output level)
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 110.0), Vec2::new(d.width() - 4.0, d.top() + 260.0 - (y0 + 110.0)));
            comp_curve(ui, g, &fp);
        }
        7 => {
            md(ModDst::FxReverbSize);
            kn(ui, setter, &b.rv_size, "SIZE", x0, y0, "Room size / decay time");
            kn(ui, setter, &b.rv_damp, "DAMPING", x0 + w, y0, "High-frequency damping");
            kn(ui, setter, &b.rv_pre, "PRE-DLY", x0 + w * 2.0, y0, "Pre-delay");
            kn(ui, setter, &b.rv_lowcut, "LOW CUT", x0 + w * 3.0, y0, "Low cut of the reverb signal");
            kn(ui, setter, &b.rv_highcut, "HIGH CUT", x0 + w * 4.0, y0, "High cut of the reverb signal");
            kn(ui, setter, &b.rv_width, "WIDTH", x0 + w * 5.0, y0, "Stereo width");
            md(ModDst::FxReverbMix);
            kn(ui, setter, &b.rv_mix, "MIX", x0 + w * 6.0, y0, "Dry / wet");
            ui.painter().text(Pos2::new(x0 + 2.0, y0 + 86.0), Align2::LEFT_CENTER, "ALGORITHM", mono(9.0), DIM);
            enum_combo(ui, setter, &b.rv_type, "rv_type", Rect::from_min_size(Pos2::new(x0 + 70.0, y0 + 74.0), Vec2::new(130.0, 24.0)));
            if b.rv_type.value() == crate::params::ReverbType::Convolve {
                let nm = p.ir.lock().map(|g| crate::sample::encoded_name(&g)).unwrap_or_default();
                let label: String = if nm.is_empty() { "LOAD IR...".to_string() } else { nm.chars().take(18).collect() };
                if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 214.0, y0 + 74.0), Vec2::new(150.0, 24.0)), &label, !nm.is_empty(), "Load a .wav impulse response (room, hall, cabinet, anything). SIZE trims the tail, MIX sets the wet level.") {
                    st.open_ir();
                }
                if nm.is_empty() {
                    ui.painter().text(Pos2::new(x0 + 2.0, y0 + 112.0), Align2::LEFT_CENTER, "No impulse response loaded: pick a .wav to hear it.", mono(9.5), DIM);
                }
            }
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 124.0), Vec2::new(d.width() - 4.0, d.top() + 260.0 - (y0 + 124.0)));
            reverb_decay(ui, g, &fp);
        }
        8 => {
            kn(ui, setter, &b.eq_lf, "LOW F", x0, y0, "Low shelf frequency");
            kn(ui, setter, &b.eq_lg, "LOW dB", x0 + w, y0, "Low shelf gain");
            kn(ui, setter, &b.eq_mf, "MID F", x0 + w * 2.0, y0, "Mid frequency");
            kn(ui, setter, &b.eq_mq, "MID Q", x0 + w * 3.0, y0, "Mid bandwidth");
            kn(ui, setter, &b.eq_mg, "MID dB", x0 + w * 4.0, y0, "Mid gain");
            kn(ui, setter, &b.eq_hf, "HIGH F", x0 + w * 5.0, y0, "High shelf frequency");
            kn(ui, setter, &b.eq_hg, "HIGH dB", x0 + w * 6.0, y0, "High shelf gain");
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 80.0), Vec2::new(d.width() - 4.0, d.top() + 260.0 - (y0 + 80.0)));
            graph(ui, g, |f| eq_response_db(&fp, f, 48000.0), (-18.0, 18.0), Some((&st.spec, st.spec_fs)));
            eq_nodes(ui, setter, b, g);
        }
        10 => {
            kn(ui, setter, &b.bd_shift, "SHIFT", x0, y0, "Frequency shift in Hz (every partial moves by the same amount, so sounds turn inharmonic)");
            kn(ui, setter, &b.bd_fb, "FEEDBACK", x0 + w, y0, "Feed the shifted signal back: endless rising / falling tones");
            kn(ui, setter, &b.bd_mix, "MIX", x0 + w * 2.0, y0, "Dry / wet");
            ui.painter().text(Pos2::new(x0 + 2.0, y1 + 10.0), Align2::LEFT_CENTER, "Bode: single-sideband frequency shifter (not a pitch shifter).", mono(9.5), DIM);
        }
        11 => {
            kn(ui, setter, &b.ut_gain, "GAIN", x0, y0, "Output gain in dB");
            knob_s(ui, setter, &b.ut_pan, "PAN", cell(x0 + w, y0, w), "Balance", true);
            kn(ui, setter, &b.ut_width, "WIDTH", x0 + w * 2.0, y0, "Stereo width: 0 = mono, 100% = unchanged, 200% = extra wide");
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + w * 3.0 + 6.0, y0 + 6.0), Vec2::new(70.0, 22.0)), "MONO", b.ut_mono.value(), "Sum to mono") {
                set_b(setter, &b.ut_mono, !b.ut_mono.value());
            }
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + w * 3.0 + 6.0, y0 + 34.0), Vec2::new(70.0, 22.0)), "INVERT", b.ut_invert.value(), "Flip the polarity") {
                set_b(setter, &b.ut_invert, !b.ut_invert.value());
            }
            ui.painter().text(Pos2::new(x0 + 2.0, y1 + 10.0), Align2::LEFT_CENTER, "Utility: gain, balance, width, mono and polarity.", mono(9.5), DIM);
        }
        _ => {
            enum_combo(ui, setter, &b.ff_type, "ff_type", Rect::from_min_size(Pos2::new(x0, y0 + 4.0), Vec2::new(110.0, 24.0)));
            md(ModDst::FxFilterCut);
            kn(ui, setter, &b.ff_cut, "CUTOFF", x0 + w * 2.0, y0, "Cutoff");
            md(ModDst::FxFilterRes);
            kn(ui, setter, &b.ff_res, "RES", x0 + w * 3.0, y0, "Resonance");
            kn(ui, setter, &b.ff_drive, "DRIVE", x0 + w * 4.0, y0, "Drive");
            kn(ui, setter, &b.ff_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet");
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 80.0), Vec2::new(d.width() - 4.0, d.top() + 260.0 - (y0 + 80.0)));
            let (fc, q, ty) = (fp.ff_cut, 0.5 + fp.ff_res * 9.0, fp.ff_type);
            graph(
                ui,
                g,
                |f| {
                    let fs = 48000.0;
                    let m = match ty {
                        FxFilterType::Lp12 => BqCoef::lowpass(fc, q, fs).mag(f, fs),
                        FxFilterType::Lp24 => BqCoef::lowpass(fc, q, fs).mag(f, fs).powi(2),
                        FxFilterType::Hp12 => BqCoef::highpass(fc, q, fs).mag(f, fs),
                        FxFilterType::Hp24 => BqCoef::highpass(fc, q, fs).mag(f, fs).powi(2),
                        FxFilterType::Band => BqCoef::bandpass(fc, q, fs).mag(f, fs),
                        FxFilterType::Notch => BqCoef::notch(fc, q, fs).mag(f, fs),
                        FxFilterType::Peak => BqCoef::peaking(fc, q, 12.0, fs).mag(f, fs),
                    };
                    20.0 * m.max(1e-6).log10()
                },
                (-36.0, 18.0),
                Some((&st.spec, st.spec_fs)),
            );
            ff_node(ui, setter, b, g);
        }
    }

    // ---- signal flow of the selected bus (fills the lower part of the page)
    let fy = d.top() + 282.0;
    ui.painter().text(Pos2::new(d.left() + 2.0, fy - 6.0), Align2::LEFT_CENTER, format!("SIGNAL FLOW  BUS {}", bus + 1), mono(9.0), DIM);
    let bw = (d.width() - 70.0 - 11.0 * 14.0) / 12.0;
    let by = fy + 6.0;
    ui.painter().text(Pos2::new(d.left() + 12.0, by + 17.0), Align2::CENTER_CENTER, "IN", mono(9.0), WHITE);
    for slot in 0..FX_SLOTS {
        let m2 = order[slot];
        let on = on_param(b, m2).value();
        let x = d.left() + 30.0 + (bw + 14.0) * slot as f32;
        let r = Rect::from_min_size(Pos2::new(x, by), Vec2::new(bw, 34.0));
        let sel = st.fx_sel == m2;
        let pnt = ui.painter();
        pnt.rect_filled(r, CornerRadius::same(4), if sel { Color32::from_rgb(40, 40, 40) } else { Color32::from_rgb(14, 14, 14) });
        pnt.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, if on { WHITE } else { Color32::from_rgb(48, 48, 48) }), StrokeKind::Inside);
        pnt.text(r.center(), Align2::CENTER_CENTER, FX_NAMES[m2], mono(8.0), if on { WHITE } else { Color32::from_rgb(70, 70, 70) });
        // arrow to the next box
        let (ax, ay) = (r.right() + 1.0, r.center().y);
        let col = if on { WHITE } else { Color32::from_rgb(48, 48, 48) };
        pnt.line_segment([Pos2::new(ax, ay), Pos2::new(ax + 10.0, ay)], Stroke::new(1.0_f32, col));
        if slot + 1 < FX_SLOTS {
            pnt.line_segment([Pos2::new(ax + 10.0, ay), Pos2::new(ax + 6.5, ay - 3.0)], Stroke::new(1.0_f32, col));
            pnt.line_segment([Pos2::new(ax + 10.0, ay), Pos2::new(ax + 6.5, ay + 3.0)], Stroke::new(1.0_f32, col));
        }
    }
    let route = match p.fx_route.value() {
        FxRoute::Series => "OUT  (bus 1 then bus 2)",
        FxRoute::Parallel => "OUT  (bus 1 + bus 2)",
        FxRoute::SplitLh => "OUT  (lows > bus 1, highs > bus 2)",
        FxRoute::SplitLmh => "OUT  (lows > 1, mids > 2, highs dry)",
        FxRoute::SplitMs => "OUT  (mid > bus 1, side > bus 2)",
    };
    ui.painter().text(Pos2::new(d.right() - 2.0, fy - 6.0), Align2::RIGHT_CENTER, route, mono(9.0), DIM);
}

fn graph(ui: &mut Ui, g: Rect, f: impl Fn(f32) -> f32, range: (f32, f32), spec: Option<(&[f32], f32)>) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    if let Some((sp, fs)) = spec {
        crate::editor::draw_spectrum(&pa, g.shrink(1.0), sp, fs, -80.0, Color32::from_rgb(46, 46, 46));
    }
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(range.0, range.1) - range.0) / (range.1 - range.0));
    for fr in [100.0f32, 1000.0, 10000.0] {
        pa.line_segment([Pos2::new(xf(fr), g.top()), Pos2::new(xf(fr), g.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(24, 24, 24)));
    }
    pa.line_segment([Pos2::new(g.left(), yd(0.0)), Pos2::new(g.right(), yd(0.0))], Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)));
    let pts: Vec<Pos2> = (0..=240).map(|i| {
        let fr = lo * (hi / lo).powf(i as f32 / 240.0);
        Pos2::new(xf(fr), yd(f(fr)))
    }).collect();
    fill_under(&pa, &pts, yd(0.0).min(g.bottom()), Color32::from_rgb(30, 30, 30));
    pa.add(egui::Shape::line(pts, PathStroke::new(1.6_f32, WHITE)));
}

/// Draggable EQ nodes on top of the response graph (drag = frequency / gain, wheel on the mid node = Q).
fn eq_nodes(ui: &mut Ui, setter: &ParamSetter, b: &FxBank, g: Rect) {
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    let fx = |x: f32| lo * (hi / lo).powf(((x - g.left()) / g.width()).clamp(0.0, 1.0));
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(-18.0, 18.0) + 18.0) / 36.0);
    let dbf = |y: f32| (((g.bottom() - y) / g.height()) * 36.0 - 18.0).clamp(-18.0, 18.0);
    let nodes: [(&FloatParam, &FloatParam, &str); 3] = [(&b.eq_lf, &b.eq_lg, "LOW"), (&b.eq_mf, &b.eq_mg, "MID"), (&b.eq_hf, &b.eq_hg, "HIGH")];
    for (i, (fp, gp, name)) in nodes.iter().enumerate() {
        let c = Pos2::new(xf(fp.value()), yd(gp.value()));
        let hit = Rect::from_center_size(c, Vec2::splat(20.0));
        let resp = ui.interact(hit, egui::Id::new(("eqnode", i)), Sense::click_and_drag()).on_hover_text(format!("{name} band: drag = frequency / gain{}", if i == 1 { ", wheel = Q" } else { "" }));
        let offk = egui::Id::new(("eqnode_off", i));
        if resp.drag_started() {
            setter.begin_set_parameter(*fp);
            setter.begin_set_parameter(*gp);
            // keep the grab point: the node must not jump to the pointer
            let off = resp.interact_pointer_pos().map_or(Vec2::ZERO, |pp| pp - c);
            ui.data_mut(|d| d.insert_temp(offk, off));
        }
        if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let off = ui.data(|d| d.get_temp::<Vec2>(offk)).unwrap_or(Vec2::ZERO);
                let pt = pt - off;
                setter.set_parameter(*fp, fx(pt.x));
                setter.set_parameter(*gp, dbf(pt.y));
            }
        }
        if resp.drag_stopped() {
            setter.end_set_parameter(*fp);
            setter.end_set_parameter(*gp);
        }
        if resp.double_clicked() {
            set_f(setter, gp, 0.0);
        }
        if i == 1 && resp.hovered() {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc != 0.0 {
                let q = b.eq_mq.unmodulated_normalized_value();
                set_norm(setter, &b.eq_mq, q + sc / 1200.0);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        ui.painter().circle_filled(c, if hot { 7.0 } else { 5.5 }, Color32::BLACK);
        ui.painter().circle_stroke(c, if hot { 7.0 } else { 5.5 }, Stroke::new(1.6_f32, WHITE));
        ui.painter().text(c + Vec2::new(0.0, -13.0), Align2::CENTER_CENTER, format!("{}", i + 1), mono(8.5), DIM);
    }
}

/// Draggable node on the FX filter graph: left / right = cutoff, up / down = resonance.
/// For the low / high pass and band types the node sits on the curve (its height is the resonance peak in dB).
fn ff_node(ui: &mut Ui, setter: &ParamSetter, b: &FxBank, g: Rect) {
    let (lo, hi) = (20.0f32, 20000.0f32);
    let (dlo, dhi) = (-36.0f32, 18.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    // dB of the peak at the cutoff for a given resonance, and its inverse (None = this type has no resonance peak)
    let ty = b.ff_type.value();
    let pw = match ty {
        FxFilterType::Lp24 | FxFilterType::Hp24 => Some(2.0f32),
        FxFilterType::Lp12 | FxFilterType::Hp12 | FxFilterType::Band => Some(1.0),
        _ => None,
    };
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(dlo, dhi) - dlo) / (dhi - dlo));
    let res = b.ff_res.unmodulated_normalized_value();
    let y_of_res = |r: f32| -> f32 {
        match pw {
            Some(p) => yd(20.0 * p * (0.5 + r * 9.0).log10()),
            None => g.bottom() - 6.0 - (g.height() - 12.0) * r,
        }
    };
    let res_of_y = |y: f32| -> f32 {
        match pw {
            Some(p) => {
                let db = dlo + (dhi - dlo) * ((g.bottom() - y) / g.height()).clamp(0.0, 1.0);
                (((10f32.powf(db / (20.0 * p))) - 0.5) / 9.0).clamp(0.0, 1.0)
            }
            None => ((g.bottom() - 6.0 - y) / (g.height() - 12.0)).clamp(0.0, 1.0),
        }
    };
    let c = Pos2::new(xf(b.ff_cut.value()).clamp(g.left() + 7.0, g.right() - 7.0), y_of_res(res).clamp(g.top() + 7.0, g.bottom() - 7.0));
    let area = ui.interact(g, egui::Id::new("ffgraph"), Sense::click_and_drag()).on_hover_text("Drag: left / right = cutoff, up / down = resonance");
    let offk = egui::Id::new("ffnode_off");
    if area.drag_started() {
        setter.begin_set_parameter(&b.ff_cut);
        setter.begin_set_parameter(&b.ff_res);
        // grabbing the dot itself keeps the grab point; grabbing elsewhere jumps the dot there
        let off = area.interact_pointer_pos().map_or(Vec2::ZERO, |pp| if (pp - c).length() < 16.0 { pp - c } else { Vec2::ZERO });
        ui.data_mut(|d| d.insert_temp(offk, off));
    }
    if area.dragged() {
        if let Some(pt) = area.interact_pointer_pos() {
            let pt = pt - ui.data(|d| d.get_temp::<Vec2>(offk)).unwrap_or(Vec2::ZERO);
            let fx = ((pt.x - g.left()) / g.width()).clamp(0.0, 1.0);
            setter.set_parameter(&b.ff_cut, lo * (hi / lo).powf(fx));
            setter.set_parameter_normalized(&b.ff_res, res_of_y(pt.y));
        }
    }
    if area.drag_stopped() {
        setter.end_set_parameter(&b.ff_cut);
        setter.end_set_parameter(&b.ff_res);
    }
    let hot = area.hovered() || area.dragged();
    ui.painter().circle_filled(c, if hot { 7.0 } else { 5.5 }, Color32::BLACK);
    ui.painter().circle_stroke(c, if hot { 7.0 } else { 5.5 }, Stroke::new(1.6_f32, WHITE));
}

fn comp_curve(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let side = g.height().min(g.width()) - 16.0;
    let sq = Rect::from_min_size(Pos2::new(g.left() + 14.0, g.top() + 8.0), Vec2::splat(side));
    let xy = |inp: f32, out: f32| Pos2::new(sq.left() + sq.width() * ((inp + 60.0) / 60.0).clamp(0.0, 1.0), sq.bottom() - sq.height() * ((out + 60.0) / 60.0).clamp(0.0, 1.0));
    for db in [-48.0f32, -36.0, -24.0, -12.0] {
        pa.line_segment([xy(db, -60.0), xy(db, 0.0)], Stroke::new(1.0_f32, Color32::from_rgb(22, 22, 22)));
        pa.line_segment([xy(-60.0, db), xy(0.0, db)], Stroke::new(1.0_f32, Color32::from_rgb(22, 22, 22)));
    }
    pa.line_segment([xy(-60.0, -60.0), xy(0.0, 0.0)], Stroke::new(1.0_f32, Color32::from_rgb(44, 44, 44)));
    let pts: Vec<Pos2> = (0..=60)
        .map(|i| {
            let inp = -60.0 + i as f32;
            let out = if inp > fp.cp_thresh { fp.cp_thresh + (inp - fp.cp_thresh) / fp.cp_ratio.max(1.0) } else { inp } + fp.cp_gain;
            xy(inp, out)
        })
        .collect();
    pa.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, WHITE)));
    let tp = xy(fp.cp_thresh, fp.cp_thresh + fp.cp_gain);
    pa.circle_filled(tp, 4.0, Color32::BLACK);
    pa.circle_stroke(tp, 4.0, Stroke::new(1.5_f32, WHITE));
    pa.text(Pos2::new(sq.right() + 14.0, sq.top() + 6.0), Align2::LEFT_CENTER, "INPUT dB  >  OUTPUT dB (dot = threshold)", mono(9.0), DIM);
}

/// Repeats of the delay: bar height = level, left channel up, right channel down.
fn delay_taps(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let (l, r) = (fp.dl_ms_l.max(1.0), fp.dl_ms_r.max(1.0));
    let tmax = (l.max(r) * 8.0).max(400.0);
    let mid = g.center().y;
    pa.line_segment([Pos2::new(g.left(), mid), Pos2::new(g.right(), mid)], Stroke::new(1.0_f32, Color32::from_rgb(34, 34, 34)));
    let xt = |t: f32| g.left() + 14.0 + (g.width() - 28.0) * (t / tmax);
    let h = g.height() * 0.42;
    pa.line_segment([Pos2::new(xt(0.0), mid - h), Pos2::new(xt(0.0), mid + h)], Stroke::new(3.0_f32, WHITE));
    let fb = fp.dl_fb.clamp(0.0, 0.98);
    let mut amp = 1.0f32;
    for k in 1..=64 {
        amp *= if k == 1 { 1.0 } else { fb };
        if amp < 0.02 {
            break;
        }
        // ping-pong: odd repeats on the left, even on the right; otherwise both channels repeat together
        let (tl, tr) = (l * k as f32, r * k as f32);
        let (show_l, show_r) = if fp.dl_pp { (k % 2 == 1, k % 2 == 0) } else { (true, true) };
        if show_l && tl <= tmax {
            pa.line_segment([Pos2::new(xt(tl), mid), Pos2::new(xt(tl), mid - h * amp)], Stroke::new(3.0_f32, Color32::from_rgb(220, 220, 220)));
        }
        if show_r && tr <= tmax {
            pa.line_segment([Pos2::new(xt(tr) + 4.0, mid), Pos2::new(xt(tr) + 4.0, mid + h * amp)], Stroke::new(3.0_f32, Color32::from_rgb(150, 150, 150)));
        }
    }
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, format!("REPEATS  (L up, R down)   0 - {:.0} ms", tmax), mono(9.0), DIM);
}

/// Rough decay curve of the reverb (level over time).
fn reverb_decay(ui: &mut Ui, g: Rect, fp: &crate::fx::FxPatch) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
    let t60 = 0.3 + fp.rv_size.clamp(0.0, 1.0) * 7.0;
    let tmax = 8.0f32;
    let pre = fp.rv_pre * 0.001;
    let pts: Vec<Pos2> = (0..=200)
        .map(|i| {
            let t = tmax * i as f32 / 200.0;
            let db = if t < pre { -80.0 } else { -60.0 * (t - pre) / t60 };
            Pos2::new(g.left() + 14.0 + (g.width() - 28.0) * t / tmax, g.bottom() - 10.0 - (g.height() - 30.0) * ((db + 60.0) / 60.0).clamp(0.0, 1.0))
        })
        .collect();
    fill_under(&pa, &pts, g.bottom() - 10.0, Color32::from_rgb(30, 30, 30));
    pa.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, WHITE)));
    pa.text(Pos2::new(g.left() + 8.0, g.top() + 9.0), Align2::LEFT_CENTER, format!("DECAY (about {:.1} s to -60 dB)   axis 0 - 8 s", t60 + pre), mono(9.0), DIM);
}

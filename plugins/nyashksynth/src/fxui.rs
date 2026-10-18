//! FX rack tab: a reorderable chain of 10 modules and a detail view for the selected one.
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, epaint::PathStroke, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::editor::{cell, dark_box, fill_under, Gui};
use crate::fx::{eq_response_db, BqCoef};
use crate::params::*;
use crate::ui::*;

fn on_param(p: &SynthParams, m: usize) -> &BoolParam {
    match m {
        0 => &p.hy_on,
        1 => &p.ds_on,
        2 => &p.fg_on,
        3 => &p.ph_on,
        4 => &p.ch_on,
        5 => &p.dl_on,
        6 => &p.cp_on,
        7 => &p.rv_on,
        8 => &p.eq_on,
        _ => &p.ff_on,
    }
}

fn kn<P: Param>(ui: &mut Ui, setter: &ParamSetter, prm: &P, label: &str, x: f32, y: f32, tip: &str) {
    knob_s(ui, setter, prm, label, cell(x, y, 62.0), tip, false);
}

pub fn fx_tab(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, st: &mut Gui, c: Rect) {
    let order: Vec<usize> = p.fx_order_params().iter().map(|q| (q.value().clamp(0, 9)) as usize).collect();
    let cw = (c.width() - 9.0 * 4.0) / 10.0;
    let chip = |i: usize| Rect::from_min_size(Pos2::new(c.left() + (cw + 4.0) * i as f32, c.top()), Vec2::new(cw, 30.0));
    let mut clicked: Option<usize> = None;
    let mut dropped: Option<(usize, usize)> = None;
    for slot in 0..10 {
        let m = order[slot];
        let r = chip(slot);
        let resp = ui.interact(r, egui::Id::new(("fxchip", slot)), Sense::click_and_drag()).on_hover_text("Click: edit. Drag onto another slot: reorder the chain. Dot: on/off");
        let on = on_param(p, m).value();
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
                    if let Some(to) = (0..10).find(|&j| chip(j).contains(pt)) {
                        if to != from {
                            dropped = Some((from, to));
                        }
                    }
                }
            }
        } else if resp.clicked() {
            if let Some(pt) = ui.input(|i| i.pointer.interact_pos()) {
                if dot.contains(pt) {
                    set_b(setter, on_param(p, m), !on);
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
        for (q, val) in p.fx_order_params().iter().zip(o.iter()) {
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
    let m = st.fx_sel.min(9);
    let d = Rect::from_min_max(Pos2::new(c.left(), c.top() + 40.0), c.right_bottom());
    ui.painter().text(Pos2::new(d.left() + 2.0, d.top() + 8.0), Align2::LEFT_CENTER, FX_NAMES[m], mono(11.5), WHITE);
    let pos_in_chain = order.iter().position(|&x| x == m).unwrap_or(0) + 1;
    ui.painter().text(Pos2::new(d.left() + 90.0, d.top() + 8.0), Align2::LEFT_CENTER, format!("slot {pos_in_chain}/10"), mono(9.0), DIM);
    let on_p = on_param(p, m);
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
    let fp = crate::fx_patch_from(p, 120.0);
    match m {
        0 => {
            kn(ui, setter, &p.hy_rate, "RATE", x0, y0, "Hyper chorus speed");
            kn(ui, setter, &p.hy_detune, "DETUNE", x0 + w, y0, "Depth of the voices' pitch wobble");
            kn(ui, setter, &p.hy_uni, "UNISON", x0 + w * 2.0, y0, "Number of chorus voices");
            kn(ui, setter, &p.hy_dim, "DIMENSION", x0 + w * 3.0, y0, "Dimension-style stereo widening");
            mdst(ModDst::FxHyperMix);
            kn(ui, setter, &p.hy_mix, "MIX", x0 + w * 4.0, y0, "Dry / wet");
            ui.painter().text(Pos2::new(x0 + 2.0, y1 + 10.0), Align2::LEFT_CENTER, "Hyper + Dimension: Serum-style thick stereo chorus.", mono(9.5), DIM);
        }
        1 => {
            enum_combo(ui, setter, &p.ds_type, "ds_type", Rect::from_min_size(Pos2::new(x0, y0 + 4.0), Vec2::new(150.0, 24.0)));
            mdst(ModDst::FxDistDrive);
            kn(ui, setter, &p.ds_drive, "DRIVE", x0, y0 + 36.0, "Distortion amount");
            kn(ui, setter, &p.ds_tone, "TONE", x0 + w, y0 + 36.0, "Tilt: dark to bright");
            mdst(ModDst::FxDistMix);
            kn(ui, setter, &p.ds_mix, "MIX", x0 + w * 2.0, y0 + 36.0, "Dry / wet");
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
            if p.fg_sync.value() {
                enum_combo(ui, setter, &p.fg_div, "fg_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
            } else {
                kn(ui, setter, &p.fg_rate, "RATE", x0, y0, "Flanger LFO speed");
            }
            mdst(ModDst::FxFlangerDepth);
            kn(ui, setter, &p.fg_depth, "DEPTH", x0 + w, y0, "Sweep depth");
            kn(ui, setter, &p.fg_delay, "DELAY", x0 + w * 2.0, y0, "Base delay");
            kn(ui, setter, &p.fg_fb, "FEEDBACK", x0 + w * 3.0, y0, "Feedback");
            kn(ui, setter, &p.fg_phase, "PHASE", x0 + w * 4.0, y0, "Stereo phase of the LFO");
            kn(ui, setter, &p.fg_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet");
            sync_pill(ui, &p.fg_sync, x0, y0 + 74.0);
        }
        3 => {
            if p.ph_sync.value() {
                enum_combo(ui, setter, &p.ph_div, "ph_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
            } else {
                kn(ui, setter, &p.ph_rate, "RATE", x0, y0, "Phaser LFO speed");
            }
            kn(ui, setter, &p.ph_depth, "DEPTH", x0 + w, y0, "Sweep depth");
            mdst(ModDst::FxPhaserFreq);
            kn(ui, setter, &p.ph_freq, "FREQ", x0 + w * 2.0, y0, "Centre frequency");
            kn(ui, setter, &p.ph_fb, "FEEDBACK", x0 + w * 3.0, y0, "Resonance");
            kn(ui, setter, &p.ph_stages, "STAGES", x0 + w * 4.0, y0, "Number of all-pass stages");
            kn(ui, setter, &p.ph_phase, "PHASE", x0 + w * 5.0, y0, "Stereo phase of the LFO");
            kn(ui, setter, &p.ph_mix, "MIX", x0 + w * 6.0, y0, "Dry / wet");
            sync_pill(ui, &p.ph_sync, x0, y0 + 74.0);
        }
        4 => {
            if p.ch_sync.value() {
                enum_combo(ui, setter, &p.ch_div, "ch_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
            } else {
                kn(ui, setter, &p.ch_rate, "RATE", x0, y0, "Chorus LFO speed");
            }
            kn(ui, setter, &p.ch_depth, "DEPTH", x0 + w, y0, "Depth");
            kn(ui, setter, &p.ch_delay, "DELAY", x0 + w * 2.0, y0, "Base delay");
            kn(ui, setter, &p.ch_fb, "FEEDBACK", x0 + w * 3.0, y0, "Feedback");
            kn(ui, setter, &p.ch_filter, "FILTER", x0 + w * 4.0, y0, "Tone of the chorus signal");
            mdst(ModDst::FxChorusMix);
            kn(ui, setter, &p.ch_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet");
            sync_pill(ui, &p.ch_sync, x0, y0 + 74.0);
        }
        5 => {
            if p.dl_sync.value() {
                enum_combo(ui, setter, &p.dl_div, "dl_div", Rect::from_min_size(Pos2::new(x0, y0 + 18.0), Vec2::new(62.0, 24.0)));
                if !p.dl_link.value() {
                    enum_combo(ui, setter, &p.dl_div_r, "dl_divr", Rect::from_min_size(Pos2::new(x0 + w, y0 + 18.0), Vec2::new(62.0, 24.0)));
                }
            } else {
                kn(ui, setter, &p.dl_time, "TIME L", x0, y0, "Delay time (left)");
                if !p.dl_link.value() {
                    kn(ui, setter, &p.dl_time_r, "TIME R", x0 + w, y0, "Delay time (right)");
                }
            }
            mdst(ModDst::FxDelayFb);
            kn(ui, setter, &p.dl_fb, "FEEDBACK", x0 + w * 2.0, y0, "Feedback");
            kn(ui, setter, &p.dl_lp, "LOW PASS", x0 + w * 3.0, y0, "Dampens repeats (high cut)");
            kn(ui, setter, &p.dl_hp, "HIGH PASS", x0 + w * 4.0, y0, "Removes lows from repeats");
            kn(ui, setter, &p.dl_width, "WIDTH", x0 + w * 5.0, y0, "Stereo width of the repeats");
            mdst(ModDst::FxDelayMix);
            kn(ui, setter, &p.dl_mix, "MIX", x0 + w * 6.0, y0, "Dry / wet");
            sync_pill(ui, &p.dl_sync, x0, y0 + 74.0);
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 60.0, y0 + 74.0), Vec2::new(54.0, 22.0)), "LINK", p.dl_link.value(), "Left and right use the same time") {
                set_b(setter, &p.dl_link, !p.dl_link.value());
            }
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 120.0, y0 + 74.0), Vec2::new(80.0, 22.0)), "PINGPONG", p.dl_pp.value(), "Repeats bounce between left and right") {
                set_b(setter, &p.dl_pp, !p.dl_pp.value());
            }
        }
        6 => {
            kn(ui, setter, &p.cp_thresh, "THRESH", x0, y0, "Threshold in dB");
            kn(ui, setter, &p.cp_ratio, "RATIO", x0 + w, y0, "Compression ratio");
            kn(ui, setter, &p.cp_attack, "ATTACK", x0 + w * 2.0, y0, "Attack time");
            kn(ui, setter, &p.cp_release, "RELEASE", x0 + w * 3.0, y0, "Release time");
            kn(ui, setter, &p.cp_gain, "GAIN", x0 + w * 4.0, y0, "Make-up gain");
            kn(ui, setter, &p.cp_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet (parallel compression)");
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0, y0 + 74.0), Vec2::new(54.0, 22.0)), "MULTI", p.cp_multi.value(), "3-band multiband mode") {
                set_b(setter, &p.cp_multi, !p.cp_multi.value());
            }
            if p.cp_multi.value() {
                kn(ui, setter, &p.cp_f1, "LOW/MID", x0 + w * 6.0, y0, "Crossover 1");
                kn(ui, setter, &p.cp_f2, "MID/HI", x0 + w * 7.0, y0, "Crossover 2");
            }
            // static curve
            let g = Rect::from_min_size(Pos2::new(x0 + 340.0, y0 + 74.0), Vec2::new(110.0, 52.0));
            dark_box(ui, g);
            let pa = ui.painter().with_clip_rect(g.shrink(1.0));
            let pts: Vec<Pos2> = (0..=60)
                .map(|i| {
                    let inp = -60.0 + 60.0 * i as f32 / 60.0;
                    let out = if inp > fp.cp_thresh { fp.cp_thresh + (inp - fp.cp_thresh) / fp.cp_ratio.max(1.0) } else { inp };
                    Pos2::new(g.left() + g.width() * i as f32 / 60.0, g.bottom() - g.height() * ((out + 60.0) / 60.0).clamp(0.0, 1.0))
                })
                .collect();
            pa.add(egui::Shape::line(pts, PathStroke::new(1.5_f32, WHITE)));
        }
        7 => {
            mdst(ModDst::FxReverbSize);
            kn(ui, setter, &p.rv_size, "SIZE", x0, y0, "Room size / decay time");
            kn(ui, setter, &p.rv_damp, "DAMPING", x0 + w, y0, "High-frequency damping");
            kn(ui, setter, &p.rv_pre, "PRE-DLY", x0 + w * 2.0, y0, "Pre-delay");
            kn(ui, setter, &p.rv_lowcut, "LOW CUT", x0 + w * 3.0, y0, "Low cut of the reverb signal");
            kn(ui, setter, &p.rv_highcut, "HIGH CUT", x0 + w * 4.0, y0, "High cut of the reverb signal");
            kn(ui, setter, &p.rv_width, "WIDTH", x0 + w * 5.0, y0, "Stereo width");
            mdst(ModDst::FxReverbMix);
            kn(ui, setter, &p.rv_mix, "MIX", x0 + w * 6.0, y0, "Dry / wet");
        }
        8 => {
            kn(ui, setter, &p.eq_lf, "LOW F", x0, y0, "Low shelf frequency");
            kn(ui, setter, &p.eq_lg, "LOW dB", x0 + w, y0, "Low shelf gain");
            kn(ui, setter, &p.eq_mf, "MID F", x0 + w * 2.0, y0, "Mid frequency");
            kn(ui, setter, &p.eq_mq, "MID Q", x0 + w * 3.0, y0, "Mid bandwidth");
            kn(ui, setter, &p.eq_mg, "MID dB", x0 + w * 4.0, y0, "Mid gain");
            kn(ui, setter, &p.eq_hf, "HIGH F", x0 + w * 5.0, y0, "High shelf frequency");
            kn(ui, setter, &p.eq_hg, "HIGH dB", x0 + w * 6.0, y0, "High shelf gain");
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 72.0), Vec2::new(d.width() - 4.0, d.bottom() - y0 - 78.0));
            graph(ui, g, |f| eq_response_db(&fp, f, 48000.0), (-18.0, 18.0));
            eq_nodes(ui, setter, p, g);
        }
        _ => {
            enum_combo(ui, setter, &p.ff_type, "ff_type", Rect::from_min_size(Pos2::new(x0, y0 + 4.0), Vec2::new(110.0, 24.0)));
            mdst(ModDst::FxFilterCut);
            kn(ui, setter, &p.ff_cut, "CUTOFF", x0 + w * 2.0, y0, "Cutoff");
            mdst(ModDst::FxFilterRes);
            kn(ui, setter, &p.ff_res, "RES", x0 + w * 3.0, y0, "Resonance");
            kn(ui, setter, &p.ff_drive, "DRIVE", x0 + w * 4.0, y0, "Drive");
            kn(ui, setter, &p.ff_mix, "MIX", x0 + w * 5.0, y0, "Dry / wet");
            let g = Rect::from_min_size(Pos2::new(x0, y0 + 72.0), Vec2::new(d.width() - 4.0, d.bottom() - y0 - 78.0));
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
            );
        }
    }
}

fn graph(ui: &mut Ui, g: Rect, f: impl Fn(f32) -> f32, range: (f32, f32)) {
    dark_box(ui, g);
    let pa = ui.painter().with_clip_rect(g.shrink(1.0));
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
fn eq_nodes(ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, g: Rect) {
    let (lo, hi) = (20.0f32, 20000.0f32);
    let xf = |fr: f32| g.left() + g.width() * ((fr / lo).ln() / (hi / lo).ln());
    let fx = |x: f32| lo * (hi / lo).powf(((x - g.left()) / g.width()).clamp(0.0, 1.0));
    let yd = |db: f32| g.bottom() - g.height() * ((db.clamp(-18.0, 18.0) + 18.0) / 36.0);
    let dbf = |y: f32| (((g.bottom() - y) / g.height()) * 36.0 - 18.0).clamp(-18.0, 18.0);
    let nodes: [(&FloatParam, &FloatParam, &str); 3] = [(&p.eq_lf, &p.eq_lg, "LOW"), (&p.eq_mf, &p.eq_mg, "MID"), (&p.eq_hf, &p.eq_hg, "HIGH")];
    for (i, (fp, gp, name)) in nodes.iter().enumerate() {
        let c = Pos2::new(xf(fp.value()), yd(gp.value()));
        let hit = Rect::from_center_size(c, Vec2::splat(20.0));
        let resp = ui.interact(hit, egui::Id::new(("eqnode", i)), Sense::click_and_drag()).on_hover_text(format!("{name} band: drag = frequency / gain{}", if i == 1 { ", wheel = Q" } else { "" }));
        if resp.drag_started() {
            setter.begin_set_parameter(*fp);
            setter.begin_set_parameter(*gp);
        }
        if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
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
                let q = p.eq_mq.unmodulated_normalized_value();
                set_norm(setter, &p.eq_mq, q + sc / 1200.0);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        ui.painter().circle_filled(c, if hot { 7.0 } else { 5.5 }, Color32::BLACK);
        ui.painter().circle_stroke(c, if hot { 7.0 } else { 5.5 }, Stroke::new(1.6_f32, WHITE));
        ui.painter().text(c + Vec2::new(0.0, -13.0), Align2::CENTER_CENTER, format!("{}", i + 1), mono(8.5), DIM);
    }
}

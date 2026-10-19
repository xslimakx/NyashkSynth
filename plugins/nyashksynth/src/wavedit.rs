//! Wavetable editor: time-domain drawing, harmonic bars (amplitude + phase), a waterfall view
//! and frame operations. Opens when the oscillator display is clicked.

use crate::params::*;
use crate::shared::Shared;
use crate::ui::*;
use crate::wavetable::{self, amp_phase, from_amp_phase, spectrum_of, time_of, Spec, Table, MAX_FRAMES, N};
use nih_plug::prelude::*;
use nih_plug_egui::egui::{self, epaint::PathStroke, Align2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use std::f32::consts::PI;
use std::sync::Arc;

const ZOOMS: [usize; 5] = [64, 128, 256, 512, 1023];

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Draw,
    Line,
    Smooth,
}

#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Frame,
    Sel,
    All,
}

#[derive(Clone, Copy, PartialEq)]
enum Ramp {
    Flat,
    In,
    Out,
    Middle,
    Edges,
}

pub struct WavEdit {
    pub osc: usize,
    tbl: Table,
    name: String,
    frame: usize,
    spec: Spec,
    time: Vec<f32>,
    tool: Tool,
    zoom: usize,
    undo: Vec<Table>,
    clip: Option<Vec<f32>>,
    last_pub: f64,
    last_edit: f64,
    dirty_pub: bool,
    dirty_enc: bool,
    line_start: Option<(f32, f32, Vec<f32>)>,
    last_pt: Option<(f32, f32)>,
    spectral: bool,
    status: String,
    edited: bool,
    formula: String,
    formula_err: String,
    example: usize,
    scope: Scope,
    sel: Option<(usize, usize)>,
    anchor: usize,
    strength: f32,
    ramp: Ramp,
    soft: f32,
    redo: Vec<Table>,
    peaks: Vec<f32>,
    peaks_dirty: bool,
    fit_hold: Option<f32>,
    saved: String,
    vary: f32,
    drift: f32,
    drag_orig: Option<(usize, usize, Vec<Vec<f32>>, Vec<f32>)>,
    multi_pending: bool,
    last_multi: f64,
    multi_cost: f64,
}

fn fill_under(painter: &egui::Painter, pts: &[Pos2], bottom: f32, color: Color32) {
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

impl WavEdit {
    pub fn new(osc: usize, tbl: Table, name: String, pos: f32) -> WavEdit {
        let frame = ((pos.clamp(0.0, 1.0) * (tbl.frames() - 1) as f32).round() as usize).min(tbl.frames() - 1);
        let mut w = WavEdit {
            osc,
            tbl,
            name,
            frame,
            spec: vec![(0.0, 0.0); N / 2],
            time: vec![0.0; N],
            tool: Tool::Draw,
            zoom: 1,
            undo: Vec::new(),
            clip: None,
            last_pub: 0.0,
            last_edit: 0.0,
            dirty_pub: false,
            dirty_enc: false,
            line_start: None,
            last_pt: None,
            spectral: true,
            status: String::new(),
            edited: false,
            formula: "sin(2*pi*x + w*6*sin(2*pi*x))".to_string(),
            formula_err: String::new(),
            example: 1,
            scope: Scope::Frame,
            sel: None,
            anchor: 0,
            strength: 1.0,
            ramp: Ramp::Flat,
            soft: 0.0,
            redo: Vec::new(),
            peaks: Vec::new(),
            peaks_dirty: true,
            fit_hold: None,
            saved: String::new(),
            vary: 0.0,
            drift: 0.0,
            drag_orig: None,
            multi_pending: false,
            last_multi: 0.0,
            multi_cost: 0.0,
        };
        w.anchor = w.frame;
        if std::env::var("NYASHK_DEMO").is_ok() {
            // developer aid for screenshots: a morphing table with a selected range
            if let Ok(ex) = crate::formula::parse("sin(2*pi*x + w*6*sin(2*pi*x)) + w*sin(10*pi*x)/3") {
                let frames: Vec<Vec<f32>> = (0..64)
                    .map(|i| {
                        let mut t = crate::formula::frame(&ex, i as f32 / 63.0, N);
                        let pk = t.iter().fold(0.0f32, |a, b| a.max(b.abs())).max(1e-6);
                        t.iter_mut().for_each(|v| *v *= 0.9 / pk);
                        t
                    })
                    .collect();
                w.tbl = Table::from_frames(&frames);
                w.frame = 30;
                w.anchor = 14;
                w.sel = Some((14, 42));
                w.scope = Scope::Sel;
                w.ramp = Ramp::Middle;
                w.soft = 0.5;
                w.strength = 0.8;
                w.vary = 0.25;
                w.drift = 0.2;
                w.name = "Demo Morph".into();
            }
        }
        w.reload();
        w
    }

    fn reload(&mut self) {
        self.frame = self.frame.min(self.tbl.frames() - 1);
        self.time = self.tbl.frame_time(self.frame);
        self.spec = spectrum_of(&self.time);
        self.peaks_dirty = true;
        let nf = self.tbl.frames();
        self.anchor = self.anchor.min(nf - 1);
        if let Some((a, b)) = self.sel {
            let (a, b) = (a.min(nf - 1), b.min(nf - 1));
            self.sel = if a < b { Some((a, b)) } else { None };
        }
    }

    fn push_undo(&mut self) {
        self.redo.clear();
        self.undo.push(self.tbl.clone());
        if self.undo.len() > 12 {
            self.undo.remove(0);
        }
    }

    fn touch(&mut self, now: f64) {
        self.dirty_pub = true;
        self.dirty_enc = true;
        self.last_edit = now;
        if !self.edited {
            self.edited = true;
            if !self.name.ends_with('*') {
                self.name.push_str(" *");
            }
        }
    }

    fn set_time(&mut self, t: Vec<f32>, now: f64) {
        self.tbl.set_frame(self.frame, &t);
        self.time = t;
        self.spec = spectrum_of(&self.time);
        self.touch(now);
    }

    fn set_spec(&mut self, s: Spec, now: f64) {
        let t = time_of(&s);
        self.tbl.set_frame(self.frame, &t);
        self.time = t;
        self.spec = s;
        self.touch(now);
    }

    /// Frames the current scope covers (inclusive).
    fn scope_range(&self) -> (usize, usize) {
        match self.scope {
            Scope::Frame => (self.frame, self.frame),
            Scope::Sel => self.sel.unwrap_or((self.frame, self.frame)),
            Scope::All => (0, self.tbl.frames() - 1),
        }
    }

    /// How strongly frame `i` of the range `lo..=hi` is affected (strength x fade ramp).
    fn weight(&self, i: usize, lo: usize, hi: usize) -> f32 {
        if hi <= lo {
            return self.strength;
        }
        let p = (i - lo) as f32 / (hi - lo) as f32;
        let r = match self.ramp {
            Ramp::Flat => {
                if self.soft <= 0.001 {
                    1.0
                } else {
                    let e = (p.min(1.0 - p) / (self.soft * 0.5)).clamp(0.0, 1.0);
                    e * e * (3.0 - 2.0 * e)
                }
            }
            Ramp::In => p,
            Ramp::Out => 1.0 - p,
            Ramp::Middle => (p * PI).sin(),
            Ramp::Edges => 1.0 - (p * PI).sin(),
        };
        self.strength * r
    }

    /// Writes `new` into frame `i`, mixed with what was there by weight `a`.
    fn blend_frame(&mut self, i: usize, new: &[f32], a: f32) {
        if a <= 0.0 {
            return;
        }
        if a >= 0.999 {
            self.tbl.set_frame(i, new);
        } else {
            let old = self.tbl.frame_time(i);
            let mixed: Vec<f32> = old.iter().zip(new).map(|(o, n)| o + (n - o) * a).collect();
            self.tbl.set_frame(i, &mixed);
        }
    }

    /// Applies a time-domain transform to every frame in the scope, faded by strength and ramp.
    fn apply_time(&mut self, now: f64, f: impl Fn(&mut Vec<f32>)) {
        self.push_undo();
        let (lo, hi) = self.scope_range();
        for i in lo..=hi {
            let a = self.weight(i, lo, hi);
            let mut t = self.tbl.frame_time(i);
            f(&mut t);
            self.blend_frame(i, &t, a);
        }
        self.reload();
        self.touch(now);
    }

    /// Interpolates the frames between the two ends of the scope.
    fn morph_range(&mut self, now: f64) -> bool {
        let (lo, hi) = self.scope_range();
        if hi < lo + 2 {
            return false;
        }
        self.push_undo();
        let a = self.tbl.frame_time(lo);
        let b = self.tbl.frame_time(hi);
        let (sa, sb) = (spectrum_of(&a), spectrum_of(&b));
        for i in lo + 1..hi {
            let u = (i - lo) as f32 / (hi - lo) as f32;
            let t: Vec<f32> = if self.spectral { time_of(&wavetable::morph_spec(&sa, &sb, u as f64)) } else { a.iter().zip(&b).map(|(p, q)| p + (q - p) * u).collect() };
            self.blend_frame(i, &t, self.strength);
        }
        self.reload();
        self.touch(now);
        true
    }

    /// Called when a drag in the wave view starts: remembers the frames the drawing will spread to.
    fn begin_multi(&mut self) {
        let (lo, hi) = self.scope_range();
        self.multi_pending = false;
        self.drag_orig = if hi > lo { Some((lo, hi, (lo..=hi).map(|i| self.tbl.frame_time(i)).collect(), self.time.clone())) } else { None };
    }

    /// The current frame takes the drawn wave exactly; the other frames in scope receive the same
    /// change, scaled by strength x fade x variation and optionally slid along the wave (drift).
    fn commit_draw(&mut self, t: Vec<f32>, now: f64) {
        self.set_time(t, now);
        if self.drag_orig.is_some() {
            self.multi_pending = true;
            if now - self.last_multi > (self.multi_cost * 3.0).max(0.1) {
                self.apply_multi(now);
            }
        }
    }

    fn apply_multi(&mut self, now: f64) {
        if !self.multi_pending {
            return;
        }
        self.multi_pending = false;
        let t_start = std::time::Instant::now();
        let Some((lo, hi, orig, cur0)) = self.drag_orig.take() else { return };
        let delta: Vec<f32> = self.time.iter().zip(&cur0).map(|(a, b)| a - b).collect();
        for i in lo..=hi {
            if i == self.frame {
                continue;
            }
            let mut a = self.weight(i, lo, hi);
            if self.vary > 0.0 {
                let h = (i as u32).wrapping_mul(2654435761) ^ 0x9E37_79B9;
                let h = (h ^ (h >> 15)).wrapping_mul(2246822519);
                a *= 1.0 - self.vary * ((h >> 8) & 0xFFFF) as f32 / 65535.0;
            }
            if a <= 0.0005 {
                continue;
            }
            let shift = (((i as i64 - self.frame as i64) as f32) * self.drift * 12.0).round() as i64;
            let o = &orig[i - lo];
            let new: Vec<f32> = (0..N).map(|k| o[k] + delta[(k as i64 - shift).rem_euclid(N as i64) as usize] * a).collect();
            self.tbl.set_frame(i, &new);
        }
        self.drag_orig = Some((lo, hi, orig, cur0));
        self.peaks_dirty = true;
        self.touch(now);
        self.last_multi = now;
        self.multi_cost = t_start.elapsed().as_secs_f64();
    }

    fn run_formula(&mut self, now: f64) {
        match crate::formula::parse(&self.formula) {
            Err(e) => self.formula_err = e,
            Ok(ex) => {
                self.formula_err.clear();
                self.push_undo();
                let nf = self.tbl.frames();
                let (lo, hi) = self.scope_range();
                for i in lo..=hi {
                    let w = if nf > 1 { i as f32 / (nf - 1) as f32 } else { 0.0 };
                    let mut t = crate::formula::frame(&ex, w, N);
                    let pk = t.iter().fold(0.0f32, |a, b| a.max(b.abs()));
                    if pk > 1.0 {
                        t.iter_mut().for_each(|x| *x *= 0.95 / pk);
                    }
                    let mean = t.iter().sum::<f32>() / N as f32;
                    t.iter_mut().for_each(|x| *x -= mean);
                    let a = self.weight(i, lo, hi);
                    self.blend_frame(i, &t, a);
                }
                self.reload();
                self.touch(now);
            }
        }
    }

    fn replace_table(&mut self, t: Table, now: f64) {
        self.tbl = t;
        self.reload();
        self.touch(now);
    }

    /// Send the working copy to the audio thread / store it in the parameters.
    pub fn flush(&mut self, shared: &Shared, p: &SynthParams, now: f64, force: bool) {
        if self.dirty_pub && (force || now - self.last_pub > 0.07) {
            shared.waves[self.osc].set(Some(Arc::new(self.tbl.clone())));
            self.dirty_pub = false;
            self.last_pub = now;
        }
        if self.dirty_enc && (force || now - self.last_edit > 0.4) {
            let s = wavetable::encode_table(&self.name, &self.tbl);
            let op = crate::editor::osc(p, self.osc);
            if let Ok(mut g) = op.wave.lock() {
                *g = s;
            }
            self.dirty_enc = false;
        }
    }

    pub fn has_pending(&self) -> bool {
        self.dirty_pub || self.dirty_enc
    }

    fn peak_normalize(v: &mut [f32]) {
        let pk = v.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        if pk > 1e-6 {
            let g = 0.95 / pk;
            v.iter_mut().for_each(|x| *x *= g);
        }
    }

    fn pat_spec(&self, kind: u8) -> Spec {
        let mut s: Spec = vec![(0.0, 0.0); N / 2];
        for k in 1..N / 2 {
            let kf = k as f64;
            let b = match kind {
                0 => {
                    if k == 1 {
                        1.0
                    } else {
                        0.0
                    }
                }
                1 => {
                    if k % 2 == 1 {
                        8.0 / (std::f64::consts::PI.powi(2)) * if ((k - 1) / 2) % 2 == 0 { 1.0 } else { -1.0 } / (kf * kf)
                    } else {
                        0.0
                    }
                }
                2 => 2.0 / std::f64::consts::PI * if k % 2 == 1 { 1.0 } else { -1.0 } / kf,
                3 => {
                    if k % 2 == 1 {
                        4.0 / (std::f64::consts::PI * kf)
                    } else {
                        0.0
                    }
                }
                _ => {
                    // narrow pulse
                    s[k].0 = 2.0 / (std::f64::consts::PI * kf) * (kf * std::f64::consts::PI * 0.15).sin();
                    0.0
                }
            };
            s[k].1 = b;
        }
        s
    }

    fn goto(&mut self, setter: &ParamSetter, op: &OscParams, f: usize) {
        let nf = self.tbl.frames();
        self.frame = f.min(nf - 1);
        self.reload();
        set_f(setter, &op.pos, self.frame as f32 / (nf - 1) as f32);
    }

    /// Move to frame `f`; with `extend` the range grows from the anchor, otherwise it is a plain move.
    fn select_to(&mut self, setter: &ParamSetter, op: &OscParams, f: usize, extend: bool) {
        if extend {
            self.sel = if self.anchor != f { Some((self.anchor.min(f), self.anchor.max(f))) } else { None };
        } else {
            self.anchor = f;
            self.sel = None;
        }
        if extend && self.sel.is_some() && self.scope == Scope::Frame {
            self.scope = Scope::Sel;
        } else if !extend && self.scope == Scope::Sel {
            self.scope = Scope::Frame;
        }
        if f != self.frame {
            self.goto(setter, op, f);
        }
    }

    fn slider(ui: &mut Ui, r: Rect, id: &str, val: &mut f32, text: &str, tip: &str) {
        let resp = ui.interact(r, egui::Id::new(id), Sense::click_and_drag()).on_hover_text(tip);
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                *val = ((pt.x - r.left()) / r.width()).clamp(0.0, 1.0);
                if *val < 0.02 {
                    *val = 0.0;
                } else if *val > 0.98 {
                    *val = 1.0;
                }
            }
        }
        dark_box(ui, r);
        let fillr = Rect::from_min_size(r.min, Vec2::new(r.width() * *val, r.height()));
        ui.painter().rect_filled(fillr.shrink(1.0), CornerRadius::same(3), Color32::from_rgb(70, 70, 70));
        ui.painter().text(r.center(), Align2::CENTER_CENTER, text, mono(10.0), WHITE);
    }

    fn timeline(&mut self, ui: &mut Ui, r: Rect, setter: &ParamSetter, op: &OscParams) {
        let nf = self.tbl.frames();
        let resp = ui
            .interact(r, egui::Id::new("wf_strip"), Sense::click_and_drag())
            .on_hover_text("Click: choose a frame. Drag: select a range of frames. Shift+click: extend the selection. Arrow keys also work (Shift extends).");
        let at = |x: f32| (((x - r.left()) / r.width() * nf as f32).floor().max(0.0) as usize).min(nf - 1);
        let shift = ui.input(|i| i.modifiers.shift);
        if resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                self.select_to(setter, op, at(pt.x), shift);
            }
        } else if resp.dragged() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let f = at(pt.x);
                if !shift {
                    if let Some(o) = ui.input(|i| i.pointer.press_origin()) {
                        self.anchor = at(o.x);
                    }
                }
                self.select_to(setter, op, f, true);
            }
        }
        dark_box(ui, r);
        let painter = ui.painter().with_clip_rect(r.shrink(1.0));
        let cw = (r.width() - 2.0) / nf as f32;
        if let Some((a, b)) = self.sel {
            let band = Rect::from_min_max(Pos2::new(r.left() + 1.0 + cw * a as f32, r.top() + 1.0), Pos2::new(r.left() + 1.0 + cw * (b + 1) as f32, r.bottom() - 1.0));
            painter.rect_filled(band, CornerRadius::ZERO, Color32::from_rgb(34, 34, 34));
        }
        let gpk = self.peaks.iter().fold(1.0f32, |a, b| a.max(*b));
        for i in 0..nf {
            let pk = self.peaks.get(i).copied().unwrap_or(0.0);
            let h = 3.0 + 21.0 * (pk / gpk).min(1.0);
            let x = r.left() + 1.0 + cw * i as f32;
            let insel = self.sel.map_or(false, |(a, b)| i >= a && i <= b);
            let c = if i == self.frame { WHITE } else if insel { Color32::from_rgb(175, 175, 175) } else { Color32::from_rgb(78, 78, 78) };
            painter.rect_filled(Rect::from_min_max(Pos2::new(x, r.bottom() - 3.0 - h), Pos2::new((x + cw - if cw > 3.0 { 1.0 } else { 0.0 }).max(x + 1.0), r.bottom() - 3.0)), CornerRadius::ZERO, c);
        }
    }

    /// Draw the editor. Returns true when it should close.
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, rect: Rect, now: f64, status: &mut String) -> bool {
        let mut close = false;
        let op = crate::editor::osc(p, self.osc);
        let nfr = self.tbl.frames();
        if self.peaks_dirty || self.peaks.len() != nfr {
            self.peaks = (0..nfr).map(|i| self.tbl.frame_time(i).iter().fold(0.0f32, |a, b| a.max(b.abs()))).collect();
            self.peaks_dirty = false;
        }
        ui.painter().rect_filled(rect, CornerRadius::same(8), Color32::from_rgb(8, 8, 8));
        ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0_f32, MID), StrokeKind::Inside);
        let x0 = rect.left() + 14.0;
        let w = rect.width() - 28.0;
        let cx = x0 + 70.0; // buttons start here, captions sit left of it
        let sel_txt = match self.sel {
            Some((a, b)) => format!("   selected {}-{} ({})", a + 1, b + 1, b - a + 1),
            None => String::new(),
        };
        ui.painter().text(
            Pos2::new(x0, rect.top() + 20.0),
            Align2::LEFT_CENTER,
            format!("WAVETABLE EDITOR   OSC {}   {}   frame {}/{}{}", crate::editor::OSC_NAMES[self.osc.min(2)], self.name.chars().take(28).collect::<String>(), self.frame + 1, nfr, sel_txt),
            mono(12.0),
            WHITE,
        );
        let mut op_clicked: Option<&str> = None;
        if pill_tip(ui, Rect::from_min_size(Pos2::new(rect.right() - 74.0, rect.top() + 8.0), Vec2::new(60.0, 24.0)), "CLOSE", false, "Close the editor (Esc)") || ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.memory(|m| m.focused().is_some()) {
            close = true;
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(rect.right() - 140.0, rect.top() + 8.0), Vec2::new(60.0, 24.0)), "REDO", false, "Redo (Ctrl+Shift+Z)") {
            op_clicked = Some("REDO");
        }
        if pill_tip(ui, Rect::from_min_size(Pos2::new(rect.right() - 206.0, rect.top() + 8.0), Vec2::new(60.0, 24.0)), "UNDO", false, "Undo the last change (Ctrl+Z)") {
            op_clicked = Some("UNDO");
        }
        let (ctrl_z, shift_k, ctrl_y) = ui.input(|i| (i.modifiers.ctrl && i.key_pressed(egui::Key::Z), i.modifiers.shift, i.modifiers.ctrl && i.key_pressed(egui::Key::Y)));
        if ctrl_y || (ctrl_z && shift_k) {
            op_clicked = Some("REDO");
        } else if ctrl_z {
            op_clicked = Some("UNDO");
        }
        // arrow keys walk through the frames
        if !ui.memory(|m| m.focused().is_some()) {
            let (l, r, sh) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight), i.modifiers.shift));
            if l || r {
                let f = if l { self.frame.saturating_sub(1) } else { (self.frame + 1).min(nfr - 1) };
                self.select_to(setter, op, f, sh);
            }
        }

        // ---------------- frame strip
        let mut y = rect.top() + 40.0;
        let strip = Rect::from_min_size(Pos2::new(x0, y), Vec2::new(w, 30.0));
        self.timeline(ui, strip, setter, op);
        y += 38.0;
        let yc = std::cell::Cell::new(y);
        let btn = |x: f32, wd: f32| Rect::from_min_size(Pos2::new(x, yc.get()), Vec2::new(wd, 24.0));
        let cap = |ui: &Ui, y: f32, t: &str| {
            ui.painter().text(Pos2::new(x0, y + 12.0), Align2::LEFT_CENTER, t, mono(10.0), DIM);
        };

        // ---------------- row: frame editing, drawing tool, basic shapes
        cap(ui, y, "EDIT");
        let mut bx = cx;
        for (label, tip) in [
            ("INSERT", "Duplicate the frame (or the whole selected range) right after itself"),
            ("DELETE", "Remove the frame (or the whole selected range)"),
            ("COPY", "Copy the current frame"),
            ("PASTE", "Paste the copied frame over the frames in APPLY TO"),
            ("CLEAR", "Silence the frames in APPLY TO"),
        ] {
            if pill_tip(ui, btn(bx, 62.0), label, false, tip) {
                op_clicked = Some(label);
            }
            bx += 66.0;
        }
        bx += 14.0;
        cap_at(ui, bx, y, "DRAW WITH");
        bx += 78.0;
        for (t, label, tip) in [(Tool::Draw, "DRAW", "Freehand drawing in the wave view"), (Tool::Line, "LINE", "Drag out a straight line"), (Tool::Smooth, "SMOOTH", "Brush that smooths the wave")] {
            if pill_tip(ui, btn(bx, 62.0), label, self.tool == t, tip) {
                self.tool = t;
            }
            bx += 66.0;
        }
        bx += 14.0;
        cap_at(ui, bx, y, "SET TO");
        bx += 54.0;
        let mut shape: Option<u8> = None;
        for (i, label) in ["SINE", "TRI", "SAW", "SQR", "PULSE"].iter().enumerate() {
            if pill_tip(ui, btn(bx, 48.0), label, false, "Replace the frames in APPLY TO with a basic shape") {
                shape = Some(i as u8);
            }
            bx += 51.0;
        }

        // ---------------- row: scope + strength
        y += 30.0;
        yc.set(y);
        cap(ui, y, "APPLY TO");
        bx = cx;
        for (sc, label, tip, wd) in [
            (Scope::Frame, "FRAME", "Operations change only the current frame", 62.0),
            (Scope::Sel, "SELECTION", "Operations change the frames selected on the strip above (drag on it to select several)", 92.0),
            (Scope::All, "ALL FRAMES", "Operations change every frame", 92.0),
        ] {
            if pill_tip(ui, btn(bx, wd), label, self.scope == sc, tip) {
                self.scope = sc;
            }
            bx += wd + 4.0;
        }
        bx += 14.0;
        cap_at(ui, bx, y, "STRENGTH");
        bx += 70.0;
        let txt = format!("{}%", (self.strength * 100.0).round() as i32);
        Self::slider(ui, btn(bx, 170.0), "wed_strength", &mut self.strength, &txt, "How much of each change is applied: 100% replaces the wave, lower values blend the result with the original");
        bx += 190.0;
        let (lo, hi) = self.scope_range();
        let n_aff = hi - lo + 1;
        ui.painter().text(
            Pos2::new(bx, y + 12.0),
            Align2::LEFT_CENTER,
            if self.scope == Scope::Sel && self.sel.is_none() { "no range selected: drag on the strip above".to_string() } else { format!("will change {} frame{} ({}-{})", n_aff, if n_aff == 1 { "" } else { "s" }, lo + 1, hi + 1) },
            mono(10.0),
            if self.scope == Scope::Sel && self.sel.is_none() { WHITE } else { DIM },
        );

        // ---------------- row: fade across the range
        y += 30.0;
        yc.set(y);
        cap(ui, y, "FADE");
        bx = cx;
        for (rp, label, tip, wd) in [
            (Ramp::Flat, "FLAT", "Same amount on every frame of the range", 52.0),
            (Ramp::In, "IN", "Starts at nothing on the first frame, full on the last", 44.0),
            (Ramp::Out, "OUT", "Full on the first frame, fades to nothing on the last", 48.0),
            (Ramp::Middle, "MIDDLE", "Strongest in the centre of the range, smooth at both ends", 66.0),
            (Ramp::Edges, "EDGES", "Strongest at both ends of the range, nothing in the centre", 62.0),
        ] {
            if pill_tip(ui, btn(bx, wd), label, self.ramp == rp, tip) {
                self.ramp = rp;
            }
            bx += wd + 4.0;
        }
        bx += 14.0;
        cap_at(ui, bx, y, "SOFT EDGES");
        bx += 84.0;
        let txt = if self.soft <= 0.001 { "HARD".to_string() } else { format!("{}%", (self.soft * 100.0).round() as i32) };
        Self::slider(ui, btn(bx, 170.0), "wed_soft", &mut self.soft, &txt, "FLAT only: how wide the smooth fade is at both ends of the range. HARD = the change starts abruptly.");
        bx += 190.0;
        bx += 4.0;
        cap_at(ui, bx, y, "VARY");
        bx += 38.0;
        let txt = format!("{}%", (self.vary * 100.0).round() as i32);
        Self::slider(ui, btn(bx, 80.0), "wed_vary", &mut self.vary, &txt, "Every frame gets a different amount of the change (random per frame) instead of the same");
        bx += 92.0;
        cap_at(ui, bx, y, "DRIFT");
        bx += 44.0;
        let txt = format!("{}%", (self.drift * 100.0).round() as i32);
        Self::slider(ui, btn(bx, 80.0), "wed_drift", &mut self.drift, &txt, "Drawing on several frames: the drawn spot slides sideways a little more on each next frame, so the change travels through the table");

        // ---------------- row: processing
        y += 30.0;
        yc.set(y);
        cap(ui, y, "PROCESS");
        bx = cx;
        for (label, tip) in [
            ("NORM", "Normalise to full level"),
            ("REVERSE", "Reverse in time"),
            ("INVERT", "Flip the polarity"),
            ("LOWPASS", "Remove the highest harmonics"),
            ("HIGHPASS", "Remove the lowest harmonics"),
            ("FOLD", "Wave folding"),
            ("CRUSH", "Bit crusher"),
            ("MIRROR", "Mirror the first half onto the second half"),
            ("SMOOTH", "Soften the corners"),
            ("RANDOM", "Random harmonic spectrum"),
            ("MORPH", "Smoothly morph the frames between the first and last frame of the range (needs SELECTION or ALL)"),
        ] {
            let wd = if label.len() > 6 { 76.0 } else { 62.0 };
            if pill_tip(ui, btn(bx, wd), label, false, tip) {
                op_clicked = Some(label);
            }
            bx += wd + 4.0;
        }

        // ---------------- row: table
        y += 30.0;
        yc.set(y);
        cap(ui, y, "TABLE");
        bx = cx;
        cap_at(ui, bx, y, "FRAMES");
        bx += 52.0;
        let mut resize: Option<usize> = None;
        for n in [2usize, 8, 16, 32, 64, 128, 256] {
            if pill_tip(ui, btn(bx, 38.0), &n.to_string(), nfr == n, "Change the number of frames (new frames are interpolated)") {
                resize = Some(n);
            }
            bx += 41.0;
        }
        bx += 10.0;
        if pill_tip(ui, btn(bx, 120.0), if self.spectral { "SMOOTH SPECTRAL" } else { "LINEAR MIX" }, false, "How new frames and MORPH are interpolated: spectral (harmonics glide, smoother) or linear crossfade (straight blend of the waves)") {
            self.spectral = !self.spectral;
        }
        bx += 136.0;
        let lib = crate::files::tables_dir().map(|d| d.join("My Waves").display().to_string()).unwrap_or_default();
        if pill_tip(ui, btn(bx, 132.0), "SAVE TO LIBRARY", false, &format!("Save this wavetable as a .wav file (Serum compatible).\nIt then shows up in the WAVETABLES browser under the category My Waves.\nFolder: {lib}")) {
            op_clicked = Some("EXPORT");
        }
        bx += 136.0;
        if pill_tip(ui, btn(bx, 100.0), "OPEN FOLDER", false, "Open the folder where saved wavetables live") {
            if let Some(d) = crate::files::tables_dir().map(|d| d.join("My Waves")) {
                let _ = std::fs::create_dir_all(&d);
                crate::files::open_folder(&d);
            }
        }

        bx += 104.0;
        if pill_tip(ui, btn(bx, 112.0), "SINGLE WAVE", false, "Make the table one single wave: the current frame is kept and the other frames become copies of it, so POS has no effect (like a one-frame table in Serum)") {
            op_clicked = Some("SINGLE");
        }

        // ---------------- row: formula
        y += 30.0;
        yc.set(y);
        cap(ui, y, "FORMULA");
        let te = Rect::from_min_size(Pos2::new(cx, y), Vec2::new(430.0, 24.0));
        ui.put(te, egui::TextEdit::singleline(&mut self.formula).font(mono(11.0)).hint_text("sin(2*pi*x + w*sin(4*pi*x))").desired_width(430.0));
        let mut run = false;
        if pill_tip(ui, btn(cx + 438.0, 90.0), "GENERATE", false, "Build the frames in APPLY TO from the formula (x = phase 0..1, w = frame position 0..1)") {
            run = true;
        }
        if pill_tip(ui, btn(cx + 532.0, 80.0), "EXAMPLE", false, "Cycle through example formulas") {
            const EX: [&str; 8] = [
                "sin(2*pi*x)",
                "sin(2*pi*x + w*6*sin(2*pi*x))",
                "sin(2*pi*x) + w*sin(6*pi*x)/2",
                "tanh(sin(2*pi*x)*(1+w*8))",
                "mix(saw(x), sqr(x), w)",
                "sin(2*pi*pow(x, 1+w*3))",
                "tri(x + w*0.5) * sin(4*pi*x)",
                "sin(2*pi*x*(1+floor(w*7)))",
            ];
            self.example = (self.example + 1) % EX.len();
            self.formula = EX[self.example].to_string();
        }
        if run {
            self.run_formula(now);
        }
        let msg = if self.formula_err.is_empty() { "x=phase  w=frame  (sin cos tanh saw tri...)".to_string() } else { format!("formula error: {}", self.formula_err) };
        ui.painter().text(Pos2::new(cx + 624.0, y + 12.0), Align2::LEFT_CENTER, msg.chars().take(48).collect::<String>(), mono(8.5), if self.formula_err.is_empty() { DIM } else { WHITE });

        // ---------------- apply toolbar operations
        if let Some(k) = shape {
            let mut t = time_of(&self.pat_spec(k));
            Self::peak_normalize(&mut t);
            self.apply_time(now, move |x| *x = t.clone());
        }
        if let Some(n) = resize {
            if n != nfr {
                self.push_undo();
                let t = self.tbl.resized(n, self.spectral);
                self.sel = None;
                self.replace_table(t, now);
                self.frame = self.frame.min(n - 1);
                self.reload();
            }
        }
        if let Some(c) = op_clicked {
            match c {
                "INSERT" => {
                    let (a, b) = if self.scope == Scope::Sel { self.sel.unwrap_or((self.frame, self.frame)) } else { (self.frame, self.frame) };
                    let cnt = b - a + 1;
                    if nfr + cnt <= MAX_FRAMES {
                        self.push_undo();
                        let mut fr = self.tbl.to_frames();
                        let copy: Vec<Vec<f32>> = fr[a..=b].to_vec();
                        for (k, f) in copy.into_iter().enumerate() {
                            fr.insert(b + 1 + k, f);
                        }
                        self.tbl = Table::from_frames(&fr);
                        self.frame = b + 1;
                        self.anchor = b + 1;
                        self.sel = if cnt > 1 { Some((b + 1, b + cnt)) } else { None };
                        self.reload();
                        self.touch(now);
                    } else {
                        *status = format!("table is full ({MAX_FRAMES} frames max)");
                    }
                }
                "DELETE" => {
                    let (a, b) = if self.scope == Scope::Sel { self.sel.unwrap_or((self.frame, self.frame)) } else { (self.frame, self.frame) };
                    let cnt = b - a + 1;
                    if nfr > cnt + 1 {
                        self.push_undo();
                        let mut fr = self.tbl.to_frames();
                        fr.drain(a..=b);
                        self.tbl = Table::from_frames(&fr);
                        self.sel = None;
                        self.frame = a.min(fr.len() - 1);
                        self.anchor = self.frame;
                        self.reload();
                        self.touch(now);
                    } else {
                        *status = "a table needs at least 2 frames".to_string();
                    }
                }
                "SINGLE" => {
                    self.push_undo();
                    let t = self.time.clone();
                    self.tbl = Table::from_frames(&[t.clone(), t]);
                    self.sel = None;
                    self.scope = Scope::Frame;
                    self.frame = 0;
                    self.anchor = 0;
                    self.reload();
                    self.touch(now);
                    set_f(setter, &op.pos, 0.0);
                    *status = "single wave: all frames are identical now".to_string();
                }
                "COPY" => self.clip = Some(self.time.clone()),
                "PASTE" => {
                    if let Some(c) = self.clip.clone() {
                        self.apply_time(now, move |x| *x = c.clone());
                    }
                }
                "CLEAR" => self.apply_time(now, |x| x.iter_mut().for_each(|v| *v = 0.0)),
                "UNDO" => {
                    if let Some(t) = self.undo.pop() {
                        let cur = std::mem::replace(&mut self.tbl, t);
                        self.redo.push(cur);
                        self.reload();
                        self.touch(now);
                    }
                }
                "REDO" => {
                    if let Some(t) = self.redo.pop() {
                        let cur = std::mem::replace(&mut self.tbl, t);
                        self.undo.push(cur);
                        self.reload();
                        self.touch(now);
                    }
                }
                "NORM" => self.apply_time(now, |t| Self::peak_normalize(t)),
                "REVERSE" => self.apply_time(now, |t| t.reverse()),
                "INVERT" => self.apply_time(now, |t| t.iter_mut().for_each(|v| *v = -*v)),
                "LOWPASS" => self.apply_time(now, |t| {
                    let mut sp = spectrum_of(t);
                    for k in 1..N / 2 {
                        let g = 1.0 / (1.0 + (k as f64 / 24.0).powi(4));
                        sp[k].0 *= g;
                        sp[k].1 *= g;
                    }
                    *t = time_of(&sp);
                }),
                "HIGHPASS" => self.apply_time(now, |t| {
                    let mut sp = spectrum_of(t);
                    for k in 1..N / 2 {
                        let g = 1.0 - 1.0 / (1.0 + (k as f64 / 6.0).powi(3));
                        sp[k].0 *= g;
                        sp[k].1 *= g;
                    }
                    *t = time_of(&sp);
                    Self::peak_normalize(t);
                }),
                "FOLD" => self.apply_time(now, |t| {
                    t.iter_mut().for_each(|v| *v = (*v * 3.0).sin());
                    Self::peak_normalize(t);
                }),
                "CRUSH" => self.apply_time(now, |t| t.iter_mut().for_each(|v| *v = (*v * 6.0).round() / 6.0)),
                "MIRROR" => self.apply_time(now, |t| {
                    let n = t.len();
                    for i in n / 2..n {
                        t[i] = t[n - 1 - i];
                    }
                }),
                "SMOOTH" => self.apply_time(now, |t| {
                    let n = t.len();
                    let src = t.clone();
                    for i in 0..n {
                        t[i] = (src[(i + n - 2) % n] + 2.0 * src[(i + n - 1) % n] + 3.0 * src[i] + 2.0 * src[(i + 1) % n] + src[(i + 2) % n]) / 9.0;
                    }
                }),
                "RANDOM" => self.apply_time(now, |t| {
                    let mut sp = spectrum_of(t);
                    let mut r = 0x2545_F491u32 ^ (t[0].to_bits());
                    for k in 1..40 {
                        r ^= r << 13;
                        r ^= r >> 17;
                        r ^= r << 5;
                        let a = (r as f64 / u32::MAX as f64) / (k as f64).powf(0.8);
                        r ^= r << 13;
                        r ^= r >> 17;
                        r ^= r << 5;
                        let ph = (r as f64 / u32::MAX as f64) * std::f64::consts::TAU;
                        sp[k] = (a * ph.cos(), a * ph.sin());
                    }
                    *t = time_of(&sp);
                    Self::peak_normalize(t);
                }),
                "MORPH" => {
                    if !self.morph_range(now) {
                        *status = "MORPH needs 3+ frames: pick SELECTION (drag on the strip) or ALL FRAMES".to_string();
                    }
                }
                "EXPORT" => match self.export() {
                    Ok(path) => {
                        *status = format!("saved wavetable {path} (WAVETABLES > My Waves)");
                        self.saved = format!("Saved to library as \"{path}\"  -  find it in WAVETABLES > My Waves.  Folder: {lib}");
                    }
                    Err(e) => *status = format!("export failed: {e}"),
                },
                _ => {}
            }
        }

        // ---------------- main views
        let top = y + 34.0;
        let view_h = 245.0;
        let wf = Rect::from_min_size(Pos2::new(x0, top), Vec2::new(350.0, view_h));
        let td = Rect::from_min_size(Pos2::new(x0 + 360.0, top), Vec2::new(w - 360.0, view_h));
        self.waterfall(ui, wf, setter, op, now);
        self.time_view(ui, td, now);

        // harmonic editor
        let htop = top + view_h + 10.0;
        let amp_r = Rect::from_min_size(Pos2::new(x0, htop), Vec2::new(w, 125.0));
        let ph_r = Rect::from_min_size(Pos2::new(x0, htop + 129.0), Vec2::new(w, 52.0));
        self.harmonics(ui, amp_r, ph_r, now);
        ui.painter().text(Pos2::new(x0, ph_r.bottom() + 16.0), Align2::LEFT_CENTER, "HARMONICS SHOWN", mono(10.0), DIM);
        for (i, z) in ZOOMS.iter().enumerate() {
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 124.0 + 52.0 * i as f32, ph_r.bottom() + 5.0), Vec2::new(48.0, 22.0)), &z.to_string(), self.zoom == i, "How many harmonics the bars cover") {
                self.zoom = i;
            }
        }
        ui.painter().text(
            Pos2::new(x0 + 420.0, ph_r.bottom() + 16.0),
            Align2::LEFT_CENTER,
            if self.saved.is_empty() { "Drag in the bars to shape the sound. Top: loudness of each harmonic, bottom: its phase.".to_string() } else { self.saved.chars().take(118).collect() },
            mono(9.5),
            if self.saved.is_empty() { DIM } else { WHITE },
        );
        let _ = &self.status;
        self.flush(shared, p, now, false);
        close
    }

    fn export(&self) -> Result<String, String> {
        let dir = crate::files::tables_dir().ok_or("no folder")?.join("My Waves");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let base = crate::files::clean_name(self.name.trim_end_matches(" *"));
        let mut path = dir.join(format!("{base}.wav"));
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("{base} {n}.wav"));
            n += 1;
        }
        crate::wav::write(&path, 44100, &wavetable::table_to_audio(&self.tbl), Some(N))?;
        Ok(path.file_name().and_then(|x| x.to_str()).unwrap_or("file").to_string())
    }

    fn waterfall(&mut self, ui: &mut Ui, r: Rect, setter: &ParamSetter, op: &OscParams, _now: f64) {
        let resp = ui.interact(r, egui::Id::new("wf_view"), Sense::click_and_drag()).on_hover_text("Click or drag to choose a frame");
        let nfr = self.tbl.frames();
        let stride = (nfr / 48).max(1);
        let ww = r.width() - 120.0;
        let (dx, dy) = (100.0 / nfr as f32, (r.height() - 70.0) / nfr as f32);
        let base_y = r.bottom() - 16.0;
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let f = ((base_y - pt.y) / dy).round().clamp(0.0, (nfr - 1) as f32) as usize;
                if f != self.frame {
                    self.frame = f;
                    self.reload();
                    set_f(setter, &op.pos, f as f32 / (nfr - 1) as f32);
                }
            }
        }
        dark_box(ui, r);
        let painter = ui.painter().with_clip_rect(r.shrink(1.0));
        let n = 120usize;
        if nfr == 2 && self.tbl.frame_time(0) == self.tbl.frame_time(1) {
            // one single wave: no stack to show
            let gp = self.time.iter().fold(1.0f32, |a, b| a.max(b.abs()));
            let pts: Vec<Pos2> = (0..=n).map(|k| Pos2::new(r.left() + 14.0 + (r.width() - 28.0) * k as f32 / n as f32, r.center().y - self.time[(k * (N - 1) / n).min(N - 1)] / gp * r.height() * 0.32)).collect();
            painter.add(egui::Shape::line(pts, PathStroke::new(2.2_f32, WHITE)));
            painter.text(Pos2::new(r.left() + 8.0, r.top() + 10.0), Align2::LEFT_CENTER, "SINGLE WAVE", mono(9.5), DIM);
            painter.text(Pos2::new(r.center().x, r.bottom() - 12.0), Align2::CENTER_CENTER, "all frames are the same, POS does nothing", mono(9.0), DIM);
            return;
        }
        // scale so that the loudest frame still fits: a wave that overshoots never gets clipped
        let gpk = self.peaks.iter().fold(1.0f32, |a, b| a.max(*b));
        let amp = 26.0 / gpk;
        let in_sel = |i: usize| self.sel.map_or(false, |(a, b)| i >= a && i <= b);
        let mut idx: Vec<usize> = (0..nfr).step_by(stride).collect();
        if let Some((a, b)) = self.sel {
            for i in (a..=b).step_by(stride) {
                if !idx.contains(&i) {
                    idx.push(i);
                }
            }
        }
        idx.retain(|&i| i != self.frame);
        idx.sort();
        let curve = |t: &[f32], i: usize| -> Vec<Pos2> {
            let ox = r.left() + 10.0 + dx * i as f32;
            let oy = base_y - dy * i as f32;
            (0..=n).map(|k| Pos2::new(ox + ww * k as f32 / n as f32, oy - t[(k * (N - 1) / n).min(N - 1)] * amp)).collect()
        };
        for &i in idx.iter() {
            let t = self.tbl.frame_time(i);
            let oy = base_y - dy * i as f32;
            let pts = curve(&t, i);
            fill_under(&painter, &pts, oy + 1.0, Color32::from_rgb(8, 8, 8));
            let depth = i as f32 / nfr as f32;
            let c = if in_sel(i) { Color32::from_rgb(200, 200, 200) } else { Color32::from_gray((60.0 + 80.0 * (1.0 - depth)) as u8) };
            painter.add(egui::Shape::line(pts, PathStroke::new(if in_sel(i) { 1.4_f32 } else { 1.0 }, c)));
        }
        // the chosen frame is drawn last, on top, so it is always visible
        let pts = curve(&self.time, self.frame);
        painter.add(egui::Shape::line(pts, PathStroke::new(2.2_f32, WHITE)));
        painter.text(Pos2::new(r.left() + 8.0, r.top() + 10.0), Align2::LEFT_CENTER, "FRAMES", mono(9.5), DIM);
    }

    fn time_view(&mut self, ui: &mut Ui, r: Rect, now: f64) {
        let resp = ui.interact(r, egui::Id::new("td_view"), Sense::click_and_drag()).on_hover_text("Draw the wave. Tool selection is in the toolbar.");
        let to_idx = |x: f32| ((x - r.left()) / r.width() * N as f32).clamp(0.0, (N - 1) as f32);
        // the view fits tall waves; the scale is held steady while drawing
        let pk = self.time.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let fit = self.fit_hold.unwrap_or(pk.max(1.0));
        let scale = r.height() * 0.42 / fit;
        let to_val = |y: f32| ((r.center().y - y) / scale).clamp(-1.2 * fit, 1.2 * fit);
        if resp.drag_started() {
            self.fit_hold = Some(fit);
            self.push_undo();
            self.begin_multi();
            self.last_pt = None;
            if let Some(pt) = resp.interact_pointer_pos() {
                self.line_start = Some((to_idx(pt.x), to_val(pt.y), self.time.clone()));
            }
        }
        if resp.clicked() && !resp.dragged() {
            self.push_undo(); // a plain click is an edit too
        }
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let (ix, v) = (to_idx(pt.x), to_val(pt.y));
                match self.tool {
                    Tool::Draw => {
                        let mut t = self.time.clone();
                        let (a, va) = self.last_pt.unwrap_or((ix, v));
                        let (lo, hi) = if a <= ix { (a, ix) } else { (ix, a) };
                        let (vl, vh) = if a <= ix { (va, v) } else { (v, va) };
                        let span = (hi - lo).max(1.0);
                        for i in (lo as usize)..=(hi as usize).min(N - 1) {
                            let f = ((i as f32 - lo) / span).clamp(0.0, 1.0);
                            t[i] = vl + (vh - vl) * f;
                        }
                        self.last_pt = Some((ix, v));
                        self.commit_draw(t, now);
                    }
                    Tool::Line => {
                        if let Some((sx, sv, orig)) = self.line_start.clone() {
                            let mut t = orig;
                            let (lo, hi, vl, vh) = if sx <= ix { (sx, ix, sv, v) } else { (ix, sx, v, sv) };
                            let span = (hi - lo).max(1.0);
                            for i in (lo as usize)..=(hi as usize).min(N - 1) {
                                t[i] = vl + (vh - vl) * ((i as f32 - lo) / span).clamp(0.0, 1.0);
                            }
                            self.commit_draw(t, now);
                        }
                    }
                    Tool::Smooth => {
                        let c = ix as i64;
                        let mut t = self.time.clone();
                        let rad = 60i64;
                        for i in (c - rad).max(0)..=(c + rad).min(N as i64 - 1) {
                            let mut acc = 0.0;
                            let mut cnt = 0.0;
                            for k in -6i64..=6 {
                                let j = (i + k).clamp(0, N as i64 - 1) as usize;
                                acc += self.time[j];
                                cnt += 1.0;
                            }
                            let fall = 1.0 - ((i - c).abs() as f32 / rad as f32);
                            t[i as usize] = self.time[i as usize] * (1.0 - 0.6 * fall) + acc / cnt * 0.6 * fall;
                        }
                        self.commit_draw(t, now);
                    }
                }
            }
        }
        if resp.drag_stopped() {
            self.apply_multi(now);
            self.drag_orig = None;
            self.line_start = None;
            self.last_pt = None;
            self.fit_hold = None;
        }
        dark_box(ui, r);
        let painter = ui.painter().with_clip_rect(r.shrink(1.0));
        for i in 1..8 {
            let x = r.left() + r.width() * i as f32 / 8.0;
            painter.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(18, 18, 18)));
        }
        for v in [-1.0f32, 0.0, 1.0] {
            let y = r.center().y - v * scale;
            painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(1.0_f32, if v == 0.0 { Color32::from_rgb(40, 40, 40) } else { Color32::from_rgb(58, 58, 58) }));
        }
        let m = 700usize;
        let pts: Vec<Pos2> = (0..=m)
            .map(|k| {
                let s = self.time[(k * (N - 1) / m).min(N - 1)];
                Pos2::new(r.left() + r.width() * k as f32 / m as f32, r.center().y - s * scale)
            })
            .collect();
        painter.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, WHITE)));
        painter.text(Pos2::new(r.left() + 8.0, r.top() + 10.0), Align2::LEFT_CENTER, &{ let (a, b) = self.scope_range(); if b > a { format!("WAVE   (lines = +1 / -1)   drawing changes frames {}-{}", a + 1, b + 1) } else { "WAVE   (lines = +1 / -1)".to_string() } }, mono(9.5), DIM);
        if pk > 1.002 {
            painter.text(Pos2::new(r.right() - 8.0, r.top() + 10.0), Align2::RIGHT_CENTER, format!("PEAK {pk:.2} is over 1.0: use NORM"), mono(9.5), WHITE);
        }
    }

    fn harmonics(&mut self, ui: &mut Ui, ar: Rect, pr: Rect, now: f64) {
        let h = ZOOMS[self.zoom];
        let bw = ar.width() / h as f32;
        // amplitude
        let resp = ui.interact(ar, egui::Id::new("hm_amp"), Sense::click_and_drag()).on_hover_text("Drag: set the loudness of the harmonics. Right-click drag: erase.");
        let slen = self.spec.len() as i64;
        let to_k = |x: f32| (((x - ar.left()) / ar.width() * h as f32).floor() as i64 + 1).clamp(1, (h as i64).min(slen - 1).max(1)) as usize;
        if resp.drag_started() {
            self.push_undo();
            self.last_pt = None;
        }
        let erase = ui.input(|i| i.pointer.secondary_down());
        if resp.clicked() && !resp.dragged() {
            self.push_undo(); // a plain click is an edit too
        }
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let k = to_k(pt.x);
                let frac = ((ar.bottom() - pt.y) / ar.height()).clamp(0.0, 1.0);
                let amp = if erase { 0.0 } else { frac * frac };
                let top = self.spec.len().saturating_sub(1);
                let (a, va) = self.last_pt.map(|(a, v)| ((a as usize).min(top), v)).unwrap_or((k, amp));
                let (lo, hi, vl, vh) = if a <= k { (a, k, va, amp) } else { (k, a, amp, va) };
                let mut s = self.spec.clone();
                for kk in lo..=hi {
                    let f = if hi == lo { 1.0 } else { (kk - lo) as f32 / (hi - lo) as f32 };
                    let val = vl + (vh - vl) * f;
                    let (old_a, old_p) = amp_phase(&s, kk);
                    let ph = if old_a < 1e-6 { PI * 0.5 } else { old_p };
                    s[kk] = from_amp_phase(val, ph);
                }
                self.last_pt = Some((k as f32, amp));
                self.set_spec(s, now);
            }
        }
        if resp.drag_stopped() {
            self.last_pt = None;
        }
        dark_box(ui, ar);
        let painter = ui.painter().with_clip_rect(ar.shrink(1.0));
        for g in [0.25f32, 0.5, 0.75] {
            let y = ar.bottom() - ar.height() * g;
            painter.line_segment([Pos2::new(ar.left(), y), Pos2::new(ar.right(), y)], Stroke::new(1.0_f32, Color32::from_rgb(20, 20, 20)));
        }
        for k in 1..=h {
            let (a, _) = amp_phase(&self.spec, k);
            let bh = a.max(0.0).sqrt().min(1.0) * ar.height();
            if bh < 0.5 {
                continue;
            }
            let x = ar.left() + (k - 1) as f32 * bw;
            let gap = (bw * 0.15).min(1.0);
            let xr = (x + bw - gap).max(x + 1.0);
            let rr = Rect::from_min_max(Pos2::new(x, ar.bottom() - bh), Pos2::new(xr, ar.bottom()));
            painter.rect_filled(rr, CornerRadius::ZERO, WHITE);
        }
        painter.text(Pos2::new(ar.left() + 8.0, ar.top() + 10.0), Align2::LEFT_CENTER, "HARMONICS", mono(9.5), DIM);
        // number ticks
        let step = if h <= 64 { 8 } else if h <= 128 { 16 } else if h <= 256 { 32 } else { 128 };
        for k in (step..=h).step_by(step) {
            painter.text(Pos2::new(ar.left() + (k as f32 - 0.5) * bw, ar.top() + 10.0), Align2::CENTER_CENTER, k.to_string(), mono(8.5), Color32::from_rgb(90, 90, 90));
        }

        // phase
        let resp = ui.interact(pr, egui::Id::new("hm_ph"), Sense::click_and_drag()).on_hover_text("Drag: set the phase of the harmonics (centre line = 0).");
        if resp.drag_started() {
            self.push_undo();
            self.last_pt = None;
        }
        if resp.clicked() && !resp.dragged() {
            self.push_undo(); // a plain click is an edit too
        }
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let k = (((pt.x - pr.left()) / pr.width() * h as f32).floor() as i64 + 1).clamp(1, (h as i64).min(slen - 1).max(1)) as usize;
                let phv = ((pr.center().y - pt.y) / (pr.height() * 0.5)).clamp(-1.0, 1.0) * PI;
                let top = self.spec.len().saturating_sub(1);
                let (a, va) = self.last_pt.map(|(a, v)| ((a as usize).min(top), v)).unwrap_or((k, phv));
                let (lo, hi, vl, vh) = if a <= k { (a, k, va, phv) } else { (k, a, phv, va) };
                let mut s = self.spec.clone();
                for kk in lo..=hi {
                    let f = if hi == lo { 1.0 } else { (kk - lo) as f32 / (hi - lo) as f32 };
                    let val = vl + (vh - vl) * f;
                    let (old_a, _) = amp_phase(&s, kk);
                    s[kk] = from_amp_phase(old_a, val);
                }
                self.last_pt = Some((k as f32, phv));
                self.set_spec(s, now);
            }
        }
        if resp.drag_stopped() {
            self.last_pt = None;
        }
        dark_box(ui, pr);
        let painter = ui.painter().with_clip_rect(pr.shrink(1.0));
        painter.line_segment([Pos2::new(pr.left(), pr.center().y), Pos2::new(pr.right(), pr.center().y)], Stroke::new(1.0_f32, Color32::from_rgb(40, 40, 40)));
        for k in 1..=h {
            let (a, ph) = amp_phase(&self.spec, k);
            if a < 1e-4 {
                continue;
            }
            let x = ar.left() + (k - 1) as f32 * bw;
            let yv = pr.center().y - ph / PI * pr.height() * 0.5;
            let xr = (x + bw - (bw * 0.12).min(1.0)).max(x + 1.0);
            let (t, b) = if yv < pr.center().y { (yv, pr.center().y) } else { (pr.center().y, yv) };
            painter.rect_filled(Rect::from_min_max(Pos2::new(x, t), Pos2::new(xr, b.max(t + 1.0))), CornerRadius::ZERO, Color32::from_rgb(190, 190, 190));
        }
        painter.text(Pos2::new(pr.left() + 8.0, pr.top() + 10.0), Align2::LEFT_CENTER, "PHASE", mono(9.5), DIM);
    }
}

fn cap_at(ui: &Ui, x: f32, y: f32, t: &str) {
    ui.painter().text(Pos2::new(x, y + 12.0), Align2::LEFT_CENTER, t, mono(10.0), DIM);
}

fn dark_box(ui: &Ui, r: Rect) {
    ui.painter().rect_filled(r, CornerRadius::same(4), Color32::from_rgb(5, 5, 5));
    ui.painter().rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize) -> Vec<f32> {
        (0..n).map(|i| (i as f32 / n as f32 * std::f32::consts::TAU).sin() * 0.5).collect()
    }

    #[test]
    fn drawing_spreads_over_selected_frames() {
        let frames: Vec<Vec<f32>> = (0..8).map(|_| sine(N)).collect();
        let mut w = WavEdit::new(0, Table::from_frames(&frames), "t".into(), 0.0);
        w.scope = Scope::Sel;
        w.sel = Some((2, 6));
        w.frame = 4;
        w.reload();
        w.ramp = Ramp::Flat;
        w.begin_multi();
        let mut t = w.time.clone();
        for v in t[100..200].iter_mut() {
            *v += 0.3;
        }
        w.commit_draw(t, 1.0);
        w.apply_multi(2.0);
        let f2 = w.tbl.frame_time(2);
        let f0 = w.tbl.frame_time(0);
        let base = sine(N);
        assert!((f2[150] - base[150] - 0.3).abs() < 0.03, "frame 2 got the bump");
        assert!((f0[150] - base[150]).abs() < 0.02, "frame 0 untouched");
        assert!((f2[1000] - base[1000]).abs() < 0.02, "outside the drawn piece unchanged");
    }

    #[test]
    fn strength_and_ramp_weights() {
        let frames: Vec<Vec<f32>> = (0..8).map(|_| sine(N)).collect();
        let mut w = WavEdit::new(0, Table::from_frames(&frames), "t".into(), 0.0);
        w.strength = 0.5;
        w.ramp = Ramp::In;
        assert!((w.weight(0, 0, 4)).abs() < 1e-6);
        assert!((w.weight(4, 0, 4) - 0.5).abs() < 1e-6);
        w.ramp = Ramp::Out;
        assert!((w.weight(0, 0, 4) - 0.5).abs() < 1e-6);
    }
}

#[cfg(test)]
mod more_tests {
    use super::*;

    fn mk(n: usize) -> WavEdit {
        let frames: Vec<Vec<f32>> = (0..n).map(|i| (0..N).map(|k| ((k as f32 / N as f32 * std::f32::consts::TAU) * (1 + i % 5) as f32).sin() * 0.6).collect()).collect();
        WavEdit::new(0, Table::from_frames(&frames), "t".into(), 0.0)
    }

    #[test]
    fn insert_delete_selection_keep_ranges_valid() {
        let mut w = mk(10);
        w.sel = Some((3, 6));
        w.scope = Scope::Sel;
        // duplicate selection
        let mut fr = w.tbl.to_frames();
        let copy: Vec<Vec<f32>> = fr[3..=6].to_vec();
        for (k, f) in copy.into_iter().enumerate() {
            fr.insert(7 + k, f);
        }
        w.tbl = Table::from_frames(&fr);
        w.reload();
        assert_eq!(w.tbl.frames(), 14);
        // selection beyond the table is clamped away, never panics
        w.sel = Some((9, 40));
        w.reload();
        assert!(w.sel.map_or(true, |(a, b)| a < b && b < 14));
        let (lo, hi) = w.scope_range();
        assert!(hi < 14 && lo <= hi);
    }

    #[test]
    fn morph_blends_between_ends_and_rejects_short_ranges() {
        let mut w = mk(9);
        w.scope = Scope::Sel;
        w.sel = Some((0, 1));
        assert!(!w.morph_range(1.0), "2 frames: nothing to morph");
        w.sel = Some((0, 8));
        let a = w.tbl.frame_time(0);
        let b = w.tbl.frame_time(8);
        assert!(w.morph_range(1.0));
        let mid = w.tbl.frame_time(4);
        // middle frame lies between the two ends (energy sanity) and is finite
        assert!(mid.iter().all(|v| v.is_finite() && v.abs() < 2.0));
        assert!((w.tbl.frame_time(0)[10] - a[10]).abs() < 1e-3);
        assert!((w.tbl.frame_time(8)[10] - b[10]).abs() < 1e-3);
        w.spectral = false;
        assert!(w.morph_range(2.0));
    }

    #[test]
    fn scope_ops_respect_range_and_undo_redo() {
        let mut w = mk(8);
        w.scope = Scope::Sel;
        w.sel = Some((2, 5));
        let before: Vec<Vec<f32>> = w.tbl.to_frames();
        w.apply_time(1.0, |t| t.iter_mut().for_each(|v| *v = -*v));
        let after = w.tbl.to_frames();
        assert!((after[0][100] - before[0][100]).abs() < 1e-3, "outside the range stays");
        assert!((after[3][100] + before[3][100]).abs() < 1e-2, "inside is inverted");
        w.strength = 0.5;
        w.apply_time(2.0, |t| t.iter_mut().for_each(|v| *v = 0.0));
        assert!(w.tbl.frame_time(3)[100].abs() < before[3][100].abs());
        // undo twice returns to the start
        let u1 = w.undo.pop().unwrap();
        w.tbl = u1;
        let u2 = w.undo.pop().unwrap();
        w.tbl = u2;
        w.reload();
        assert!((w.tbl.frame_time(3)[100] - before[3][100]).abs() < 1e-3);
    }

    #[test]
    fn drawing_on_all_frames_is_fast_enough() {
        let mut w = mk(256);
        w.scope = Scope::All;
        w.frame = 100;
        w.reload();
        w.begin_multi();
        let mut t = w.time.clone();
        t[50..300].iter_mut().for_each(|v| *v += 0.2);
        let t0 = std::time::Instant::now();
        w.commit_draw(t, 10.0);
        w.apply_multi(10.0);
        let ms = t0.elapsed().as_millis();
        eprintln!("apply to 256 frames: {ms} ms");
        assert!(ms < 1500);
    }

    #[test]
    fn formula_scope_and_weights_never_nan() {
        let mut w = mk(16);
        w.scope = Scope::All;
        w.ramp = Ramp::Edges;
        w.soft = 1.0;
        w.formula = "sin(2*pi*x*(1+floor(w*7)))".into();
        w.run_formula(1.0);
        assert!(w.formula_err.is_empty());
        for f in w.tbl.to_frames() {
            assert!(f.iter().all(|v| v.is_finite()));
        }
        w.formula = "garbage((".into();
        w.run_formula(2.0);
        assert!(!w.formula_err.is_empty());
    }
}

#[cfg(test)]
mod single_tests {
    use super::*;
    #[test]
    fn single_wave_table_is_identical_frames() {
        let t: Vec<f32> = (0..N).map(|k| (k as f32 / N as f32 * std::f32::consts::TAU).sin()).collect();
        let tb = Table::from_frames(&[t.clone(), t]);
        assert_eq!(tb.frames(), 2);
        assert_eq!(tb.frame_time(0), tb.frame_time(1));
    }
}

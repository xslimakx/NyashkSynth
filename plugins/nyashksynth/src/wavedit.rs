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
    all: bool,
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
            all: false,
        };
        w.reload();
        w
    }

    fn reload(&mut self) {
        self.frame = self.frame.min(self.tbl.frames() - 1);
        self.time = self.tbl.frame_time(self.frame);
        self.spec = spectrum_of(&self.time);
    }

    fn push_undo(&mut self) {
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

    /// Applies a time-domain transform to the current frame (or to every frame when ALL is on).
    fn apply_time(&mut self, now: f64, f: impl Fn(&mut Vec<f32>)) {
        self.push_undo();
        if self.all {
            for i in 0..self.tbl.frames() {
                let mut t = self.tbl.frame_time(i);
                f(&mut t);
                self.tbl.set_frame(i, &t);
            }
            self.reload();
            self.touch(now);
        } else {
            let mut t = self.time.clone();
            f(&mut t);
            self.set_time(t, now);
        }
    }

    fn run_formula(&mut self, now: f64, all_frames: bool) {
        match crate::formula::parse(&self.formula) {
            Err(e) => self.formula_err = e,
            Ok(ex) => {
                self.formula_err.clear();
                self.push_undo();
                let nf = self.tbl.frames();
                let range: Vec<usize> = if all_frames { (0..nf).collect() } else { vec![self.frame] };
                for i in range {
                    let w = if nf > 1 { i as f32 / (nf - 1) as f32 } else { 0.0 };
                    let mut t = crate::formula::frame(&ex, w, N);
                    let pk = t.iter().fold(0.0f32, |a, b| a.max(b.abs()));
                    if pk > 1.0 {
                        t.iter_mut().for_each(|x| *x *= 0.95 / pk);
                    }
                    let mean = t.iter().sum::<f32>() / N as f32;
                    t.iter_mut().for_each(|x| *x -= mean);
                    self.tbl.set_frame(i, &t);
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
            let op = if self.osc == 0 { &p.a } else { &p.b };
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

    /// Draw the editor. Returns true when it should close.
    #[allow(clippy::too_many_arguments)]
    pub fn show(&mut self, ui: &mut Ui, setter: &ParamSetter, p: &SynthParams, shared: &Shared, rect: Rect, now: f64, status: &mut String) -> bool {
        let mut close = false;
        let op = if self.osc == 0 { &p.a } else { &p.b };
        ui.painter().rect_filled(rect, CornerRadius::same(8), Color32::from_rgb(8, 8, 8));
        ui.painter().rect_stroke(rect, CornerRadius::same(8), Stroke::new(1.0_f32, MID), StrokeKind::Inside);
        let x0 = rect.left() + 14.0;
        let w = rect.width() - 28.0;
        ui.painter().text(Pos2::new(x0, rect.top() + 20.0), Align2::LEFT_CENTER, format!("WAVETABLE EDITOR   OSC {}   {}   {} frames", if self.osc == 0 { "A" } else { "B" }, self.name, self.tbl.frames()), mono(12.0), WHITE);
        if pill_tip(ui, Rect::from_min_size(Pos2::new(rect.right() - 74.0, rect.top() + 8.0), Vec2::new(60.0, 24.0)), "CLOSE", false, "Close the editor (Esc)") || ui.input(|i| i.key_pressed(egui::Key::Escape)) && !ui.memory(|m| m.focused().is_some()) {
            close = true;
        }

        // ---------------- toolbar row 1: frames
        let mut y = rect.top() + 42.0;
        let yc = std::cell::Cell::new(y);
        let btn = |x: f32, wd: f32| Rect::from_min_size(Pos2::new(x, yc.get()), Vec2::new(wd, 24.0));
        let nfr = self.tbl.frames();
        let mut new_frame: Option<usize> = None;
        if pill_tip(ui, btn(x0, 28.0), "<", false, "Previous frame") {
            new_frame = Some(self.frame.saturating_sub(1));
        }
        ui.painter().text(Pos2::new(x0 + 84.0, y + 12.0), Align2::CENTER_CENTER, format!("FRAME {}/{}", self.frame + 1, nfr), mono(11.0), WHITE);
        if pill_tip(ui, btn(x0 + 140.0, 28.0), ">", false, "Next frame") {
            new_frame = Some((self.frame + 1).min(nfr - 1));
        }
        let mut bx = x0 + 180.0;
        let mut op_clicked: Option<&str> = None;
        for (label, tip) in [
            ("INSERT", "Duplicate this frame after itself"),
            ("DELETE", "Remove this frame"),
            ("COPY", "Copy this frame"),
            ("PASTE", "Paste over this frame"),
            ("CLEAR", "Silence this frame"),
            ("UNDO", "Undo the last change (Ctrl+Z)"),
        ] {
            if pill_tip(ui, btn(bx, 66.0), label, false, tip) {
                op_clicked = Some(label);
            }
            bx += 70.0;
        }
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Z)) {
            op_clicked = Some("UNDO");
        }
        // tools
        bx += 16.0;
        ui.painter().text(Pos2::new(bx, y + 12.0), Align2::LEFT_CENTER, "TOOL", mono(10.0), DIM);
        bx += 42.0;
        for (t, label, tip) in [(Tool::Draw, "DRAW", "Freehand drawing in the wave view"), (Tool::Line, "LINE", "Drag out a straight line"), (Tool::Smooth, "SMOOTH", "Brush that smooths the wave")] {
            if pill_tip(ui, btn(bx, 66.0), label, self.tool == t, tip) {
                self.tool = t;
            }
            bx += 70.0;
        }

        // ---------------- toolbar row 2: shapes, processing, frame count
        y += 30.0;
        yc.set(y);
        let mut bx = x0;
        ui.painter().text(Pos2::new(bx, y + 12.0), Align2::LEFT_CENTER, "FRAME =", mono(10.0), DIM);
        bx += 58.0;
        let mut shape: Option<u8> = None;
        for (i, label) in ["SINE", "TRI", "SAW", "SQR", "PULSE"].iter().enumerate() {
            if pill_tip(ui, btn(bx, 52.0), label, false, "Replace this frame with a basic shape") {
                shape = Some(i as u8);
            }
            bx += 56.0;
        }
        bx += 14.0;
        for (label, tip) in [("NORM", "Normalise this frame"), ("NORM ALL", "Normalise every frame to the same peak"), ("REVERSE", "Reverse this frame in time"), ("INVERT", "Flip the polarity of this frame"), ("LOWPASS", "Remove the highest harmonics of this frame")] {
            let wd = if label.len() > 6 { 76.0 } else { 62.0 };
            if pill_tip(ui, btn(bx, wd), label, false, tip) {
                op_clicked = Some(label);
            }
            bx += wd + 4.0;
        }
        y += 30.0;
        yc.set(y);
        bx = x0;
        ui.painter().text(Pos2::new(bx, y + 12.0), Align2::LEFT_CENTER, "FRAMES", mono(10.0), DIM);
        bx += 52.0;
        let mut resize: Option<usize> = None;
        for n in [2usize, 8, 16, 32, 64, 128, 256] {
            if pill_tip(ui, btn(bx, 38.0), &n.to_string(), nfr == n, "Change the number of frames (new frames are interpolated)") {
                resize = Some(n);
            }
            bx += 41.0;
        }
        if pill_tip(ui, btn(bx + 6.0, 66.0), if self.spectral { "SPECTRAL" } else { "LINEAR" }, false, "How new frames are interpolated: spectral (smooth harmonics) or linear crossfade") {
            self.spectral = !self.spectral;
        }
        bx += 82.0;
        if pill_tip(ui, btn(bx, 80.0), "EXPORT WAV", false, "Save this wavetable as a .wav file in the Tables folder (Serum compatible)") {
            op_clicked = Some("EXPORT");
        }
        bx += 96.0;
        if pill_tip(ui, btn(bx, 40.0), "ALL", self.all, "Process buttons (and the toolbar above) affect every frame instead of only the current one") {
            self.all = !self.all;
        }
        bx += 46.0;
        for (label, tip) in [
            ("HIGHPASS", "Remove the lowest harmonics"),
            ("FOLD", "Wave folding"),
            ("CRUSH", "Bit crusher"),
            ("MIRROR", "Mirror the first half onto the second half"),
            ("SMOOTH", "Soften the corners"),
            ("RANDOM", "Random harmonic spectrum"),
        ] {
            let wd = if label.len() > 6 { 76.0 } else { 62.0 };
            if pill_tip(ui, btn(bx, wd), label, false, tip) {
                op_clicked = Some(label);
            }
            bx += wd + 4.0;
        }
        // ---------------- row 4: formula
        y += 30.0;
        yc.set(y);
        ui.painter().text(Pos2::new(x0, y + 12.0), Align2::LEFT_CENTER, "FORMULA", mono(10.0), DIM);
        let te = Rect::from_min_size(Pos2::new(x0 + 66.0, y), Vec2::new(430.0, 24.0));
        let ed = ui.put(te, egui::TextEdit::singleline(&mut self.formula).font(mono(11.0)).hint_text("sin(2*pi*x + w*sin(4*pi*x))").desired_width(430.0));
        let _ = ed;
        let mut run: Option<bool> = None;
        if pill_tip(ui, btn(x0 + 504.0, 90.0), "THIS FRAME", false, "Generate the current frame from the formula (x = phase 0..1, w = frame position 0..1)") {
            run = Some(false);
        }
        if pill_tip(ui, btn(x0 + 598.0, 90.0), "ALL FRAMES", false, "Generate every frame (w sweeps from 0 to 1 across the table)") {
            run = Some(true);
        }
        if pill_tip(ui, btn(x0 + 692.0, 80.0), "EXAMPLE", false, "Cycle through example formulas") {
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
        if let Some(a) = run {
            self.run_formula(now, a);
        }
        let msg = if self.formula_err.is_empty() { "x=phase  w=frame  (sin cos tanh saw tri...)".to_string() } else { format!("formula error: {}", self.formula_err) };
        ui.painter().text(Pos2::new(x0 + 780.0, y + 12.0), Align2::LEFT_CENTER, msg.chars().take(44).collect::<String>(), mono(8.5), if self.formula_err.is_empty() { DIM } else { WHITE });

        // ---------------- apply toolbar operations
        if let Some(n) = new_frame {
            self.frame = n;
            self.reload();
            set_f(setter, &op.pos, n as f32 / (nfr - 1) as f32);
        }
        if let Some(k) = shape {
            self.push_undo();
            let s = self.pat_spec(k);
            self.set_spec(s, now);
            let mut t = self.time.clone();
            Self::peak_normalize(&mut t);
            self.set_time(t, now);
        }
        if let Some(n) = resize {
            if n != nfr {
                self.push_undo();
                let t = self.tbl.resized(n, self.spectral);
                self.replace_table(t, now);
                self.frame = self.frame.min(n - 1);
                self.reload();
            }
        }
        if let Some(c) = op_clicked {
            match c {
                "INSERT" => {
                    if nfr < MAX_FRAMES {
                        self.push_undo();
                        let mut fr = self.tbl.to_frames();
                        fr.insert(self.frame + 1, fr[self.frame].clone());
                        self.tbl = Table::from_frames(&fr);
                        self.frame += 1;
                        self.reload();
                        self.touch(now);
                    }
                }
                "DELETE" => {
                    if nfr > 2 {
                        self.push_undo();
                        let mut fr = self.tbl.to_frames();
                        fr.remove(self.frame);
                        self.tbl = Table::from_frames(&fr);
                        self.reload();
                        self.touch(now);
                    }
                }
                "COPY" => self.clip = Some(self.time.clone()),
                "PASTE" => {
                    if let Some(c) = self.clip.clone() {
                        self.push_undo();
                        self.set_time(c, now);
                    }
                }
                "CLEAR" => {
                    self.push_undo();
                    self.set_time(vec![0.0; N], now);
                }
                "UNDO" => {
                    if let Some(t) = self.undo.pop() {
                        self.tbl = t;
                        self.reload();
                        self.touch(now);
                    }
                }
                "NORM" => self.apply_time(now, |t| Self::peak_normalize(t)),
                "NORM ALL" => {
                    self.push_undo();
                    let fr: Vec<Vec<f32>> = self.tbl.to_frames().into_iter().map(|mut f| {
                        Self::peak_normalize(&mut f);
                        f
                    }).collect();
                    self.tbl = Table::from_frames(&fr);
                    self.reload();
                    self.touch(now);
                }
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
                "EXPORT" => match self.export() {
                    Ok(path) => *status = format!("saved {path}"),
                    Err(e) => *status = format!("export failed: {e}"),
                },
                _ => {}
            }
        }

        // ---------------- main views
        let top = y + 34.0;
        let view_h = 262.0;
        let wf = Rect::from_min_size(Pos2::new(x0, top), Vec2::new(350.0, view_h));
        let td = Rect::from_min_size(Pos2::new(x0 + 360.0, top), Vec2::new(w - 360.0, view_h));
        self.waterfall(ui, wf, setter, op, now);
        self.time_view(ui, td, now);

        // harmonic editor
        let htop = top + view_h + 12.0;
        let amp_r = Rect::from_min_size(Pos2::new(x0, htop), Vec2::new(w, 150.0));
        let ph_r = Rect::from_min_size(Pos2::new(x0, htop + 154.0), Vec2::new(w, 64.0));
        self.harmonics(ui, amp_r, ph_r, now);
        // zoom pills
        let zy = htop - 10.0;
        let _ = zy;
        ui.painter().text(Pos2::new(x0, ph_r.bottom() + 16.0), Align2::LEFT_CENTER, "HARMONICS SHOWN", mono(10.0), DIM);
        for (i, z) in ZOOMS.iter().enumerate() {
            if pill_tip(ui, Rect::from_min_size(Pos2::new(x0 + 124.0 + 52.0 * i as f32, ph_r.bottom() + 5.0), Vec2::new(48.0, 22.0)), &z.to_string(), self.zoom == i, "How many harmonics the bars cover") {
                self.zoom = i;
            }
        }
        ui.painter().text(
            Pos2::new(x0 + 420.0, ph_r.bottom() + 16.0),
            Align2::LEFT_CENTER,
            "Drag in the bars to shape the sound. Top: loudness of each harmonic, bottom: its phase.",
            mono(9.5),
            DIM,
        );
        let _ = &self.status;
        self.flush(shared, p, now, false);
        close
    }

    fn export(&self) -> Result<String, String> {
        let dir = crate::files::tables_dir().ok_or("no folder")?.join("Exports");
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
        let mut idx: Vec<usize> = (0..nfr).step_by(stride).collect();
        if !idx.contains(&self.frame) {
            idx.push(self.frame);
            idx.sort();
        }
        for &i in idx.iter() {
            let t = self.tbl.frame_time(i);
            let ox = r.left() + 10.0 + dx * i as f32;
            let oy = base_y - dy * i as f32;
            let pts: Vec<Pos2> = (0..=n).map(|k| {
                let s = t[(k * (N - 1) / n).min(N - 1)];
                Pos2::new(ox + ww * k as f32 / n as f32, oy - s * 24.0)
            }).collect();
            fill_under(&painter, &pts, oy + 1.0, Color32::from_rgb(8, 8, 8));
            let depth = i as f32 / nfr as f32;
            let c = if i == self.frame { WHITE } else { Color32::from_gray((60.0 + 80.0 * (1.0 - depth)) as u8) };
            painter.add(egui::Shape::line(pts, PathStroke::new(if i == self.frame { 2.0_f32 } else { 1.0 }, c)));
        }
    }

    fn time_view(&mut self, ui: &mut Ui, r: Rect, now: f64) {
        let resp = ui.interact(r, egui::Id::new("td_view"), Sense::click_and_drag()).on_hover_text("Draw the wave. Tool selection is in the toolbar.");
        let to_idx = |x: f32| ((x - r.left()) / r.width() * N as f32).clamp(0.0, (N - 1) as f32);
        let to_val = |y: f32| ((r.center().y - y) / (r.height() * 0.42)).clamp(-1.6, 1.6);
        if resp.drag_started() {
            self.push_undo();
            self.last_pt = None;
            if let Some(pt) = resp.interact_pointer_pos() {
                self.line_start = Some((to_idx(pt.x), to_val(pt.y), self.time.clone()));
            }
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
                        self.set_time(t, now);
                    }
                    Tool::Line => {
                        if let Some((sx, sv, orig)) = self.line_start.clone() {
                            let mut t = orig;
                            let (lo, hi, vl, vh) = if sx <= ix { (sx, ix, sv, v) } else { (ix, sx, v, sv) };
                            let span = (hi - lo).max(1.0);
                            for i in (lo as usize)..=(hi as usize).min(N - 1) {
                                t[i] = vl + (vh - vl) * ((i as f32 - lo) / span).clamp(0.0, 1.0);
                            }
                            self.set_time(t, now);
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
                        self.set_time(t, now);
                    }
                }
            }
        }
        if resp.drag_stopped() {
            self.line_start = None;
            self.last_pt = None;
        }
        dark_box(ui, r);
        let painter = ui.painter().with_clip_rect(r.shrink(1.0));
        for i in 1..8 {
            let x = r.left() + r.width() * i as f32 / 8.0;
            painter.line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(18, 18, 18)));
        }
        for v in [-1.0f32, 0.0, 1.0] {
            let y = r.center().y - v * r.height() * 0.42;
            painter.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], Stroke::new(1.0_f32, if v == 0.0 { Color32::from_rgb(40, 40, 40) } else { Color32::from_rgb(22, 22, 22) }));
        }
        let m = 700usize;
        let pts: Vec<Pos2> = (0..=m)
            .map(|k| {
                let s = self.time[(k * (N - 1) / m).min(N - 1)];
                Pos2::new(r.left() + r.width() * k as f32 / m as f32, r.center().y - s * r.height() * 0.42)
            })
            .collect();
        painter.add(egui::Shape::line(pts, PathStroke::new(1.8_f32, WHITE)));
        painter.text(Pos2::new(r.left() + 8.0, r.top() + 10.0), Align2::LEFT_CENTER, "WAVE", mono(9.5), DIM);
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
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let k = to_k(pt.x);
                let frac = ((ar.bottom() - pt.y) / ar.height()).clamp(0.0, 1.0);
                let amp = if erase { 0.0 } else { frac * frac };
                let (a, va) = self.last_pt.map(|(a, v)| (a as usize, v)).unwrap_or((k, amp));
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
        if resp.dragged() || resp.clicked() {
            if let Some(pt) = resp.interact_pointer_pos() {
                let k = (((pt.x - pr.left()) / pr.width() * h as f32).floor() as i64 + 1).clamp(1, (h as i64).min(slen - 1).max(1)) as usize;
                let phv = ((pr.center().y - pt.y) / (pr.height() * 0.5)).clamp(-1.0, 1.0) * PI;
                let (a, va) = self.last_pt.map(|(a, v)| (a as usize, v)).unwrap_or((k, phv));
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

fn dark_box(ui: &Ui, r: Rect) {
    ui.painter().rect_filled(r, CornerRadius::same(4), Color32::from_rgb(5, 5, 5));
    ui.painter().rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0_f32, LINE), StrokeKind::Inside);
}

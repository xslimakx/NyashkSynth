//! Effects rack: ten modules (hyper/dimension, distortion, flanger, phaser, chorus, delay,
//! compressor, reverb, EQ, filter) that can be put in any order.

use crate::params::{DistType, FxFilterType};
use std::f32::consts::{PI, TAU};

#[derive(Clone)]
pub struct FxPatch {
    /// Module index (see `FX_NAMES`) for every slot of the chain.
    pub order: [usize; 10],
    pub on: [bool; 10],

    pub hy_rate: f32,
    pub hy_detune: f32,
    pub hy_uni: usize,
    pub hy_dim: f32,
    pub hy_mix: f32,

    pub ds_type: DistType,
    pub ds_drive: f32,
    pub ds_tone: f32,
    pub ds_mix: f32,

    pub fg_rate: f32,
    pub fg_depth: f32,
    pub fg_delay: f32,
    pub fg_fb: f32,
    pub fg_phase: f32,
    pub fg_mix: f32,

    pub ph_rate: f32,
    pub ph_depth: f32,
    pub ph_freq: f32,
    pub ph_fb: f32,
    pub ph_stages: usize,
    pub ph_phase: f32,
    pub ph_mix: f32,

    pub ch_rate: f32,
    pub ch_depth: f32,
    pub ch_delay: f32,
    pub ch_fb: f32,
    pub ch_filter: f32,
    pub ch_mix: f32,

    pub dl_ms_l: f32,
    pub dl_ms_r: f32,
    pub dl_fb: f32,
    pub dl_pp: bool,
    pub dl_lp: f32,
    pub dl_hp: f32,
    pub dl_width: f32,
    pub dl_mix: f32,

    pub cp_thresh: f32,
    pub cp_ratio: f32,
    pub cp_attack: f32,
    pub cp_release: f32,
    pub cp_gain: f32,
    pub cp_multi: bool,
    pub cp_f1: f32,
    pub cp_f2: f32,
    pub cp_mix: f32,

    pub rv_size: f32,
    pub rv_damp: f32,
    pub rv_pre: f32,
    pub rv_lowcut: f32,
    pub rv_highcut: f32,
    pub rv_width: f32,
    pub rv_mix: f32,

    pub eq_lf: f32,
    pub eq_lg: f32,
    pub eq_mf: f32,
    pub eq_mq: f32,
    pub eq_mg: f32,
    pub eq_hf: f32,
    pub eq_hg: f32,

    pub ff_type: FxFilterType,
    pub ff_cut: f32,
    pub ff_res: f32,
    pub ff_drive: f32,
    pub ff_mix: f32,
}

impl FxPatch {
    /// Applies the global modulation values (see `Engine::fx_mod`).
    pub fn apply_mod(&mut self, m: &[f32; 12]) {
        let u = |v: f32| v.clamp(0.0, 1.0);
        self.hy_mix = u(self.hy_mix + m[0]);
        self.ds_drive = u(self.ds_drive + m[1]);
        self.ds_mix = u(self.ds_mix + m[2]);
        self.ch_mix = u(self.ch_mix + m[3]);
        self.fg_depth = u(self.fg_depth + m[4]);
        self.ph_freq = (self.ph_freq * 2f32.powf(m[5] * 3.0)).clamp(60.0, 12000.0);
        self.dl_mix = u(self.dl_mix + m[6]);
        self.dl_fb = u(self.dl_fb + m[7]).min(0.97);
        self.rv_mix = u(self.rv_mix + m[8]);
        self.rv_size = u(self.rv_size + m[9]);
        self.ff_cut = (self.ff_cut * 2f32.powf(m[10] * 6.0)).clamp(20.0, 20000.0);
        self.ff_res = u(self.ff_res + m[11]);
    }
}

// ---------------------------------------------------------------- building blocks

struct DelayLine {
    buf: Vec<f32>,
    w: usize,
}

impl DelayLine {
    fn new(n: usize) -> Self {
        Self { buf: vec![0.0; n.max(8)], w: 0 }
    }
    #[inline]
    fn push(&mut self, x: f32) {
        self.buf[self.w] = x;
        self.w += 1;
        if self.w >= self.buf.len() {
            self.w = 0;
        }
    }
    /// Fractional read, `d` samples behind the write head.
    #[inline]
    fn read(&self, d: f32) -> f32 {
        let n = self.buf.len();
        let d = d.clamp(1.0, (n - 2) as f32);
        let p = self.w as f32 - d;
        let p = if p < 0.0 { p + n as f32 } else { p };
        let i = p as usize % n;
        let f = p - p.floor();
        let a = self.buf[i];
        let b = self.buf[(i + 1) % n];
        a + (b - a) * f
    }
    /// Integer tap.
    #[inline]
    fn tap(&self, d: usize) -> f32 {
        let n = self.buf.len();
        let d = d.clamp(1, n - 1);
        self.buf[(self.w + n - d) % n]
    }
    fn clear(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
    }
}

#[derive(Clone, Copy, Default)]
struct OnePole {
    z: f32,
}
impl OnePole {
    #[inline]
    fn lp(&mut self, x: f32, a: f32) -> f32 {
        self.z += a * (x - self.z);
        self.z
    }
    #[inline]
    fn hp(&mut self, x: f32, a: f32) -> f32 {
        self.z += a * (x - self.z);
        x - self.z
    }
}

fn op_coef(f: f32, fs: f32) -> f32 {
    1.0 - (-TAU * f.clamp(5.0, 0.45 * fs) / fs).exp()
}

#[derive(Clone, Copy)]
pub struct BqCoef {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BqCoef {
    pub const FLAT: BqCoef = BqCoef { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 };

    fn norm(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> BqCoef {
        BqCoef { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 }
    }

    pub fn lowpass(f: f32, q: f32, fs: f32) -> BqCoef {
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / (2.0 * q);
        Self::norm((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0, 1.0 + al, -2.0 * c, 1.0 - al)
    }
    pub fn highpass(f: f32, q: f32, fs: f32) -> BqCoef {
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / (2.0 * q);
        Self::norm((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0, 1.0 + al, -2.0 * c, 1.0 - al)
    }
    pub fn bandpass(f: f32, q: f32, fs: f32) -> BqCoef {
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / (2.0 * q);
        Self::norm(al, 0.0, -al, 1.0 + al, -2.0 * c, 1.0 - al)
    }
    pub fn notch(f: f32, q: f32, fs: f32) -> BqCoef {
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / (2.0 * q);
        Self::norm(1.0, -2.0 * c, 1.0, 1.0 + al, -2.0 * c, 1.0 - al)
    }
    pub fn peaking(f: f32, q: f32, db: f32, fs: f32) -> BqCoef {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / (2.0 * q);
        Self::norm(1.0 + al * a, -2.0 * c, 1.0 - al * a, 1.0 + al / a, -2.0 * c, 1.0 - al / a)
    }
    pub fn low_shelf(f: f32, db: f32, fs: f32) -> BqCoef {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / 2.0 * 2f32.sqrt();
        let sq = 2.0 * a.sqrt() * al;
        Self::norm(
            a * ((a + 1.0) - (a - 1.0) * c + sq),
            2.0 * a * ((a - 1.0) - (a + 1.0) * c),
            a * ((a + 1.0) - (a - 1.0) * c - sq),
            (a + 1.0) + (a - 1.0) * c + sq,
            -2.0 * ((a - 1.0) + (a + 1.0) * c),
            (a + 1.0) + (a - 1.0) * c - sq,
        )
    }
    pub fn high_shelf(f: f32, db: f32, fs: f32) -> BqCoef {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * f.clamp(10.0, 0.49 * fs) / fs;
        let (s, c) = (w.sin(), w.cos());
        let al = s / 2.0 * 2f32.sqrt();
        let sq = 2.0 * a.sqrt() * al;
        Self::norm(
            a * ((a + 1.0) + (a - 1.0) * c + sq),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
            a * ((a + 1.0) + (a - 1.0) * c - sq),
            (a + 1.0) - (a - 1.0) * c + sq,
            2.0 * ((a - 1.0) - (a + 1.0) * c),
            (a + 1.0) - (a - 1.0) * c - sq,
        )
    }

    /// Magnitude at a frequency (used by the GUI response curves).
    pub fn mag(&self, f: f32, fs: f32) -> f32 {
        let w = TAU * f / fs;
        let (c1, s1, c2, s2) = (w.cos(), w.sin(), (2.0 * w).cos(), (2.0 * w).sin());
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = -(self.b1 * s1 + self.b2 * s2);
        let dr = 1.0 + self.a1 * c1 + self.a2 * c2;
        let di = -(self.a1 * s1 + self.a2 * s2);
        ((nr * nr + ni * ni) / (dr * dr + di * di).max(1e-12)).sqrt()
    }
}

#[derive(Clone, Copy, Default)]
struct Bq {
    z1: f32,
    z2: f32,
}
impl Bq {
    #[inline]
    fn tick(&mut self, c: &BqCoef, x: f32) -> f32 {
        let y = c.b0 * x + self.z1;
        self.z1 = c.b1 * x - c.a1 * y + self.z2;
        self.z2 = c.b2 * x - c.a2 * y;
        y
    }
}

/// Combined EQ response in dB at a frequency (GUI).
pub fn eq_response_db(p: &FxPatch, f: f32, fs: f32) -> f32 {
    let m = BqCoef::low_shelf(p.eq_lf, p.eq_lg, fs).mag(f, fs) * BqCoef::peaking(p.eq_mf, p.eq_mq, p.eq_mg, fs).mag(f, fs) * BqCoef::high_shelf(p.eq_hf, p.eq_hg, fs).mag(f, fs);
    20.0 * m.max(1e-6).log10()
}

// ---------------------------------------------------------------- modules

struct Hyper {
    dl: [DelayLine; 2],
    ph: f32,
    dim: [DelayLine; 2],
    dim_hp: [OnePole; 2],
}

struct Dist {
    hold: [f32; 2],
    cnt: f32,
    tone: [OnePole; 2],
    dc: [OnePole; 2],
}

struct Flanger {
    dl: [DelayLine; 2],
    ph: f32,
    fb: [f32; 2],
}

struct Phaser {
    z: [[f32; 12]; 2],
    ph: f32,
    fb: [f32; 2],
}

struct Chorus {
    dl: [DelayLine; 2],
    ph: f32,
    fb: [f32; 2],
    lp: [OnePole; 2],
}

struct Delay {
    dl: [DelayLine; 2],
    lp: [OnePole; 2],
    hp: [OnePole; 2],
}

#[derive(Default, Clone, Copy)]
struct CompBand {
    env: f32,
}

struct Comp {
    bands: [CompBand; 3],
    lo: [[Bq; 2]; 2],  // [ch][2 cascaded] low-pass at f1
    hi1: [[Bq; 2]; 2], // high-pass at f1
    lo2: [[Bq; 2]; 2], // low-pass at f2 (applied to the high part)
    hi2: [[Bq; 2]; 2],
}

struct Plate {
    pre: DelayLine,
    inp_ap: [(DelayLine, f32); 4],
    l_ap1: DelayLine,
    l_d1: DelayLine,
    l_ap2: DelayLine,
    l_d2: DelayLine,
    r_ap1: DelayLine,
    r_d1: DelayLine,
    r_ap2: DelayLine,
    r_d2: DelayLine,
    damp: [f32; 2],
    bw: f32,
    lfo: f32,
    fl: [f32; 2],
    lowcut: [OnePole; 2],
    highcut: [OnePole; 2],
    sc: f32,
}

struct EqM {
    ls: [Bq; 2],
    pk: [Bq; 2],
    hs: [Bq; 2],
}

struct FiltFx {
    s: [[f32; 2]; 4], // ch, stage
}

pub struct Fx {
    fs: f32,
    hyper: Hyper,
    dist: Dist,
    flanger: Flanger,
    phaser: Phaser,
    chorus: Chorus,
    delay: Delay,
    comp: Comp,
    plate: Plate,
    eq: EqM,
    ff: FiltFx,
}

fn ms(fs: f32, t: f32) -> usize {
    (fs * t * 0.001) as usize + 4
}

impl Plate {
    fn new(fs: f32) -> Self {
        let sc = fs / 29761.0;
        let n = |x: usize| ((x as f32 * sc) as usize).max(8) + 64;
        Plate {
            pre: DelayLine::new(ms(fs, 260.0)),
            inp_ap: [(DelayLine::new(n(142)), 0.75), (DelayLine::new(n(107)), 0.75), (DelayLine::new(n(379)), 0.625), (DelayLine::new(n(277)), 0.625)],
            l_ap1: DelayLine::new(n(672) + 64),
            l_d1: DelayLine::new(n(4453)),
            l_ap2: DelayLine::new(n(1800)),
            l_d2: DelayLine::new(n(3720)),
            r_ap1: DelayLine::new(n(908) + 64),
            r_d1: DelayLine::new(n(4217)),
            r_ap2: DelayLine::new(n(2656)),
            r_d2: DelayLine::new(n(3163)),
            damp: [0.0; 2],
            bw: 0.0,
            lfo: 0.0,
            fl: [0.0; 2],
            lowcut: [OnePole::default(); 2],
            highcut: [OnePole::default(); 2],
            sc,
        }
    }
}

#[inline]
fn allpass(dl: &mut DelayLine, x: f32, g: f32, delay: f32) -> f32 {
    let d = dl.read(delay);
    let v = x - g * d;
    dl.push(v);
    d + g * v
}

impl Fx {
    pub fn new(fs: f32) -> Self {
        Self {
            fs,
            hyper: Hyper { dl: [DelayLine::new(ms(fs, 60.0)), DelayLine::new(ms(fs, 60.0))], ph: 0.0, dim: [DelayLine::new(ms(fs, 40.0)), DelayLine::new(ms(fs, 40.0))], dim_hp: [OnePole::default(); 2] },
            dist: Dist { hold: [0.0; 2], cnt: 0.0, tone: [OnePole::default(); 2], dc: [OnePole::default(); 2] },
            flanger: Flanger { dl: [DelayLine::new(ms(fs, 30.0)), DelayLine::new(ms(fs, 30.0))], ph: 0.0, fb: [0.0; 2] },
            phaser: Phaser { z: [[0.0; 12]; 2], ph: 0.0, fb: [0.0; 2] },
            chorus: Chorus { dl: [DelayLine::new(ms(fs, 80.0)), DelayLine::new(ms(fs, 80.0))], ph: 0.0, fb: [0.0; 2], lp: [OnePole::default(); 2] },
            delay: Delay { dl: [DelayLine::new((fs * 2.0) as usize + 8), DelayLine::new((fs * 2.0) as usize + 8)], lp: [OnePole::default(); 2], hp: [OnePole::default(); 2] },
            comp: Comp { bands: [CompBand::default(); 3], lo: [[Bq::default(); 2]; 2], hi1: [[Bq::default(); 2]; 2], lo2: [[Bq::default(); 2]; 2], hi2: [[Bq::default(); 2]; 2] },
            plate: Plate::new(fs),
            eq: EqM { ls: [Bq::default(); 2], pk: [Bq::default(); 2], hs: [Bq::default(); 2] },
            ff: FiltFx { s: [[0.0; 2]; 4] },
        }
    }

    pub fn reset(&mut self) {
        let fs = self.fs;
        *self = Fx::new(fs);
    }

    pub fn process(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let mut used = [false; 10];
        for slot in 0..10 {
            let m = p.order[slot].min(9);
            if used[m] {
                continue;
            }
            used[m] = true;
            if !p.on[m] {
                continue;
            }
            match m {
                0 => self.run_hyper(p, l, r),
                1 => self.run_dist(p, l, r),
                2 => self.run_flanger(p, l, r),
                3 => self.run_phaser(p, l, r),
                4 => self.run_chorus(p, l, r),
                5 => self.run_delay(p, l, r),
                6 => self.run_comp(p, l, r),
                7 => self.run_reverb(p, l, r),
                8 => self.run_eq(p, l, r),
                _ => self.run_filter(p, l, r),
            }
        }
    }

    // ------------------------------------------------------------ hyper / dimension
    fn run_hyper(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let h = &mut self.hyper;
        let n = p.hy_uni.clamp(2, 8);
        let inc = p.hy_rate / fs;
        let depth = (0.5 + p.hy_detune * 11.0) * 0.001 * fs; // samples
        let base = 0.016 * fs;
        let norm = 1.0 / (n as f32).sqrt();
        let a_hp = op_coef(300.0, fs);
        for i in 0..l.len() {
            h.ph += inc;
            if h.ph >= 1.0 {
                h.ph -= 1.0;
            }
            let dry = [l[i], r[i]];
            h.dl[0].push(dry[0]);
            h.dl[1].push(dry[1]);
            let (mut wl, mut wr) = (0.0, 0.0);
            if p.hy_mix > 0.0 {
                for v in 0..n {
                    // every voice has its own phase and a slightly different speed
                    let ph = (h.ph * (1.0 + 0.071 * v as f32) + v as f32 / n as f32).fract();
                    let m = (TAU * ph).sin();
                    let d = base + depth * m * (0.6 + 0.4 * (v as f32 / n as f32));
                    let pan = (v as f32 + 0.5) / n as f32;
                    let (gl, gr) = ((pan * PI * 0.5).cos(), (pan * PI * 0.5).sin());
                    let sl = h.dl[0].read(d);
                    let sr = h.dl[1].read(d * 1.013);
                    wl += (sl * gl + sr * (1.0 - gl) * 0.4) * norm * 1.4;
                    wr += (sr * gr + sl * (1.0 - gr) * 0.4) * norm * 1.4;
                }
            }
            let mut ol = dry[0] * (1.0 - p.hy_mix * 0.5) + wl * p.hy_mix;
            let mut or = dry[1] * (1.0 - p.hy_mix * 0.5) + wr * p.hy_mix;
            if p.hy_dim > 0.0 {
                h.dim[0].push(ol);
                h.dim[1].push(or);
                let dl_ = h.dim_hp[0].hp(h.dim[0].read(0.011 * fs), a_hp);
                let dr_ = h.dim_hp[1].hp(h.dim[1].read(0.0173 * fs), a_hp);
                let a = p.hy_dim * 0.7;
                let (nl, nr) = (ol - a * dr_, or + a * dl_);
                ol = nl;
                or = nr;
            }
            l[i] = ol;
            r[i] = or;
        }
    }

    // ------------------------------------------------------------ distortion
    fn run_dist(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let d = &mut self.dist;
        let g = 1.0 + p.ds_drive * 24.0;
        let a_tone = op_coef(p.ds_tone, fs);
        let a_dc = op_coef(18.0, fs);
        let down = 1.0 + p.ds_drive * 40.0;
        for i in 0..l.len() {
            let tick = if p.ds_type == DistType::Downsample {
                d.cnt += 1.0;
                if d.cnt >= down {
                    d.cnt -= down;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            for ch in 0..2 {
                let x = if ch == 0 { l[i] } else { r[i] };
                let y = match p.ds_type {
                    DistType::Soft => (x * g).tanh() / g.sqrt().max(1.0),
                    DistType::Hard => (x * g).clamp(-1.0, 1.0) / g.sqrt().max(1.0),
                    DistType::Fold => {
                        let mut v = x * (1.0 + p.ds_drive * 6.0);
                        for _ in 0..4 {
                            if v > 1.0 {
                                v = 2.0 - v;
                            } else if v < -1.0 {
                                v = -2.0 - v;
                            }
                        }
                        v * 0.8
                    }
                    DistType::Crush => {
                        let q = 2f32.powf(12.0 - p.ds_drive * 10.0);
                        (x * q).round() / q
                    }
                    DistType::Tube => ((x * g + 0.25).tanh() - 0.25f32.tanh()) / g.sqrt().max(1.0),
                    DistType::Diode1 => x.signum() * (1.0 - (-(x.abs() * g)).exp()) / g.sqrt().max(1.0),
                    DistType::Diode2 => {
                        if x > 0.0 {
                            (1.0 - (-(x * g)).exp()) / g.sqrt().max(1.0)
                        } else {
                            -0.45 * (1.0 - (x * g * 0.5).exp()) / g.sqrt().max(1.0)
                        }
                    }
                    DistType::SinFold => (x * (1.0 + p.ds_drive * 8.0) * 1.8).sin() * 0.8,
                    DistType::ZeroSq => (x * g * 12.0).clamp(-1.0, 1.0) * 0.5,
                    DistType::Asym => {
                        let v = x * g;
                        (if v > 0.0 { v.tanh() } else { (v * 0.4).tanh() * 1.8 }) / g.sqrt().max(1.0)
                    }
                    DistType::Rectify => {
                        let v = x * (1.0 + p.ds_drive * 3.0);
                        v + (v.abs() - v) * p.ds_drive
                    }
                    DistType::SineShaper => ((x * g).clamp(-1.0, 1.0) * PI * 0.5).sin() / g.sqrt().max(1.0),
                    DistType::Downsample => {
                        if tick {
                            d.hold[ch] = x;
                        }
                        d.hold[ch]
                    }
                };
                let y = d.tone[ch].lp(y, a_tone);
                let y = d.dc[ch].hp(y, a_dc);
                let o = x * (1.0 - p.ds_mix) + y * p.ds_mix;
                if ch == 0 {
                    l[i] = o;
                } else {
                    r[i] = o;
                }
            }
        }
    }

    // ------------------------------------------------------------ flanger
    fn run_flanger(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let f = &mut self.flanger;
        let inc = p.fg_rate / fs;
        let base = p.fg_delay * 0.001 * fs;
        let depth = p.fg_depth * base.max(0.0008 * fs) * 0.95;
        for i in 0..l.len() {
            f.ph += inc;
            if f.ph >= 1.0 {
                f.ph -= 1.0;
            }
            let dry = [l[i], r[i]];
            let mut wet = [0.0f32; 2];
            for ch in 0..2 {
                let ph = (f.ph + ch as f32 * p.fg_phase * 0.5).fract();
                let d = (base + depth * (0.5 + 0.5 * (TAU * ph).sin())).max(1.0);
                f.dl[ch].push(dry[ch] + f.fb[ch] * p.fg_fb.clamp(-0.95, 0.95));
                let y = f.dl[ch].read(d);
                f.fb[ch] = y;
                wet[ch] = y;
            }
            l[i] = dry[0] * (1.0 - p.fg_mix) + wet[0] * p.fg_mix;
            r[i] = dry[1] * (1.0 - p.fg_mix) + wet[1] * p.fg_mix;
        }
    }

    // ------------------------------------------------------------ phaser
    fn run_phaser(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let ph_ = &mut self.phaser;
        let inc = p.ph_rate / fs;
        let n = p.ph_stages.clamp(2, 12);
        for i in 0..l.len() {
            ph_.ph += inc;
            if ph_.ph >= 1.0 {
                ph_.ph -= 1.0;
            }
            let dry = [l[i], r[i]];
            let mut wet = [0.0f32; 2];
            for ch in 0..2 {
                let ph = (ph_.ph + ch as f32 * p.ph_phase * 0.5).fract();
                let m = (TAU * ph).sin();
                let f = (p.ph_freq * 2f32.powf(m * p.ph_depth * 2.5)).clamp(30.0, 0.45 * fs);
                let t = (PI * f / fs).tan();
                let a = (t - 1.0) / (t + 1.0);
                let mut s = dry[ch] + ph_.fb[ch] * p.ph_fb * 0.95;
                for k in 0..n {
                    let z = ph_.z[ch][k];
                    let v = s - a * z;
                    let y = a * v + z;
                    ph_.z[ch][k] = v;
                    s = y;
                }
                ph_.fb[ch] = s;
                wet[ch] = s;
            }
            l[i] = dry[0] * (1.0 - p.ph_mix) + wet[0] * p.ph_mix;
            r[i] = dry[1] * (1.0 - p.ph_mix) + wet[1] * p.ph_mix;
        }
    }

    // ------------------------------------------------------------ chorus
    fn run_chorus(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let c = &mut self.chorus;
        let inc = p.ch_rate / fs;
        let a_lp = op_coef(p.ch_filter, fs);
        let base = p.ch_delay * 0.001 * fs;
        for i in 0..l.len() {
            c.ph += inc;
            if c.ph >= 1.0 {
                c.ph -= 1.0;
            }
            let dry = [l[i], r[i]];
            for ch in 0..2 {
                c.dl[ch].push(dry[ch] + c.fb[ch] * p.ch_fb * 0.9);
            }
            let mut wet = [0.0f32; 2];
            for ch in 0..2 {
                for v in 0..2 {
                    let ph = c.ph + ch as f32 * 0.25 + v as f32 * 0.5;
                    let lfo = (TAU * ph).sin();
                    let d = base * (1.0 + 0.5 * p.ch_depth * lfo) + 0.002 * fs * v as f32;
                    wet[ch] += c.dl[ch].read(d) * 0.5;
                }
                wet[ch] = c.lp[ch].lp(wet[ch], a_lp);
                c.fb[ch] = wet[ch];
            }
            l[i] = dry[0] * (1.0 - p.ch_mix * 0.5) + wet[0] * p.ch_mix;
            r[i] = dry[1] * (1.0 - p.ch_mix * 0.5) + wet[1] * p.ch_mix;
        }
    }

    // ------------------------------------------------------------ delay
    fn run_delay(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let d = &mut self.delay;
        let tl = (p.dl_ms_l * 0.001 * fs).clamp(2.0, fs * 1.9);
        let tr = (p.dl_ms_r * 0.001 * fs).clamp(2.0, fs * 1.9);
        let a_lp = op_coef(p.dl_lp, fs);
        let a_hp = op_coef(p.dl_hp, fs);
        let fbk = p.dl_fb.min(0.95);
        for i in 0..l.len() {
            let dry = [l[i], r[i]];
            let t = [d.dl[0].read(tl), d.dl[1].read(tr)];
            let mut f = [0.0f32; 2];
            for c in 0..2 {
                let x = d.lp[c].lp(t[c], a_lp);
                f[c] = d.hp[c].hp(x, a_hp);
            }
            if p.dl_pp {
                d.dl[0].push((dry[0] + dry[1]) * 0.5 + f[1] * fbk);
                d.dl[1].push(f[0] * fbk);
            } else {
                d.dl[0].push(dry[0] + f[0] * fbk);
                d.dl[1].push(dry[1] + f[1] * fbk);
            }
            let (m, s) = ((t[0] + t[1]) * 0.5, (t[0] - t[1]) * 0.5 * p.dl_width);
            l[i] = dry[0] + (m + s) * p.dl_mix;
            r[i] = dry[1] + (m - s) * p.dl_mix;
        }
    }

    // ------------------------------------------------------------ compressor
    fn run_comp(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let c = &mut self.comp;
        let at = (-1.0 / (p.cp_attack.max(0.05) * 0.001 * fs)).exp();
        let rl = (-1.0 / (p.cp_release.max(1.0) * 0.001 * fs)).exp();
        let makeup = 10f32.powf(p.cp_gain / 20.0);
        let slope = 1.0 - 1.0 / p.cp_ratio.max(1.0);
        let (cl1, ch1) = (BqCoef::lowpass(p.cp_f1, std::f32::consts::FRAC_1_SQRT_2, fs), BqCoef::highpass(p.cp_f1, std::f32::consts::FRAC_1_SQRT_2, fs));
        let (cl2, ch2) = (BqCoef::lowpass(p.cp_f2, std::f32::consts::FRAC_1_SQRT_2, fs), BqCoef::highpass(p.cp_f2, std::f32::consts::FRAC_1_SQRT_2, fs));
        let gain_for = |band: &mut CompBand, level: f32| -> f32 {
            let db = 20.0 * level.max(1e-6).log10();
            let over = db - p.cp_thresh;
            // 6 dB soft knee
            let gr = if over <= -3.0 {
                0.0
            } else if over < 3.0 {
                slope * (over + 3.0) * (over + 3.0) / 12.0
            } else {
                slope * over
            };
            let coef = if gr > band.env { at } else { rl };
            band.env = gr + coef * (band.env - gr);
            10f32.powf(-band.env / 20.0)
        };
        for i in 0..l.len() {
            let dry = [l[i], r[i]];
            if !p.cp_multi {
                let level = dry[0].abs().max(dry[1].abs());
                let g = gain_for(&mut c.bands[0], level) * makeup;
                let (wl, wr) = (dry[0] * g, dry[1] * g);
                l[i] = dry[0] * (1.0 - p.cp_mix) + wl * p.cp_mix;
                r[i] = dry[1] * (1.0 - p.cp_mix) + wr * p.cp_mix;
            } else {
                let mut bands = [[0.0f32; 2]; 3];
                for ch in 0..2 {
                    let x = dry[ch];
                    let t1 = c.lo[ch][0].tick(&cl1, x);
                    let lo = c.lo[ch][1].tick(&cl1, t1);
                    let t1 = c.hi1[ch][0].tick(&ch1, x);
                    let rest = c.hi1[ch][1].tick(&ch1, t1);
                    let t1 = c.lo2[ch][0].tick(&cl2, rest);
                    let mid = c.lo2[ch][1].tick(&cl2, t1);
                    let t1 = c.hi2[ch][0].tick(&ch2, rest);
                    let hi = c.hi2[ch][1].tick(&ch2, t1);
                    bands[0][ch] = lo;
                    bands[1][ch] = mid;
                    bands[2][ch] = hi;
                }
                let mut out = [0.0f32; 2];
                for b in 0..3 {
                    let level = bands[b][0].abs().max(bands[b][1].abs());
                    let g = gain_for(&mut c.bands[b], level) * makeup;
                    out[0] += bands[b][0] * g;
                    out[1] += bands[b][1] * g;
                }
                l[i] = dry[0] * (1.0 - p.cp_mix) + out[0] * p.cp_mix;
                r[i] = dry[1] * (1.0 - p.cp_mix) + out[1] * p.cp_mix;
            }
        }
    }

    // ------------------------------------------------------------ reverb (plate)
    fn run_reverb(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let pl = &mut self.plate;
        let sc = pl.sc;
        let decay = 0.35 + 0.62 * p.rv_size;
        let damp = 0.02 + 0.9 * p.rv_damp;
        let a_lc = op_coef(p.rv_lowcut, fs);
        let a_hc = op_coef(p.rv_highcut, fs);
        let pre = (p.rv_pre * 0.001 * fs).max(1.0);
        let t = |x: f32| x * sc;
        for i in 0..l.len() {
            pl.lfo += 0.7 / fs;
            if pl.lfo >= 1.0 {
                pl.lfo -= 1.0;
            }
            let m1 = (TAU * pl.lfo).sin() * t(8.0);
            let m2 = (TAU * (pl.lfo + 0.37)).sin() * t(8.0);
            let dry = [l[i], r[i]];
            pl.pre.push((dry[0] + dry[1]) * 0.5 * 0.5);
            let mut x = pl.pre.read(pre);
            // input bandwidth + diffusion
            pl.bw += 0.9995 * (x - pl.bw);
            x = pl.bw;
            for (dl, g) in pl.inp_ap.iter_mut() {
                let d = dl.buf.len() as f32 - 66.0;
                x = allpass(dl, x, *g, d);
            }
            // tank
            let in_l = x + pl.fl[1] * decay;
            let in_r = x + pl.fl[0] * decay;
            // left
            let a = allpass(&mut pl.l_ap1, in_l, -0.7, t(672.0) + m1);
            pl.l_d1.push(a);
            let d1 = pl.l_d1.read(t(4453.0) - 1.0);
            pl.damp[0] += (1.0 - damp) * (d1 - pl.damp[0]);
            let b = allpass(&mut pl.l_ap2, pl.damp[0] * decay, 0.5, t(1800.0));
            pl.l_d2.push(b);
            pl.fl[0] = pl.l_d2.read(t(3720.0) - 1.0);
            // right
            let a = allpass(&mut pl.r_ap1, in_r, -0.7, t(908.0) + m2);
            pl.r_d1.push(a);
            let d1 = pl.r_d1.read(t(4217.0) - 1.0);
            pl.damp[1] += (1.0 - damp) * (d1 - pl.damp[1]);
            let b = allpass(&mut pl.r_ap2, pl.damp[1] * decay, 0.5, t(2656.0));
            pl.r_d2.push(b);
            pl.fl[1] = pl.r_d2.read(t(3163.0) - 1.0);
            // output taps
            let tp = |v: f32| (v * sc) as usize;
            let yl = pl.r_d1.tap(tp(266.0)) + pl.r_d1.tap(tp(2974.0)) - pl.r_ap2.tap(tp(1913.0)) + pl.r_d2.tap(tp(1996.0)) - pl.l_d1.tap(tp(1990.0)) - pl.l_ap2.tap(tp(187.0)) - pl.l_d2.tap(tp(1066.0));
            let yr = pl.l_d1.tap(tp(353.0)) + pl.l_d1.tap(tp(3627.0)) - pl.l_ap2.tap(tp(1228.0)) + pl.l_d2.tap(tp(2111.0)) - pl.r_d1.tap(tp(121.0)) - pl.r_ap2.tap(tp(335.0)) - pl.r_d2.tap(tp(1990.0));
            let (mut wl, mut wr) = (yl * 0.6, yr * 0.6);
            let (m, s) = ((wl + wr) * 0.5, (wl - wr) * 0.5 * p.rv_width);
            wl = m + s;
            wr = m - s;
            wl = pl.highcut[0].lp(pl.lowcut[0].hp(wl, a_lc), a_hc);
            wr = pl.highcut[1].lp(pl.lowcut[1].hp(wr, a_lc), a_hc);
            l[i] = dry[0] + wl * p.rv_mix * 2.0;
            r[i] = dry[1] + wr * p.rv_mix * 2.0;
        }
    }

    // ------------------------------------------------------------ EQ
    fn run_eq(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let e = &mut self.eq;
        let ls = BqCoef::low_shelf(p.eq_lf, p.eq_lg, fs);
        let pk = BqCoef::peaking(p.eq_mf, p.eq_mq, p.eq_mg, fs);
        let hs = BqCoef::high_shelf(p.eq_hf, p.eq_hg, fs);
        for i in 0..l.len() {
            let mut x = l[i];
            x = e.ls[0].tick(&ls, x);
            x = e.pk[0].tick(&pk, x);
            l[i] = e.hs[0].tick(&hs, x);
            let mut y = r[i];
            y = e.ls[1].tick(&ls, y);
            y = e.pk[1].tick(&pk, y);
            r[i] = e.hs[1].tick(&hs, y);
        }
    }

    // ------------------------------------------------------------ filter
    fn run_filter(&mut self, p: &FxPatch, l: &mut [f32], r: &mut [f32]) {
        let fs = self.fs;
        let f = &mut self.ff;
        let fc = p.ff_cut.clamp(20.0, 0.45 * fs);
        let g = (PI * fc / fs).tan();
        let k = 0.05 + 1.95 * (1.0 - p.ff_res).powi(2);
        let a1 = 1.0 / (1.0 + g * (g + k));
        let (a2, a3) = (g * a1, g * g * a1);
        let gain = 10f32.powf(p.ff_res * 18.0 / 20.0);
        let gd = 1.0 + p.ff_drive * 8.0;
        let two = matches!(p.ff_type, FxFilterType::Lp24 | FxFilterType::Hp24);
        for i in 0..l.len() {
            for ch in 0..2 {
                let x0 = if ch == 0 { l[i] } else { r[i] };
                let x = x0 * (1.0 - p.ff_drive) + p.ff_drive * (gd * x0).tanh() / gd.sqrt();
                let run = |s: &mut [f32; 2], x: f32| -> (f32, f32, f32) {
                    let v3 = x - s[1];
                    let v1 = a1 * s[0] + a2 * v3;
                    let v2 = s[1] + a2 * s[0] + a3 * v3;
                    s[0] = 2.0 * v1 - s[0];
                    s[1] = 2.0 * v2 - s[1];
                    (v1, v2, x - k * v1 - v2)
                };
                let (band, low, high) = run(&mut f.s[ch * 2], x);
                let mut y = match p.ff_type {
                    FxFilterType::Lp12 | FxFilterType::Lp24 => low,
                    FxFilterType::Hp12 | FxFilterType::Hp24 => high,
                    FxFilterType::Band => band * k,
                    FxFilterType::Notch => low + high,
                    FxFilterType::Peak => x + (gain - 1.0) * k * band,
                };
                if two {
                    let (_, l2, h2) = run(&mut f.s[ch * 2 + 1], y);
                    y = if p.ff_type == FxFilterType::Lp24 { l2 } else { h2 };
                }
                if !y.is_finite() {
                    f.s = [[0.0; 2]; 4];
                    y = 0.0;
                }
                let o = x0 * (1.0 - p.ff_mix) + y * p.ff_mix;
                if ch == 0 {
                    l[i] = o;
                } else {
                    r[i] = o;
                }
            }
        }
    }
}

#[allow(dead_code)]
fn _unused(d: &mut DelayLine) {
    d.clear();
}

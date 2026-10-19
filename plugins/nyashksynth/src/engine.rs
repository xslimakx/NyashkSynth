//! Voice engine: wavetable oscillators with unison and warp, sub, noise, filter, envelopes,
//! LFOs and a modulation matrix.

use crate::arp::{Arp, ArpP};
use crate::clip::{ClipP, MAX_STEPS};
use crate::fx::BqCoef;
use crate::params::*;
use crate::sample::{LfoCurve, NSample};
use crate::wavetable::SH;
use crate::shared::Shared;
use crate::wavetable::{tables, Table};
use nih_plug::prelude::Enum;
use std::f32::consts::{PI, TAU};
use std::sync::Arc;

pub const VOICES: usize = 16;
const K: usize = 16; // control-rate block
const DSTS: usize = 64;
const MAX_UNI: usize = 16;

// ---------------------------------------------------------------- patch snapshot

#[derive(Clone)]
pub struct OscP {
    pub on: bool,
    pub tbl: Arc<Table>,
    pub pos: f32,
    pub wmode: WarpMode,
    pub warp: f32,
    pub wmode2: WarpMode,
    pub warp2: f32,
    pub level: f32,
    pub pan: f32,
    pub oct: i32,
    pub semi: i32,
    pub tmode: crate::params::OscTune,
    pub ratio: f32,
    pub harm: i32,
    pub fine: f32,
    pub uni: usize,
    pub detune: f32,
    pub blend: f32,
    pub umode: UniMode,
    pub spread: f32,
    pub phase: f32,
    pub rand: f32,
    pub mode: OscMode,
    pub s_start: f32,
    pub s_end: f32,
    pub s_loop: SampleLoop,
    pub s_xfade: f32,
    pub s_root: f32,
    pub sp_tilt: f32,
    pub sp_shift: f32,
    pub sp_eo: f32,
    pub g_size: f32,
    pub g_dens: f32,
    pub g_spray: f32,
    pub sample: Option<Arc<NSample>>,
    pub multi: Option<Arc<crate::sample::MultiS>>,
}

#[derive(Clone, Copy)]
pub struct EnvP {
    pub delay_n: f32,
    pub a_inc: f32,
    pub hold_n: f32,
    pub d_coef: f32,
    pub s: f32,
    pub r_coef: f32,
    pub ka: f32,
    pub kd: f32,
    pub kr: f32,
    pub relegato: bool,
}

#[derive(Clone, Copy)]
pub struct SlotP {
    pub src: ModSrc,
    pub dst: ModDst,
    pub amount: f32,
    pub aux: ModSrc,
    pub curve: f32,
    pub uni: bool,
}

impl SlotP {
    /// A bypassed slot behaves like an empty one.
    pub fn into_enabled(mut self, on: bool) -> Self {
        if !on {
            self.src = ModSrc::None;
        }
        self
    }
}

#[derive(Clone)]
pub struct LfoP {
    pub shape: LfoShape,
    pub rate: f32,
    pub sync: bool,
    pub div: Div,
    pub trig: bool,
    pub rise_s: f32,
    pub smooth: f32,
    pub delay_s: f32,
    pub oneshot: bool,
    pub phase0: f32,
}

/// Settings of one filter.
#[derive(Clone, Copy)]
pub struct FilP {
    pub on: bool,
    pub ty: FilterType,
    pub cutoff: f32,
    pub res: f32,
    pub drive: f32,
    pub keytrack: f32,
    pub mix: f32,
    /// A, B, C, sub, noise
    pub route: [bool; 5],
    pub cb: [CbP; CBANDS],
}

impl FilP {
    pub fn read(f: &FilterParams) -> FilP {
        FilP {
            on: f.fl_on.value(),
            ty: f.fl_type.value(),
            cutoff: f.cutoff.value(),
            res: f.res.value(),
            drive: f.drive.value(),
            keytrack: f.keytrack.value(),
            mix: f.fl_mix.value(),
            route: [f.fl_a.value(), f.fl_b.value(), f.fl_c.value(), f.fl_sub.value(), f.fl_noise.value()],
            cb: std::array::from_fn(|i| CbP { on: f.cb[i].on.value(), kind: f.cb[i].kind.value(), freq: f.cb[i].freq.value(), gain: f.cb[i].gain.value(), q: f.cb[i].q.value() }),
        }
    }
}

#[derive(Clone, Copy)]
pub struct CbP {
    pub on: bool,
    pub kind: BandKind,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
}

/// Biquad of one custom-filter point (`shift` = frequency ratio from modulation / key tracking).
pub fn cband_coef(b: &CbP, shift: f32, fs: f32) -> BqCoef {
    cband_coef_m(b, shift, 0.0, 1.0, fs)
}

/// Point-filter band with modulation: frequency shift (ratio), gain offset (dB) and Q multiplier.
pub fn cband_coef_m(b: &CbP, shift: f32, dgain: f32, qmul: f32, fs: f32) -> BqCoef {
    let f = (b.freq * shift).clamp(20.0, 0.45 * fs);
    let g = (b.gain + dgain).clamp(-30.0, 30.0);
    let q = (b.q * qmul).clamp(0.1, 30.0);
    match b.kind {
        BandKind::Bell => BqCoef::peaking(f, q, g, fs),
        BandKind::LowShelf => BqCoef::low_shelf(f, g, fs),
        BandKind::HighShelf => BqCoef::high_shelf(f, g, fs),
        BandKind::HighPass => BqCoef::highpass(f, q.max(0.3) * 0.7071, fs),
        BandKind::LowPass => BqCoef::lowpass(f, q.max(0.3) * 0.7071, fs),
        BandKind::Notch => BqCoef::notch(f, q, fs),
        BandKind::BandPass => BqCoef::bandpass(f, q, fs),
    }
}

#[derive(Clone)]
pub struct Patch {
    pub osc: [OscP; 3],
    pub sub_on: bool,
    pub sub_wave: SubWave,
    pub sub_oct: i32,
    pub sub_level: f32,
    pub sub_pan: f32,
    pub noise: f32,
    pub noise_type: NoiseType,
    pub noise_pan: f32,
    pub noise_pitch: f32,
    pub noise_key: bool,
    pub noise_loop: bool,
    pub noise_phase: f32,
    pub noise_rand: f32,
    pub sample: Option<Arc<NSample>>,
    pub fl: [FilP; 2],
    pub fl_serial: bool,
    pub env: [EnvP; NENV],
    pub lfo: [LfoP; NLFO],
    pub lfo_curve: [Option<Arc<LfoCurve>>; NLFO],
    pub slots: [SlotP; MOD_SLOTS],
    pub arp: ArpP,
    pub clip: ClipP,
    pub macros: [f32; NMAC],
    pub vmode: VoiceMode,
    pub glide_ms: f32,
    pub bend: f32,
    pub poly: usize,
    pub tune: f32,
    pub scale: crate::params::ScaleType,
    pub scale_root: i32,
    pub a4: f32,
    pub tun: [f32; 12],
    pub tun_root: i32,
    pub vel_curve: f32,
}

fn osc_read(o: &OscParams, tbl: Arc<Table>, sample: Option<Arc<NSample>>, multi: Option<Arc<crate::sample::MultiS>>) -> OscP {
    OscP {
        mode: o.mode.value(),
        s_start: o.s_start.value(),
        s_end: o.s_end.value(),
        s_loop: o.s_loop.value(),
        s_xfade: o.s_xfade.value(),
        s_root: o.s_root.value() as f32,
        sp_tilt: o.sp_tilt.value(),
        sp_shift: o.sp_shift.value(),
        sp_eo: o.sp_eo.value(),
        g_size: o.g_size.value(),
        g_dens: o.g_dens.value(),
        g_spray: o.g_spray.value(),
        sample,
        multi,
        on: o.on.value(),
        tbl,
        pos: o.pos.value(),
        wmode: o.warp_mode.value(),
        warp: o.warp.value(),
        wmode2: o.warp2_mode.value(),
        warp2: o.warp2.value(),
        level: o.level.value(),
        pan: o.pan.value(),
        oct: o.oct.value(),
        semi: o.semi.value(),
        tmode: o.tune_mode.value(),
        ratio: o.ratio.value(),
        harm: o.harm.value(),
        fine: o.fine.value(),
        uni: o.uni.value().clamp(1, MAX_UNI as i32) as usize,
        detune: o.detune.value(),
        blend: o.blend.value(),
        umode: o.uni_mode.value(),
        spread: o.spread.value(),
        phase: o.phase.value(),
        rand: o.rand.value(),
    }
}

fn env_read(e: &EnvParams, fs: f32) -> EnvP {
    let ms = |v: f32| if v <= 0.6 { 0.0 } else { v * 0.001 * fs };
    let a = (e.attack.value() * 0.001 * fs).max(0.0015 * fs); // >= 1.5 ms: no click on note start
    let d = (e.decay.value() * 0.001 / 4.6 * fs).max(1.0);
    let r = (e.release.value() * 0.001 / 4.6 * fs).max(0.006 / 4.6 * fs); // >= 6 ms: no click on note end
    let k = |c: f32| 2f32.powf(c * 2.0);
    EnvP { delay_n: ms(e.delay.value()), a_inc: 1.0 / a, hold_n: ms(e.hold.value()), d_coef: (-1.0 / d).exp(), s: e.sustain.value(), r_coef: (-1.0 / r).exp(), ka: k(e.atk_curve.value()), kd: k(e.dec_curve.value()), kr: k(e.rel_curve.value()), relegato: e.relegato.value() }
}

fn lfo_read(l: &LfoParams) -> LfoP {
    let rise = l.rise.value();
    LfoP { shape: l.shape.value(), rate: l.rate.value(), sync: l.sync.value(), div: l.div.value(), trig: l.trig.value(), rise_s: if rise <= 0.6 { 0.0 } else { rise * 0.001 }, smooth: l.smooth.value(), delay_s: { let d = l.delay.value(); if d <= 0.6 { 0.0 } else { d * 0.001 } }, oneshot: l.oneshot.value(), phase0: l.phase.value() }
}

impl Patch {
    /// Snapshot using only the built-in wavetables (tests / offline tools).
    pub fn read(p: &SynthParams, fs: f32) -> Patch {
        Self::read_with(p, fs, None, None)
    }

    /// Snapshot with the user data in `shared`. If a slot is busy the previous patch's value is kept.
    pub fn read_with(p: &SynthParams, fs: f32, shared: Option<&Shared>, prev: Option<&Patch>) -> Patch {
        let pick = |o: usize, idx: usize| -> Arc<Table> {
            let builtin = || tables().t[idx.min(tables().t.len() - 1)].clone();
            match shared.map(|s| s.waves[o].try_get()) {
                Some(Some(Some(t))) => t,
                Some(Some(None)) | None => builtin(),
                Some(None) => prev.map(|pp| pp.osc[o].tbl.clone()).unwrap_or_else(builtin),
            }
        };
        let pick_s = |o: usize| -> Option<Arc<NSample>> {
            match shared.map(|s| s.smp[o].try_get()) {
                Some(Some(x)) => x,
                Some(None) => prev.and_then(|pp| pp.osc[o].sample.clone()),
                None => None,
            }
        };
        let pick_m = |o: usize| -> Option<Arc<crate::sample::MultiS>> {
            match shared.map(|s| s.ms[o].try_get()) {
                Some(Some(x)) => x,
                Some(None) => prev.and_then(|pp| pp.osc[o].multi.clone()),
                None => None,
            }
        };
        let sample = match shared.map(|s| s.noise.try_get()) {
            Some(Some(x)) => x,
            Some(None) => prev.and_then(|pp| pp.sample.clone()),
            None => None,
        };
        let lfo_curve: [Option<Arc<LfoCurve>>; NLFO] = std::array::from_fn(|i| match shared.map(|s| s.lfo[i].try_get()) {
            Some(Some(x)) => x,
            Some(None) => prev.and_then(|pp| pp.lfo_curve[i].clone()),
            None => None,
        });
        Patch {
            osc: [osc_read(&p.a, pick(0, p.a.table.value().to_index()), pick_s(0), pick_m(0)), osc_read(&p.b, pick(1, p.b.table.value().to_index()), pick_s(1), pick_m(1)), osc_read(&p.c, pick(2, p.c.table.value().to_index()), pick_s(2), pick_m(2))],
            sub_on: p.sub_on.value(),
            sub_wave: p.sub_wave.value(),
            sub_oct: p.sub_oct.value(),
            sub_level: p.sub_level.value(),
            sub_pan: p.sub_pan.value(),
            noise: p.noise_level.value(),
            noise_type: p.noise_type.value(),
            noise_pan: p.noise_pan.value(),
            noise_pitch: p.noise_pitch.value(),
            noise_key: p.noise_key.value(),
            noise_loop: p.noise_loop.value(),
            noise_phase: p.noise_phase.value(),
            noise_rand: p.noise_rand.value(),
            sample,
            fl: [FilP::read(&p.f1), FilP::read(&p.f2)],
            fl_serial: p.fl_serial.value(),
            env: [env_read(&p.e1, fs), env_read(&p.e2, fs), env_read(&p.e3, fs), env_read(&p.e4, fs)],
            lfo: std::array::from_fn(|i| lfo_read(&p.lfo[i])),
            lfo_curve,
            slots: std::array::from_fn(|i| SlotP { src: p.slot[i].src.value(), dst: p.slot[i].dst.value(), amount: p.slot[i].amount.value(), aux: p.slot[i].aux.value(), curve: p.slot[i].curve.value(), uni: p.slot[i].uni.value() }.into_enabled(p.slot[i].on.value())),
            clip: {
                let sel = (p.clip_sel.value().clamp(1, 12) - 1) as usize;
                match p.clips.try_lock() {
                    Ok(g) => {
                        let mut c = crate::clip::ClipP::from_clip(p.clip_on.value(), p.clip_div.value().beats(), &crate::clip::Clip::decode(g.split('#').nth(sel).unwrap_or("")));
                        c.swing = p.clip_swing.value();
                        c.sync = p.clip_sync.value();
                        c
                    }
                    Err(_) => {
                        let mut c = prev.map(|pp| pp.clip).unwrap_or_default();
                        c.on = p.clip_on.value();
                        c.swing = p.clip_swing.value();
                        c.sync = p.clip_sync.value();
                        c
                    }
                }
            },
            arp: {
                let (pat, plen) = match p.arp_pat.try_lock() {
                    Ok(g) => crate::arp::parse_pattern(g.split('#').nth((p.arp_sel.value().clamp(1, 12) - 1) as usize).unwrap_or("")),
                    Err(_) => prev.map(|pp| (pp.arp.pat, pp.arp.plen)).unwrap_or_else(crate::arp::default_pattern),
                };
                ArpP { pat, plen, on: p.arp_on.value(), mode: p.arp_mode.value(), beats: p.arp_div.value().beats(), octs: p.arp_oct.value().clamp(1, 4) as usize, gate: p.arp_gate.value(), swing: p.arp_swing.value(), hold: p.arp_hold.value() }
            },
            macros: p.macros().map(|m| m.value()),
            vmode: p.vmode.value(),
            glide_ms: p.glide.value(),
            bend: p.bend.value() as f32,
            poly: p.poly.value().clamp(1, VOICES as i32) as usize,
            tune: p.tune.value(),
            scale: p.key_scale.value(),
            scale_root: p.key_root.value(),
            a4: p.a4.value(),
            tun: p.tuning.value().cents(),
            tun_root: p.tun_root.value(),
            vel_curve: p.vel_curve.value(),
        }
    }
}

// ---------------------------------------------------------------- helpers

#[inline]
fn hash01(x: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E37_79B1) ^ 0x85EB_CA6B;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

const CHAOS_N: usize = 4096;

/// One cycle of a chaotic attractor (x variable, normalised), made loopable with a short cross-fade.
fn chaos_lut(kind: usize) -> &'static [f32; CHAOS_N] {
    use std::sync::OnceLock;
    static L: OnceLock<[Box<[f32; CHAOS_N]>; 2]> = OnceLock::new();
    let l = L.get_or_init(|| {
        let mk = |kind: usize| -> Box<[f32; CHAOS_N]> {
            let n = CHAOS_N + 512;
            let mut raw = vec![0.0f64; n];
            let (mut x, mut y, mut z) = if kind == 0 { (1.0f64, 1.0, 1.0) } else { (0.1f64, 0.0, 0.0) };
            let (dt, stride) = if kind == 0 { (0.004f64, 2usize) } else { (0.02f64, 1usize) };
            let step = |x: &mut f64, y: &mut f64, z: &mut f64| {
                let (dx, dy, dz) = if kind == 0 {
                    (10.0 * (*y - *x), *x * (28.0 - *z) - *y, *x * *y - 8.0 / 3.0 * *z)
                } else {
                    (-*y - *z, *x + 0.2 * *y, 0.2 + *z * (*x - 5.7))
                };
                *x += dx * dt;
                *y += dy * dt;
                *z += dz * dt;
            };
            for _ in 0..4000 {
                step(&mut x, &mut y, &mut z);
            }
            for r in raw.iter_mut() {
                for _ in 0..stride {
                    step(&mut x, &mut y, &mut z);
                }
                *r = x;
            }
            let mean = raw.iter().sum::<f64>() / n as f64;
            let mx = raw.iter().map(|v| (v - mean).abs()).fold(1e-9, f64::max);
            let mut out = Box::new([0.0f32; CHAOS_N]);
            for i in 0..CHAOS_N {
                let a = (raw[i] - mean) / mx;
                out[i] = if i < 512 {
                    // the head blends in the samples that would follow the end: the loop is seamless
                    let t = i as f64 / 512.0;
                    let b = (raw[CHAOS_N + i] - mean) / mx;
                    (a * t + b * (1.0 - t)) as f32
                } else {
                    a as f32
                };
            }
            out
        };
        [mk(0), mk(1)]
    });
    &l[kind]
}

fn chaos_eval(kind: usize, f: f32, seed: u32) -> f32 {
    let l = chaos_lut(kind);
    let off = (seed & 0x3FF) as usize * 3;
    let x = f * CHAOS_N as f32;
    let i = x as usize;
    let fr = x - i as f32;
    let a = l[(i + off) % CHAOS_N];
    let b = l[(i + 1 + off) % CHAOS_N];
    a + (b - a) * fr
}

pub fn lfo_eval(shape: LfoShape, ph: f64, seed: u32) -> f32 {
    lfo_eval_c(shape, ph, seed, None)
}

pub fn lfo_eval_c(shape: LfoShape, ph: f64, seed: u32, curve: Option<&LfoCurve>) -> f32 {
    let c = ph.floor();
    let f = (ph - c) as f32;
    let ci = c as i64 as u32;
    match shape {
        LfoShape::Sine => (TAU * f).sin(),
        LfoShape::Tri => {
            if f < 0.25 {
                4.0 * f
            } else if f < 0.75 {
                2.0 - 4.0 * f
            } else {
                4.0 * f - 4.0
            }
        }
        LfoShape::SawUp => 2.0 * f - 1.0,
        LfoShape::SawDown => 1.0 - 2.0 * f,
        LfoShape::Square => {
            if f < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoShape::Sh => hash01(ci ^ seed) * 2.0 - 1.0,
        LfoShape::Smooth => {
            let a = hash01(ci ^ seed) * 2.0 - 1.0;
            let b = hash01(ci.wrapping_add(1) ^ seed) * 2.0 - 1.0;
            let s = f * f * (3.0 - 2.0 * f);
            a + (b - a) * s
        }
        LfoShape::Custom => match curve {
            Some(cv) => cv.eval(f),
            None => (TAU * f).sin(),
        },
        LfoShape::TriSoft => {
            let t = if f < 0.25 {
                4.0 * f
            } else if f < 0.75 {
                2.0 - 4.0 * f
            } else {
                4.0 * f - 4.0
            };
            (t * std::f32::consts::FRAC_PI_2).sin()
        }
        LfoShape::Stairs => ((f * 6.0).floor() / 2.5 - 1.0).clamp(-1.0, 1.0),
        LfoShape::Lorenz => chaos_eval(0, f, seed),
        LfoShape::Rossler => chaos_eval(1, f, seed),
    }
}

/// Phase warp shared with the GUI. `p` and the result are in 0..1.
pub fn warp_phase(mode: WarpMode, amt: f32, p: f32, fm: f32) -> f32 {
    let p = if p.is_finite() { p.clamp(0.0, 1.0) } else { 0.0 };
    let fm = if fm.is_finite() { fm.clamp(-8.0, 8.0) } else { 0.0 };
    let r = match mode {
        WarpMode::Off | WarpMode::Am | WarpMode::Rm | WarpMode::SelfAm | WarpMode::SelfRm | WarpMode::Flip | WarpMode::Fold | WarpMode::Clip => p,
        WarpMode::BendPlus => p.powf(1.0 + amt * 3.0),
        WarpMode::BendMinus => 1.0 - (1.0 - p).powf(1.0 + amt * 3.0),
        WarpMode::BendBoth => {
            let e = 1.0 + amt * 3.0;
            if p < 0.5 {
                0.5 * (2.0 * p).powf(e)
            } else {
                1.0 - 0.5 * (2.0 * (1.0 - p)).powf(e)
            }
        }
        WarpMode::Pwm => {
            let w = 0.5 - 0.45 * amt;
            if p < w {
                0.5 * p / w
            } else {
                0.5 + 0.5 * (p - w) / (1.0 - w)
            }
        }
        WarpMode::Sync => {
            let x = p * (1.0 + amt * 7.0);
            x - x.floor()
        }
        WarpMode::Mirror => {
            let t = if p < 0.5 { 2.0 * p } else { 2.0 - 2.0 * p };
            p + (t - p) * amt
        }
        WarpMode::Quantize => {
            if amt < 0.01 {
                p
            } else {
                let steps = 3.0 + 253.0 * (1.0 - amt).powi(3);
                (p * steps).floor() / steps
            }
        }
        WarpMode::Fm | WarpMode::SelfFm => {
            let x = p + amt * fm * 3.0;
            x - x.floor()
        }
        WarpMode::SineBend => p + amt * 0.13 * (TAU * p).sin(),
        WarpMode::Pinch => {
            let e = 1.0 + amt * 3.0;
            if p < 0.5 {
                0.5 * (2.0 * p).powf(1.0 / e)
            } else {
                1.0 - 0.5 * (2.0 * (1.0 - p)).powf(1.0 / e)
            }
        }
        WarpMode::Ripple => p + amt * 0.035 * (TAU * 5.0 * p).sin(),
        WarpMode::Twist => {
            let x = p + amt * 0.5 * (TAU * p).sin() * (TAU * p).sin().abs();
            x - x.floor()
        }
        WarpMode::Asym => {
            if p < 0.5 {
                p
            } else {
                0.5 + 0.5 * (2.0 * (p - 0.5)).powf(1.0 + amt * 3.0)
            }
        }
    };
    if r.is_finite() {
        r.clamp(0.0, 0.999_999)
    } else {
        0.0
    }
}

/// Warps that act on the amplitude of the wave (after the table lookup).
#[inline]
fn amp_warp(mode: WarpMode, amt: f32, smp: f32, w: f32, fm: f32, own: f32) -> f32 {
    match mode {
        WarpMode::Am => smp * (1.0 - amt + amt * (0.5 + 0.5 * fm)),
        WarpMode::Rm => smp * (1.0 - amt + amt * fm),
        WarpMode::SelfAm => smp * (1.0 - amt + amt * (0.5 + 0.5 * own)),
        WarpMode::SelfRm => smp * (1.0 - amt + amt * own),
        WarpMode::Flip => {
            if w < amt * 0.5 {
                -smp
            } else {
                smp
            }
        }
        WarpMode::Fold => {
            let y = smp * (1.0 + amt * 5.0);
            1.0 - (((y + 1.0).rem_euclid(4.0)) - 2.0).abs()
        }
        WarpMode::Clip => (smp * (1.0 + amt * 7.0)).clamp(-1.0, 1.0),
        _ => smp,
    }
}

/// How much the warp raises the highest frequency content (for choosing the mip level).
fn warp_bandwidth(mode: WarpMode, amt: f32) -> f32 {
    match mode {
        WarpMode::Sync => 1.0 + amt * 7.0,
        WarpMode::BendPlus | WarpMode::BendMinus | WarpMode::BendBoth | WarpMode::Asym => 1.0 + amt * 2.0,
        WarpMode::Pwm => 1.0 + amt * 4.0,
        WarpMode::Fm => 1.0 + amt * 3.0,
        WarpMode::Mirror | WarpMode::SineBend => 1.0 + amt,
        WarpMode::Quantize => 1.0 + amt * 2.0,
        WarpMode::Am | WarpMode::Rm => 1.0 + amt * 2.0,
        WarpMode::SelfFm => 1.0 + amt * 3.0,
        WarpMode::SelfAm | WarpMode::SelfRm => 1.0 + amt * 2.0,
        WarpMode::Pinch | WarpMode::Twist | WarpMode::Ripple => 1.0 + amt * 3.0,
        WarpMode::Flip => 1.0 + amt * 2.0,
        WarpMode::Fold | WarpMode::Clip => 1.0 + amt * 4.0,
        WarpMode::Off => 1.0,
    }
}

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}

fn sub_wave(w: SubWave, ph: f32, dt: f32) -> f32 {
    match w {
        SubWave::Sine => (TAU * ph).sin(),
        SubWave::Tri => {
            if ph < 0.25 {
                4.0 * ph
            } else if ph < 0.75 {
                2.0 - 4.0 * ph
            } else {
                4.0 * ph - 4.0
            }
        }
        SubWave::Saw => 2.0 * ph - 1.0 - poly_blep(ph, dt),
        SubWave::Square => {
            let mut s = if ph < 0.5 { 1.0 } else { -1.0 };
            s += poly_blep(ph, dt);
            s -= poly_blep((ph + 0.5).fract(), dt);
            s
        }
    }
}

// ---------------------------------------------------------------- envelope

#[derive(Clone, Copy, Default)]
struct Env {
    stage: u8, // 0 idle, 1 attack, 2 decay, 3 sustain, 4 release, 5 delay, 6 hold
    lvl: f32,  // internal level (exponential segments)
    out: f32,  // shaped output
    cnt: f32,
    rl0: f32,
    ro0: f32,
}

impl Env {
    fn start(&mut self, p: &EnvP) {
        self.cnt = 0.0;
        // keep the audible level on retrigger (no click): move it into attack space
        self.lvl = if p.ka == 1.0 { self.out } else { self.out.max(0.0).powf(1.0 / p.ka) };
        self.stage = if p.delay_n > 0.0 { 5 } else { 1 };
    }

    fn begin_release(&mut self) {
        if self.stage != 0 {
            self.rl0 = self.lvl.max(1e-6);
            self.ro0 = self.out;
            self.stage = 4;
        }
    }

    #[inline]
    fn tick(&mut self, p: &EnvP) -> f32 {
        match self.stage {
            5 => {
                self.cnt += 1.0;
                if self.cnt >= p.delay_n {
                    self.stage = 1;
                }
            }
            1 => {
                self.lvl += p.a_inc;
                if self.lvl >= 1.0 {
                    self.lvl = 1.0;
                    self.cnt = 0.0;
                    self.stage = if p.hold_n > 0.0 { 6 } else { 2 };
                }
                self.out = if p.ka == 1.0 { self.lvl } else { self.lvl.powf(p.ka) };
            }
            6 => {
                self.cnt += 1.0;
                self.out = 1.0;
                if self.cnt >= p.hold_n {
                    self.stage = 2;
                }
            }
            2 => {
                self.lvl = p.s + (self.lvl - p.s) * p.d_coef;
                if (self.lvl - p.s).abs() < 1e-4 {
                    self.lvl = p.s;
                    self.stage = 3;
                }
                self.out = if p.kd == 1.0 || p.s >= 1.0 { self.lvl } else { p.s + (1.0 - p.s) * ((self.lvl - p.s) / (1.0 - p.s)).clamp(0.0, 1.0).powf(p.kd) };
            }
            3 => {
                self.lvl = p.s;
                self.out = p.s;
            }
            4 => {
                self.lvl *= p.r_coef;
                let u = (self.lvl / self.rl0).clamp(0.0, 1.0);
                self.out = self.ro0 * if p.kr == 1.0 { u } else { u.powf(p.kr) };
                if self.lvl < 1e-5 || self.out < 1e-5 {
                    self.lvl = 0.0;
                    self.out = 0.0;
                    self.stage = 0;
                }
            }
            _ => {}
        }
        self.out
    }
}

// ---------------------------------------------------------------- voice

/// Filter memory of one voice.
#[derive(Clone)]
struct Fst {
    svf: [[[f32; 2]; 2]; 2], // channel, stage, (ic1eq, ic2eq)
    op: [f32; 2],            // one-pole
    lad: [[f32; 4]; 2],      // ladder
    pha: [[f32; 12]; 2],     // phaser all-pass states
    phy: [f32; 2],           // phaser feedback
    frm: [[[f32; 2]; 3]; 2], // formant bandpasses
    cus: [[[f32; 2]; CBANDS]; 2], // custom points (TDF2)
    comb: [Vec<f32>; 2],
    cw: usize,
}

impl Fst {
    fn new(comb_len: usize) -> Self {
        Self { svf: [[[0.0; 2]; 2]; 2], op: [0.0; 2], lad: [[0.0; 4]; 2], pha: [[0.0; 12]; 2], phy: [0.0; 2], frm: [[[0.0; 2]; 3]; 2], cus: [[[0.0; 2]; CBANDS]; 2], comb: [vec![0.0; comb_len], vec![0.0; comb_len]], cw: 0 }
    }
    fn clear(&mut self) {
        self.svf = [[[0.0; 2]; 2]; 2];
        self.op = [0.0; 2];
        self.lad = [[0.0; 4]; 2];
        self.pha = [[0.0; 12]; 2];
        self.phy = [0.0; 2];
        self.frm = [[[0.0; 2]; 3]; 2];
        self.cus = [[[0.0; 2]; CBANDS]; 2];
        for c in self.comb.iter_mut() {
            c.iter_mut().for_each(|v| *v = 0.0);
        }
    }
}

#[derive(Clone)]
struct Voice {
    active: bool,
    key_down: bool,
    note: u8,
    vel: f32,
    order: u64,
    freq: f32,
    tfreq: f32,
    t: f32,
    env: [Env; NENV],
    ph: [[f32; MAX_UNI]; 3],
    sp: [[f32; MAX_UNI]; 3],
    grains: [[Grain; GN]; 3],
    gtimer: [f32; 3],
    sdir: [[f32; MAX_UNI]; 3],
    sph: [[f32; SH]; 3],
    msel: [Option<Arc<NSample>>; 3],
    mroot: [f32; 3],
    sub_ph: f32,
    rnd: u32,
    seed: u32,
    rnd_val: f32,
    prev: [f32; 3],
    fprev: [f32; 2],
    nz: [f32; 10],
    nph: f32,
    nplaying: bool,
    lfo_s: [f32; NLFO],
    last_lfo: [f32; NLFO],
    fl: [Fst; 2],
    /// de-click on retrigger: the old output level decays away while the new note fades in
    dc: [f32; 2],
    last: [f32; 2],
    fade: f32,
}

impl Voice {
    fn new(comb_len: usize) -> Self {
        Self {
            active: false,
            key_down: false,
            note: 0,
            vel: 0.0,
            order: 0,
            freq: 440.0,
            tfreq: 440.0,
            t: 0.0,
            env: [Env::default(); NENV],
            ph: [[0.0; MAX_UNI]; 3],
            sp: [[0.0; MAX_UNI]; 3],
            grains: [[Grain::default(); GN]; 3],
            gtimer: [0.0; 3],
            sdir: [[1.0; MAX_UNI]; 3],
            sph: [[0.0; SH]; 3],
            msel: [None, None, None],
            mroot: [0.0; 3],
            sub_ph: 0.0,
            rnd: 1,
            seed: 1,
            rnd_val: 0.0,
            prev: [0.0; 3],
            fprev: [0.0; 2],
            dc: [0.0; 2],
            last: [0.0; 2],
            fade: 1.0,
            nz: [0.0; 10],
            nph: 0.0,
            nplaying: false,
            lfo_s: [0.0; NLFO],
            last_lfo: [0.0; NLFO],
            fl: [Fst::new(comb_len), Fst::new(comb_len)],
        }
    }
}

struct Ctx {
    fs: f32,
    bend: f32,
    modwheel: f32,
    aftertouch: f32,
    glfo: [f32; NLFO],
    bpm: f32,
    glide_coef: f32,
    tune: f32,
    lane: f32,
}

fn midi_freq(p: &Patch, n: f32) -> f32 {
    let deg = ((n.round() as i32 - p.tun_root).rem_euclid(12)) as usize;
    p.a4 * 2f32.powf((n - 69.0) / 12.0 + p.tun[deg] / 1200.0)
}

/// Grains per voice and oscillator in Granular mode.
const GN: usize = 48;

#[derive(Clone, Copy, Default)]
struct Grain {
    pos: f32,
    age: f32,
    len: f32,
    inc: f32,
}

#[derive(Clone, Copy)]
struct OscRun {
    on: bool,
    mip: usize,
    pos: f32,
    f0: usize,
    ff: f32,
    wmode: WarpMode,
    warp: f32,
    wmode2: WarpMode,
    warp2: f32,
    smp: bool,
    grn: bool,
    spec: bool,
    kn: usize,
    sa: [f32; SH],
    sb: [f32; SH],
    sdk: [f32; SH],
    g_len: f32,
    g_rate: f32,
    g_spray: f32,
    g_pos: f32,
    g_norm: f32,
    s0: f32,
    s1: f32,
    sloop: SampleLoop,
    sxf: f32,
    n: usize,
    inc: [f32; MAX_UNI],
    gl: [f32; MAX_UNI],
    gr: [f32; MAX_UNI],
}

impl OscRun {
    fn off() -> Self {
        Self { on: false, mip: 0, pos: 0.0, f0: 0, ff: 0.0, wmode: WarpMode::Off, warp: 0.0, wmode2: WarpMode::Off, warp2: 0.0, smp: false, grn: false, spec: false, kn: 0, sa: [0.0; SH], sb: [0.0; SH], sdk: [0.0; SH], g_len: 1.0, g_rate: 0.0, g_spray: 0.0, g_pos: 0.0, g_norm: 1.0, s0: 0.0, s1: 1.0, sloop: SampleLoop::Loop, sxf: 0.0, n: 1, inc: [0.0; MAX_UNI], gl: [0.0; MAX_UNI], gr: [0.0; MAX_UNI] }
    }
}

/// Unison voice offset in -1..1 for voice `j` of `n`.
fn uni_offset(mode: UniMode, j: usize, n: usize, seed: u32) -> f32 {
    if n == 1 {
        return 0.0;
    }
    let lin = j as f32 / (n - 1) as f32 * 2.0 - 1.0;
    match mode {
        UniMode::Linear => lin,
        UniMode::Super => lin.signum() * lin.abs().powf(0.6),
        UniMode::Random => hash01(seed.wrapping_add(j as u32 * 7919)) * 2.0 - 1.0,
        UniMode::Stack => lin * 0.15,
    }
}

const STACK_ST: [f32; 16] = [0.0, 12.0, -12.0, 7.0, 19.0, 24.0, -24.0, 5.0, 17.0, 3.0, 15.0, 10.0, -5.0, 31.0, 28.0, -7.0];

/// Everything the filter needs for one control block.
struct FilRun {
    co: Fc,
    drive: f32,
    mix: f32,
    gd: f32,
    dcomp: f32,
}

struct Fc {
    ty: FilterType,
    k1: f32,
    k2: f32,
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
    kc: f32,
    g1: f32,
    gl: f32,
    kl: f32,
    gain: f32,
    comb_d: f32,
    comb_g: f32,
    pa: [f32; 12],
    pn: usize,
    pfb: f32,
    form: [([f32; 3], f32, f32); 3], // (a1,a2,a3), k, weight
    cn: usize,
    cc: [BqCoef; CBANDS],
}

fn svf_coefs(g: f32, k: f32) -> [f32; 3] {
    let a1 = 1.0 / (1.0 + g * (g + k));
    [a1, g * a1, g * g * a1]
}

fn filter_coefs(ty: FilterType, fc: f32, res: f32, fs: f32, comb_len: usize) -> Fc {
    let g = (PI * fc / fs).tan();
    let k1 = 0.05 + 1.95 * (1.0 - res).powi(2);
    let k2 = k1.max(0.9);
    let mut f = Fc {
        ty,
        k1,
        k2,
        a: svf_coefs(g, k1),
        b: svf_coefs(g, k2),
        c: [0.0; 3],
        kc: 1.2,
        g1: 1.0 - (-TAU * fc / fs).exp(),
        gl: 0.0,
        kl: 0.0,
        gain: 1.0,
        comb_d: 2.0,
        comb_g: 0.0,
        pa: [0.0; 12],
        pn: 6,
        pfb: 0.0,
        form: [([0.0; 3], 0.0, 0.0); 3],
        cn: 0,
        cc: [BqCoef::FLAT; CBANDS],
    };
    match ty {
        FilterType::Ladder => {
            f.gl = (1.0 - (-PI * fc / fs).exp()).min(0.45);
            f.kl = res * 3.95;
        }
        FilterType::Peak => {
            f.kc = 1.1;
            f.c = svf_coefs(g, f.kc);
            f.gain = 10f32.powf(res * 18.0 / 20.0);
        }
        FilterType::LowShelf | FilterType::HighShelf => {
            f.kc = 1.4;
            f.c = svf_coefs(g, f.kc);
            f.gain = 10f32.powf(res * 18.0 / 20.0);
        }
        FilterType::CombPlus | FilterType::CombMinus => {
            f.comb_d = (fs / fc.max(30.0)).clamp(2.0, (comb_len - 3) as f32);
            let sgn = if ty == FilterType::CombPlus { 1.0 } else { -1.0 };
            f.comb_g = sgn * res.min(0.97) * 0.97;
        }
        FilterType::PhaserF => {
            f.pn = 6;
            for i in 0..6 {
                let fi = (fc * 2f32.powf((i as f32 - 2.5) * 0.3)).clamp(20.0, 0.45 * fs);
                let t = (PI * fi / fs).tan();
                f.pa[i] = (t - 1.0) / (t + 1.0);
            }
            f.pfb = res * 0.9;
        }
        FilterType::Custom => {}
        FilterType::Formant => {
            const V: [[f32; 3]; 5] = [[800.0, 1200.0, 2500.0], [400.0, 2000.0, 2600.0], [300.0, 2300.0, 3000.0], [500.0, 900.0, 2500.0], [350.0, 600.0, 2400.0]];
            let t = ((fc / 20.0).ln() / 1000f32.ln()).clamp(0.0, 0.9999) * 4.0;
            let i = t as usize;
            let fr = t - i as f32;
            let kq = 0.06 + (1.0 - res) * 0.5;
            let w = [1.0, 0.6, 0.35];
            for j in 0..3 {
                let fj = (V[i][j] + (V[i + 1][j] - V[i][j]) * fr).min(0.45 * fs);
                f.form[j] = (svf_coefs((PI * fj / fs).tan(), kq), kq, w[j]);
            }
        }
        _ => {}
    }
    f
}

#[inline]
fn svf_tick(st: &mut [f32; 2], x: f32, c: &[f32; 3]) -> (f32, f32, f32) {
    // returns (v1 = band-ish, v2 = low)
    let v3 = x - st[1];
    let v1 = c[0] * st[0] + c[1] * v3;
    let v2 = st[1] + c[1] * st[0] + c[2] * v3;
    st[0] = 2.0 * v1 - st[0];
    st[1] = 2.0 * v2 - st[1];
    (v1, v2, 0.0)
}

#[inline]
fn run_filter(f: &Fc, st: &mut Fst, ch: usize, x: f32) -> f32 {
    match f.ty {
        FilterType::Lp12 | FilterType::Lp24 | FilterType::Hp12 | FilterType::Hp24 | FilterType::Band | FilterType::Notch => {
            let (band, low, _) = svf_tick(&mut st.svf[ch][0], x, &f.a);
            let high = x - f.k1 * band - low;
            let mut y = match f.ty {
                FilterType::Lp12 | FilterType::Lp24 => low,
                FilterType::Hp12 | FilterType::Hp24 => high,
                FilterType::Band => band * f.k1,
                _ => low + high,
            };
            if matches!(f.ty, FilterType::Lp24 | FilterType::Hp24) {
                let (w1, w2, _) = svf_tick(&mut st.svf[ch][1], y, &f.b);
                y = if f.ty == FilterType::Lp24 { w2 } else { y - f.k2 * w1 - w2 };
            }
            y
        }
        FilterType::Lp6 => {
            st.op[ch] += f.g1 * (x - st.op[ch]);
            st.op[ch]
        }
        FilterType::Hp6 => {
            st.op[ch] += f.g1 * (x - st.op[ch]);
            x - st.op[ch]
        }
        FilterType::Ladder => {
            let l = &mut st.lad[ch];
            let mut out = 0.0;
            for _ in 0..2 {
                let inp = (x * 1.2 - f.kl * l[3]).tanh();
                let mut u = inp;
                for s in 0..4 {
                    l[s] += f.gl * (u.tanh() - l[s].tanh());
                    u = l[s];
                }
                out = l[3];
            }
            out * (1.0 + f.kl * 0.3)
        }
        FilterType::Peak => {
            let (band, _, _) = svf_tick(&mut st.svf[ch][0], x, &f.c);
            x + (f.gain - 1.0) * f.kc * band
        }
        FilterType::LowShelf => {
            let (_, low, _) = svf_tick(&mut st.svf[ch][0], x, &f.c);
            x + (f.gain - 1.0) * low
        }
        FilterType::HighShelf => {
            let (band, low, _) = svf_tick(&mut st.svf[ch][0], x, &f.c);
            let high = x - f.kc * band - low;
            x + (f.gain - 1.0) * high
        }
        FilterType::CombPlus | FilterType::CombMinus => {
            let n = st.comb[ch].len();
            let rp = st.cw as f32 - f.comb_d;
            let rp = if rp < 0.0 { rp + n as f32 } else { rp };
            let i = rp as usize % n;
            let fr = rp - rp.floor();
            let d = st.comb[ch][i] + (st.comb[ch][(i + 1) % n] - st.comb[ch][i]) * fr;
            let y = x + f.comb_g * d;
            st.comb[ch][st.cw] = y;
            y / (1.0 + f.comb_g.abs() * 0.6)
        }
        FilterType::PhaserF => {
            let mut s = x + f.pfb * st.phy[ch];
            for i in 0..f.pn {
                let z = st.pha[ch][i];
                let v = s - f.pa[i] * z;
                let y = f.pa[i] * v + z;
                st.pha[ch][i] = v;
                s = y;
            }
            st.phy[ch] = s;
            (x + s) * 0.5
        }
        FilterType::Custom => {
            let mut y = x;
            for i in 0..f.cn {
                let c = &f.cc[i];
                let s = &mut st.cus[ch][i];
                let o = c.b0 * y + s[0];
                s[0] = c.b1 * y - c.a1 * o + s[1];
                s[1] = c.b2 * y - c.a2 * o;
                y = o;
            }
            y.clamp(-16.0, 16.0)
        }
        FilterType::Formant => {
            let mut y = 0.0;
            for j in 0..3 {
                let (c, k, w) = (&f.form[j].0, f.form[j].1, f.form[j].2);
                let (band, _, _) = svf_tick(&mut st.frm[ch][j], x, c);
                y += band * k * w * 3.0;
            }
            y
        }
    }
}

/// Bipolar sources are mapped to 0..1 when a slot is unipolar or used as scaler.
pub fn src_bipolar(s: ModSrc) -> bool {
    matches!(s, ModSrc::Lfo1 | ModSrc::Lfo2 | ModSrc::Lfo3 | ModSrc::Lfo4 | ModSrc::Lfo5 | ModSrc::Lfo6 | ModSrc::Lfo7 | ModSrc::Lfo8 | ModSrc::Random | ModSrc::Key | ModSrc::Bend | ModSrc::OscA | ModSrc::OscB | ModSrc::OscC | ModSrc::ClipLane | ModSrc::Filt1 | ModSrc::Filt2)
}

/// Value of one matrix slot (curve, polarity, amount and aux scaling applied).
#[inline]
pub fn slot_value(sl: &SlotP, src: impl Fn(ModSrc) -> f32) -> f32 {
    let mut x = src(sl.src);
    if sl.uni && src_bipolar(sl.src) {
        x = 0.5 + 0.5 * x;
    }
    if sl.curve != 0.0 {
        x = x.signum() * x.abs().powf(2f32.powf(sl.curve * 2.0));
    }
    let mut val = x * sl.amount;
    if sl.aux != ModSrc::None {
        let a = src(sl.aux);
        val *= if src_bipolar(sl.aux) { 0.5 + 0.5 * a } else { a };
    }
    val
}

/// 4-point Hermite interpolation of a sample buffer at fractional position `pos`.
#[inline]
fn hermite_read(d: &[f32], pos: f32) -> f32 {
    let n = d.len();
    let i = pos as usize;
    let f = pos - i as f32;
    let g = |k: isize| d[((i as isize + k).clamp(0, n as isize - 1)) as usize];
    let (ym, y0, y1, y2) = (g(-1), g(0), g(1), g(2));
    let c1 = 0.5 * (y1 - ym);
    let c2 = ym - 2.5 * y0 + 2.0 * y1 - 0.5 * y2;
    let c3 = 0.5 * (y2 - ym) + 1.5 * (y0 - y1);
    ((c3 * f + c2) * f + c1) * f + y0
}

fn render_voice(p: &Patch, v: &mut Voice, c: &Ctx, out_l: &mut [f32], out_r: &mut [f32]) {
    let m = out_l.len();
    let fs = c.fs;
    let dc_coef = (-1.0 / (0.004 * fs)).exp();
    let fade_inc = 1.0 / (0.003 * fs);
    // glide
    v.freq += (v.tfreq - v.freq) * c.glide_coef;
    v.t += m as f32 / fs;

    // ---- modulation sources
    let mut lfo = [0.0f32; NLFO];
    for i in 0..NLFO {
        let raw = if p.lfo[i].trig {
            let lp = &p.lfo[i];
            let rate = if lp.sync { c.bpm.max(20.0) / 60.0 / lp.div.beats() } else { lp.rate };
            if v.t < lp.delay_s {
                0.0
            } else {
                let t = v.t - lp.delay_s;
                let mut ph = t as f64 * rate as f64;
                if lp.oneshot {
                    ph = ph.min(0.9999);
                }
                let x = lfo_eval_c(lp.shape, ph + lp.phase0 as f64, v.seed, p.lfo_curve[i].as_deref());
                if lp.rise_s > 0.0 {
                    x * (t / lp.rise_s).min(1.0)
                } else {
                    x
                }
            }
        } else {
            c.glfo[i]
        };
        let sm = p.lfo[i].smooth;
        // free-running LFOs are already smoothed once in process_inner (c.glfo)
        lfo[i] = if sm > 0.001 && p.lfo[i].trig {
            let alpha = (1.0 - sm).powi(3) + 0.0008;
            v.lfo_s[i] += (raw - v.lfo_s[i]) * alpha.min(1.0);
            v.lfo_s[i]
        } else {
            v.lfo_s[i] = raw;
            raw
        };
    }
    v.last_lfo = lfo;
    let src = |s: ModSrc, v: &Voice| -> f32 {
        match s {
            ModSrc::None => 0.0,
            ModSrc::Env2 => v.env[1].out,
            ModSrc::Env3 => v.env[2].out,
            ModSrc::Lfo1 => lfo[0],
            ModSrc::Lfo2 => lfo[1],
            ModSrc::Lfo3 => lfo[2],
            ModSrc::Lfo4 => lfo[3],
            ModSrc::Lfo5 => lfo[4],
            ModSrc::Lfo6 => lfo[5],
            ModSrc::Lfo7 => lfo[6],
            ModSrc::Lfo8 => lfo[7],
            ModSrc::Env4 => v.env[3].out,
            ModSrc::Macro5 => p.macros[4],
            ModSrc::Macro6 => p.macros[5],
            ModSrc::Macro7 => p.macros[6],
            ModSrc::Macro8 => p.macros[7],
            ModSrc::Vel => v.vel,
            ModSrc::Key => ((v.note as f32 - 60.0) / 60.0).clamp(-1.0, 1.0),
            ModSrc::ModWheel => c.modwheel,
            ModSrc::Aftertouch => c.aftertouch,
            ModSrc::Random => v.rnd_val,
            ModSrc::Macro1 => p.macros[0],
            ModSrc::Macro2 => p.macros[1],
            ModSrc::Macro3 => p.macros[2],
            ModSrc::Macro4 => p.macros[3],
            ModSrc::Env1 => v.env[0].out,
            ModSrc::Bend => (c.bend / p.bend.max(1.0)).clamp(-1.0, 1.0),
            ModSrc::OscA => v.prev[0].clamp(-1.0, 1.0),
            ModSrc::OscB => v.prev[1].clamp(-1.0, 1.0),
            ModSrc::OscC => v.prev[2].clamp(-1.0, 1.0),
            ModSrc::ClipLane => c.lane,
            ModSrc::Filt1 => v.fprev[0].clamp(-1.0, 1.0),
            ModSrc::Filt2 => v.fprev[1].clamp(-1.0, 1.0),
        }
    };
    let mut md = [0.0f32; DSTS];
    for sl in p.slots.iter() {
        if sl.src != ModSrc::None && sl.dst != ModDst::None && sl.amount != 0.0 {
            md[sl.dst as usize] += slot_value(sl, |s| src(s, v));
        }
    }

    // ---- oscillators set-up
    let mut run = [OscRun::off(), OscRun::off(), OscRun::off()];
    for o in 0..3 {
        let op = &p.osc[o];
        if !op.on {
            continue;
        }
        let (pos_i, lvl_i, pit_i, wrp_i, pan_i, bl_i) = [(1, 3, 5, 7, 17, 19), (2, 4, 6, 8, 18, 20), (37, 38, 39, 40, 41, 42)][o];
        let tune_st = match op.tmode {
            crate::params::OscTune::Semitone => op.semi as f32,
            crate::params::OscTune::Harmonic => 12.0 * (op.harm.max(1) as f32).log2(),
            crate::params::OscTune::Ratio => 12.0 * op.ratio.max(0.01).log2(),
            crate::params::OscTune::Step => p.scale.step_offset(v.note, p.scale_root, op.semi) as f32,
        };
        let semis = (op.oct * 12) as f32 + tune_st + op.fine * 0.01 + md[pit_i] * 24.0 + c.bend;
        let base = v.freq * c.tune * 2f32.powf(semis / 12.0);
        let r = &mut run[o];
        r.on = true;
        let smp_len = if op.mode == OscMode::Multisample { v.msel[o].as_ref().map_or(0, |x| x.data.len()) } else if op.mode == OscMode::Sample || op.mode == OscMode::Granular { op.sample.as_ref().map_or(0, |x| x.data.len()) } else { 0 };
        if op.mode == OscMode::Granular && smp_len >= 16 {
            r.grn = true;
            let len = smp_len as f32;
            r.g_pos = (op.s_start + md[pos_i]).clamp(0.0, 1.0) * len;
            r.g_len = ((op.g_size + md[wrp_i] * 400.0).clamp(5.0, 500.0) * 0.001 * fs).max(32.0);
            let dens = (op.g_dens * (1.0 + md[[23, 24, 43][o]] * 4.0)).clamp(1.0, 256.0);
            r.g_rate = dens / fs;
            r.g_spray = op.g_spray;
            let overlap = (dens * r.g_len / fs).max(1.0);
            r.g_norm = 1.0 / overlap.sqrt();
            r.n = 1;
        } else if op.mode == OscMode::Sample || op.mode == OscMode::Multisample {
            if smp_len < 16 {
                r.on = false;
                continue;
            }
            r.smp = true;
            let len = smp_len as f32;
            let a = (op.s_start + md[pos_i]).clamp(0.0, 1.0) * len;
            let b = (op.s_end + md[wrp_i]).clamp(0.0, 1.0) * len;
            r.s0 = a.min(len - 9.0);
            r.s1 = b.max(r.s0 + 8.0).min(len - 1.0);
            r.sloop = op.s_loop;
            r.sxf = (op.s_xfade + md[[23, 24, 43][o]]).clamp(0.0, 1.0) * 0.5 * (r.s1 - r.s0);
        }
        r.pos = (op.pos + md[pos_i]).clamp(0.0, 1.0);
        let (f0, ff) = op.tbl.frames_at(r.pos);
        r.f0 = f0;
        r.ff = ff;
        r.wmode = op.wmode;
        r.warp = (op.warp + md[wrp_i]).clamp(0.0, 1.0);
        r.wmode2 = op.wmode2;
        r.warp2 = (op.warp2 + md[[23, 24, 43][o]]).clamp(0.0, 1.0);
        let spectral = op.mode == OscMode::Spectral;
        let n = if spectral || r.grn { 1 } else { op.uni };
        r.n = n;
        let det = (op.detune + md[12]).clamp(0.0, 1.0);
        let cents_max = det.powf(1.5) * 100.0;
        let level = (op.level + md[lvl_i]).clamp(0.0, 1.0);
        let pan = (op.pan + md[13] + md[pan_i]).clamp(-1.0, 1.0);
        let blend = (op.blend + md[bl_i]).clamp(0.0, 1.0);
        let (pl, pr) = ((1.0 - pan.max(0.0)), (1.0 + pan.min(0.0)));
        let mut wsum = 0.0;
        let mut ws = [0.0f32; MAX_UNI];
        let mut fmax = base;
        for j in 0..n {
            let off = uni_offset(op.umode, j, n, v.seed);
            let mut f = base * 2f32.powf(off * cents_max / 1200.0);
            if op.umode == UniMode::Stack {
                f = base * 2f32.powf((STACK_ST[j % 16] + off * cents_max / 100.0) / 12.0);
            }
            fmax = fmax.max(f);
            r.inc[j] = if r.smp || r.grn { (f / 261.6256 * 2f32.powf(-(op.s_root + v.mroot[o]) / 12.0) * crate::sample::SAMPLE_RATE as f32 / fs).clamp(0.0, 16.0) } else { (f / fs).clamp(0.0, 0.49) };
            let lin = if n == 1 { 0.0 } else { j as f32 / (n - 1) as f32 * 2.0 - 1.0 };
            let w = 1.0 - (1.0 - blend) * 0.75 * lin.abs();
            ws[j] = w;
            wsum += w * w;
            let ang = (1.0 + lin * op.spread) * 0.25 * PI;
            r.gl[j] = ang.cos() * std::f32::consts::SQRT_2 * pl;
            r.gr[j] = ang.sin() * std::f32::consts::SQRT_2 * pr;
        }
        let norm = level / wsum.sqrt().max(1e-3);
        // wide unison keeps its average level; one voice stays at unity
        for j in 0..n {
            let g = ws[j] * norm;
            r.gl[j] *= g;
            r.gr[j] *= g;
        }
        if spectral {
            r.spec = true;
            let (mut a, mut b) = ([0.0f32; SH], [0.0f32; SH]);
            op.tbl.spectral_at(r.f0, r.ff, &mut a, &mut b);
            let inc0 = r.inc[0];
            let tilt = op.sp_tilt.clamp(-1.0, 1.0);
            let shift = op.sp_shift.clamp(0.0, 1.0);
            let eo = op.sp_eo.clamp(-1.0, 1.0);
            let (mut p0, mut p1) = (0.0f32, 0.0f32);
            let mut kn = 0usize;
            for k in 0..SH {
                let kf = (k + 1) as f32;
                let keff = kf * (1.0 + shift * 0.03 * (kf - 1.0));
                let dk = inc0 * keff;
                p0 += a[k] * a[k] + b[k] * b[k];
                if dk >= 0.45 {
                    r.sa[k] = 0.0;
                    r.sb[k] = 0.0;
                    r.sdk[k] = dk;
                    continue;
                }
                let odd = k % 2 == 0;
                let g = kf.powf(tilt * 1.2) * if odd { 1.0 + eo.min(0.0) } else { 1.0 - eo.max(0.0) };
                let g = g.max(0.0);
                r.sa[k] = a[k] * g;
                r.sb[k] = b[k] * g;
                r.sdk[k] = dk;
                p1 += r.sa[k] * r.sa[k] + r.sb[k] * r.sb[k];
                kn = k + 1;
            }
            r.kn = kn;
            // keep the loudness when the spectrum is reshaped
            let gain = if p1 > 1e-12 { (p0 / p1).sqrt().clamp(0.0, 8.0) } else { 0.0 };
            for k in 0..kn {
                r.sa[k] *= gain;
                r.sb[k] *= gain;
            }
        }
        if !r.smp && !r.grn && !r.spec {
            r.mip = op.tbl.mip_for(fmax * warp_bandwidth(r.wmode, r.warp) * warp_bandwidth(r.wmode2, r.warp2), fs);
        }
    }

    // ---- sub / noise / filter set-up
    let sub_level = (p.sub_level + md[15]).clamp(0.0, 1.0);
    let noise_level = (p.noise + md[16]).clamp(0.0, 1.0);
    let sub_inc = v.freq * c.tune * 2f32.powf((p.sub_oct * 12) as f32 / 12.0 + c.bend / 12.0) / fs;
    let sub_inc = sub_inc.clamp(0.0, 0.49);
    let comb_len = v.fl[0].comb[0].len();
    let mut fcs: [Option<FilRun>; 2] = [None, None];
    for fi in 0..2 {
        let fp = &p.fl[fi];
        if !fp.on {
            continue;
        }
        let (i_cut, i_res, i_drv, i_mix) = if fi == 0 { (9, 10, 11, 22) } else { (44, 45, 46, 47) };
        let drive = (fp.drive + md[i_drv]).clamp(0.0, 1.0);
        let res = (fp.res + md[i_res]).clamp(0.0, 1.0);
        let mix = (fp.mix + md[i_mix]).clamp(0.0, 1.0);
        let shift = 2f32.powf(md[i_cut] * 6.0 + fp.keytrack * (v.note as f32 - 60.0) / 12.0);
        let fc = (fp.cutoff * shift).clamp(20.0, (0.45 * fs).min(20000.0));
        let mut co = filter_coefs(fp.ty, fc, res, fs, comb_len);
        if fp.ty == FilterType::Custom {
            let (i_pg, i_pq) = if fi == 0 { (60, 61) } else { (62, 63) };
            let (dg, qm) = (md[i_pg] * 24.0, 2f32.powf(md[i_pq] * 3.0));
            for b in fp.cb.iter().filter(|b| b.on) {
                co.cc[co.cn] = cband_coef_m(b, shift, dg, qm, fs);
                co.cn += 1;
            }
        }
        let gd = 1.0 + drive * 8.0;
        fcs[fi] = Some(FilRun { co, drive, mix, gd, dcomp: gd.sqrt() });
    }
    let vol = (1.0 + md[14]).clamp(0.0, 2.0);
    let vg = (0.25 + 0.75 * v.vel) * vol;
    let sub_pan = (p.sub_pan + md[13]).clamp(-1.0, 1.0);
    let (sub_l, sub_r) = (1.0 - sub_pan.max(0.0), 1.0 + sub_pan.min(0.0));
    // noise sample playback rate
    let nz_rate = if p.noise_type == NoiseType::Sample {
        let st = p.noise_pitch + md[21] * 24.0 + if p.noise_key { v.note as f32 - 60.0 } else { 0.0 } + c.bend;
        2f32.powf(st / 12.0) * crate::sample::SAMPLE_RATE as f32 / fs
    } else {
        0.0
    };

    // spectral oscillators: one rotating phasor per partial, re-seeded from exact phases every block
    let mut rc = [[0.0f32; SH]; 3];
    let mut rs_ = [[0.0f32; SH]; 3];
    let mut cc = [[0.0f32; SH]; 3];
    let mut cs = [[0.0f32; SH]; 3];
    for o in 0..3 {
        let r = &run[o];
        if r.on && r.spec {
            for k in 0..r.kn {
                let (sn, cn) = (std::f32::consts::TAU * v.sph[o][k]).sin_cos();
                cs[o][k] = sn;
                cc[o][k] = cn;
                let (sd, cd) = (std::f32::consts::TAU * r.sdk[k]).sin_cos();
                rs_[o][k] = sd;
                rc[o][k] = cd;
                v.sph[o][k] = (v.sph[o][k] + m as f32 * r.sdk[k]).fract();
            }
        }
    }
    for s in 0..m {
        // oscillators
        let mut sig = [[0.0f32; 2]; 3]; // [osc][ch]
        for o in 0..3 {
            let r = &run[o];
            if !r.on {
                continue;
            }
            if r.spec {
                let mut x = 0.0f32;
                for k in 0..r.kn {
                    let (c, sn) = (cc[o][k], cs[o][k]);
                    x += r.sa[k] * c + r.sb[k] * sn;
                    cc[o][k] = c * rc[o][k] - sn * rs_[o][k];
                    cs[o][k] = sn * rc[o][k] + c * rs_[o][k];
                }
                v.ph[o][0] = (v.ph[o][0] + r.inc[0]).fract();
                sig[o] = [x * r.gl[0], x * r.gr[0]];
                continue;
            }
            if r.grn {
                let hold = v.msel[o].clone();
                let data = match &hold {
                    Some(a) if p.osc[o].mode == OscMode::Multisample => &a.data,
                    _ => &p.osc[o].sample.as_ref().unwrap().data,
                };
                let n = data.len() as f32;
                v.gtimer[o] += r.g_rate;
                while v.gtimer[o] >= 1.0 {
                    v.gtimer[o] -= 1.0;
                    v.rnd ^= v.rnd << 13;
                    v.rnd ^= v.rnd >> 17;
                    v.rnd ^= v.rnd << 5;
                    let rn = (v.rnd as f32 / u32::MAX as f32) * 2.0 - 1.0;
                    if let Some(g) = v.grains[o].iter_mut().find(|g| g.len <= 0.0) {
                        g.pos = (r.g_pos + rn * r.g_spray * 0.5 * n).rem_euclid(n - 2.0);
                        g.age = 0.0;
                        g.len = r.g_len;
                        g.inc = r.inc[0];
                    }
                }
                let mut acc = 0.0f32;
                for g in v.grains[o].iter_mut() {
                    if g.len <= 0.0 {
                        continue;
                    }
                    let ph = g.age / g.len;
                    let win = 0.5 - 0.5 * (std::f32::consts::TAU * ph).cos();
                    let i = g.pos as usize;
                    let f = g.pos - i as f32;
                    let a = data[i.min(data.len() - 1)];
                    let b = data[(i + 1).min(data.len() - 1)];
                    acc += (a + (b - a) * f) * win;
                    g.pos += g.inc;
                    if g.pos >= n - 1.0 {
                        g.pos -= n - 2.0;
                    }
                    g.age += 1.0;
                    if g.age >= g.len {
                        g.len = 0.0;
                    }
                }
                let x = acc * r.g_norm;
                sig[o] = [x * r.gl[0], x * r.gr[0]];
                continue;
            }
            if r.smp {
                let hold = v.msel[o].clone();
                let data = match &hold {
                    Some(a) if p.osc[o].mode == OscMode::Multisample => &a.data,
                    _ => &p.osc[o].sample.as_ref().unwrap().data,
                };
                let (mut l, mut rr) = (0.0, 0.0);
                for j in 0..r.n {
                    let mut pos = v.sp[o][j];
                    let mut dir = v.sdir[o][j];
                    pos += r.inc[j] * dir;
                    match r.sloop {
                        SampleLoop::Off => {
                            if pos >= r.s1 {
                                pos = r.s1;
                                dir = 0.0;
                            }
                        }
                        SampleLoop::Loop => {
                            if pos >= r.s1 {
                                pos = r.s0 + (pos - r.s1) % (r.s1 - r.s0);
                            }
                        }
                        SampleLoop::PingPong => {
                            if pos >= r.s1 {
                                pos = (2.0 * r.s1 - pos).max(r.s0);
                                dir = -1.0;
                            } else if pos <= r.s0 && dir < 0.0 {
                                pos = (2.0 * r.s0 - pos).min(r.s1);
                                dir = 1.0;
                            }
                        }
                    }
                    v.sp[o][j] = pos;
                    v.sdir[o][j] = dir;
                    let mut smp = if dir == 0.0 { 0.0 } else { hermite_read(data, pos) };
                    if r.sloop == SampleLoop::Loop && r.sxf > 1.0 && dir != 0.0 {
                        let xs = r.s1 - r.sxf;
                        if pos > xs {
                            let g = ((pos - xs) / r.sxf).clamp(0.0, 1.0);
                            smp = smp * (1.0 - g).sqrt() + hermite_read(data, r.s0 + (pos - xs)) * g.sqrt();
                        }
                    }
                    l += smp * r.gl[j];
                    rr += smp * r.gr[j];
                }
                sig[o] = [l, rr];
                continue;
            }
            let t = &p.osc[o].tbl;
            let fm = v.prev[[1, 0, 1][o]].clamp(-1.0, 1.0);
            let (mut l, mut rr) = (0.0, 0.0);
            for j in 0..r.n {
                let ph = &mut v.ph[o][j];
                *ph += r.inc[j];
                if *ph >= 1.0 {
                    *ph -= 1.0;
                }
                let mut w = *ph;
                let own = v.prev[o].clamp(-1.0, 1.0);
                if r.wmode != WarpMode::Off {
                    w = warp_phase(r.wmode, r.warp, w, if r.wmode == WarpMode::SelfFm { own } else { fm });
                }
                if r.wmode2 != WarpMode::Off {
                    w = warp_phase(r.wmode2, r.warp2, w, if r.wmode2 == WarpMode::SelfFm { own } else { fm });
                }
                let mut smp = t.sample_fr(r.mip, r.f0, r.ff, w);
                if r.wmode != WarpMode::Off {
                    smp = amp_warp(r.wmode, r.warp, smp, *ph, fm, own);
                }
                if r.wmode2 != WarpMode::Off {
                    smp = amp_warp(r.wmode2, r.warp2, smp, *ph, fm, own);
                }
                l += smp * r.gl[j];
                rr += smp * r.gr[j];
            }
            sig[o] = [l, rr];
        }
        v.prev = [(sig[0][0] + sig[0][1]) * 0.5, (sig[1][0] + sig[1][1]) * 0.5, (sig[2][0] + sig[2][1]) * 0.5];

        // sub + noise
        let mut sub = [0.0f32; 2];
        if p.sub_on && sub_level > 0.0 {
            v.sub_ph += sub_inc;
            if v.sub_ph >= 1.0 {
                v.sub_ph -= 1.0;
            }
            let x = sub_wave(p.sub_wave, v.sub_ph, sub_inc.min(0.45)) * sub_level * 0.7;
            sub = [x * sub_l, x * sub_r];
        }
        let mut noise = [0.0f32; 2];
        if noise_level > 0.0 {
            v.rnd ^= v.rnd << 13;
            v.rnd ^= v.rnd >> 17;
            v.rnd ^= v.rnd << 5;
            let w = (v.rnd as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let x = match p.noise_type {
                NoiseType::White => w,
                NoiseType::Pink => {
                    v.nz[0] = 0.99886 * v.nz[0] + w * 0.055_517_9;
                    v.nz[1] = 0.99332 * v.nz[1] + w * 0.075_075_9;
                    v.nz[2] = 0.96900 * v.nz[2] + w * 0.153_852;
                    (v.nz[0] + v.nz[1] + v.nz[2] + w * 0.1848) * 0.45
                }
                NoiseType::Brown => {
                    v.nz[3] = (v.nz[3] + 0.02 * w) / 1.02;
                    v.nz[3] * 6.0
                }
                NoiseType::Hiss => {
                    v.nz[4] += 0.35 * (w - v.nz[4]);
                    (w - v.nz[4]) * 0.9
                }
                NoiseType::Crackle => {
                    if (w.abs() * 0.5 + 0.5) > 0.9997 {
                        v.nz[5] = w.signum() * (0.4 + 0.6 * w.abs());
                    } else {
                        v.nz[5] *= 0.55;
                    }
                    v.nz[5] * 1.6
                }
                NoiseType::Blue => {
                    let d = w - v.nz[6];
                    v.nz[6] = w;
                    d * 0.7
                }
                NoiseType::Violet => {
                    let d1 = w - v.nz[6];
                    v.nz[6] = w;
                    let d2 = d1 - v.nz[7];
                    v.nz[7] = d1;
                    d2 * 0.4
                }
                NoiseType::Dust => {
                    if (w.abs() * 0.5 + 0.5) > 0.9985 {
                        w.signum() * (0.3 + 0.7 * hash01(v.rnd >> 3))
                    } else {
                        0.0
                    }
                }
                NoiseType::Vinyl => {
                    if (w.abs() * 0.5 + 0.5) > 0.9992 {
                        v.nz[5] = w.signum() * (0.3 + 0.7 * w.abs());
                    } else {
                        v.nz[5] *= 0.6;
                    }
                    v.nz[3] = (v.nz[3] + 0.02 * w) / 1.02;
                    v.nz[4] += 0.35 * (w - v.nz[4]);
                    v.nz[5] * 1.2 + v.nz[3] * 1.4 + (w - v.nz[4]) * 0.08
                }
                NoiseType::Tape => {
                    v.nz[4] += 0.5 * (w - v.nz[4]); // low-pass ~ 8 kHz
                    v.nz[8] += 0.08 * (v.nz[4] - v.nz[8]); // slow part, subtracted -> band-pass
                    (v.nz[4] - v.nz[8]) * 1.1
                }
                NoiseType::Wind => {
                    // slow random gusts through a low-pass
                    if v.rnd & 0xFFF == 0 {
                        v.nz[9] = hash01(v.rnd >> 5);
                    }
                    // state may be left over from another noise type after a live switch: keep it in range
                    v.nz[9] = v.nz[9].clamp(0.0, 1.0);
                    v.nz[8] = (v.nz[8] + 0.0004 * (v.nz[9] - v.nz[8])).clamp(0.0, 1.0);
                    v.nz[3] = ((v.nz[3] + 0.02 * w) / 1.02).clamp(-4.0, 4.0);
                    v.nz[4] += (0.01 + 0.08 * v.nz[8]) * (v.nz[3] * 6.0 - v.nz[4]);
                    v.nz[4] = v.nz[4].clamp(-30.0, 30.0);
                    v.nz[4] * (0.5 + 1.5 * v.nz[8])
                }
                NoiseType::Digital => {
                    v.nz[6] += 1.0;
                    if v.nz[6] >= 6.0 {
                        v.nz[6] = 0.0;
                        v.nz[7] = w;
                    }
                    v.nz[7]
                }
                NoiseType::Sample => {
                    if let Some(sm) = p.sample.as_ref().filter(|_| v.nplaying) {
                        let len = sm.data.len() as f32;
                        let i = v.nph as usize;
                        let fr = v.nph - i as f32;
                        let a = sm.data[i.min(sm.data.len() - 1)];
                        let b = sm.data[(i + 1).min(sm.data.len() - 1)];
                        let out = a + (b - a) * fr;
                        v.nph += nz_rate;
                        if v.nph >= len {
                            if p.noise_loop {
                                v.nph -= len;
                            } else {
                                v.nplaying = false;
                            }
                        }
                        out * 1.4
                    } else {
                        0.0
                    }
                }
            };
            let nz = x * noise_level * 0.5;
            let pan = (p.noise_pan + md[13]).clamp(-1.0, 1.0);
            noise = [nz * (1.0 - pan.max(0.0)), nz * (1.0 + pan.min(0.0))];
        }

        // routing: every source goes to filter 1, filter 2 or straight to the output
        let mut ro = [[0.0f32; 2]; 2];
        let mut dr = [0.0f32; 2];
        for ch in 0..2 {
            let srcs = [sig[0][ch], sig[1][ch], sig[2][ch], sub[ch], noise[ch]];
            for (i, x) in srcs.iter().enumerate() {
                let to1 = fcs[0].is_some() && p.fl[0].route[i];
                let to2 = fcs[1].is_some() && p.fl[1].route[i] && (!p.fl_serial || fcs[0].is_none());
                let g = if to1 && to2 { 0.5 } else { 1.0 }; // balanced split between the two filters
                if to1 {
                    ro[0][ch] += *x * g;
                }
                if to2 {
                    ro[1][ch] += *x * g;
                }
                if !to1 && !to2 {
                    dr[ch] += *x;
                }
            }
        }

        // filters (parallel, or filter 1 feeding filter 2)
        let mut f = [0.0f32; 2];
        for fi in 0..2 {
            let Some(fr) = &fcs[fi] else { continue };
            let mut facc = 0.0f32;
            for ch in 0..2 {
                let x0 = if fi == 1 && p.fl_serial && fcs[0].is_some() { f[ch] } else { ro[fi][ch] };
                let x = x0 * (1.0 - fr.drive) + fr.drive * (fr.gd * x0).tanh() / fr.dcomp;
                let mut y = run_filter(&fr.co, &mut v.fl[fi], ch, x);
                if !y.is_finite() {
                    v.fl[fi].clear();
                    y = 0.0;
                }
                let out = y * fr.mix + x0 * (1.0 - fr.mix);
                facc += out * 0.5;
                if fi == 1 && p.fl_serial && fcs[0].is_some() {
                    f[ch] = out;
                } else {
                    f[ch] += out;
                }
            }
            v.fprev[fi] = facc;
            v.fl[fi].cw += 1;
            if v.fl[fi].cw >= comb_len {
                v.fl[fi].cw = 0;
            }
        }

        let amp = v.env[0].tick(&p.env[0]);
        v.env[1].tick(&p.env[1]);
        v.env[2].tick(&p.env[2]);
        v.env[3].tick(&p.env[3]);
        let ga = amp * vg;
        if v.fade < 1.0 {
            v.fade = (v.fade + fade_inc).min(1.0);
        }
        let fg = v.fade * v.fade * (3.0 - 2.0 * v.fade);
        let ol = (f[0] + dr[0]) * ga * fg + v.dc[0];
        let or = (f[1] + dr[1]) * ga * fg + v.dc[1];
        v.dc[0] *= dc_coef;
        v.dc[1] *= dc_coef;
        v.last = [ol, or];
        out_l[s] += ol;
        out_r[s] += or;
    }
    if v.env[0].stage == 0 {
        v.active = false;
    }
}

// ---------------------------------------------------------------- engine

pub struct Engine {
    fs: f32,
    comb_len: usize,
    voices: Vec<Voice>,
    pub patch: Patch,
    glfo_ph: [f64; NLFO],
    glfo_s: [f32; NLFO],
    glfo_out: [f32; NLFO],
    bend_norm: f32,
    modwheel: f32,
    aftertouch: f32,
    sustain: bool,
    sustained: Vec<u8>,
    /// keys that are physically held right now (independent of voice mode / arp)
    down: [bool; 128],
    stack: Vec<u8>,
    last_freq: f32,
    last_note: u8,
    last_vel: f32,
    pub bpm: f32,
    /// host transport position in beats at the current event (if the host tells)
    pub host_beat: Option<f64>,
    /// 1-based step the clip sequencer is on (0 = idle), for the play-head
    pub clip_pos: usize,
    clip_lane_t: f32,
    clip_lane_s: f32,
    /// MIDI out: arp/clip notes as (offset in the current segment, on, note, velocity)
    pub midi_out: bool,
    dc_x: [f32; 2],
    dc_y: [f32; 2],
    pub out_ev: Vec<(u32, bool, u8, f32)>,
    pub(crate) seg_off: usize,
    counter: u64,
    rng: u32,
    arp: Arp,
    clip_held: Vec<(u8, f32)>,
    clip_timer: f64,
    clip_step: usize,
    clip_act: Vec<(u8, f64)>,
}

impl Engine {
    pub fn new(fs: f32, patch: Patch) -> Self {
        let comb_len = (fs / 20.0) as usize + 8;
        Self {
            fs,
            comb_len,
            voices: vec![Voice::new(comb_len); VOICES],
            patch,
            glfo_ph: [0.0; NLFO],
            glfo_s: [0.0; NLFO],
            glfo_out: [0.0; NLFO],
            bend_norm: 0.0,
            modwheel: 0.0,
            aftertouch: 0.0,
            sustain: false,
            sustained: Vec::with_capacity(256),
            down: [false; 128],
            stack: Vec::with_capacity(256),
            last_freq: 0.0,
            last_note: 60,
            last_vel: 0.8,
            bpm: 120.0,
            host_beat: None,
            clip_pos: 0,
            clip_lane_t: 0.0,
            midi_out: false,
            dc_x: [0.0; 2],
            dc_y: [0.0; 2],
            out_ev: Vec::with_capacity(512),
            seg_off: 0,
            clip_lane_s: 0.0,
            counter: 0,
            rng: 0x1234_5678,
            arp: Arp::new(),
            clip_held: Vec::with_capacity(64),
            clip_timer: f64::INFINITY,
            clip_step: 0,
            clip_act: Vec::with_capacity(64),
        }
    }

    pub fn set_fs(&mut self, fs: f32) {
        self.fs = fs;
    }

    pub fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
            v.key_down = false;
            v.env = [Env::default(); NENV];
            v.fl[0].clear();
            v.fl[1].clear();
        }
        self.stack.clear();
        self.sustained.clear();
        self.sustain = false;
        self.clip_stop();
        self.arp_stop();
        self.down = [false; 128];
        self.out_ev.clear();
    }

    /// `norm` 0..1 with 0.5 as centre (as delivered by the host).
    /// Current pitch-wheel (-1..1) and mod-wheel (0..1) positions, for the GUI wheels.
    pub fn wheels(&self) -> (f32, f32) {
        (self.bend_norm, self.modwheel)
    }
    pub fn set_bend(&mut self, norm: f32) {
        self.bend_norm = (norm * 2.0 - 1.0).clamp(-1.0, 1.0);
    }
    pub fn set_modwheel(&mut self, v: f32) {
        self.modwheel = v;
    }
    pub fn set_aftertouch(&mut self, v: f32) {
        self.aftertouch = v;
    }

    /// Sustain pedal (CC 64).
    pub fn set_sustain(&mut self, on: bool) {
        if on == self.sustain {
            return;
        }
        self.sustain = on;
        if !on {
            let mut list = std::mem::take(&mut self.sustained);
            for n in list.drain(..) {
                self.real_note_off(n);
            }
            self.sustained = list;
        }
    }

    pub fn reset_controllers(&mut self) {
        self.bend_norm = 0.0;
        self.modwheel = 0.0;
        self.aftertouch = 0.0;
        self.set_sustain(false);
    }

    /// CC 120: silence everything immediately.
    pub fn sound_off(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
            v.key_down = false;
            v.env = [Env::default(); NENV];
        }
        self.stack.clear();
        self.sustained.clear();
        self.clip_stop();
        self.arp_stop();
        self.down = [false; 128];
    }

    fn rand01(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as f32 / u32::MAX as f32
    }

    fn trigger(&mut self, idx: usize, note: u8, vel: f32) {
        let tfreq = midi_freq(&self.patch, note as f32);
        let glide = self.patch.glide_ms > 0.6 && self.last_freq > 0.0;
        let start = if glide { self.last_freq } else { tfreq };
        let r1 = self.rand01();
        let r2 = self.rand01();
        let r3 = self.rand01();
        let mut phases = [0.0f32; 3 * MAX_UNI];
        for p in phases.iter_mut() {
            *p = self.rand01();
        }
        let order = {
            self.counter += 1;
            self.counter
        };
        let patch = &self.patch;
        let v = &mut self.voices[idx];
        let was_sounding = v.active;
        if was_sounding {
            v.dc = v.last;
            v.fade = 0.0;
        } else {
            v.dc = [0.0; 2];
            v.fade = 1.0;
        }
        v.active = true;
        v.key_down = true;
        v.note = note;
        v.vel = vel;
        v.order = order;
        v.tfreq = tfreq;
        v.freq = start;
        v.t = 0.0;
        v.rnd = ((r1 * 4_000_000_000.0) as u32) | 1;
        v.seed = v.rnd;
        v.rnd_val = r2 * 2.0 - 1.0;
        for o in 0..3 {
            // multisample: choose the recording for this key
            match (&patch.osc[o].multi, patch.osc[o].mode) {
                (Some(m), OscMode::Multisample) => {
                    if let Some(rg) = m.pick(note, (vel * 127.0).round() as u8) {
                        v.msel[o] = Some(rg.smp.clone());
                        v.mroot[o] = rg.root as f32 - 60.0;
                    } else {
                        v.msel[o] = None;
                        v.mroot[o] = 0.0;
                    }
                }
                _ => {
                    v.msel[o] = None;
                    v.mroot[o] = 0.0;
                }
            }
            for j in 0..MAX_UNI {
                v.ph[o][j] = (patch.osc[o].phase + patch.osc[o].rand * phases[o * MAX_UNI + j]).fract();
                v.sdir[o][j] = 1.0;
                if j == 0 {
                    for k in 0..SH {
                        v.sph[o][k] = ((k + 1) as f32 * v.ph[o][0]).fract();
                    }
                }
                v.sp[o][j] = match (&patch.osc[o].sample, patch.osc[o].mode) {
                    (Some(sm), OscMode::Sample) => sm.data.len() as f32 * patch.osc[o].s_start,
                    _ => match (&v.msel[o], patch.osc[o].mode) {
                        (Some(sm), OscMode::Multisample) => sm.data.len() as f32 * patch.osc[o].s_start,
                        _ => 0.0,
                    },
                };
            }
        }
        v.sub_ph = 0.0;
        v.grains = [[Grain::default(); GN]; 3];
        v.gtimer = [0.0; 3];
        for (i, e) in v.env.iter_mut().enumerate() {
            if !was_sounding {
                *e = Env::default();
            }
            e.start(&patch.env[i]);
        }
        if !was_sounding {
            v.fl[0].clear();
            v.fl[1].clear();
        }
        v.nz = [0.0; 10];
        v.lfo_s = [0.0; NLFO];
        if let Some(sm) = patch.sample.as_ref() {
            v.nph = ((patch.noise_phase + patch.noise_rand * r3).fract() * sm.data.len() as f32).min(sm.data.len() as f32 - 1.0);
            v.nplaying = true;
        } else {
            v.nplaying = false;
        }
        self.last_freq = tfreq;
        self.last_note = note;
        self.last_vel = vel;
    }

    // ---- note routing: sustain pedal -> arpeggiator -> voices

    pub fn note_on(&mut self, note: u8, vel: f32) {
        let vel = vel.clamp(0.0, 1.0).powf(2f32.powf(-self.patch.vel_curve * 1.5));
        if (note as usize) < 128 {
            self.down[note as usize] = true;
        }
        if let Some(i) = self.sustained.iter().position(|n| *n == note) {
            // re-pressed while the pedal held it: let the old one go first
            self.sustained.swap_remove(i);
            self.real_note_off(note);
        }
        if self.patch.clip.on {
            self.clip_held.retain(|(n, _)| *n != note);
            self.clip_held.push((note, vel));
            if self.clip_timer.is_infinite() {
                let c = self.patch.clip;
                match self.host_beat {
                    Some(hb) if c.sync && hb.is_finite() => {
                        // start on the host's step grid, so the clip lines up with the song
                        let cb = (c.beats as f64).max(1e-3);
                        let k = (hb / cb - 1e-6).ceil().max(0.0);
                        self.clip_timer = (k * cb - hb).max(0.0) * 60.0 / (self.bpm.max(20.0) as f64) * self.fs as f64;
                        self.clip_step = k as usize;
                    }
                    _ => {
                        self.clip_timer = 0.0;
                        self.clip_step = 0;
                    }
                }
            }
        } else if self.patch.arp.on {
            let hold = self.patch.arp.hold;
            self.arp.press(note, vel, hold);
        } else {
            self.voice_note_on(note, vel);
        }
    }

    pub fn note_off(&mut self, note: u8) {
        if (note as usize) < 128 {
            self.down[note as usize] = false;
        }
        if self.sustain {
            if !self.sustained.contains(&note) && self.sustained.len() < 250 {
                self.sustained.push(note);
            }
            return;
        }
        self.real_note_off(note);
    }

    fn real_note_off(&mut self, note: u8) {
        if self.patch.clip.on || !self.clip_held.is_empty() {
            self.clip_held.retain(|(n, _)| *n != note);
            if self.clip_held.is_empty() {
                self.clip_stop();
            }
            return;
        }
        if self.patch.arp.on {
            let hold = self.patch.arp.hold;
            self.arp.release(note, hold);
            if !self.arp.playing.contains(&note) {
                self.voice_note_off(note);
            }
        } else {
            self.voice_note_off(note);
        }
    }

    /// Note produced by the arpeggiator / clip sequencer: also reported for MIDI out.
    fn gen_on(&mut self, note: u8, vel: f32) {
        if self.midi_out && self.out_ev.len() < 512 {
            self.out_ev.push((self.seg_off as u32, true, note, vel));
        }
        self.voice_note_on(note, vel);
    }

    fn gen_off(&mut self, note: u8) {
        if self.midi_out && self.out_ev.len() < 512 {
            self.out_ev.push((self.seg_off as u32, false, note, 0.0));
        }
        self.voice_note_off(note);
    }

    fn voice_note_on(&mut self, note: u8, vel: f32) {
        if self.patch.vmode != VoiceMode::Poly {
            self.stack.retain(|n| *n != note);
            if self.stack.len() < 250 {
                self.stack.push(note);
            }
            let held = self.voices[0].active && self.voices[0].key_down;
            if self.patch.vmode == VoiceMode::Legato && held {
                self.voices[0].tfreq = midi_freq(&self.patch, note as f32);
                self.voices[0].note = note;
                for i in 0..NENV {
                    if self.patch.env[i].relegato {
                        let ep = self.patch.env[i];
                        self.voices[0].env[i].start(&ep);
                    }
                }
                self.last_freq = self.voices[0].tfreq;
                self.last_note = note;
            } else {
                self.trigger(0, note, vel);
            }
            return;
        }
        let max = self.patch.poly.clamp(1, VOICES);
        let vs = &self.voices[..max];
        let idx = vs
            .iter()
            .position(|v| !v.active)
            .or_else(|| vs.iter().enumerate().filter(|(_, v)| !v.key_down).min_by_key(|(_, v)| v.order).map(|(i, _)| i))
            .unwrap_or_else(|| vs.iter().enumerate().min_by_key(|(_, v)| v.order).map(|(i, _)| i).unwrap_or(0));
        self.trigger(idx, note, vel);
    }

    fn voice_note_off(&mut self, note: u8) {
        if self.patch.vmode != VoiceMode::Poly {
            self.stack.retain(|n| *n != note);
            if let Some(&top) = self.stack.last() {
                self.voices[0].tfreq = midi_freq(&self.patch, top as f32);
                self.voices[0].note = top;
                self.last_freq = self.voices[0].tfreq;
            } else {
                Self::release(&mut self.voices[0]);
            }
            return;
        }
        for v in self.voices.iter_mut() {
            if v.active && v.key_down && v.note == note {
                Self::release(v);
            }
        }
    }

    fn release(v: &mut Voice) {
        v.key_down = false;
        for e in v.env.iter_mut() {
            e.begin_release();
        }
    }

    pub fn all_notes_off(&mut self) {
        self.down = [false; 128];
        self.stack.clear();
        self.sustained.clear();
        self.clip_stop();
        self.arp_stop();
        for v in self.voices.iter_mut() {
            if v.active {
                Self::release(v);
            }
        }
    }

    /// Bit mask of the notes that are currently held down (sounding).
    pub fn note_mask(&self) -> u128 {
        let mut m = 0u128;
        for v in self.voices.iter() {
            if v.active && v.key_down && v.note < 128 {
                m |= 1u128 << v.note;
            }
        }
        m
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    // ---- arpeggiator

    fn clip_stop(&mut self) {
        let mut act = std::mem::take(&mut self.clip_act);
        for (n, _) in act.drain(..) {
            self.gen_off(n);
        }
        self.clip_act = act;
        self.clip_held.clear();
        self.clip_timer = f64::INFINITY;
        self.clip_step = 0;
        self.clip_pos = 0;
        self.clip_lane_t = 0.0;
    }

    fn clip_fire(&mut self) {
        let c = self.patch.clip;
        let Some(&(root, kvel)) = self.clip_held.last() else {
            self.clip_timer = f64::INFINITY;
            return;
        };
        let step_len = ((self.fs as f64) * 60.0 / (self.bpm.max(20.0) as f64) * c.beats as f64).max(16.0);
        let st = self.clip_step % c.steps.clamp(1, MAX_STEPS);
        for k in 0..c.n {
            let nt = c.notes[k];
            if nt.start as usize != st {
                continue;
            }
            let note = (root as i32 + nt.pitch as i32).clamp(0, 127) as u8;
            if let Some(i) = self.clip_act.iter().position(|(n, _)| *n == note) {
                self.clip_act.swap_remove(i);
                self.gen_off(note);
            }
            if self.clip_act.len() < 120 {
                self.gen_on(note, (nt.vel as f32 / 100.0 * kvel.max(0.3)).clamp(0.02, 1.0));
                self.clip_act.push((note, (nt.len as f64 * step_len * 0.97).max(8.0)));
            }
        }
        self.clip_pos = st + 1;
        self.clip_lane_t = if c.has_lane { c.lane[st] as f32 / 100.0 } else { 0.0 };
        let sw = c.swing as f64 * 0.5;
        let gap = if st % 2 == 0 { step_len * (1.0 + sw) } else { step_len * (1.0 - sw) };
        self.clip_step = self.clip_step.wrapping_add(1);
        self.clip_timer += gap.max(8.0);
    }

    fn process_clip(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let mut i = 0;
        while i < n {
            self.seg_off = i;
            if self.clip_timer <= 0.0 {
                self.clip_fire();
            }
            let mut j = 0;
            while j < self.clip_act.len() {
                if self.clip_act[j].1 <= 0.0 {
                    let (nt, _) = self.clip_act.swap_remove(j);
                    self.gen_off(nt);
                } else {
                    j += 1;
                }
            }
            let mut m = n - i;
            if self.clip_timer.is_finite() {
                m = m.min(self.clip_timer.ceil().max(1.0) as usize);
            }
            for (_, rem) in self.clip_act.iter() {
                m = m.min(rem.ceil().max(1.0) as usize);
            }
            self.process_inner(&mut l[i..i + m], &mut r[i..i + m]);
            if self.clip_timer.is_finite() {
                self.clip_timer -= m as f64;
            }
            for (_, rem) in self.clip_act.iter_mut() {
                *rem -= m as f64;
            }
            i += m;
        }
    }

    fn arp_stop(&mut self) {
        let mut pl = std::mem::take(&mut self.arp.playing);
        for n in pl.drain(..) {
            self.gen_off(n);
        }
        self.arp.playing = pl;
        self.arp.clear();
    }

    fn arp_gate_off(&mut self) {
        let mut pl = std::mem::take(&mut self.arp.playing);
        for n in pl.drain(..) {
            self.gen_off(n);
        }
        self.arp.playing = pl;
        self.arp.gate = f64::INFINITY;
    }

    fn arp_fire(&mut self) {
        self.arp_gate_off();
        let ap = self.patch.arp;
        self.arp.advance(&ap);
        if self.arp.out.is_empty() && !self.arp.rested {
            self.arp.timer = f64::INFINITY;
            return;
        }
        let out = std::mem::take(&mut self.arp.out);
        for &(n, v) in out.iter() {
            self.gen_on(n, v);
            self.arp.playing.push(n);
        }
        self.arp.out = out;
        let step = (self.fs as f64) * 60.0 / (self.bpm.max(20.0) as f64) * ap.beats as f64;
        let sw = ap.swing as f64 * 0.5;
        let len = if self.arp.odd { step * (1.0 - sw) } else { step * (1.0 + sw) };
        self.arp.odd = !self.arp.odd;
        self.arp.timer += len.max(8.0);
        self.arp.gate = len * (ap.gate as f64).clamp(0.05, 1.0);
    }

    /// Current value of every modulation source (index = `ModSrc` index) for the GUI.
    /// Per-voice sources come from the most recently started sounding voice.
    pub fn live_sources(&self) -> [f32; N_SRC] {
        let mut o = [0.0f32; N_SRC];
        let newest = self.voices.iter().filter(|v| v.active).max_by_key(|v| v.order);
        let (e1, e2, e3, e4, lf, vel, key, rnd) = match newest {
            Some(v) => (v.env[0].out, v.env[1].out, v.env[2].out, v.env[3].out, v.last_lfo, v.vel, ((v.note as f32 - 60.0) / 60.0).clamp(-1.0, 1.0), v.rnd_val),
            None => (0.0, 0.0, 0.0, 0.0, self.glfo_out, self.last_vel, ((self.last_note as f32 - 60.0) / 60.0).clamp(-1.0, 1.0), 0.0),
        };
        o[ModSrc::Env1 as usize] = e1;
        o[ModSrc::Env2 as usize] = e2;
        o[ModSrc::Env3 as usize] = e3;
        o[ModSrc::Env4 as usize] = e4;
        for i in 0..NLFO {
            o[lfo_src(i) as usize] = if self.patch.lfo[i].trig { lf[i] } else { self.glfo_out[i] };
        }
        o[ModSrc::Vel as usize] = vel;
        o[ModSrc::Key as usize] = key;
        o[ModSrc::ModWheel as usize] = self.modwheel;
        o[ModSrc::Aftertouch as usize] = self.aftertouch;
        o[ModSrc::Random as usize] = rnd;
        for i in 0..NMAC {
            o[mac_src(i) as usize] = self.patch.macros[i];
        }
        o[ModSrc::Bend as usize] = self.bend_norm;
        o[ModSrc::ClipLane as usize] = self.clip_lane_s;
        if let Some(v) = newest {
            o[ModSrc::OscA as usize] = v.prev[0].clamp(-1.0, 1.0);
            o[ModSrc::OscB as usize] = v.prev[1].clamp(-1.0, 1.0);
            o[ModSrc::OscC as usize] = v.prev[2].clamp(-1.0, 1.0);
            o[ModSrc::Filt1 as usize] = v.fprev[0].clamp(-1.0, 1.0);
            o[ModSrc::Filt2 as usize] = v.fprev[1].clamp(-1.0, 1.0);
        }
        o
    }

    /// Global values of the modulation matrix for the effect section (None when unused).
    pub fn fx_mod(&self) -> Option<[f32; 12]> {
        self.fx_mod_bus(25)
    }

    /// Same for FX bus 2.
    pub fn fx_mod2(&self) -> Option<[f32; 12]> {
        self.fx_mod_bus(48)
    }

    fn fx_mod_bus(&self, base: usize) -> Option<[f32; 12]> {
        let mut m = [0.0f32; 12];
        let mut any = false;
        let src = |s: ModSrc| -> f32 {
            match s {
                ModSrc::Lfo1 => self.glfo_out[0],
                ModSrc::Lfo2 => self.glfo_out[1],
                ModSrc::Lfo3 => self.glfo_out[2],
                ModSrc::Lfo4 => self.glfo_out[3],
                ModSrc::Lfo5 => self.glfo_out[4],
                ModSrc::Lfo6 => self.glfo_out[5],
                ModSrc::Lfo7 => self.glfo_out[6],
                ModSrc::Lfo8 => self.glfo_out[7],
                ModSrc::Macro5 => self.patch.macros[4],
                ModSrc::Macro6 => self.patch.macros[5],
                ModSrc::Macro7 => self.patch.macros[6],
                ModSrc::Macro8 => self.patch.macros[7],
                ModSrc::Vel => self.last_vel,
                ModSrc::Key => ((self.last_note as f32 - 60.0) / 60.0).clamp(-1.0, 1.0),
                ModSrc::ModWheel => self.modwheel,
                ModSrc::Aftertouch => self.aftertouch,
                ModSrc::Macro1 => self.patch.macros[0],
                ModSrc::Macro2 => self.patch.macros[1],
                ModSrc::Macro3 => self.patch.macros[2],
                ModSrc::Macro4 => self.patch.macros[3],
                ModSrc::Bend => self.bend_norm,
                ModSrc::ClipLane => self.clip_lane_s,
                _ => 0.0,
            }
        };
        for sl in self.patch.slots.iter() {
            let d = sl.dst as usize;
            if sl.src != ModSrc::None && sl.amount != 0.0 && (base..base + 12).contains(&d) {
                m[d - base] += slot_value(sl, src);
                any = true;
            }
        }
        if any {
            Some(m)
        } else {
            None
        }
    }

    /// Adds the synth output to the buffers.
    /// Safety net: a voice that is held but whose key is no longer down (and not held by the pedal or
    /// the arpeggiator) is released. Heals every way a note-off could get lost, e.g. a voice-mode,
    /// arp or voice-count change (preset switch) while keys are held.
    fn reconcile_notes(&mut self) {
        let down = self.down;
        let sus = &self.sustained;
        self.stack.retain(|n| down.get(*n as usize).copied().unwrap_or(false) || sus.contains(n));
        let arp_on = self.patch.arp.on;
        for v in self.voices.iter_mut() {
            if !(v.active && v.key_down) {
                continue;
            }
            let n = v.note;
            let legit = down.get(n as usize).copied().unwrap_or(false)
                || sus.contains(&n)
                || self.clip_act.iter().any(|(c, _)| *c == n)
                || (arp_on && (self.arp.playing.contains(&n) || self.arp.held.iter().any(|(h, _)| *h == n)));
            if !legit {
                Self::release(v);
            }
        }
    }

    pub fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        self.seg_off = 0;
        if !self.patch.arp.hold {
            self.arp.unlatch();
        }
        self.reconcile_notes();
        if !self.patch.clip.on && !self.clip_held.is_empty() {
            self.clip_stop();
        }
        if self.patch.clip.on {
            self.process_clip(l, r);
            return;
        }
        if !self.patch.arp.on {
            if !self.arp.idle() || !self.arp.held.is_empty() {
                self.arp_stop();
            }
            self.process_inner(l, r);
            return;
        }
        let n = l.len();
        let mut i = 0;
        while i < n {
            self.seg_off = i;
            if self.arp.timer <= 0.0 {
                self.arp_fire();
            }
            if self.arp.gate <= 0.0 {
                self.arp_gate_off();
            }
            let mut m = n - i;
            if self.arp.timer.is_finite() {
                m = m.min(self.arp.timer.ceil().max(1.0) as usize);
            }
            if self.arp.gate.is_finite() {
                m = m.min(self.arp.gate.ceil().max(1.0) as usize);
            }
            self.process_inner(&mut l[i..i + m], &mut r[i..i + m]);
            if self.arp.timer.is_finite() {
                self.arp.timer -= m as f64;
            }
            if self.arp.gate.is_finite() {
                self.arp.gate -= m as f64;
            }
            i += m;
        }
    }

    fn process_inner(&mut self, l: &mut [f32], r: &mut [f32]) {
        let n = l.len();
        let mut i = 0;
        let fs = self.fs;
        let _ = self.comb_len;
        while i < n {
            let m = (n - i).min(K);
            let mut glfo = [0.0f32; NLFO];
            for k in 0..NLFO {
                let lp = &self.patch.lfo[k];
                let rate = if lp.sync { self.bpm.max(20.0) / 60.0 / lp.div.beats() } else { lp.rate } as f64;
                self.glfo_ph[k] += rate * m as f64 / fs as f64;
                if self.glfo_ph[k] > 1.0e6 {
                    self.glfo_ph[k] -= 1.0e6;
                }
                let raw = lfo_eval_c(lp.shape, self.glfo_ph[k] + lp.phase0 as f64, 0x51ED_u32 + k as u32 * 977, self.patch.lfo_curve[k].as_deref());
                glfo[k] = if lp.smooth > 0.001 {
                    let alpha = ((1.0 - lp.smooth).powi(3) + 0.0008).min(1.0);
                    self.glfo_s[k] += (raw - self.glfo_s[k]) * alpha;
                    self.glfo_s[k]
                } else {
                    self.glfo_s[k] = raw;
                    raw
                };
            }
            self.glfo_out = glfo;
            let tau = (self.patch.glide_ms * 0.001 / 3.0).max(1e-4);
            let glide_coef = if self.patch.glide_ms <= 0.6 { 1.0 } else { 1.0 - (-(m as f32) / (fs * tau)).exp() };
            self.clip_lane_s += (self.clip_lane_t - self.clip_lane_s) * (1.0 - (-(m as f32) / (fs * 0.008)).exp());
            let ctx = Ctx { lane: self.clip_lane_s, fs, bend: self.bend_norm * self.patch.bend, modwheel: self.modwheel, aftertouch: self.aftertouch, glfo, bpm: self.bpm, glide_coef, tune: 2f32.powf(self.patch.tune / 1200.0) };
            for v in self.voices.iter_mut() {
                if v.active {
                    render_voice(&self.patch, v, &ctx, &mut l[i..i + m], &mut r[i..i + m]);
                }
            }
            // DC blocker (5 Hz high-pass): pulse waves, shaping and distortion can leave an offset that
            // would eat headroom and click on note changes
            let rc = 1.0 - std::f32::consts::TAU * 5.0 / fs;
            for s in i..i + m {
                let (xl, xr) = (if l[s].is_finite() { l[s] } else { 0.0 }, if r[s].is_finite() { r[s] } else { 0.0 });
                let yl = xl - self.dc_x[0] + rc * self.dc_y[0];
                let yr = xr - self.dc_x[1] + rc * self.dc_y[1];
                self.dc_x = [xl, xr];
                self.dc_y = [yl, yr];
                l[s] = yl;
                r[s] = yr;
            }
            i += m;
        }
    }
}

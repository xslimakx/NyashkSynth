//! Effect descriptors: every effect kind is described by a list of knobs (name, range, curve, format).
//! A rack slot stores only "kind + 14 normalised values"; this table turns them into real settings,
//! text for the GUI, and converts the old fixed-bank presets.

use crate::fx::FxPatch;
use crate::params::{Div, DistType, FxBank, FxFilterType, ReverbType};
use nih_plug::prelude::Enum;

/// Values per slot.
pub const NV: usize = 14;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cv {
    Lin,
    /// exponential (frequency, time)
    Log,
    /// symmetrical around zero, finer near the middle
    Sym,
    /// quadratic from min (fine control of small values)
    Pow,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fm {
    Pct,
    Hz,
    Ms,
    Ms1,
    Db,
    Ratio,
    Rate,
    Int,
    Fix2,
    Choice,
    Toggle,
    HzSigned,
}

#[derive(Debug)]
pub struct D {
    pub name: &'static str,
    pub tip: &'static str,
    pub min: f32,
    pub max: f32,
    pub def: f32,
    pub cv: Cv,
    pub fm: Fm,
    pub choices: &'static [&'static str],
}

const fn d(name: &'static str, tip: &'static str, min: f32, max: f32, def: f32, cv: Cv, fm: Fm) -> D {
    D { name, tip, min, max, def, cv, fm, choices: &[] }
}
const fn ch(name: &'static str, tip: &'static str, choices: &'static [&'static str], def: usize) -> D {
    D { name, tip, min: 0.0, max: (choices.len() - 1) as f32, def: def as f32, cv: Cv::Lin, fm: Fm::Choice, choices }
}
const fn tg(name: &'static str, tip: &'static str, def: bool) -> D {
    D { name, tip, min: 0.0, max: 1.0, def: if def { 1.0 } else { 0.0 }, cv: Cv::Lin, fm: Fm::Toggle, choices: &[] }
}

pub const DIV_NAMES: [&str; 18] = ["1/32T", "1/32", "1/16T", "1/16", "1/16.", "1/8T", "1/8", "1/8.", "1/4T", "1/4", "1/4.", "1/2T", "1/2", "1/2.", "1 bar", "2 bars", "4 bars", "8 bars"];
pub const DIST_NAMES: [&str; 14] = ["Soft", "Hard", "Fold", "Crush", "Tube", "Diode 1", "Diode 2", "Sin Fold", "Zero Square", "Asym", "Rectify", "Sine Shaper", "Downsample", "Overdrive"];
pub const REVERB_NAMES: [&str; 5] = ["Plate", "Vintage", "Nitrous", "Basin", "Convolve"];
pub const FILT_NAMES: [&str; 7] = ["LP 12", "LP 24", "HP 12", "HP 24", "Band", "Notch", "Peak"];
pub const BODE_MODES: [&str; 3] = ["Shift", "Ring mod", "Spread"];

use Cv::*;
use Fm::*;

static HYPER: [D; 5] = [
    d("RATE", "Speed of the detune movement", 0.05, 8.0, 0.6, Log, Rate),
    d("DETUNE", "How far the copies drift apart", 0.0, 1.0, 0.5, Lin, Pct),
    d("UNISON", "Number of copies", 2.0, 8.0, 4.0, Lin, Int),
    d("DIMENSION", "Short stereo delays that widen the sound", 0.0, 1.0, 0.4, Lin, Pct),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.5, Lin, Pct),
];
static DIST: [D; 5] = [
    ch("TYPE", "Distortion algorithm", &DIST_NAMES, 0),
    d("DRIVE", "Amount of distortion", 0.0, 1.0, 0.3, Lin, Pct),
    d("TONE", "Low-pass after the distortion", 200.0, 20000.0, 20000.0, Log, Hz),
    d("BIAS", "DC offset before the shaper (even harmonics)", -1.0, 1.0, 0.0, Lin, Fix2),
    d("MIX", "Dry / wet", 0.0, 1.0, 1.0, Lin, Pct),
];
static FLANGER: [D; 8] = [
    d("RATE", "LFO speed", 0.02, 8.0, 0.3, Log, Rate),
    tg("SYNC", "Lock the rate to the host tempo", false),
    ch("DIV", "Note length when synced", &DIV_NAMES, 14),
    d("DEPTH", "Sweep depth", 0.0, 1.0, 0.6, Lin, Pct),
    d("DELAY", "Centre delay time", 0.1, 10.0, 2.0, Lin, Ms1),
    d("FEEDBACK", "Feedback (negative = metallic)", -1.0, 1.0, 0.5, Lin, Pct),
    d("PHASE", "Stereo phase offset of the LFO", 0.0, 1.0, 0.25, Lin, Pct),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.5, Lin, Pct),
];
static PHASER: [D; 9] = [
    d("RATE", "LFO speed", 0.02, 8.0, 0.4, Log, Rate),
    tg("SYNC", "Lock the rate to the host tempo", false),
    ch("DIV", "Note length when synced", &DIV_NAMES, 14),
    d("DEPTH", "Sweep depth", 0.0, 1.0, 0.7, Lin, Pct),
    d("FREQ", "Centre frequency of the sweep", 100.0, 8000.0, 900.0, Log, Hz),
    d("FEEDBACK", "Resonance", 0.0, 1.0, 0.4, Lin, Pct),
    d("STAGES", "Number of all-pass stages (notches = stages / 2)", 2.0, 12.0, 6.0, Lin, Int),
    d("PHASE", "Stereo phase offset of the LFO", 0.0, 1.0, 0.25, Lin, Pct),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.5, Lin, Pct),
];
static CHORUS: [D; 8] = [
    d("RATE", "LFO speed", 0.05, 5.0, 0.6, Log, Rate),
    tg("SYNC", "Lock the rate to the host tempo", false),
    ch("DIV", "Note length when synced", &DIV_NAMES, 14),
    d("DEPTH", "Modulation depth", 0.0, 1.0, 0.5, Lin, Pct),
    d("DELAY", "Centre delay time", 2.0, 30.0, 10.0, Lin, Ms1),
    d("FEEDBACK", "Feedback", 0.0, 1.0, 0.0, Lin, Pct),
    d("FILTER", "Low-pass on the wet signal", 400.0, 20000.0, 20000.0, Log, Hz),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.4, Lin, Pct),
];
static DELAY: [D; 13] = [
    d("TIME L", "Left delay time", 10.0, 1500.0, 350.0, Log, Ms),
    d("TIME R", "Right delay time (needs LINK off)", 10.0, 1500.0, 350.0, Log, Ms),
    tg("LINK", "Use the left time for both sides", true),
    tg("SYNC", "Lock the times to the host tempo", false),
    ch("DIV L", "Left note length when synced", &DIV_NAMES, 6),
    ch("DIV R", "Right note length when synced (needs LINK off)", &DIV_NAMES, 6),
    d("FEEDBACK", "Repeats", 0.0, 1.0, 0.35, Lin, Pct),
    tg("PING-PONG", "Bounce the repeats between left and right", true),
    d("LOW-PASS", "Darkens every repeat", 500.0, 20000.0, 8000.0, Log, Hz),
    d("HIGH-PASS", "Thins every repeat", 20.0, 2000.0, 20.0, Log, Hz),
    d("WIDTH", "Stereo width of the repeats", 0.0, 1.0, 1.0, Lin, Pct),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.3, Lin, Pct),
    tg("HQ", "Cubic interpolation (cleaner when the time is modulated)", false),
];
static COMP: [D; 9] = [
    d("THRESH", "Level where compression starts", -60.0, 0.0, -18.0, Lin, Db),
    d("RATIO", "Compression ratio", 1.0, 20.0, 4.0, Log, Ratio),
    d("ATTACK", "How fast it clamps down", 0.1, 200.0, 5.0, Log, Ms),
    d("RELEASE", "How fast it lets go", 5.0, 1000.0, 120.0, Log, Ms),
    d("GAIN", "Make-up gain", 0.0, 24.0, 0.0, Lin, Db),
    tg("MULTIBAND", "Compress low / mid / high separately", false),
    d("SPLIT 1", "Low / mid crossover (multiband)", 60.0, 500.0, 150.0, Log, Hz),
    d("SPLIT 2", "Mid / high crossover (multiband)", 800.0, 8000.0, 2500.0, Log, Hz),
    d("MIX", "Dry / wet (parallel compression)", 0.0, 1.0, 1.0, Lin, Pct),
];
static REVERB: [D; 8] = [
    ch("TYPE", "Reverb algorithm", &REVERB_NAMES, 0),
    d("SIZE", "Room size / decay", 0.0, 1.0, 0.6, Lin, Pct),
    d("DAMP", "High-frequency absorption", 0.0, 1.0, 0.5, Lin, Pct),
    d("PRE-DELAY", "Gap before the reverb starts", 0.5, 250.0, 0.5, Log, Ms),
    d("LOW CUT", "High-pass on the reverb", 20.0, 1000.0, 20.0, Log, Hz),
    d("HIGH CUT", "Low-pass on the reverb", 1000.0, 20000.0, 20000.0, Log, Hz),
    d("WIDTH", "Stereo width", 0.0, 1.0, 1.0, Lin, Pct),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.25, Lin, Pct),
];
static EQ: [D; 7] = [
    d("LOW FREQ", "Low shelf frequency", 20.0, 1000.0, 120.0, Log, Hz),
    d("LOW GAIN", "Low shelf gain", -18.0, 18.0, 0.0, Lin, Db),
    d("MID FREQ", "Peak frequency", 100.0, 10000.0, 1000.0, Log, Hz),
    d("MID Q", "Peak width (higher = narrower)", 0.2, 10.0, 1.0, Log, Ratio),
    d("MID GAIN", "Peak gain", -18.0, 18.0, 0.0, Lin, Db),
    d("HIGH FREQ", "High shelf frequency", 1000.0, 20000.0, 6000.0, Log, Hz),
    d("HIGH GAIN", "High shelf gain", -18.0, 18.0, 0.0, Lin, Db),
];
static FILTER: [D; 5] = [
    ch("TYPE", "Filter type", &FILT_NAMES, 0),
    d("CUTOFF", "Cutoff frequency", 20.0, 20000.0, 2000.0, Log, Hz),
    d("RESONANCE", "Resonance", 0.0, 1.0, 0.2, Lin, Pct),
    d("DRIVE", "Saturation inside the filter", 0.0, 1.0, 0.0, Lin, Pct),
    d("MIX", "Dry / wet", 0.0, 1.0, 1.0, Lin, Pct),
];
static BODE: [D; 8] = [
    ch("MODE", "Shift = every partial moves by the same number of Hz (inharmonic). Ring mod = both side bands. Spread = left goes up, right goes down.", &BODE_MODES, 0),
    d("SHIFT", "Frequency shift in Hz (negative = down)", -5000.0, 5000.0, 0.0, Sym, HzSigned),
    d("FINE", "Fine shift", -50.0, 50.0, 0.0, Lin, HzSigned),
    d("LFO RATE", "Speed of the shift LFO", 0.05, 20.0, 0.5, Log, Rate),
    d("LFO DEPTH", "How far the LFO moves the shift (Hz)", 0.0, 1000.0, 0.0, Pow, Hz),
    d("FEEDBACK", "Feed the shifted signal back (endless rising / falling spiral)", 0.0, 1.0, 0.0, Lin, Pct),
    d("TONE", "Low-pass inside the feedback loop", 500.0, 20000.0, 8000.0, Log, Hz),
    d("MIX", "Dry / wet", 0.0, 1.0, 0.5, Lin, Pct),
];
static UTILITY: [D; 5] = [
    d("GAIN", "Level", -48.0, 24.0, 0.0, Lin, Db),
    d("PAN", "Balance", -1.0, 1.0, 0.0, Lin, Pct),
    d("WIDTH", "Stereo width (0 = mono, 200% = extra wide)", 0.0, 2.0, 1.0, Lin, Pct),
    tg("MONO", "Sum to mono", false),
    tg("INVERT", "Flip the polarity", false),
];

pub const NKINDS: usize = 12;
pub const KIND_NAMES: [&str; NKINDS] = ["HYPER", "DIST", "FLANGER", "PHASER", "CHORUS", "DELAY", "COMP", "REVERB", "EQ", "FILTER", "BODE", "UTILITY"];

/// Kinds in the order of the "add effect" menu, with a one-line description.
pub const MENU: [(usize, &str, &str); NKINDS] = [
    (0, "Hyper / Dimension", "Unison detune + stereo widening"),
    (1, "Distortion", "14 shapers: soft, fold, tube, diode, crush ..."),
    (4, "Chorus", "Classic modulated delay"),
    (2, "Flanger", "Short swept delay with feedback"),
    (3, "Phaser", "All-pass sweep, up to 12 stages"),
    (5, "Delay", "Stereo / ping-pong, tempo sync"),
    (7, "Reverb", "Plate, vintage, nitrous, basin, convolution"),
    (6, "Compressor", "Single or 3-band, with parallel mix"),
    (8, "EQ", "Low shelf, peak, high shelf"),
    (9, "Filter", "LP / HP / band / notch / peak"),
    (10, "Bode Shifter", "Frequency shifter / ring mod / spread"),
    (11, "Utility", "Gain, pan, width, mono, polarity"),
];

pub fn descs(kind: usize) -> &'static [D] {
    match kind {
        0 => &HYPER,
        1 => &DIST,
        2 => &FLANGER,
        3 => &PHASER,
        4 => &CHORUS,
        5 => &DELAY,
        6 => &COMP,
        7 => &REVERB,
        8 => &EQ,
        9 => &FILTER,
        10 => &BODE,
        _ => &UTILITY,
    }
}

impl D {
    /// normalised 0..1 -> real value
    pub fn real(&self, n: f32) -> f32 {
        let n = n.clamp(0.0, 1.0);
        let v = match self.cv {
            Lin => self.min + (self.max - self.min) * n,
            Log => self.min * (self.max / self.min).powf(n),
            Sym => {
                let u = 2.0 * n - 1.0;
                self.max * u.abs() * u
            }
            Pow => self.min + (self.max - self.min) * n * n,
        };
        match self.fm {
            Int | Choice | Toggle => v.round(),
            _ => v,
        }
    }

    /// real value -> normalised 0..1
    pub fn norm(&self, v: f32) -> f32 {
        let v = v.clamp(self.min.min(self.max), self.max);
        let n = match self.cv {
            Lin => (v - self.min) / (self.max - self.min),
            Log => (v / self.min).ln() / (self.max / self.min).ln(),
            Sym => {
                let u = (v / self.max).clamp(-1.0, 1.0);
                0.5 + 0.5 * u.signum() * u.abs().sqrt()
            }
            Pow => ((v - self.min) / (self.max - self.min)).max(0.0).sqrt(),
        };
        n.clamp(0.0, 1.0)
    }

    pub fn def_norm(&self) -> f32 {
        self.norm(self.def)
    }

    pub fn text(&self, n: f32) -> String {
        let v = self.real(n);
        match self.fm {
            Pct => format!("{:.0}%", v * 100.0),
            Hz => {
                if v >= 1000.0 {
                    format!("{:.2} kHz", v / 1000.0)
                } else if v >= 100.0 {
                    format!("{:.0} Hz", v)
                } else {
                    format!("{:.1} Hz", v)
                }
            }
            HzSigned => format!("{:+.1} Hz", v),
            Ms => {
                if v >= 1000.0 {
                    format!("{:.2} s", v / 1000.0)
                } else if v >= 100.0 {
                    format!("{:.0} ms", v)
                } else {
                    format!("{:.1} ms", v)
                }
            }
            Ms1 => format!("{:.1} ms", v),
            Db => format!("{:+.1} dB", v),
            Ratio => format!("{:.1}", v),
            Rate => format!("{:.2} Hz", v),
            Int => format!("{:.0}", v),
            Fix2 => format!("{:+.2}", v),
            Choice => self.choices.get(v as usize).copied().unwrap_or("?").to_string(),
            Toggle => if v > 0.5 { "ON" } else { "OFF" }.to_string(),
        }
    }

    /// Parses what the user typed into a real value.
    pub fn parse(&self, s: &str) -> Option<f32> {
        let t = s.trim().to_lowercase();
        let num = |x: &str| x.trim().parse::<f32>().ok();
        match self.fm {
            Pct => num(t.trim_end_matches('%')).map(|v| v / 100.0),
            Hz | HzSigned | Rate => {
                let t = t.trim_end_matches("hz").trim();
                if let Some(k) = t.strip_suffix('k') {
                    num(k).map(|v| v * 1000.0)
                } else {
                    num(t)
                }
            }
            Ms | Ms1 => {
                if let Some(x) = t.strip_suffix("ms") {
                    num(x)
                } else if let Some(x) = t.strip_suffix('s') {
                    num(x).map(|v| v * 1000.0)
                } else {
                    num(&t)
                }
            }
            Db => num(t.trim_end_matches("db")),
            Choice => self.choices.iter().position(|c| c.to_lowercase() == t).map(|i| i as f32),
            Toggle => Some(if matches!(t.as_str(), "1" | "on" | "yes" | "true") { 1.0 } else { 0.0 }),
            _ => num(&t),
        }
    }
}

/// Real values of all knobs of a slot.
pub fn reals(kind: usize, v: &[f32; NV]) -> [f32; NV] {
    let ds = descs(kind);
    let mut r = [0.0f32; NV];
    for (i, dd) in ds.iter().enumerate().take(NV) {
        r[i] = dd.real(v[i]);
    }
    r
}

/// Normalised default values of a new slot of this kind.
pub fn defaults(kind: usize) -> [f32; NV] {
    let mut v = [0.0f32; NV];
    for (i, dd) in descs(kind).iter().enumerate().take(NV) {
        v[i] = dd.def_norm();
    }
    v
}

fn div_of(i: f32) -> Div {
    <Div as Enum>::from_index((i as usize).min(17))
}

/// Settings of one effect from its real knob values.
pub fn patch_for(kind: usize, r: &[f32; NV], bpm: f32) -> FxPatch {
    let mut p = FxPatch::default();
    let bpm = bpm.max(20.0);
    let rate = |hz: f32, sync: f32, div: f32| if sync > 0.5 { bpm / 60.0 / div_of(div).beats() } else { hz };
    match kind {
        0 => {
            p.hy_rate = r[0];
            p.hy_detune = r[1];
            p.hy_uni = r[2] as usize;
            p.hy_dim = r[3];
            p.hy_mix = r[4];
        }
        1 => {
            p.ds_type = <DistType as Enum>::from_index((r[0] as usize).min(13));
            p.ds_drive = r[1];
            p.ds_tone = r[2];
            p.ds_bias = r[3];
            p.ds_mix = r[4];
        }
        2 => {
            p.fg_rate = rate(r[0], r[1], r[2]);
            p.fg_depth = r[3];
            p.fg_delay = r[4];
            p.fg_fb = r[5];
            p.fg_phase = r[6];
            p.fg_mix = r[7];
        }
        3 => {
            p.ph_rate = rate(r[0], r[1], r[2]);
            p.ph_depth = r[3];
            p.ph_freq = r[4];
            p.ph_fb = r[5];
            p.ph_stages = r[6] as usize;
            p.ph_phase = r[7];
            p.ph_mix = r[8];
        }
        4 => {
            p.ch_rate = rate(r[0], r[1], r[2]);
            p.ch_depth = r[3];
            p.ch_delay = r[4];
            p.ch_fb = r[5];
            p.ch_filter = r[6];
            p.ch_mix = r[7];
        }
        5 => {
            let beat_ms = 60000.0 / bpm;
            let fit = |mut ms: f32| {
                while ms > 1800.0 {
                    ms *= 0.5;
                }
                ms
            };
            let sync = r[3] > 0.5;
            let l = if sync { fit(beat_ms * div_of(r[4]).beats()) } else { r[0] };
            p.dl_ms_l = l;
            p.dl_ms_r = if r[2] > 0.5 {
                l
            } else if sync {
                fit(beat_ms * div_of(r[5]).beats())
            } else {
                r[1]
            };
            p.dl_fb = r[6];
            p.dl_pp = r[7] > 0.5;
            p.dl_lp = r[8];
            p.dl_hp = r[9];
            p.dl_width = r[10];
            p.dl_mix = r[11];
            p.dl_hq = r[12] > 0.5;
        }
        6 => {
            p.cp_thresh = r[0];
            p.cp_ratio = r[1];
            p.cp_attack = r[2];
            p.cp_release = r[3];
            p.cp_gain = r[4];
            p.cp_multi = r[5] > 0.5;
            p.cp_f1 = r[6];
            p.cp_f2 = r[7];
            p.cp_mix = r[8];
        }
        7 => {
            p.rv_type = <ReverbType as Enum>::from_index((r[0] as usize).min(4));
            p.rv_size = r[1];
            p.rv_damp = r[2];
            p.rv_pre = r[3];
            p.rv_lowcut = r[4];
            p.rv_highcut = r[5];
            p.rv_width = r[6];
            p.rv_mix = r[7];
        }
        8 => {
            p.eq_lf = r[0];
            p.eq_lg = r[1];
            p.eq_mf = r[2];
            p.eq_mq = r[3];
            p.eq_mg = r[4];
            p.eq_hf = r[5];
            p.eq_hg = r[6];
        }
        9 => {
            p.ff_type = <FxFilterType as Enum>::from_index((r[0] as usize).min(6));
            p.ff_cut = r[1];
            p.ff_res = r[2];
            p.ff_drive = r[3];
            p.ff_mix = r[4];
        }
        10 => {
            p.bd_mode = r[0] as usize;
            p.bd_shift = r[1];
            p.bd_fine = r[2];
            p.bd_lfo_rate = r[3];
            p.bd_lfo_depth = r[4];
            p.bd_fb = r[5];
            p.bd_tone = r[6];
            p.bd_mix = r[7];
        }
        _ => {
            p.ut_gain = r[0];
            p.ut_pan = r[1];
            p.ut_width = r[2];
            p.ut_mono = r[3] > 0.5;
            p.ut_invert = r[4] > 0.5;
        }
    }
    p
}

/// Real knob values taken from the old fixed bank (for converting old presets).
pub fn reals_from_bank(kind: usize, b: &FxBank) -> [f32; NV] {
    let mut r = [0.0f32; NV];
    let t = |x: bool| if x { 1.0 } else { 0.0 };
    let dv = |x: Div| x as usize as f32;
    match kind {
        0 => r[..5].copy_from_slice(&[b.hy_rate.value(), b.hy_detune.value(), b.hy_uni.value() as f32, b.hy_dim.value(), b.hy_mix.value()]),
        1 => r[..5].copy_from_slice(&[b.ds_type.value() as usize as f32, b.ds_drive.value(), b.ds_tone.value(), b.ds_bias.value(), b.ds_mix.value()]),
        2 => r[..8].copy_from_slice(&[b.fg_rate.value(), t(b.fg_sync.value()), dv(b.fg_div.value()), b.fg_depth.value(), b.fg_delay.value(), b.fg_fb.value(), b.fg_phase.value(), b.fg_mix.value()]),
        3 => r[..9].copy_from_slice(&[b.ph_rate.value(), t(b.ph_sync.value()), dv(b.ph_div.value()), b.ph_depth.value(), b.ph_freq.value(), b.ph_fb.value(), b.ph_stages.value() as f32, b.ph_phase.value(), b.ph_mix.value()]),
        4 => r[..8].copy_from_slice(&[b.ch_rate.value(), t(b.ch_sync.value()), dv(b.ch_div.value()), b.ch_depth.value(), b.ch_delay.value(), b.ch_fb.value(), b.ch_filter.value(), b.ch_mix.value()]),
        5 => r[..13].copy_from_slice(&[
            b.dl_time.value(),
            b.dl_time_r.value(),
            t(b.dl_link.value()),
            t(b.dl_sync.value()),
            dv(b.dl_div.value()),
            dv(b.dl_div_r.value()),
            b.dl_fb.value(),
            t(b.dl_pp.value()),
            b.dl_lp.value(),
            b.dl_hp.value(),
            b.dl_width.value(),
            b.dl_mix.value(),
            t(b.dl_hq.value()),
        ]),
        6 => r[..9].copy_from_slice(&[b.cp_thresh.value(), b.cp_ratio.value(), b.cp_attack.value(), b.cp_release.value(), b.cp_gain.value(), t(b.cp_multi.value()), b.cp_f1.value(), b.cp_f2.value(), b.cp_mix.value()]),
        7 => r[..8].copy_from_slice(&[b.rv_type.value() as usize as f32, b.rv_size.value(), b.rv_damp.value(), b.rv_pre.value(), b.rv_lowcut.value(), b.rv_highcut.value(), b.rv_width.value(), b.rv_mix.value()]),
        8 => r[..7].copy_from_slice(&[b.eq_lf.value(), b.eq_lg.value(), b.eq_mf.value(), b.eq_mq.value(), b.eq_mg.value(), b.eq_hf.value(), b.eq_hg.value()]),
        9 => r[..5].copy_from_slice(&[b.ff_type.value() as usize as f32, b.ff_cut.value(), b.ff_res.value(), b.ff_drive.value(), b.ff_mix.value()]),
        10 => r[..8].copy_from_slice(&[0.0, b.bd_shift.value(), 0.0, 0.5, 0.0, b.bd_fb.value(), 8000.0, b.bd_mix.value()]),
        _ => r[..5].copy_from_slice(&[b.ut_gain.value(), b.ut_pan.value(), b.ut_width.value(), t(b.ut_mono.value()), t(b.ut_invert.value())]),
    }
    r
}

/// Normalised values from real ones.
pub fn norms(kind: usize, r: &[f32; NV]) -> [f32; NV] {
    let mut v = [0.0f32; NV];
    for (i, dd) in descs(kind).iter().enumerate().take(NV) {
        v[i] = dd.norm(r[i]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_round_trips_and_defaults_match_the_old_bank() {
        for k in 0..NKINDS {
            for (i, dd) in descs(k).iter().enumerate() {
                for n in [0.0f32, 0.13, 0.5, 0.77, 1.0] {
                    let v = dd.real(n);
                    let n2 = dd.norm(v);
                    let v2 = dd.real(n2);
                    assert!((v - v2).abs() <= 1e-3 * v.abs().max(1.0), "{} {} {} -> {} -> {}", KIND_NAMES[k], dd.name, n, v, v2);
                }
                assert!(dd.def >= dd.min.min(dd.max) && dd.def <= dd.max, "{} {} default out of range", KIND_NAMES[k], dd.name);
                assert!(dd.text(dd.def_norm()).len() > 0, "{} {} text", KIND_NAMES[k], i);
            }
            assert!(descs(k).len() <= NV);
        }
    }

    #[test]
    fn defaults_build_patches_equal_to_the_old_defaults() {
        let bank = FxBank::new();
        for k in 0..NKINDS {
            let from_new = patch_for(k, &reals(k, &defaults(k)), 120.0);
            let from_old = patch_for(k, &reals_from_bank(k, &bank), 120.0);
            // compare through the audible fields of this kind
            let a = format!("{:?}", fields_of(k, &from_new));
            let b = format!("{:?}", fields_of(k, &from_old));
            assert_eq!(a, b, "{}", KIND_NAMES[k]);
        }
    }

    fn fields_of(k: usize, p: &FxPatch) -> Vec<String> {
        let f = |x: f32| format!("{:.2}", x);
        match k {
            0 => vec![f(p.hy_rate), f(p.hy_detune), f(p.hy_uni as f32), f(p.hy_dim), f(p.hy_mix)],
            1 => vec![format!("{:?}", p.ds_type), f(p.ds_drive), f(p.ds_tone), f(p.ds_bias), f(p.ds_mix)],
            2 => vec![f(p.fg_rate), f(p.fg_depth), f(p.fg_delay), f(p.fg_fb), f(p.fg_phase), f(p.fg_mix)],
            3 => vec![f(p.ph_rate), f(p.ph_depth), f(p.ph_freq), f(p.ph_fb), f(p.ph_stages as f32), f(p.ph_phase), f(p.ph_mix)],
            4 => vec![f(p.ch_rate), f(p.ch_depth), f(p.ch_delay), f(p.ch_fb), f(p.ch_filter), f(p.ch_mix)],
            5 => vec![f(p.dl_ms_l), f(p.dl_ms_r), f(p.dl_fb), format!("{}", p.dl_pp), f(p.dl_lp), f(p.dl_hp), f(p.dl_width), f(p.dl_mix), format!("{}", p.dl_hq)],
            6 => vec![f(p.cp_thresh), f(p.cp_ratio), f(p.cp_attack), f(p.cp_release), f(p.cp_gain), format!("{}", p.cp_multi), f(p.cp_f1), f(p.cp_f2), f(p.cp_mix)],
            7 => vec![format!("{:?}", p.rv_type), f(p.rv_size), f(p.rv_damp), f(p.rv_pre), f(p.rv_lowcut), f(p.rv_highcut), f(p.rv_width), f(p.rv_mix)],
            8 => vec![f(p.eq_lf), f(p.eq_lg), f(p.eq_mf), f(p.eq_mq), f(p.eq_mg), f(p.eq_hf), f(p.eq_hg)],
            9 => vec![format!("{:?}", p.ff_type), f(p.ff_cut), f(p.ff_res), f(p.ff_drive), f(p.ff_mix)],
            10 => vec![f(p.bd_shift), f(p.bd_fb), f(p.bd_mix)],
            _ => vec![f(p.ut_gain), f(p.ut_pan), f(p.ut_width), format!("{}", p.ut_mono), format!("{}", p.ut_invert)],
        }
    }
}

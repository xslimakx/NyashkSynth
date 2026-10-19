//! Parameters of NyashkSynth.

use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum TableId {
    #[name = "Basic Shapes"]
    Basic,
    #[name = "Analog PWM"]
    Pwm,
    #[name = "Saw Sweep"]
    SawSweep,
    #[name = "Vowels"]
    Vowels,
    #[name = "Comb Sweep"]
    Comb,
    #[name = "FM Bells"]
    Fm,
    #[name = "Organ"]
    Organ,
    #[name = "Rough Noise"]
    Noise,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum WarpMode {
    #[name = "Off"]
    Off,
    #[name = "Bend +"]
    BendPlus,
    #[name = "Bend -"]
    BendMinus,
    #[name = "Bend +/-"]
    BendBoth,
    #[name = "PWM"]
    Pwm,
    #[name = "Sync"]
    Sync,
    #[name = "Mirror"]
    Mirror,
    #[name = "Quantize"]
    Quantize,
    #[name = "FM (other osc)"]
    Fm,
    #[name = "AM (other osc)"]
    Am,
    #[name = "RM (other osc)"]
    Rm,
    #[name = "Sine Bend"]
    SineBend,
    #[name = "Asym"]
    Asym,
    #[name = "Flip"]
    Flip,
    #[name = "Self FM"]
    SelfFm,
    #[name = "Self AM"]
    SelfAm,
    #[name = "Self RM"]
    SelfRm,
    #[name = "Pinch"]
    Pinch,
    #[name = "Fold"]
    Fold,
    #[name = "Clip"]
    Clip,
    #[name = "Ripple"]
    Ripple,
    #[name = "Twist"]
    Twist,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum NoiseType {
    #[name = "White"]
    White,
    #[name = "Pink"]
    Pink,
    #[name = "Brown"]
    Brown,
    #[name = "Hiss"]
    Hiss,
    #[name = "Crackle"]
    Crackle,
    #[name = "Blue"]
    Blue,
    #[name = "Violet"]
    Violet,
    #[name = "Dust"]
    Dust,
    #[name = "Vinyl"]
    Vinyl,
    #[name = "Tape Hiss"]
    Tape,
    #[name = "Wind"]
    Wind,
    #[name = "Digital"]
    Digital,
    #[name = "Sample (file)"]
    Sample,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum SubWave {
    #[name = "Sine"]
    Sine,
    #[name = "Triangle"]
    Tri,
    #[name = "Saw"]
    Saw,
    #[name = "Square"]
    Square,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum FilterType {
    #[name = "LP 12"]
    Lp12,
    #[name = "LP 24"]
    Lp24,
    #[name = "HP 12"]
    Hp12,
    #[name = "HP 24"]
    Hp24,
    #[name = "Band"]
    Band,
    #[name = "Notch"]
    Notch,
    #[name = "Ladder 24"]
    Ladder,
    #[name = "LP 6"]
    Lp6,
    #[name = "HP 6"]
    Hp6,
    #[name = "Peak"]
    Peak,
    #[name = "Low Shelf"]
    LowShelf,
    #[name = "High Shelf"]
    HighShelf,
    #[name = "Comb +"]
    CombPlus,
    #[name = "Comb -"]
    CombMinus,
    #[name = "Phaser"]
    PhaserF,
    #[name = "Formant"]
    Formant,
    #[name = "Custom (points)"]
    Custom,
}

/// Kind of one point of the custom filter.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum BandKind {
    #[name = "Bell"]
    Bell,
    #[name = "Low Shelf"]
    LowShelf,
    #[name = "High Shelf"]
    HighShelf,
    #[name = "High Pass"]
    HighPass,
    #[name = "Low Pass"]
    LowPass,
    #[name = "Notch"]
    Notch,
    #[name = "Band Pass"]
    BandPass,
}

/// One draggable point of the custom filter.
#[derive(Params)]
pub struct CBand {
    #[id = "cbon"]
    pub on: BoolParam,
    #[id = "cbty"]
    pub kind: EnumParam<BandKind>,
    #[id = "cbfr"]
    pub freq: FloatParam,
    #[id = "cbgn"]
    pub gain: FloatParam,
    #[id = "cbq"]
    pub q: FloatParam,
}

pub const CBANDS: usize = 8;

impl CBand {
    fn new(i: usize) -> Self {
        let defaults = [60.0f32, 150.0, 400.0, 1000.0, 2500.0, 6000.0, 12000.0, 18000.0];
        Self {
            on: BoolParam::new("Point On", false),
            kind: EnumParam::new("Point Type", BandKind::Bell),
            freq: FloatParam::new("Point Freq", defaults[i % 8], FloatRange::Skewed { min: 20.0, max: 20000.0, factor: FloatRange::skew_factor(-2.2) })
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(0)),
            gain: db_param("Point Gain", 0.0, -36.0, 36.0),
            q: FloatParam::new("Point Q", 1.0, FloatRange::Skewed { min: 0.15, max: 16.0, factor: FloatRange::skew_factor(-1.0) }).with_value_to_string(formatters::v2s_f32_rounded(2)),
        }
    }
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum LfoShape {
    #[name = "Sine"]
    Sine,
    #[name = "Triangle"]
    Tri,
    #[name = "Saw Up"]
    SawUp,
    #[name = "Saw Down"]
    SawDown,
    #[name = "Square"]
    Square,
    #[name = "Sample&Hold"]
    Sh,
    #[name = "Smooth Rnd"]
    Smooth,
    #[name = "Custom (draw)"]
    Custom,
    #[name = "Triangle Soft"]
    TriSoft,
    #[name = "Stairs"]
    Stairs,
    #[name = "Chaos Lorenz"]
    Lorenz,
    #[name = "Chaos Rossler"]
    Rossler,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Div {
    #[name = "1/32T"]
    D32T,
    #[name = "1/32"]
    D32,
    #[name = "1/16T"]
    D16T,
    #[name = "1/16"]
    D16,
    #[name = "1/16."]
    D16D,
    #[name = "1/8T"]
    D8T,
    #[name = "1/8"]
    D8,
    #[name = "1/8."]
    D8D,
    #[name = "1/4T"]
    D4T,
    #[name = "1/4"]
    D4,
    #[name = "1/4."]
    D4D,
    #[name = "1/2T"]
    D2T,
    #[name = "1/2"]
    D2,
    #[name = "1/2."]
    D2D,
    #[name = "1 bar"]
    B1,
    #[name = "2 bars"]
    B2,
    #[name = "4 bars"]
    B4,
    #[name = "8 bars"]
    B8,
}

impl Div {
    pub fn beats(self) -> f32 {
        match self {
            Div::D32T => 0.0833333,
            Div::D32 => 0.125,
            Div::D16T => 0.166667,
            Div::D16 => 0.25,
            Div::D16D => 0.375,
            Div::D8T => 0.333333,
            Div::D8 => 0.5,
            Div::D8D => 0.75,
            Div::D4T => 0.666667,
            Div::D4 => 1.0,
            Div::D4D => 1.5,
            Div::D2T => 1.33333,
            Div::D2 => 2.0,
            Div::D2D => 3.0,
            Div::B1 => 4.0,
            Div::B2 => 8.0,
            Div::B4 => 16.0,
            Div::B8 => 32.0,
        }
    }
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ModSrc {
    #[name = "-"]
    None,
    #[name = "Env 2"]
    Env2,
    #[name = "Env 3"]
    Env3,
    #[name = "LFO 1"]
    Lfo1,
    #[name = "LFO 2"]
    Lfo2,
    #[name = "LFO 3"]
    Lfo3,
    #[name = "LFO 4"]
    Lfo4,
    #[name = "Velocity"]
    Vel,
    #[name = "Key"]
    Key,
    #[name = "Mod Wheel"]
    ModWheel,
    #[name = "Aftertouch"]
    Aftertouch,
    #[name = "Random"]
    Random,
    #[name = "Macro 1"]
    Macro1,
    #[name = "Macro 2"]
    Macro2,
    #[name = "Macro 3"]
    Macro3,
    #[name = "Macro 4"]
    Macro4,
    #[name = "Env 1"]
    Env1,
    #[name = "Pitch Bend"]
    Bend,
    #[name = "Env 4"]
    Env4,
    #[name = "LFO 5"]
    Lfo5,
    #[name = "LFO 6"]
    Lfo6,
    #[name = "LFO 7"]
    Lfo7,
    #[name = "LFO 8"]
    Lfo8,
    #[name = "Macro 5"]
    Macro5,
    #[name = "Macro 6"]
    Macro6,
    #[name = "Macro 7"]
    Macro7,
    #[name = "Macro 8"]
    Macro8,
}

pub const N_SRC: usize = 27;
pub const NLFO: usize = 8;
pub const NENV: usize = 4;
pub const NMAC: usize = 8;

pub fn lfo_src(i: usize) -> ModSrc {
    [ModSrc::Lfo1, ModSrc::Lfo2, ModSrc::Lfo3, ModSrc::Lfo4, ModSrc::Lfo5, ModSrc::Lfo6, ModSrc::Lfo7, ModSrc::Lfo8][i.min(7)]
}
pub fn env_src(i: usize) -> ModSrc {
    [ModSrc::Env1, ModSrc::Env2, ModSrc::Env3, ModSrc::Env4][i.min(3)]
}
pub fn mac_src(i: usize) -> ModSrc {
    [ModSrc::Macro1, ModSrc::Macro2, ModSrc::Macro3, ModSrc::Macro4, ModSrc::Macro5, ModSrc::Macro6, ModSrc::Macro7, ModSrc::Macro8][i.min(7)]
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ModDst {
    #[name = "-"]
    None,
    #[name = "A Position"]
    APos,
    #[name = "B Position"]
    BPos,
    #[name = "A Level"]
    ALevel,
    #[name = "B Level"]
    BLevel,
    #[name = "A Pitch"]
    APitch,
    #[name = "B Pitch"]
    BPitch,
    #[name = "A Warp"]
    AWarp,
    #[name = "B Warp"]
    BWarp,
    #[name = "Cutoff"]
    Cutoff,
    #[name = "Resonance"]
    Res,
    #[name = "Drive"]
    Drive,
    #[name = "Unison Detune"]
    Detune,
    #[name = "Pan"]
    Pan,
    #[name = "Volume"]
    Volume,
    #[name = "Sub Level"]
    SubLevel,
    #[name = "Noise Level"]
    NoiseLevel,
    #[name = "A Pan"]
    APan,
    #[name = "B Pan"]
    BPan,
    #[name = "A Blend"]
    ABlend,
    #[name = "B Blend"]
    BBlend,
    #[name = "Noise Pitch"]
    NoisePitch,
    #[name = "Filter Mix"]
    FilterMix,
    #[name = "A Warp 2"]
    AWarp2,
    #[name = "B Warp 2"]
    BWarp2,
    #[name = "FX Hyper Mix"]
    FxHyperMix,
    #[name = "FX Dist Drive"]
    FxDistDrive,
    #[name = "FX Dist Mix"]
    FxDistMix,
    #[name = "FX Chorus Mix"]
    FxChorusMix,
    #[name = "FX Flanger Depth"]
    FxFlangerDepth,
    #[name = "FX Phaser Freq"]
    FxPhaserFreq,
    #[name = "FX Delay Mix"]
    FxDelayMix,
    #[name = "FX Delay Feedback"]
    FxDelayFb,
    #[name = "FX Reverb Mix"]
    FxReverbMix,
    #[name = "FX Reverb Size"]
    FxReverbSize,
    #[name = "FX Filter Cutoff"]
    FxFilterCut,
    #[name = "FX Filter Res"]
    FxFilterRes,
    #[name = "C Position"]
    CPos,
    #[name = "C Level"]
    CLevel,
    #[name = "C Pitch"]
    CPitch,
    #[name = "C Warp"]
    CWarp,
    #[name = "C Pan"]
    CPan,
    #[name = "C Blend"]
    CBlend,
    #[name = "C Warp 2"]
    CWarp2,
    #[name = "Filter 2 Cutoff"]
    Cutoff2,
    #[name = "Filter 2 Resonance"]
    Res2,
    #[name = "Filter 2 Drive"]
    Drive2,
    #[name = "Filter 2 Mix"]
    FilterMix2,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ArpMode {
    #[name = "Up"]
    Up,
    #[name = "Down"]
    Down,
    #[name = "Up / Down"]
    UpDown,
    #[name = "Down / Up"]
    DownUp,
    #[name = "Random"]
    Random,
    #[name = "As Played"]
    AsPlayed,
    #[name = "Chord"]
    Chord,
    #[name = "Pattern"]
    Pattern,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum DistType {
    #[name = "Soft"]
    Soft,
    #[name = "Hard"]
    Hard,
    #[name = "Fold"]
    Fold,
    #[name = "Crush"]
    Crush,
    #[name = "Tube"]
    Tube,
    #[name = "Diode 1"]
    Diode1,
    #[name = "Diode 2"]
    Diode2,
    #[name = "Sin Fold"]
    SinFold,
    #[name = "Zero Square"]
    ZeroSq,
    #[name = "Asym"]
    Asym,
    #[name = "Rectify"]
    Rectify,
    #[name = "Sine Shaper"]
    SineShaper,
    #[name = "Downsample"]
    Downsample,
    #[name = "Overdrive"]
    Overdrive,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ReverbType {
    #[name = "Plate"]
    Plate,
    #[name = "Vintage"]
    Vintage,
    #[name = "Nitrous"]
    Nitrous,
    #[name = "Basin"]
    Basin,
    #[name = "Convolve"]
    Convolve,
}

/// Key lock: incoming MIDI notes are moved to the nearest note of the scale.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ScaleType {
    #[name = "Off"]
    Off,
    #[name = "Major"]
    Major,
    #[name = "Natural minor"]
    Minor,
    #[name = "Harmonic minor"]
    HarmMinor,
    #[name = "Melodic minor"]
    MelMinor,
    #[name = "Dorian"]
    Dorian,
    #[name = "Phrygian"]
    Phrygian,
    #[name = "Lydian"]
    Lydian,
    #[name = "Mixolydian"]
    Mixolydian,
    #[name = "Pentatonic major"]
    PentMaj,
    #[name = "Pentatonic minor"]
    PentMin,
    #[name = "Blues"]
    Blues,
    #[name = "Whole tone"]
    WholeTone,
}

impl ScaleType {
    /// Semitone offsets (0..12) that belong to the scale.
    pub fn degrees(self) -> &'static [i32] {
        match self {
            ScaleType::Off => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            ScaleType::Major => &[0, 2, 4, 5, 7, 9, 11],
            ScaleType::Minor => &[0, 2, 3, 5, 7, 8, 10],
            ScaleType::HarmMinor => &[0, 2, 3, 5, 7, 8, 11],
            ScaleType::MelMinor => &[0, 2, 3, 5, 7, 9, 11],
            ScaleType::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            ScaleType::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            ScaleType::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            ScaleType::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            ScaleType::PentMaj => &[0, 2, 4, 7, 9],
            ScaleType::PentMin => &[0, 3, 5, 7, 10],
            ScaleType::Blues => &[0, 3, 5, 6, 7, 10],
            ScaleType::WholeTone => &[0, 2, 4, 6, 8, 10],
        }
    }

    /// Nearest in-scale note (ties go down).
    pub fn quantize(self, note: u8, root: i32) -> u8 {
        if self == ScaleType::Off {
            return note;
        }
        let deg = self.degrees();
        for d in 0..=6i32 {
            for s in [-d, d] {
                let n = note as i32 + s;
                if (0..=127).contains(&n) && deg.contains(&(n - root).rem_euclid(12)) {
                    return n as u8;
                }
            }
        }
        note
    }
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum Tuning {
    #[name = "Equal (12-TET)"]
    Equal,
    #[name = "Just intonation"]
    Just,
    #[name = "Pythagorean"]
    Pythagorean,
    #[name = "Meantone 1/4"]
    Meantone,
    #[name = "Werckmeister III"]
    Werckmeister,
}

impl Tuning {
    /// Deviation in cents from equal temperament for the 12 scale degrees above the root.
    pub fn cents(self) -> [f32; 12] {
        let t: [f32; 12] = match self {
            Tuning::Equal => return [0.0; 12],
            Tuning::Just => [0.0, 111.73, 203.91, 315.64, 386.31, 498.04, 582.51, 701.96, 813.69, 884.36, 1017.60, 1088.27],
            Tuning::Pythagorean => [0.0, 90.22, 203.91, 294.13, 407.82, 498.04, 611.73, 701.96, 792.18, 905.87, 996.09, 1109.78],
            Tuning::Meantone => [0.0, 76.05, 193.16, 310.26, 386.31, 503.42, 579.47, 696.58, 772.63, 889.74, 1006.84, 1082.89],
            Tuning::Werckmeister => [0.0, 90.225, 192.18, 294.135, 390.225, 498.045, 588.27, 696.09, 792.18, 888.27, 996.09, 1092.18],
        };
        std::array::from_fn(|i| t[i] - 100.0 * i as f32)
    }
}

/// How an oscillator's pitch offset is entered.
#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum OscTune {
    #[name = "Semitone"]
    Semitone,
    #[name = "Harmonic"]
    Harmonic,
    #[name = "Ratio"]
    Ratio,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum OscMode {
    #[name = "Wavetable"]
    Wavetable,
    #[name = "Sample"]
    Sample,
    #[name = "Granular"]
    Granular,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum SampleLoop {
    #[name = "One-shot"]
    Off,
    #[name = "Loop"]
    Loop,
    #[name = "Ping-pong"]
    PingPong,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum UniMode {
    #[name = "Linear"]
    Linear,
    #[name = "Super"]
    Super,
    #[name = "Random"]
    Random,
    #[name = "Stack"]
    Stack,
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum FxFilterType {
    #[name = "LP 12"]
    Lp12,
    #[name = "LP 24"]
    Lp24,
    #[name = "HP 12"]
    Hp12,
    #[name = "HP 24"]
    Hp24,
    #[name = "Band"]
    Band,
    #[name = "Notch"]
    Notch,
    #[name = "Peak"]
    Peak,
}

/// Effect modules (the order of the chain is stored as module indices).
pub const FX_NAMES: [&str; 12] = ["HYPER", "DIST", "FLANGER", "PHASER", "CHORUS", "DELAY", "COMP", "REVERB", "EQ", "FILTER", "BODE", "UTILITY"];

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum VoiceMode {
    #[name = "Poly"]
    Poly,
    #[name = "Mono"]
    Mono,
    #[name = "Legato"]
    Legato,
}

fn pct() -> (Arc<dyn Fn(f32) -> String + Send + Sync>, Arc<dyn Fn(&str) -> Option<f32> + Send + Sync>) {
    (
        Arc::new(|v| format!("{:.0}%", v * 100.0)),
        Arc::new(|s| s.trim().trim_end_matches('%').trim().parse::<f32>().ok().map(|v| v / 100.0)),
    )
}

fn unit_param(name: &str, default: f32) -> FloatParam {
    let (a, b) = pct();
    FloatParam::new(name, default, FloatRange::Linear { min: 0.0, max: 1.0 }).with_value_to_string(a).with_string_to_value(b)
}

fn bi_param(name: &str, default: f32) -> FloatParam {
    let (a, b) = pct();
    FloatParam::new(name, default, FloatRange::Linear { min: -1.0, max: 1.0 }).with_value_to_string(a).with_string_to_value(b)
}

fn time_param(name: &str, default_ms: f32, max_ms: f32) -> FloatParam {
    FloatParam::new(name, default_ms, FloatRange::Skewed { min: 0.5, max: max_ms, factor: FloatRange::skew_factor(-2.0) })
        .with_value_to_string(Arc::new(fmt_ms))
        .with_string_to_value(Arc::new(parse_ms))
}

fn fmt_ms(v: f32) -> String {
    if v >= 1000.0 {
        format!("{:.2} s", v / 1000.0)
    } else if v >= 100.0 {
        format!("{:.0} ms", v)
    } else {
        format!("{:.1} ms", v)
    }
}

fn parse_ms(s: &str) -> Option<f32> {
    let t = s.trim().to_lowercase();
    if let Some(x) = t.strip_suffix("ms") {
        x.trim().parse().ok()
    } else if let Some(x) = t.strip_suffix('s') {
        x.trim().parse::<f32>().ok().map(|v| v * 1000.0)
    } else {
        t.parse().ok()
    }
}

fn ms_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-1.5) })
        .with_value_to_string(Arc::new(fmt_ms))
        .with_string_to_value(Arc::new(parse_ms))
}

fn hz_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-2.0) })
        .with_value_to_string(Arc::new(|v| if v >= 1000.0 { format!("{:.2}k", v / 1000.0) } else { format!("{:.0}", v) }))
        .with_string_to_value(Arc::new(|s| {
            let t = s.trim().to_lowercase();
            let t = t.trim_end_matches("hz").trim().to_string();
            if let Some(x) = t.strip_suffix('k') {
                x.trim().parse::<f32>().ok().map(|v| v * 1000.0)
            } else {
                t.parse().ok()
            }
        }))
}

fn rate_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Skewed { min, max, factor: FloatRange::skew_factor(-1.5) })
        .with_unit(" Hz")
        .with_value_to_string(formatters::v2s_f32_rounded(2))
}

fn db_param(name: &str, default: f32, min: f32, max: f32) -> FloatParam {
    FloatParam::new(name, default, FloatRange::Linear { min, max })
        .with_unit(" dB")
        .with_step_size(0.1)
        .with_value_to_string(formatters::v2s_f32_rounded(1))
}

/// One complete filter (the synth has two).
#[derive(Params)]
pub struct FilterParams {
    #[id = "fl_on"]
    pub fl_on: BoolParam,
    #[id = "fl_ty"]
    pub fl_type: EnumParam<FilterType>,
    #[id = "fl_cut"]
    pub cutoff: FloatParam,
    #[id = "fl_res"]
    pub res: FloatParam,
    #[id = "fl_drv"]
    pub drive: FloatParam,
    #[id = "fl_kt"]
    pub keytrack: FloatParam,
    #[id = "fl_mix"]
    pub fl_mix: FloatParam,
    #[id = "fl_a"]
    pub fl_a: BoolParam,
    #[id = "fl_b"]
    pub fl_b: BoolParam,
    #[id = "fl_c"]
    pub fl_c: BoolParam,
    #[id = "fl_s"]
    pub fl_sub: BoolParam,
    #[id = "fl_n"]
    pub fl_noise: BoolParam,
    #[nested(array, group = "Filter Points")]
    pub cb: [CBand; CBANDS],
}

impl FilterParams {
    pub fn new(on: bool) -> Self {
        Self {
            fl_on: BoolParam::new("Filter On", on),
            fl_type: EnumParam::new("Filter Type", FilterType::Lp24),
            cutoff: FloatRange::Skewed { min: 20.0, max: 20000.0, factor: FloatRange::skew_factor(-2.2) }.pipe(|r| {
                FloatParam::new("Cutoff", 20000.0, r).with_unit(" Hz").with_value_to_string(formatters::v2s_f32_rounded(0))
            }),
            res: unit_param("Resonance", 0.1),
            drive: unit_param("Drive", 0.0),
            keytrack: unit_param("Key Track", 0.0),
            fl_mix: unit_param("Filter Mix", 1.0),
            fl_a: BoolParam::new("Filter A", true),
            fl_b: BoolParam::new("Filter B", true),
            fl_c: BoolParam::new("Filter C", true),
            fl_sub: BoolParam::new("Filter Sub", true),
            fl_noise: BoolParam::new("Filter Noise", true),
            cb: std::array::from_fn(CBand::new),
        }
    }
}

#[derive(Params)]
pub struct FxBank {
    // ---- effect order (slot -> module index, see FX_NAMES)
    #[id = "fo_1"]
    pub fo1: IntParam,
    #[id = "fo_2"]
    pub fo2: IntParam,
    #[id = "fo_3"]
    pub fo3: IntParam,
    #[id = "fo_4"]
    pub fo4: IntParam,
    #[id = "fo_5"]
    pub fo5: IntParam,
    #[id = "fo_6"]
    pub fo6: IntParam,
    #[id = "fo_7"]
    pub fo7: IntParam,
    #[id = "fo_8"]
    pub fo8: IntParam,
    #[id = "fo_9"]
    pub fo9: IntParam,
    #[id = "fo_10"]
    pub fo10: IntParam,
    #[id = "fo_11"]
    pub fo11: IntParam,
    #[id = "fo_12"]
    pub fo12: IntParam,

    // ---- hyper / dimension
    #[id = "hy_on"]
    pub hy_on: BoolParam,
    #[id = "hy_rt"]
    pub hy_rate: FloatParam,
    #[id = "hy_dt"]
    pub hy_detune: FloatParam,
    #[id = "hy_un"]
    pub hy_uni: IntParam,
    #[id = "hy_dm"]
    pub hy_dim: FloatParam,
    #[id = "hy_mx"]
    pub hy_mix: FloatParam,

    // ---- distortion
    #[id = "ds_on"]
    pub ds_on: BoolParam,
    #[id = "ds_ty"]
    pub ds_type: EnumParam<DistType>,
    #[id = "ds_dr"]
    pub ds_drive: FloatParam,
    #[id = "ds_tn"]
    pub ds_tone: FloatParam,
    #[id = "ds_mx"]
    pub ds_mix: FloatParam,
    #[id = "ds_bs"]
    pub ds_bias: FloatParam,
    #[id = "rv_ty"]
    pub rv_type: EnumParam<ReverbType>,
    #[id = "dl_hq"]
    pub dl_hq: BoolParam,

    // ---- flanger
    #[id = "fg_on"]
    pub fg_on: BoolParam,
    #[id = "fg_rt"]
    pub fg_rate: FloatParam,
    #[id = "fg_sy"]
    pub fg_sync: BoolParam,
    #[id = "fg_dv"]
    pub fg_div: EnumParam<Div>,
    #[id = "fg_dp"]
    pub fg_depth: FloatParam,
    #[id = "fg_dl"]
    pub fg_delay: FloatParam,
    #[id = "fg_fb"]
    pub fg_fb: FloatParam,
    #[id = "fg_ph"]
    pub fg_phase: FloatParam,
    #[id = "fg_mx"]
    pub fg_mix: FloatParam,

    // ---- phaser
    #[id = "ph_on"]
    pub ph_on: BoolParam,
    #[id = "ph_rt"]
    pub ph_rate: FloatParam,
    #[id = "ph_sy"]
    pub ph_sync: BoolParam,
    #[id = "ph_dv"]
    pub ph_div: EnumParam<Div>,
    #[id = "ph_dp"]
    pub ph_depth: FloatParam,
    #[id = "ph_fr"]
    pub ph_freq: FloatParam,
    #[id = "ph_fb"]
    pub ph_fb: FloatParam,
    #[id = "ph_st"]
    pub ph_stages: IntParam,
    #[id = "ph_pa"]
    pub ph_phase: FloatParam,
    #[id = "ph_mx"]
    pub ph_mix: FloatParam,

    // ---- chorus
    #[id = "ch_on"]
    pub ch_on: BoolParam,
    #[id = "ch_rt"]
    pub ch_rate: FloatParam,
    #[id = "ch_sy"]
    pub ch_sync: BoolParam,
    #[id = "ch_dv"]
    pub ch_div: EnumParam<Div>,
    #[id = "ch_dp"]
    pub ch_depth: FloatParam,
    #[id = "ch_dl"]
    pub ch_delay: FloatParam,
    #[id = "ch_fb"]
    pub ch_fb: FloatParam,
    #[id = "ch_fc"]
    pub ch_filter: FloatParam,
    #[id = "ch_mx"]
    pub ch_mix: FloatParam,

    // ---- delay
    #[id = "dl_on"]
    pub dl_on: BoolParam,
    #[id = "dl_tm"]
    pub dl_time: FloatParam,
    #[id = "dl_tr"]
    pub dl_time_r: FloatParam,
    #[id = "dl_lk"]
    pub dl_link: BoolParam,
    #[id = "dl_sy"]
    pub dl_sync: BoolParam,
    #[id = "dl_dv"]
    pub dl_div: EnumParam<Div>,
    #[id = "dl_dr"]
    pub dl_div_r: EnumParam<Div>,
    #[id = "dl_fb"]
    pub dl_fb: FloatParam,
    #[id = "dl_pp"]
    pub dl_pp: BoolParam,
    #[id = "dl_lp"]
    pub dl_lp: FloatParam,
    #[id = "dl_hp"]
    pub dl_hp: FloatParam,
    #[id = "dl_wd"]
    pub dl_width: FloatParam,
    #[id = "dl_mx"]
    pub dl_mix: FloatParam,

    // ---- compressor
    #[id = "cp_on"]
    pub cp_on: BoolParam,
    #[id = "cp_th"]
    pub cp_thresh: FloatParam,
    #[id = "cp_rt"]
    pub cp_ratio: FloatParam,
    #[id = "cp_at"]
    pub cp_attack: FloatParam,
    #[id = "cp_rl"]
    pub cp_release: FloatParam,
    #[id = "cp_gn"]
    pub cp_gain: FloatParam,
    #[id = "cp_mb"]
    pub cp_multi: BoolParam,
    #[id = "cp_f1"]
    pub cp_f1: FloatParam,
    #[id = "cp_f2"]
    pub cp_f2: FloatParam,
    #[id = "cp_mx"]
    pub cp_mix: FloatParam,

    // ---- reverb
    #[id = "rv_on"]
    pub rv_on: BoolParam,
    #[id = "rv_sz"]
    pub rv_size: FloatParam,
    #[id = "rv_dm"]
    pub rv_damp: FloatParam,
    #[id = "rv_pd"]
    pub rv_pre: FloatParam,
    #[id = "rv_lc"]
    pub rv_lowcut: FloatParam,
    #[id = "rv_hc"]
    pub rv_highcut: FloatParam,
    #[id = "rv_wd"]
    pub rv_width: FloatParam,
    #[id = "rv_mx"]
    pub rv_mix: FloatParam,

    // ---- EQ
    #[id = "eq_on"]
    pub eq_on: BoolParam,
    #[id = "eq_lf"]
    pub eq_lf: FloatParam,
    #[id = "eq_lg"]
    pub eq_lg: FloatParam,
    #[id = "eq_mf"]
    pub eq_mf: FloatParam,
    #[id = "eq_mq"]
    pub eq_mq: FloatParam,
    #[id = "eq_mg"]
    pub eq_mg: FloatParam,
    #[id = "eq_hf"]
    pub eq_hf: FloatParam,
    #[id = "eq_hg"]
    pub eq_hg: FloatParam,

    // ---- filter effect
    #[id = "ff_on"]
    pub ff_on: BoolParam,
    #[id = "ff_ty"]
    pub ff_type: EnumParam<FxFilterType>,
    #[id = "ff_fc"]
    pub ff_cut: FloatParam,
    #[id = "ff_q"]
    pub ff_res: FloatParam,
    #[id = "ff_dr"]
    pub ff_drive: FloatParam,
    #[id = "ff_mx"]
    pub ff_mix: FloatParam,

    // ---- Bode frequency shifter
    #[id = "bd_on"]
    pub bd_on: BoolParam,
    #[id = "bd_sh"]
    pub bd_shift: FloatParam,
    #[id = "bd_fb"]
    pub bd_fb: FloatParam,
    #[id = "bd_mx"]
    pub bd_mix: FloatParam,

    // ---- utility
    #[id = "ut_on"]
    pub ut_on: BoolParam,
    #[id = "ut_gn"]
    pub ut_gain: FloatParam,
    #[id = "ut_pn"]
    pub ut_pan: FloatParam,
    #[id = "ut_wd"]
    pub ut_width: FloatParam,
    #[id = "ut_mo"]
    pub ut_mono: BoolParam,
    #[id = "ut_iv"]
    pub ut_invert: BoolParam,
}

impl FxBank {
    pub fn new() -> Self {
        Self {
            fo1: order_param(0),
            fo2: order_param(1),
            fo3: order_param(2),
            fo4: order_param(3),
            fo5: order_param(4),
            fo6: order_param(5),
            fo7: order_param(6),
            fo8: order_param(7),
            fo9: order_param(8),
            fo10: order_param(9),
            fo11: order_param(10),
            fo12: order_param(11),

            hy_on: BoolParam::new("Hyper On", false),
            hy_rate: rate_param("Hyper Rate", 0.6, 0.05, 8.0),
            hy_detune: unit_param("Hyper Detune", 0.5),
            hy_uni: IntParam::new("Hyper Unison", 4, IntRange::Linear { min: 2, max: 8 }),
            hy_dim: unit_param("Dimension", 0.4),
            hy_mix: unit_param("Hyper Mix", 0.5),

            ds_on: BoolParam::new("Dist On", false),
            ds_type: EnumParam::new("Dist Type", DistType::Soft),
            ds_drive: unit_param("Dist Drive", 0.3),
            ds_tone: hz_param("Dist Tone", 20000.0, 200.0, 20000.0),
            ds_mix: unit_param("Dist Mix", 1.0),
            ds_bias: FloatParam::new("Dist DC Bias", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }).with_value_to_string(formatters::v2s_f32_rounded(2)),
            rv_type: EnumParam::new("Reverb Type", ReverbType::Plate),
            dl_hq: BoolParam::new("Delay HQ", false),

            fg_on: BoolParam::new("Flanger On", false),
            fg_rate: rate_param("Flanger Rate", 0.3, 0.02, 8.0),
            fg_sync: BoolParam::new("Flanger Sync", false),
            fg_div: EnumParam::new("Flanger Division", Div::B1),
            fg_depth: unit_param("Flanger Depth", 0.6),
            fg_delay: FloatParam::new("Flanger Delay", 2.0, FloatRange::Linear { min: 0.1, max: 10.0 }).with_unit(" ms").with_value_to_string(formatters::v2s_f32_rounded(1)),
            fg_fb: bi_param("Flanger Feedback", 0.5),
            fg_phase: unit_param("Flanger Phase", 0.25),
            fg_mix: unit_param("Flanger Mix", 0.5),

            ph_on: BoolParam::new("Phaser On", false),
            ph_rate: rate_param("Phaser Rate", 0.4, 0.02, 8.0),
            ph_sync: BoolParam::new("Phaser Sync", false),
            ph_div: EnumParam::new("Phaser Division", Div::B1),
            ph_depth: unit_param("Phaser Depth", 0.7),
            ph_freq: hz_param("Phaser Frequency", 900.0, 100.0, 8000.0),
            ph_fb: unit_param("Phaser Feedback", 0.4),
            ph_stages: IntParam::new("Phaser Stages", 6, IntRange::Linear { min: 2, max: 12 }),
            ph_phase: unit_param("Phaser Phase", 0.25),
            ph_mix: unit_param("Phaser Mix", 0.5),

            ch_on: BoolParam::new("Chorus On", false),
            ch_rate: FloatParam::new("Chorus Rate", 0.6, FloatRange::Skewed { min: 0.05, max: 5.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            ch_sync: BoolParam::new("Chorus Sync", false),
            ch_div: EnumParam::new("Chorus Division", Div::B1),
            ch_depth: unit_param("Chorus Depth", 0.5),
            ch_delay: FloatParam::new("Chorus Delay", 10.0, FloatRange::Linear { min: 2.0, max: 30.0 }).with_unit(" ms").with_value_to_string(formatters::v2s_f32_rounded(1)),
            ch_fb: unit_param("Chorus Feedback", 0.0),
            ch_filter: hz_param("Chorus Filter", 20000.0, 400.0, 20000.0),
            ch_mix: unit_param("Chorus Mix", 0.4),

            dl_on: BoolParam::new("Delay On", false),
            dl_time: FloatParam::new("Delay Time", 350.0, FloatRange::Skewed { min: 10.0, max: 1500.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms")
                .with_value_to_string(formatters::v2s_f32_rounded(0)),
            dl_time_r: FloatParam::new("Delay Time R", 350.0, FloatRange::Skewed { min: 10.0, max: 1500.0, factor: FloatRange::skew_factor(-1.5) })
                .with_unit(" ms")
                .with_value_to_string(formatters::v2s_f32_rounded(0)),
            dl_link: BoolParam::new("Delay Link", true),
            dl_sync: BoolParam::new("Delay Sync", false),
            dl_div: EnumParam::new("Delay Division", Div::D8),
            dl_div_r: EnumParam::new("Delay Division R", Div::D8),
            dl_fb: unit_param("Delay Feedback", 0.35),
            dl_pp: BoolParam::new("Ping-Pong", true),
            dl_lp: hz_param("Delay Low-pass", 8000.0, 500.0, 20000.0),
            dl_hp: hz_param("Delay High-pass", 20.0, 20.0, 2000.0),
            dl_width: unit_param("Delay Width", 1.0),
            dl_mix: unit_param("Delay Mix", 0.3),

            cp_on: BoolParam::new("Comp On", false),
            cp_thresh: db_param("Comp Threshold", -18.0, -60.0, 0.0),
            cp_ratio: FloatParam::new("Comp Ratio", 4.0, FloatRange::Skewed { min: 1.0, max: 20.0, factor: FloatRange::skew_factor(-1.5) }).with_value_to_string(Arc::new(|v| format!("{:.1}:1", v))),
            cp_attack: ms_param("Comp Attack", 5.0, 0.1, 200.0),
            cp_release: ms_param("Comp Release", 120.0, 5.0, 1000.0),
            cp_gain: db_param("Comp Gain", 0.0, 0.0, 24.0),
            cp_multi: BoolParam::new("Comp Multiband", false),
            cp_f1: hz_param("Comp Split 1", 150.0, 60.0, 500.0),
            cp_f2: hz_param("Comp Split 2", 2500.0, 800.0, 8000.0),
            cp_mix: unit_param("Comp Mix", 1.0),

            rv_on: BoolParam::new("Reverb On", false),
            rv_size: unit_param("Reverb Size", 0.6),
            rv_damp: unit_param("Reverb Damp", 0.5),
            rv_pre: ms_param("Reverb Pre-delay", 0.5, 0.5, 250.0),
            rv_lowcut: hz_param("Reverb Low Cut", 20.0, 20.0, 1000.0),
            rv_highcut: hz_param("Reverb High Cut", 20000.0, 1000.0, 20000.0),
            rv_width: unit_param("Reverb Width", 1.0),
            rv_mix: unit_param("Reverb Mix", 0.25),

            eq_on: BoolParam::new("EQ On", false),
            eq_lf: hz_param("EQ Low Freq", 120.0, 20.0, 1000.0),
            eq_lg: db_param("EQ Low Gain", 0.0, -18.0, 18.0),
            eq_mf: hz_param("EQ Mid Freq", 1000.0, 100.0, 10000.0),
            eq_mq: FloatParam::new("EQ Mid Q", 1.0, FloatRange::Skewed { min: 0.2, max: 10.0, factor: FloatRange::skew_factor(-1.0) }).with_value_to_string(formatters::v2s_f32_rounded(2)),
            eq_mg: db_param("EQ Mid Gain", 0.0, -18.0, 18.0),
            eq_hf: hz_param("EQ High Freq", 6000.0, 1000.0, 20000.0),
            eq_hg: db_param("EQ High Gain", 0.0, -18.0, 18.0),

            ff_on: BoolParam::new("Filter FX On", false),
            ff_type: EnumParam::new("Filter FX Type", FxFilterType::Lp12),
            ff_cut: hz_param("Filter FX Cutoff", 2000.0, 20.0, 20000.0),
            ff_res: unit_param("Filter FX Resonance", 0.2),
            ff_drive: unit_param("Filter FX Drive", 0.0),
            ff_mix: unit_param("Filter FX Mix", 1.0),
            bd_on: BoolParam::new("Bode On", false),
            bd_shift: FloatParam::new("Bode Shift", 0.0, FloatRange::SymmetricalSkewed { min: -4000.0, max: 4000.0, factor: FloatRange::skew_factor(-2.0), center: 0.0 })
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            bd_fb: unit_param("Bode Feedback", 0.0),
            bd_mix: unit_param("Bode Mix", 0.5),
            ut_on: BoolParam::new("Utility On", false),
            ut_gain: db_param("Utility Gain", 0.0, -48.0, 24.0),
            ut_pan: bi_param("Utility Pan", 0.0),
            ut_width: FloatParam::new("Utility Width", 1.0, FloatRange::Linear { min: 0.0, max: 2.0 }).with_value_to_string(Arc::new(|v| format!("{:.0}%", v * 100.0))),
            ut_mono: BoolParam::new("Utility Mono", false),
            ut_invert: BoolParam::new("Utility Invert", false),
        }
    }

    /// The effect-order parameters in slot order.
    pub fn order_params(&self) -> [&IntParam; FX_SLOTS] {
        [&self.fo1, &self.fo2, &self.fo3, &self.fo4, &self.fo5, &self.fo6, &self.fo7, &self.fo8, &self.fo9, &self.fo10, &self.fo11, &self.fo12]
    }
}

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum FxRoute {
    #[name = "Bus 1 > Bus 2"]
    Series,
    #[name = "Parallel"]
    Parallel,
    #[name = "Split Low / High"]
    SplitLh,
    #[name = "Split Mid / Side"]
    SplitMs,
    #[name = "Split Low / Mid / High"]
    SplitLmh,
}

pub const FX_SLOTS: usize = 12;

#[derive(Params)]
pub struct OscParams {
    #[persist = "wave"]
    pub wave: Arc<std::sync::Mutex<String>>,
    #[id = "on"]
    pub on: BoolParam,
    #[id = "tb"]
    pub table: EnumParam<TableId>,
    #[id = "pos"]
    pub pos: FloatParam,
    #[id = "wm"]
    pub warp_mode: EnumParam<WarpMode>,
    #[id = "wa"]
    pub warp: FloatParam,
    #[id = "w2m"]
    pub warp2_mode: EnumParam<WarpMode>,
    #[id = "w2a"]
    pub warp2: FloatParam,
    #[id = "lv"]
    pub level: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "oc"]
    pub oct: IntParam,
    #[id = "st"]
    pub semi: IntParam,
    #[id = "fn"]
    pub fine: FloatParam,
    #[id = "un"]
    pub uni: IntParam,
    #[id = "dt"]
    pub detune: FloatParam,
    #[id = "bl"]
    pub blend: FloatParam,
    #[id = "um"]
    pub uni_mode: EnumParam<UniMode>,
    #[id = "sp"]
    pub spread: FloatParam,
    #[id = "ph"]
    pub phase: FloatParam,
    #[id = "rn"]
    pub rand: FloatParam,
    #[persist = "smp"]
    pub smp: Arc<std::sync::Mutex<String>>,
    #[id = "om"]
    pub mode: EnumParam<OscMode>,
    #[id = "ss"]
    pub s_start: FloatParam,
    #[id = "se"]
    pub s_end: FloatParam,
    #[id = "sl"]
    pub s_loop: EnumParam<SampleLoop>,
    #[id = "sx"]
    pub s_xfade: FloatParam,
    #[id = "sr"]
    pub s_root: IntParam,
    #[id = "gs"]
    pub g_size: FloatParam,
    #[id = "gd"]
    pub g_dens: FloatParam,
    #[id = "gy"]
    pub g_spray: FloatParam,
    #[id = "tm"]
    pub tune_mode: EnumParam<OscTune>,
    #[id = "tr"]
    pub ratio: FloatParam,
    #[id = "th"]
    pub harm: IntParam,
}

impl OscParams {
    fn new(on: bool, level: f32) -> Self {
        Self {
            wave: Arc::new(std::sync::Mutex::new(String::new())),
            on: BoolParam::new("On", on),
            table: EnumParam::new("Table", TableId::Basic),
            pos: unit_param("Position", 0.0),
            warp_mode: EnumParam::new("Warp", WarpMode::Off),
            warp: unit_param("Warp Amount", 0.0),
            warp2_mode: EnumParam::new("Warp 2", WarpMode::Off),
            warp2: unit_param("Warp 2 Amount", 0.0),
            level: unit_param("Level", level),
            pan: FloatParam::new("Pan", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }).with_step_size(0.01),
            oct: IntParam::new("Octave", 0, IntRange::Linear { min: -4, max: 4 }),
            semi: IntParam::new("Semi", 0, IntRange::Linear { min: -12, max: 12 }),
            fine: FloatParam::new("Fine", 0.0, FloatRange::Linear { min: -100.0, max: 100.0 }).with_unit(" ct").with_step_size(0.1),
            uni: IntParam::new("Unison", 1, IntRange::Linear { min: 1, max: 16 }),
            detune: unit_param("Detune", 0.25),
            blend: unit_param("Blend", 0.75),
            uni_mode: EnumParam::new("Unison Mode", UniMode::Linear),
            spread: unit_param("Stereo Spread", 1.0),
            phase: FloatParam::new("Phase", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(|v| format!("{:.0}", v * 360.0)))
                .with_unit(" deg"),
            rand: unit_param("Random", 1.0),
            smp: Arc::new(std::sync::Mutex::new(String::new())),
            mode: EnumParam::new("Mode", OscMode::Wavetable),
            s_start: unit_param("Sample Start", 0.0),
            s_end: unit_param("Sample End", 1.0),
            s_loop: EnumParam::new("Sample Loop", SampleLoop::Loop),
            s_xfade: unit_param("Loop Crossfade", 0.0),
            s_root: IntParam::new("Sample Root", 0, IntRange::Linear { min: -36, max: 36 }),
            g_size: FloatParam::new("Grain Size", 80.0, FloatRange::Skewed { min: 5.0, max: 500.0, factor: FloatRange::skew_factor(-1.0) }).with_unit(" ms").with_step_size(0.1),
            g_dens: FloatParam::new("Grain Density", 30.0, FloatRange::Skewed { min: 1.0, max: 256.0, factor: FloatRange::skew_factor(-1.0) }).with_unit(" /s").with_step_size(0.1),
            g_spray: unit_param("Grain Spray", 0.1),
            tune_mode: EnumParam::new("Tune Mode", OscTune::Semitone),
            ratio: FloatParam::new("Freq Ratio", 1.0, FloatRange::Skewed { min: 0.25, max: 16.0, factor: FloatRange::skew_factor(-1.5) }).with_step_size(0.01).with_value_to_string(formatters::v2s_f32_rounded(2)),
            harm: IntParam::new("Harmonic", 1, IntRange::Linear { min: 1, max: 32 }),
        }
    }
}

#[derive(Params)]
pub struct EnvParams {
    #[id = "de"]
    pub delay: FloatParam,
    #[id = "a"]
    pub attack: FloatParam,
    #[id = "h"]
    pub hold: FloatParam,
    #[id = "d"]
    pub decay: FloatParam,
    #[id = "s"]
    pub sustain: FloatParam,
    #[id = "r"]
    pub release: FloatParam,
    #[id = "ac"]
    pub atk_curve: FloatParam,
    #[id = "dc"]
    pub dec_curve: FloatParam,
    #[id = "rc"]
    pub rel_curve: FloatParam,
    #[id = "il"]
    pub relegato: BoolParam,
}

impl EnvParams {
    fn new(a: f32, d: f32, s: f32, r: f32) -> Self {
        Self {
            delay: time_param("Delay", 0.5, 10000.0),
            attack: time_param("Attack", a, 10000.0),
            hold: time_param("Hold", 0.5, 10000.0),
            decay: time_param("Decay", d, 10000.0),
            sustain: unit_param("Sustain", s),
            release: time_param("Release", r, 10000.0),
            atk_curve: bi_param("Attack Slope", 0.0),
            dec_curve: bi_param("Decay Slope", 0.0),
            rel_curve: bi_param("Release Slope", 0.0),
            relegato: BoolParam::new("Invert Legato", false),
        }
    }
}

#[derive(Params)]
pub struct LfoParams {
    #[persist = "draw"]
    pub draw: Arc<std::sync::Mutex<String>>,
    #[id = "sh"]
    pub shape: EnumParam<LfoShape>,
    #[id = "rt"]
    pub rate: FloatParam,
    #[id = "sy"]
    pub sync: BoolParam,
    #[id = "dv"]
    pub div: EnumParam<Div>,
    #[id = "tg"]
    pub trig: BoolParam,
    #[id = "ri"]
    pub rise: FloatParam,
    #[id = "sm"]
    pub smooth: FloatParam,
    #[id = "ld"]
    pub delay: FloatParam,
    #[id = "os"]
    pub oneshot: BoolParam,
    #[id = "lp"]
    pub phase: FloatParam,
}

impl LfoParams {
    fn new() -> Self {
        Self {
            draw: Arc::new(std::sync::Mutex::new(String::new())),
            shape: EnumParam::new("Shape", LfoShape::Sine),
            rate: FloatParam::new("Rate", 2.0, FloatRange::Skewed { min: 0.02, max: 1000.0, factor: FloatRange::skew_factor(-2.6) })
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            sync: BoolParam::new("Sync", false),
            div: EnumParam::new("Division", Div::D4),
            trig: BoolParam::new("Retrigger", false),
            rise: time_param("Rise", 0.5, 8000.0),
            smooth: unit_param("Smooth", 0.0),
            delay: time_param("Delay", 0.5, 8000.0),
            oneshot: BoolParam::new("One Shot", false),
            phase: unit_param("Start Phase", 0.0),
        }
    }
}

#[derive(Params)]
pub struct ModSlot {
    #[id = "ms"]
    pub src: EnumParam<ModSrc>,
    #[id = "md"]
    pub dst: EnumParam<ModDst>,
    #[id = "ma"]
    pub amount: FloatParam,
    #[id = "mx"]
    pub aux: EnumParam<ModSrc>,
    #[id = "mc"]
    pub curve: FloatParam,
    #[id = "mu"]
    pub uni: BoolParam,
    #[id = "mo"]
    pub on: BoolParam,
}

impl ModSlot {
    fn new() -> Self {
        Self {
            src: EnumParam::new("Source", ModSrc::None),
            dst: EnumParam::new("Destination", ModDst::None),
            amount: FloatParam::new("Amount", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 })
                .with_step_size(0.01)
                .with_value_to_string(Arc::new(|v| format!("{:+.0}", v * 100.0))),
            aux: EnumParam::new("Amount Scaled By", ModSrc::None),
            curve: bi_param("Curve", 0.0),
            uni: BoolParam::new("Unipolar", false),
            on: BoolParam::new("Slot Enabled", true),
        }
    }
}

pub const MOD_SLOTS: usize = 16;

#[derive(Params)]
pub struct SynthParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,
    #[persist = "preset-name"]
    pub preset_name: Arc<std::sync::Mutex<String>>,
    #[persist = "noise-sample"]
    pub noise_sample: Arc<std::sync::Mutex<String>>,
    /// MIDI learn: "cc=param id;cc=param id" (kept with the plugin state, not with presets)
    #[persist = "cc-map"]
    pub cc_map: Arc<std::sync::Mutex<String>>,
    /// keyboard transposition: octaves and semitones (applies to MIDI input and the on-screen keys)
    #[id = "tr_oc"]
    pub kb_octave: IntParam,
    #[id = "tr_st"]
    pub kb_transpose: IntParam,

    #[nested(id_prefix = "a", group = "Osc A")]
    pub a: OscParams,
    #[nested(id_prefix = "b", group = "Osc B")]
    pub b: OscParams,
    #[nested(id_prefix = "c", group = "Osc C")]
    pub c: OscParams,

    #[id = "sub_on"]
    pub sub_on: BoolParam,
    #[id = "sub_wv"]
    pub sub_wave: EnumParam<SubWave>,
    #[id = "sub_oc"]
    pub sub_oct: IntParam,
    #[id = "sub_lv"]
    pub sub_level: FloatParam,
    #[id = "sub_pn"]
    pub sub_pan: FloatParam,
    #[id = "noi_lv"]
    pub noise_level: FloatParam,
    #[id = "noi_ty"]
    pub noise_type: EnumParam<NoiseType>,
    #[id = "noi_pan"]
    pub noise_pan: FloatParam,
    #[id = "noi_pit"]
    pub noise_pitch: FloatParam,
    #[id = "noi_key"]
    pub noise_key: BoolParam,
    #[id = "noi_lp"]
    pub noise_loop: BoolParam,
    #[id = "noi_ph"]
    pub noise_phase: FloatParam,
    #[id = "noi_rn"]
    pub noise_rand: FloatParam,

    #[nested(group = "Filter")]
    pub f1: FilterParams,
    #[nested(id_prefix = "f2", group = "Filter 2")]
    pub f2: FilterParams,
    #[id = "fl_ser"]
    pub fl_serial: BoolParam,

    #[nested(id_prefix = "e1", group = "Env 1 (Amp)")]
    pub e1: EnvParams,
    #[nested(id_prefix = "e2", group = "Env 2")]
    pub e2: EnvParams,
    #[nested(id_prefix = "e3", group = "Env 3")]
    pub e3: EnvParams,
    #[nested(id_prefix = "e4", group = "Env 4")]
    pub e4: EnvParams,

    #[nested(array, group = "LFO")]
    pub lfo: [LfoParams; 8],
    #[nested(array, group = "Mod")]
    pub slot: [ModSlot; MOD_SLOTS],

    #[id = "mc1"]
    pub macro1: FloatParam,
    #[id = "mc2"]
    pub macro2: FloatParam,
    #[id = "mc3"]
    pub macro3: FloatParam,
    #[id = "mc4"]
    pub macro4: FloatParam,
    #[id = "mc5"]
    pub macro5: FloatParam,
    #[id = "mc6"]
    pub macro6: FloatParam,
    #[id = "mc7"]
    pub macro7: FloatParam,
    #[id = "mc8"]
    pub macro8: FloatParam,

    #[id = "vm"]
    pub vmode: EnumParam<VoiceMode>,
    #[id = "gl"]
    pub glide: FloatParam,
    #[id = "pb"]
    pub bend: IntParam,
    #[id = "pc"]
    pub poly: IntParam,
    #[id = "mt"]
    pub tune: FloatParam,
    #[id = "a4hz"]
    pub a4: FloatParam,
    #[id = "tun"]
    pub tuning: EnumParam<Tuning>,
    #[id = "tnr"]
    pub tun_root: IntParam,
    #[id = "vcv"]
    pub vel_curve: FloatParam,
    #[id = "mch"]
    pub midi_ch: IntParam,
    #[id = "mv"]
    pub master: FloatParam,

    // ---- effects: two busses (each a full rack), routing between them
    #[nested(group = "FX")]
    pub fx: FxBank,
    #[nested(id_prefix = "b2", group = "FX Bus 2")]
    pub fx2: FxBank,
    #[id = "fxrt"]
    pub fx_route: EnumParam<FxRoute>,
    #[id = "fxsp"]
    pub fx_split: FloatParam,
    #[id = "fxs2"]
    pub fx_split2: FloatParam,
    #[id = "fxl2"]
    pub fx2_level: FloatParam,

    // ---- arpeggiator
    #[id = "ar_on"]
    pub arp_on: BoolParam,
    #[id = "ar_md"]
    pub arp_mode: EnumParam<ArpMode>,
    #[id = "ar_dv"]
    pub arp_div: EnumParam<Div>,
    #[id = "ar_oc"]
    pub arp_oct: IntParam,
    #[id = "ar_gt"]
    pub arp_gate: FloatParam,
    #[id = "ar_sw"]
    pub arp_swing: FloatParam,
    #[id = "ar_hd"]
    pub arp_hold: BoolParam,
    #[persist = "ir"]
    pub ir: Arc<std::sync::Mutex<String>>,
    #[persist = "arp-pat"]
    pub arp_pat: Arc<std::sync::Mutex<String>>,
    #[persist = "clips"]
    pub clips: Arc<std::sync::Mutex<String>>,
    #[id = "clp_on"]
    pub clip_on: BoolParam,
    #[id = "clp_sl"]
    pub clip_sel: IntParam,
    #[id = "clp_dv"]
    pub clip_div: EnumParam<Div>,
    #[id = "clp_sw"]
    pub clip_swing: FloatParam,
    #[id = "arp_sl"]
    pub arp_sel: IntParam,
    #[id = "scl"]
    pub key_scale: EnumParam<ScaleType>,
    #[id = "scr"]
    pub key_root: IntParam,
}

impl SynthParams {
    pub fn bank(&self, bus: usize) -> &FxBank {
        if bus == 0 {
            &self.fx
        } else {
            &self.fx2
        }
    }

    pub fn macros(&self) -> [&FloatParam; 8] {
        [&self.macro1, &self.macro2, &self.macro3, &self.macro4, &self.macro5, &self.macro6, &self.macro7, &self.macro8]
    }
}

fn order_param(i: i32) -> IntParam {
    IntParam::new(format!("FX Slot {}", i + 1), i, IntRange::Linear { min: 0, max: 11 }).non_automatable()
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1458, 902),
            preset_name: Arc::new(std::sync::Mutex::new("Init".to_string())),
            noise_sample: Arc::new(std::sync::Mutex::new(String::new())),
            cc_map: Arc::new(std::sync::Mutex::new(String::new())),
            kb_octave: IntParam::new("Octave", 0, IntRange::Linear { min: -4, max: 4 }),
            kb_transpose: IntParam::new("Transpose", 0, IntRange::Linear { min: -24, max: 24 }),
            a: OscParams::new(true, 0.75),
            b: OscParams::new(false, 0.75),
            c: OscParams::new(false, 0.75),
            sub_on: BoolParam::new("Sub On", false),
            sub_wave: EnumParam::new("Sub Wave", SubWave::Sine),
            sub_oct: IntParam::new("Sub Octave", -1, IntRange::Linear { min: -3, max: 0 }),
            sub_level: unit_param("Sub Level", 0.5),
            sub_pan: bi_param("Sub Pan", 0.0),
            noise_level: unit_param("Noise Level", 0.0),
            noise_type: EnumParam::new("Noise Type", NoiseType::White),
            noise_pan: FloatParam::new("Noise Pan", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }).with_step_size(0.01),
            noise_pitch: FloatParam::new("Noise Pitch", 0.0, FloatRange::Linear { min: -48.0, max: 48.0 }).with_step_size(0.1).with_unit(" st"),
            noise_key: BoolParam::new("Noise Key Track", false),
            noise_loop: BoolParam::new("Noise Loop", true),
            noise_phase: unit_param("Noise Phase", 0.0),
            noise_rand: unit_param("Noise Random", 0.0),
            f1: FilterParams::new(true),
            f2: FilterParams::new(false),
            fl_serial: BoolParam::new("Filters In Series", false),
            e1: EnvParams::new(1.0, 400.0, 0.8, 120.0),
            e2: EnvParams::new(1.0, 300.0, 0.0, 200.0),
            e3: EnvParams::new(1.0, 300.0, 0.0, 200.0),
            e4: EnvParams::new(1.0, 300.0, 0.0, 200.0),
            lfo: std::array::from_fn(|_| LfoParams::new()),
            slot: std::array::from_fn(|_| ModSlot::new()),
            macro1: unit_param("Macro 1", 0.0),
            macro2: unit_param("Macro 2", 0.0),
            macro3: unit_param("Macro 3", 0.0),
            macro4: unit_param("Macro 4", 0.0),
            macro5: unit_param("Macro 5", 0.0),
            macro6: unit_param("Macro 6", 0.0),
            macro7: unit_param("Macro 7", 0.0),
            macro8: unit_param("Macro 8", 0.0),
            vmode: EnumParam::new("Voice Mode", VoiceMode::Poly),
            glide: FloatParam::new("Glide", 0.5, FloatRange::Skewed { min: 0.5, max: 2000.0, factor: FloatRange::skew_factor(-2.5) })
                .with_unit(" ms")
                .with_value_to_string(formatters::v2s_f32_rounded(0)),
            bend: IntParam::new("Bend Range", 2, IntRange::Linear { min: 0, max: 24 }),
            poly: IntParam::new("Voices", 16, IntRange::Linear { min: 1, max: 16 }),
            tune: FloatParam::new("Master Tune", 0.0, FloatRange::Linear { min: -100.0, max: 100.0 }).with_unit(" ct").with_step_size(0.1),
            a4: FloatParam::new("Tuning A4", 440.0, FloatRange::Linear { min: 400.0, max: 480.0 }).with_unit(" Hz").with_step_size(0.1),
            tuning: EnumParam::new("Scale Tuning", Tuning::Equal),
            tun_root: IntParam::new("Tuning Root", 0, IntRange::Linear { min: 0, max: 11 }),
            vel_curve: FloatParam::new("Velocity Curve", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }).with_step_size(0.01).with_value_to_string(Arc::new(|v| format!("{:+.0}%", v * 100.0))),
            midi_ch: IntParam::new("MIDI Channel", 0, IntRange::Linear { min: 0, max: 16 }).with_value_to_string(Arc::new(|v| if v == 0 { "Omni".to_string() } else { format!("Ch {v}") })),
            master: FloatParam::new("Master", -6.0, FloatRange::Linear { min: -60.0, max: 6.0 })
                .with_unit(" dB")
                .with_step_size(0.1)
                .with_value_to_string(formatters::v2s_f32_rounded(1)),
            fx: FxBank::new(),
            fx2: FxBank::new(),
            fx_route: EnumParam::new("FX Routing", FxRoute::Series),
            fx_split: hz_param("FX Split Freq", 400.0, 40.0, 12000.0),
            fx_split2: hz_param("FX Split Freq 2", 3000.0, 100.0, 16000.0),
            fx2_level: unit_param("FX Bus 2 Level", 1.0),

            arp_on: BoolParam::new("Arp On", false),
            arp_mode: EnumParam::new("Arp Mode", ArpMode::Up),
            arp_div: EnumParam::new("Arp Rate", Div::D16),
            arp_oct: IntParam::new("Arp Octaves", 1, IntRange::Linear { min: 1, max: 4 }),
            arp_gate: FloatParam::new("Arp Gate", 0.6, FloatRange::Linear { min: 0.05, max: 1.0 }).with_step_size(0.01).with_value_to_string(Arc::new(|v| format!("{:.0}%", v * 100.0))),
            arp_swing: unit_param("Arp Swing", 0.0),
            arp_hold: BoolParam::new("Arp Hold", false),
            ir: Arc::new(std::sync::Mutex::new(String::new())),
            arp_pat: Arc::new(std::sync::Mutex::new(String::new())),
            clips: Arc::new(std::sync::Mutex::new(String::new())),
            clip_on: BoolParam::new("Clip On", false),
            clip_sel: IntParam::new("Clip Slot", 1, IntRange::Linear { min: 1, max: 12 }),
            clip_div: EnumParam::new("Clip Step", Div::D16),
            clip_swing: unit_param("Clip Swing", 0.0),
            arp_sel: IntParam::new("Arp Pattern Slot", 1, IntRange::Linear { min: 1, max: 12 }),
            key_scale: EnumParam::new("Key Lock Scale", ScaleType::Off),
            key_root: IntParam::new("Key Lock Root", 0, IntRange::Linear { min: 0, max: 11 }),
        }
    }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}

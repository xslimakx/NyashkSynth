//! NyashkSynth: wavetable synthesizer (VST3 instrument).

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub mod arp;
pub mod clip;
pub mod spectrum;
pub mod engine;
mod editor;
mod browse;
mod fxui;
mod wavedit;
pub mod fx;
pub mod params;
pub mod presets;
mod ui;
pub mod files;
pub mod formula;
pub mod sample;
pub mod shared;
pub mod wav;
pub mod wavetable;

use engine::{Engine, Patch};
use fx::{FxPatch, FxRack};
pub use params::*;

pub const SCOPE_LEN: usize = 1024;

pub struct Meters {
    pub peak: AtomicU32,
    pub voices: AtomicU32,
    /// decimated copy of the output for the oscilloscope
    pub scope: Vec<AtomicU32>,
    pub scope_w: AtomicU32,
    /// full-rate mono copy of the last output samples for the spectrum analyser
    pub spec: Vec<AtomicU32>,
    pub spec_w: AtomicU32,
    pub fs: AtomicU32,
    /// keys pressed with the mouse on the on-screen keyboard
    pub gui_keys: Vec<std::sync::atomic::AtomicBool>,
    /// incremented by every GUI frame (lets the audio thread release mouse keys if the GUI vanishes)
    pub gui_beat: AtomicU32,
    /// set by the GUI "panic" click: all notes off
    pub panic: std::sync::atomic::AtomicBool,
    /// notes currently sounding (bit mask, low / high 64 notes)
    pub sounding: [std::sync::atomic::AtomicU64; 2],
    /// current value of every modulation source (GUI animation)
    pub live: Vec<AtomicU32>,
    /// MIDI learn: controller -> parameter pointers, applied on the audio thread
    pub cc_map: std::sync::Mutex<Vec<(u8, nih_plug::params::internals::ParamPtr)>>,
    /// last controller number that arrived: low byte = cc, upper bits = running counter
    pub last_cc: AtomicU32,
    /// on-screen wheels: value bits (f32), written by the GUI when the user touches them
    pub gui_bend: AtomicU32,
    pub gui_mod: AtomicU32,
    /// positions of the wheels as the engine sees them (MIDI or GUI), for drawing
    pub bend_val: AtomicU32,
    pub mod_val: AtomicU32,
}

impl Meters {
    fn new() -> Self {
        Self {
            peak: AtomicU32::new(0),
            voices: AtomicU32::new(0),
            scope: (0..SCOPE_LEN).map(|_| AtomicU32::new(0)).collect(),
            scope_w: AtomicU32::new(0),
            spec: (0..spectrum::SPEC_N).map(|_| AtomicU32::new(0)).collect(),
            spec_w: AtomicU32::new(0),
            fs: AtomicU32::new(48000),
            gui_keys: (0..128).map(|_| std::sync::atomic::AtomicBool::new(false)).collect(),
            gui_beat: AtomicU32::new(0),
            panic: std::sync::atomic::AtomicBool::new(false),
            sounding: [std::sync::atomic::AtomicU64::new(0), std::sync::atomic::AtomicU64::new(0)],
            live: (0..N_SRC).map(|_| AtomicU32::new(0)).collect(),
            cc_map: std::sync::Mutex::new(Vec::new()),
            last_cc: AtomicU32::new(0),
            gui_bend: AtomicU32::new(f32::NAN.to_bits()),
            gui_mod: AtomicU32::new(f32::NAN.to_bits()),
            bend_val: AtomicU32::new(0),
            mod_val: AtomicU32::new(0),
        }
    }
    /// Latest scope samples, oldest first.
    /// Latest output samples for the spectrum analyser, oldest first.
    pub fn spec_snapshot(&self) -> Vec<f32> {
        let w = self.spec_w.load(Ordering::Relaxed) as usize % spectrum::SPEC_N;
        (0..spectrum::SPEC_N).map(|i| f32::from_bits(self.spec[(w + i) % spectrum::SPEC_N].load(Ordering::Relaxed))).collect()
    }

    pub fn scope_snapshot(&self) -> Vec<f32> {
        let w = self.scope_w.load(Ordering::Relaxed) as usize % SCOPE_LEN;
        (0..SCOPE_LEN).map(|i| f32::from_bits(self.scope[(w + i) % SCOPE_LEN].load(Ordering::Relaxed))).collect()
    }
    pub fn live_sources(&self) -> [f32; N_SRC] {
        let mut o = [0.0f32; N_SRC];
        for (i, x) in self.live.iter().enumerate().take(N_SRC) {
            o[i] = f32::from_bits(x.load(Ordering::Relaxed));
        }
        o
    }
    pub fn is_sounding(&self, note: u8) -> bool {
        let n = note as usize;
        n < 128 && (self.sounding[n / 64].load(Ordering::Relaxed) >> (n % 64)) & 1 == 1
    }
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak.load(Ordering::Relaxed))
    }
    pub fn voices(&self) -> u32 {
        self.voices.load(Ordering::Relaxed)
    }
}

pub struct NyashkSynth {
    params: Arc<SynthParams>,
    meters: Arc<Meters>,
    shared: Arc<shared::Shared>,
    engine: Engine,
    fx: FxRack,
    fs: f32,
    peak_hold: f32,
    fx_mod_s: [f32; 12],
    gui_prev: [bool; 128],
    tmap: [u8; 128],
    cc_count: u32,
    gui_bend_prev: u32,
    gui_mod_prev: u32,
    beat_last: u32,
    beat_age: usize,
    dec_n: u32,
    dec_acc: f32,
    spec_pos: usize,
}

impl Default for NyashkSynth {
    fn default() -> Self {
        let params = Arc::new(SynthParams::default());
        let patch = Patch::read(&params, 44100.0);
        Self { engine: Engine::new(44100.0, patch), fx: FxRack::new(44100.0), params, meters: Arc::new(Meters::new()), shared: Arc::new(shared::Shared::new()), fs: 44100.0, peak_hold: 0.0, fx_mod_s: [0.0; 12], gui_prev: [false; 128], tmap: std::array::from_fn(|i| i as u8), cc_count: 1, gui_bend_prev: f32::NAN.to_bits(), gui_mod_prev: f32::NAN.to_bits(), beat_last: 0, beat_age: 0, dec_n: 0, dec_acc: 0.0, spec_pos: 0 }
    }
}

/// Snapshot of all effect parameters (also used by offline tools).
pub fn fx_patch_from_bank(p: &params::FxBank, bpm: f32) -> FxPatch {
    {
        let beat_ms = 60000.0 / bpm.max(20.0);
        let rate = |hz: f32, sync: bool, div: Div| if sync { bpm.max(20.0) / 60.0 / div.beats() } else { hz };
        let dl_l = if p.dl_sync.value() { beat_ms * p.dl_div.value().beats() } else { p.dl_time.value() };
        let dl_r = if p.dl_link.value() {
            dl_l
        } else if p.dl_sync.value() {
            beat_ms * p.dl_div_r.value().beats()
        } else {
            p.dl_time_r.value()
        };
        let mut order = [0usize; FX_SLOTS];
        for (i, o) in p.order_params().iter().enumerate() {
            order[i] = o.value().clamp(0, 11) as usize;
        }
        FxPatch {
            order,
            on: [p.hy_on.value(), p.ds_on.value(), p.fg_on.value(), p.ph_on.value(), p.ch_on.value(), p.dl_on.value(), p.cp_on.value(), p.rv_on.value(), p.eq_on.value(), p.ff_on.value(), p.bd_on.value(), p.ut_on.value()],
            hy_rate: p.hy_rate.value(),
            hy_detune: p.hy_detune.value(),
            hy_uni: p.hy_uni.value() as usize,
            hy_dim: p.hy_dim.value(),
            hy_mix: p.hy_mix.value(),
            ds_type: p.ds_type.value(),
            ds_drive: p.ds_drive.value(),
            ds_tone: p.ds_tone.value(),
            ds_mix: p.ds_mix.value(),
            ds_bias: p.ds_bias.value(),
            rv_type: p.rv_type.value(),
            dl_hq: p.dl_hq.value(),
            fg_rate: rate(p.fg_rate.value(), p.fg_sync.value(), p.fg_div.value()),
            fg_depth: p.fg_depth.value(),
            fg_delay: p.fg_delay.value(),
            fg_fb: p.fg_fb.value(),
            fg_phase: p.fg_phase.value(),
            fg_mix: p.fg_mix.value(),
            ph_rate: rate(p.ph_rate.value(), p.ph_sync.value(), p.ph_div.value()),
            ph_depth: p.ph_depth.value(),
            ph_freq: p.ph_freq.value(),
            ph_fb: p.ph_fb.value(),
            ph_stages: p.ph_stages.value() as usize,
            ph_phase: p.ph_phase.value(),
            ph_mix: p.ph_mix.value(),
            ch_rate: rate(p.ch_rate.value(), p.ch_sync.value(), p.ch_div.value()),
            ch_depth: p.ch_depth.value(),
            ch_delay: p.ch_delay.value(),
            ch_fb: p.ch_fb.value(),
            ch_filter: p.ch_filter.value(),
            ch_mix: p.ch_mix.value(),
            dl_ms_l: dl_l,
            dl_ms_r: dl_r,
            dl_fb: p.dl_fb.value(),
            dl_pp: p.dl_pp.value(),
            dl_lp: p.dl_lp.value(),
            dl_hp: p.dl_hp.value(),
            dl_width: p.dl_width.value(),
            dl_mix: p.dl_mix.value(),
            cp_thresh: p.cp_thresh.value(),
            cp_ratio: p.cp_ratio.value(),
            cp_attack: p.cp_attack.value(),
            cp_release: p.cp_release.value(),
            cp_gain: p.cp_gain.value(),
            cp_multi: p.cp_multi.value(),
            cp_f1: p.cp_f1.value(),
            cp_f2: p.cp_f2.value(),
            cp_mix: p.cp_mix.value(),
            rv_size: p.rv_size.value(),
            rv_damp: p.rv_damp.value(),
            rv_pre: p.rv_pre.value(),
            rv_lowcut: p.rv_lowcut.value(),
            rv_highcut: p.rv_highcut.value(),
            rv_width: p.rv_width.value(),
            rv_mix: p.rv_mix.value(),
            eq_lf: p.eq_lf.value(),
            eq_lg: p.eq_lg.value(),
            eq_mf: p.eq_mf.value(),
            eq_mq: p.eq_mq.value(),
            eq_mg: p.eq_mg.value(),
            eq_hf: p.eq_hf.value(),
            eq_hg: p.eq_hg.value(),
            ff_type: p.ff_type.value(),
            ff_cut: p.ff_cut.value(),
            ff_res: p.ff_res.value(),
            ff_drive: p.ff_drive.value(),
            ff_mix: p.ff_mix.value(),
            bd_shift: p.bd_shift.value(),
            bd_fb: p.bd_fb.value(),
            bd_mix: p.bd_mix.value(),
            ut_gain: p.ut_gain.value(),
            ut_pan: p.ut_pan.value(),
            ut_width: p.ut_width.value(),
            ut_mono: p.ut_mono.value(),
            ut_invert: p.ut_invert.value(),
        }
    }
}

pub fn fx_patch_from(p: &SynthParams, bpm: f32) -> FxPatch {
    fx_patch_from_bank(&p.fx, bpm)
}

impl NyashkSynth {
    /// Incoming note + keyboard transposition (octave + semitones).
    fn transposed(&self, note: u8) -> u8 {
        let t = self.params.kb_octave.value() * 12 + self.params.kb_transpose.value();
        let n = (note as i32 + t).clamp(0, 127) as u8;
        self.params.key_scale.value().quantize(n, self.params.key_root.value())
    }
}

/// Rebuild the audio-side controller table from the saved "cc=id;..." string.
pub fn rebuild_cc_map(params: &SynthParams, meters: &Meters) {
    let txt = params.cc_map.lock().map(|g| g.clone()).unwrap_or_default();
    let pm = params.param_map();
    let mut v = Vec::new();
    for part in txt.split(';') {
        let Some((c, id)) = part.split_once('=') else { continue };
        let Ok(c) = c.trim().parse::<u8>() else { continue };
        if let Some((_, ptr, _)) = pm.iter().find(|(pid, _, _)| pid == id.trim()) {
            v.push((c, *ptr));
        }
    }
    if let Ok(mut g) = meters.cc_map.lock() {
        *g = v;
    }
}

impl Plugin for NyashkSynth {
    const NAME: &'static str = "NyashkSynth";
    const VENDOR: &'static str = "Nyashk";
    const URL: &'static str = "https://nyashk.com";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(self.params.clone(), self.meters.clone(), self.shared.clone())
    }

    fn initialize(&mut self, _layout: &AudioIOLayout, cfg: &BufferConfig, _ctx: &mut impl InitContext<Self>) -> bool {
        self.fs = cfg.sample_rate;
        self.meters.fs.store(cfg.sample_rate as u32, Ordering::Relaxed);
        self.shared.sync_from_params(&self.params);
        let patch = Patch::read_with(&self.params, self.fs, Some(&self.shared), None);
        self.engine = Engine::new(self.fs, patch);
        self.fx = FxRack::new(self.fs);
        rebuild_cc_map(&self.params, &self.meters);
        let _ = wavetable::tables();
        true
    }

    fn reset(&mut self) {
        self.engine.reset();
        self.fx.reset();
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, ctx: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let bpm = ctx.transport().tempo.map(|t| t as f32).unwrap_or(120.0);
        self.engine.bpm = bpm;
        let np = Patch::read_with(&self.params, self.fs, Some(&self.shared), Some(&self.engine.patch));
        self.engine.patch = np;
        let fxp = fx_patch_from(&self.params, bpm);
        let fxp2 = fx_patch_from_bank(&self.params.fx2, bpm);
        let (route, split, split2, lvl2) = (self.params.fx_route.value(), self.params.fx_split.value(), self.params.fx_split2.value(), self.params.fx2_level.value());
        let master = util::db_to_gain(self.params.master.value());

        // safety nets against stuck notes: GUI gone / frozen, or the PANIC click
        let beat = self.meters.gui_beat.load(Ordering::Relaxed);
        if beat != self.beat_last {
            self.beat_last = beat;
            self.beat_age = 0;
        } else {
            self.beat_age += buffer.samples();
        }
        let gui_dead = self.beat_age as f32 > self.fs * 0.8;
        if self.meters.panic.swap(false, Ordering::Relaxed) {
            for k in self.meters.gui_keys.iter() {
                k.store(false, Ordering::Relaxed);
            }
            self.engine.all_notes_off();
            self.engine.sound_off();
        }
        for k in 0..128usize {
            if gui_dead && self.meters.gui_keys[k].load(Ordering::Relaxed) {
                self.meters.gui_keys[k].store(false, Ordering::Relaxed);
            }
            let want = self.meters.gui_keys[k].load(Ordering::Relaxed);
            if want != self.gui_prev[k] {
                self.gui_prev[k] = want;
                if want {
                    let t = self.transposed(k as u8);
                    self.tmap[k] = t;
                    self.engine.note_on(t, 0.8);
                } else {
                    self.engine.note_off(self.tmap[k]);
                }
            }
        }
        // on-screen wheels
        let gb = self.meters.gui_bend.load(Ordering::Relaxed);
        if gb != self.gui_bend_prev {
            self.gui_bend_prev = gb;
            let v = f32::from_bits(gb);
            if v.is_finite() {
                self.engine.set_bend(v);
            }
        }
        let gm = self.meters.gui_mod.load(Ordering::Relaxed);
        if gm != self.gui_mod_prev {
            self.gui_mod_prev = gm;
            let v = f32::from_bits(gm);
            if v.is_finite() {
                self.engine.set_modwheel(v);
            }
        }
        let (wb, wm) = self.engine.wheels();
        self.meters.bend_val.store(wb.to_bits(), Ordering::Relaxed);
        self.meters.mod_val.store(wm.to_bits(), Ordering::Relaxed);
        let n = buffer.samples();
        let mut next = ctx.next_event();
        let mut pos = 0usize;
        let out = buffer.as_slice();
        for ch in out.iter_mut() {
            ch.fill(0.0);
        }
        let (a, b) = out.split_at_mut(1);
        let (l_all, r_all) = (&mut a[0], &mut b[0]);
        while pos < n {
            // handle all events at or before `pos`, then render until the next event
            while let Some(ev) = next {
                if ev.timing() as usize > pos {
                    break;
                }
                let mch = self.params.midi_ch.value();
                let ev_ch = match ev {
                    NoteEvent::NoteOn { channel, .. } | NoteEvent::NoteOff { channel, .. } | NoteEvent::MidiPitchBend { channel, .. } | NoteEvent::MidiChannelPressure { channel, .. } | NoteEvent::MidiCC { channel, .. } => Some(channel),
                    _ => None,
                };
                if mch > 0 && ev_ch.map_or(false, |c| c as i32 != mch - 1) {
                    next = ctx.next_event();
                    continue;
                }
                match ev {
                    NoteEvent::NoteOn { note, velocity, .. } => {
                        let t = self.transposed(note);
                        self.tmap[note as usize & 127] = t;
                        self.engine.note_on(t, velocity)
                    }
                    NoteEvent::NoteOff { note, .. } => self.engine.note_off(self.tmap[note as usize & 127]),
                    NoteEvent::MidiPitchBend { value, .. } => self.engine.set_bend(value),
                    NoteEvent::MidiChannelPressure { pressure, .. } => self.engine.set_aftertouch(pressure),
                    NoteEvent::MidiCC { cc, value, .. } => {
                        self.meters.last_cc.store((cc as u32) | ((self.cc_count & 0xFFFFFF) << 8), Ordering::Relaxed);
                        self.cc_count = self.cc_count.wrapping_add(1);
                        if let Ok(map) = self.meters.cc_map.try_lock() {
                            for (c, ptr) in map.iter() {
                                if *c == cc {
                                    // SAFETY: the pointers point into `self.params`, which lives as long as the plugin
                                    unsafe {
                                        ptr.set_normalized_value(value.clamp(0.0, 1.0));
                                    }
                                }
                            }
                        }
                        match cc {
                            1 => self.engine.set_modwheel(value),
                            64 => self.engine.set_sustain(value >= 0.5),
                            120 => self.engine.sound_off(),
                            121 => self.engine.reset_controllers(),
                            123 => self.engine.all_notes_off(),
                            _ => {}
                        }
                    }
                    _ => {}
                }
                next = ctx.next_event();
            }
            let end = next.map(|e| (e.timing() as usize).min(n)).unwrap_or(n).max(pos + 1).min(n);
            self.engine.process(&mut l_all[pos..end], &mut r_all[pos..end]);
            pos = end;
        }

        match self.engine.fx_mod() {
            None => self.fx.process(&fxp, &fxp2, route, split, split2, lvl2, l_all, r_all),
            Some(m) => {
                // effect modulation: update the parameters every 64 samples
                let mut fxp = fxp;
                let mut s = 0;
                while s < n {
                    let e = (s + 64).min(n);
                    for k in 0..12 {
                        self.fx_mod_s[k] += (m[k] - self.fx_mod_s[k]) * 0.35;
                    }
                    let mut cur = fxp.clone();
                    cur.apply_mod(&self.fx_mod_s);
                    self.fx.process(&cur, &fxp2, route, split, split2, lvl2, &mut l_all[s..e], &mut r_all[s..e]);
                    s = e;
                }
                let _ = &mut fxp;
            }
        }

        let mut pk = 0.0f32;
        for i in 0..n {
            let (mut l, mut r) = (l_all[i] * master, r_all[i] * master);
            if !l.is_finite() {
                l = 0.0;
            }
            if !r.is_finite() {
                r = 0.0;
            }
            // gentle safety clip
            l = soft_clip(l);
            r = soft_clip(r);
            l_all[i] = l;
            r_all[i] = r;
            pk = pk.max(l.abs()).max(r.abs());
            {
                let w = self.spec_pos;
                self.meters.spec[w].store((0.5 * (l + r)).to_bits(), Ordering::Relaxed);
                self.spec_pos = (w + 1) % spectrum::SPEC_N;
            }
            self.dec_acc += 0.5 * (l + r);
            self.dec_n += 1;
            if self.dec_n >= 4 {
                let w = self.meters.scope_w.load(Ordering::Relaxed) as usize % SCOPE_LEN;
                self.meters.scope[w].store((self.dec_acc * 0.25).to_bits(), Ordering::Relaxed);
                self.meters.scope_w.store(((w + 1) % SCOPE_LEN) as u32, Ordering::Relaxed);
                self.dec_n = 0;
                self.dec_acc = 0.0;
            }
        }
        self.meters.spec_w.store(self.spec_pos as u32, Ordering::Relaxed);
        let live = self.engine.live_sources();
        for (i, v) in live.iter().enumerate() {
            self.meters.live[i].store(v.to_bits(), Ordering::Relaxed);
        }
        let mask = self.engine.note_mask();
        self.meters.sounding[0].store(mask as u64, Ordering::Relaxed);
        self.meters.sounding[1].store((mask >> 64) as u64, Ordering::Relaxed);
        self.peak_hold = (self.peak_hold * 0.85).max(pk);
        self.meters.peak.store(self.peak_hold.to_bits(), Ordering::Relaxed);
        self.meters.voices.store(self.engine.active_voices() as u32, Ordering::Relaxed);
        ProcessStatus::Normal
    }
}

#[inline]
fn soft_clip(x: f32) -> f32 {
    if x.abs() < 0.85 {
        x
    } else {
        x.signum() * (0.85 + 0.15 * ((x.abs() - 0.85) / 0.15).tanh())
    }
}

impl Vst3Plugin for NyashkSynth {
    const VST3_CLASS_ID: [u8; 16] = *b"NyashkSynthVST3!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_vst3!(NyashkSynth);

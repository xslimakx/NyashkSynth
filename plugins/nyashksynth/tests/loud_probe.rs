//! The whole plugin as a host drives it: initialize, then process blocks with MIDI note events
//! from the host. Catches anything between the host's events and the sound (channel filter,
//! event loop, master, safety stage), which the engine-only tests do not cover.
use nih_plug::prelude::*;
use nyashksynth::NyashkSynth;
use std::collections::VecDeque;

struct Init;
impl InitContext<NyashkSynth> for Init {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Vst3
    }
    fn execute(&self, _task: ()) {}
    fn set_latency_samples(&self, _s: u32) {}
    fn set_current_voice_capacity(&self, _c: u32) {}
}

struct Ctx {
    tr: Transport,
    ev: VecDeque<NoteEvent<()>>,
}
impl ProcessContext<NyashkSynth> for Ctx {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Vst3
    }
    fn execute_background(&self, _t: ()) {}
    fn execute_gui(&self, _t: ()) {}
    fn transport(&self) -> &Transport {
        &self.tr
    }
    fn next_event(&mut self) -> Option<NoteEvent<()>> {
        self.ev.pop_front()
    }
    fn send_event(&mut self, _e: NoteEvent<()>) {}
    fn set_latency_samples(&self, _s: u32) {}
    fn set_current_voice_capacity(&self, _c: u32) {}
}

fn transport(fs: f32) -> Transport {
    // Transport has crate-private fields: start from all-zero (= None / false / 0) and set the public ones
    let mut t: Transport = unsafe { std::mem::zeroed() };
    t.sample_rate = fs;
    t.tempo = Some(120.0);
    t.playing = true;
    t
}

/// Renders `blocks` blocks of `bs` samples; `events(block)` gives the host events of each block.
fn run(p: &mut NyashkSynth, fs: f32, bs: usize, blocks: usize, events: impl Fn(usize) -> Vec<NoteEvent<()>>) -> Vec<f32> {
    let mut out = Vec::new();
    let (mut l, mut r) = (vec![0.0f32; bs], vec![0.0f32; bs]);
    let mut aux = AuxiliaryBuffers { inputs: &mut [], outputs: &mut [] };
    for b in 0..blocks {
        let mut buf = Buffer::default();
        unsafe {
            buf.set_slices(bs, |s| {
                s.clear();
                s.push(&mut l[..]);
                s.push(&mut r[..]);
            });
        }
        let mut ctx = Ctx { tr: transport(fs), ev: events(b).into() };
        p.process(&mut buf, &mut aux, &mut ctx);
        out.extend_from_slice(&l);
    }
    out
}

fn new_plugin(fs: f32, bs: u32) -> NyashkSynth {
    let mut p = NyashkSynth::default();
    let layout = NyashkSynth::AUDIO_IO_LAYOUTS[0];
    let cfg = BufferConfig { sample_rate: fs, min_buffer_size: None, max_buffer_size: bs, process_mode: ProcessMode::Realtime };
    assert!(p.initialize(&layout, &cfg, &mut Init));
    p.reset();
    p
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |a, b| a.max(b.abs()))
}


fn load_preset(p: &NyashkSynth, name: &str) {
    let pr = nyashksynth::presets::PRESETS.iter().find(|x| x.name == name).unwrap();
    let full = nyashksynth::presets::with_rack(pr.values.iter().map(|(k, v)| (k.to_string(), *v)).collect());
    let map = p.params().param_map();
    for (id, v) in &full {
        if let Some((_, ptr, _)) = map.iter().find(|(i, _, _)| i == id) {
            unsafe { ptr.set_normalized_value(ptr.preview_normalized(*v)) };
        }
    }
}

/// Every factory preset played as a 4-note chord at full velocity through the whole plugin:
/// peak level, and flat tops (runs of equal loud samples = something hard-clipping inside).
#[test]
#[ignore]
fn loud_chords() {
    let fs = 48000.0;
    for pr in nyashksynth::presets::PRESETS.iter() {
        if std::env::var("ONLY").is_ok_and(|o| !o.split(',').any(|n| n == pr.name)) {
            continue;
        }
        let mut p = NyashkSynth::default();
        load_preset(&p, pr.name);
        let layout = NyashkSynth::AUDIO_IO_LAYOUTS[0];
        let cfg = BufferConfig { sample_rate: fs, min_buffer_size: None, max_buffer_size: 512, process_mode: ProcessMode::Realtime };
        assert!(p.initialize(&layout, &cfg, &mut Init));
        p.reset();
        let notes = [48u8, 52, 55, 60];
        let out = run(&mut p, fs, 512, 220, |b| match b {
            1 => notes.iter().map(|&n| NoteEvent::NoteOn { timing: 0, voice_id: None, channel: 0, note: n, velocity: 1.0 }).collect(),
            150 => notes.iter().map(|&n| NoteEvent::NoteOff { timing: 0, voice_id: None, channel: 0, note: n, velocity: 0.0 }).collect(),
            _ => vec![],
        });
        let pk = out.iter().fold(0.0f32, |a, b| a.max(b.abs()));
        let over = out.iter().filter(|x| x.abs() > 1.0).count();
        // flat tops: 3+ equal samples above half the peak
        let mut flat = 0;
        for w in out.windows(3) {
            if w[0].abs() > pk * 0.5 && w[0] == w[1] && w[1] == w[2] {
                flat += 1;
            }
        }
        let nan = out.iter().any(|x| !x.is_finite());
        println!("LOUD\t{}\t{:.2}\t{}\t{}\t{}", pr.name, 20.0 * pk.max(1e-9).log10(), over, flat, nan);
    }
}

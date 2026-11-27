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

#[test]
fn host_midi_notes_make_sound_and_stop() {
    for (fs, bs) in [(44100.0f32, 64usize), (48000.0, 512), (96000.0, 1024)] {
        let mut p = new_plugin(fs, bs as u32);
        let blocks = (fs as usize / bs).max(4);
        let on_at = 3;
        let off_at = blocks / 2;
        let out = run(&mut p, fs, bs, blocks * 2, |b| {
            if b == on_at {
                vec![NoteEvent::NoteOn { timing: 5, voice_id: None, channel: 0, note: 48, velocity: 0.9 }]
            } else if b == off_at {
                vec![NoteEvent::NoteOff { timing: 0, voice_id: None, channel: 0, note: 48, velocity: 0.0 }]
            } else {
                vec![]
            }
        });
        let before = peak(&out[..on_at * bs]);
        let during = peak(&out[(on_at + 2) * bs..off_at * bs]);
        let after = peak(&out[out.len() - bs * 4..]);
        assert!(before < 1e-6, "{fs}/{bs}: sound before the note ({before})");
        assert!(during > 0.1, "{fs}/{bs}: host MIDI note is silent (peak {during})");
        assert!(after < 1e-3, "{fs}/{bs}: note does not stop ({after})");
    }
}

#[test]
fn host_midi_on_any_channel_and_chords() {
    let mut p = new_plugin(48000.0, 256);
    for ch in [0u8, 1, 9, 15] {
        let out = run(&mut p, 48000.0, 256, 40, |b| match b {
            0 => (0..4).map(|k| NoteEvent::NoteOn { timing: k * 3, voice_id: None, channel: ch, note: 60 + 4 * k as u8, velocity: 0.8 }).collect(),
            20 => (0..4).map(|k| NoteEvent::NoteOff { timing: 0, voice_id: None, channel: ch, note: 60 + 4 * k as u8, velocity: 0.0 }).collect(),
            _ => vec![],
        });
        assert!(peak(&out[256 * 4..256 * 20]) > 0.1, "channel {ch}: silent");
    }
}

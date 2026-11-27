//! Renders one factory preset (8-note chord) for profiling: `cargo run --release --example bench -- "Preset name" [seconds] [nofx]`
use nih_plug::prelude::*;
use nyashksynth::engine::{Engine, Patch};
use nyashksynth::fx::FxRack;
use nyashksynth::presets::PRESETS;
use nyashksynth::shared::Shared;
use nyashksynth::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let name = a.get(1).cloned().unwrap_or("Trance Gate Chords".into());
    let secs: f32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(3.0);
    let nofx = a.iter().any(|x| x == "nofx");
    let fs = 48000.0f32;
    let pr = PRESETS.iter().find(|p| p.name == name).expect("preset");
    let p = SynthParams::default();
    let map = p.param_map();
    for (id, v) in pr.values {
        if let Some((_, ptr, _)) = map.iter().find(|(i, _, _)| i == id) {
            unsafe { ptr.set_normalized_value(ptr.preview_normalized(*v)) };
        }
    }
    let sh = Shared::new();
    sh.sync_from_params(&p);
    let patch = Patch::read_with(&p, fs, Some(&sh), None);
    let mut eng = Engine::new(fs, patch);
    let c1 = chain_from_params(&p, 0, 120.0);
    let c2 = chain_from_params(&p, 1, 120.0);
    let mut rack = FxRack::new(fs);
    let (route, split, split2, lvl2) = (p.fx_route.value(), p.fx_split.value(), p.fx_split2.value(), p.fx2_level.value());
    for n in [48u8, 52, 55, 59, 60, 64, 67, 72] {
        eng.note_on(n, 0.8);
    }
    let (mut l, mut r) = (vec![0.0f32; 512], vec![0.0f32; 512]);
    let t0 = std::time::Instant::now();
    let blocks = (fs * secs / 512.0) as usize;
    let mut acc = 0.0f32;
    for _ in 0..blocks {
        l.fill(0.0);
        r.fill(0.0);
        eng.process(&mut l, &mut r);
        if !nofx {
            rack.process(&c1, &c2, route, split, split2, lvl2, &mut l, &mut r);
        }
        acc += l[100].abs();
    }
    let el = t0.elapsed().as_secs_f32();
    println!("{name}: {:.1}x realtime ({acc:.1})", secs / el);
}

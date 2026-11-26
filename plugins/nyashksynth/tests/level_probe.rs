use nih_plug::prelude::*;
use nyashksynth::engine::{Engine, Patch};
use nyashksynth::fx::FxRack;
use nyashksynth::presets::PRESETS;
use nyashksynth::shared::Shared;
use nyashksynth::*;

fn load(p: &SynthParams, vals: &[(&str, f32)]) {
    let map = p.param_map();
    for (id, v) in vals {
        if let Some((_, ptr, _)) = map.iter().find(|(i, _, _)| i == id) {
            unsafe { ptr.set_normalized_value(ptr.preview_normalized(*v)) };
        }
    }
}

/// peak (dBFS, before the master) of a preset: a bass note and a middle note, 1.5 s each
pub fn preset_peak(vals: &[(&str, f32)]) -> (f32, f32) {
    let fs = 48000.0f32;
    let p = SynthParams::default();
    load(&p, vals);
    let sh = Shared::new();
    sh.sync_from_params(&p);
    let patch = Patch::read_with(&p, fs, Some(&sh), None);
    let mut eng = Engine::new(fs, patch);
    eng.bpm = 120.0;
    let fx1 = fx_patch_from(&p, 120.0);
    let fx2 = fx_patch_from_bank(&p.fx2, 120.0);
    let mut rack = FxRack::new(fs);
    let (route, split, split2, lvl2) = (p.fx_route.value(), p.fx_split.value(), p.fx_split2.value(), p.fx2_level.value());
    let (mut pk, mut e, mut n) = (0.0f32, 0.0f64, 0usize);
    for note in [36u8, 60] {
        eng.note_on(note, 0.9);
        for blk in 0..(fs as usize * 3 / 2 / 256) {
            let mut l = vec![0.0f32; 256];
            let mut r = vec![0.0f32; 256];
            eng.process(&mut l, &mut r);
            rack.process(&(&fx1).into(), &(&fx2).into(), route, split, split2, lvl2, &mut l, &mut r);
            for i in 0..256 {
                pk = pk.max(l[i].abs()).max(r[i].abs());
                if blk > 10 {
                    e += (l[i] * l[i] + r[i] * r[i]) as f64 * 0.5;
                    n += 1;
                }
            }
        }
        eng.note_off(note);
        for _ in 0..(fs as usize / 2 / 256) {
            let mut l = vec![0.0f32; 256];
            let mut r = vec![0.0f32; 256];
            eng.process(&mut l, &mut r);
        }
    }
    (20.0 * pk.max(1e-9).log10(), 10.0 * (e / n.max(1) as f64).max(1e-12).log10() as f32)
}

#[test]
#[ignore]
fn print_preset_levels() {
    for pr in PRESETS {
        let (pk, rms) = preset_peak(pr.values);
        println!("LEVEL\t{}\t{:.2}\t{:.2}", pr.name, pk, rms);
    }
}

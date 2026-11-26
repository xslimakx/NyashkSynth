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

fn run(name: &str, vals: &[(&str, f32)], note: u8, vel: f32) -> (f32, f32) {
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
    let master = util::db_to_gain(p.master.value());
    eng.note_on(note, vel);
    let (mut pk, mut rms, mut cnt) = (0.0f32, 0.0f64, 0usize);
    let mut mono = Vec::new();
    for blk in 0..(fs as usize / 256) {
        let mut l = vec![0.0f32; 256];
        let mut r = vec![0.0f32; 256];
        eng.process(&mut l, &mut r);
        rack.process(&(&fx1).into(), &(&fx2).into(), route, split, split2, lvl2, &mut l, &mut r);
        for i in 0..256 {
            let (a, b) = (l[i] * master, r[i] * master);
            if blk > 40 {
                pk = pk.max(a.abs()).max(b.abs());
                rms += (a * a + b * b) as f64 * 0.5;
                cnt += 1;
                mono.push(0.5 * (a + b));
            }
        }
    }
    // energy below 120 Hz vs total (Goertzel-free: one-pole low-pass)
    let k = 1.0 - (-std::f32::consts::TAU * 120.0 / fs).exp();
    let (mut lp, mut el, mut et) = (0.0f32, 0.0f64, 0.0f64);
    for x in &mono {
        lp += (x - lp) * k;
        el += (lp * lp) as f64;
        et += (x * x) as f64;
    }
    let (pk_db, rms_db) = (20.0 * pk.max(1e-9).log10(), (10.0 * (rms / cnt as f64).max(1e-12).log10()) as f32);
    eprintln!("{name:28} note {note} vel {vel}: peak {pk_db:6.1} dBFS  rms {rms_db:6.1} dBFS  <120Hz share {:5.1}%  master {:.1} dB", 100.0 * el / et.max(1e-12), p.master.value());
    (pk_db, rms_db)
}

#[test]
/// 1.1 gain staging: the init saw and a bass with its sub are full-level, and nothing clips
fn bass_is_full_level() {
    let (pk, rms) = run("default", &[], 36, 0.8);
    assert!(pk > -6.0 && pk < 0.0 && rms > -14.0, "init saw too quiet / hot: {pk} {rms}");
    let (pk, rms) = run("default + sub", &[("sub_on", 1.0), ("sub_lv", 1.0)], 36, 0.8);
    assert!(pk > -6.0 && rms > -10.0, "bass with sub too quiet: {pk} {rms}");
    run("default", &[], 24, 0.8);
    for pr in PRESETS.iter().filter(|p| p.cat.to_lowercase().contains("bass")).take(12) {
        run(pr.name, pr.values, 36, 0.8);
    }
}


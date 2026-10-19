//! Sound audit: plays every factory preset through engine + effects and reports problems.
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
            // SAFETY: pointer into `p`, single thread
            unsafe { ptr.set_normalized_value(ptr.preview_normalized(*v)) };
        }
    }
}

#[test]
fn every_factory_preset_sounds_sane() {
    let fs = 48000.0f32;
    let mut problems: Vec<String> = Vec::new();
    let mut report = String::new();
    for pr in PRESETS {
        let p = SynthParams::default();
        load(&p, pr.values);
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
        let mut peak_all = 0.0f32;
        let mut worst_dc = 0.0f32;
        let mut sounding = 0.0f32;
        for (note, vel) in [(36u8, 0.9f32), (60, 0.8), (84, 1.0), (60, 0.2)] {
            eng.note_on(note, vel);
            let mut hold_rms = 0.0f32;
            let mut sum = 0.0f64;
            let mut cnt = 0usize;
            let mut bad = false;
            for blk in 0..(fs as usize * 2 / 256) {
                let mut l = vec![0.0f32; 256];
                let mut r = vec![0.0f32; 256];
                eng.process(&mut l, &mut r);
                rack.process(&fx1, &fx2, route, split, split2, lvl2, &mut l, &mut r);
                for i in 0..256 {
                    let (a, b) = (l[i] * master, r[i] * master);
                    if !a.is_finite() || !b.is_finite() {
                        bad = true;
                    }
                    peak_all = peak_all.max(a.abs()).max(b.abs());
                    if blk > 20 {
                        hold_rms += a * a + b * b;
                        sum += (a + b) as f64;
                        cnt += 2;
                    }
                }
            }
            if bad {
                problems.push(format!("{}: NaN/inf (note {note})", pr.name));
            }
            sounding = sounding.max((hold_rms / cnt.max(1) as f32).sqrt());
            worst_dc = worst_dc.max((sum / cnt.max(1) as f64).abs() as f32);
            eng.note_off(note);
            // let it ring out for 10 s, check that it ends
            let mut tail = 0.0f32;
            for blk in 0..(fs as usize * 10 / 256) {
                let mut l = vec![0.0f32; 256];
                let mut r = vec![0.0f32; 256];
                eng.process(&mut l, &mut r);
                rack.process(&fx1, &fx2, route, split, split2, lvl2, &mut l, &mut r);
                if blk > fs as usize * 8 / 256 {
                    for i in 0..256 {
                        tail = tail.max(l[i].abs()).max(r[i].abs());
                    }
                }
            }
            if tail > 0.1 && !p.arp_on.value() && !p.clip_on.value() {
                problems.push(format!("{}: still {:.3} loud 8 s after note off (note {note})", pr.name, tail));
            }
            eng.all_notes_off();
            eng.sound_off();
        }
        report.push_str(&format!("{:<28} peak {:>5.2} rms {:>6.3} dc {:>6.4}\n", pr.name, peak_all, sounding, worst_dc));
        if sounding < 0.0005 && peak_all < 0.02 && !p.arp_on.value() {
            problems.push(format!("{}: silent ({sounding})", pr.name));
        }
        if peak_all > 4.0 {
            problems.push(format!("{}: very loud peak {peak_all:.1}", pr.name));
        }
        if worst_dc > 0.02 {
            problems.push(format!("{}: DC offset {worst_dc:.3}", pr.name));
        }
        if sounding > 0.9 {
            problems.push(format!("{}: too loud rms {sounding:.2}", pr.name));
        }
    }
    println!("{report}");
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

/// CPU use: how many times faster than real time each preset renders a 8-note chord.
#[test]
fn cpu_cost_per_preset() {
    let fs = 48000.0f32;
    let mut rows: Vec<(f32, String)> = Vec::new();
    for pr in PRESETS {
        let p = SynthParams::default();
        load(&p, pr.values);
        let sh = Shared::new();
        sh.sync_from_params(&p);
        let patch = Patch::read_with(&p, fs, Some(&sh), None);
        let mut eng = Engine::new(fs, patch);
        let fx1 = fx_patch_from(&p, 120.0);
        let fx2 = fx_patch_from_bank(&p.fx2, 120.0);
        let mut rack = FxRack::new(fs);
        let (route, split, split2, lvl2) = (p.fx_route.value(), p.fx_split.value(), p.fx_split2.value(), p.fx2_level.value());
        for n in [48u8, 52, 55, 59, 60, 64, 67, 72] {
            eng.note_on(n, 0.8);
        }
        let t0 = std::time::Instant::now();
        let secs = 3usize;
        for _ in 0..(fs as usize * secs / 512) {
            let mut l = vec![0.0f32; 512];
            let mut r = vec![0.0f32; 512];
            eng.process(&mut l, &mut r);
            rack.process(&fx1, &fx2, route, split, split2, lvl2, &mut l, &mut r);
        }
        let el = t0.elapsed().as_secs_f32();
        rows.push((secs as f32 / el, pr.name.to_string()));
    }
    rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    for (x, n) in rows.iter().take(12) {
        println!("{:>6.1}x realtime  {}", x, n);
    }
    assert!(rows[0].0 > 2.0, "slowest preset only {:.1}x realtime: {}", rows[0].0, rows[0].1);
}

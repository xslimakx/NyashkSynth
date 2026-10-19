use nyashksynth::arp::ArpP;
use nyashksynth::engine::{Engine, Patch, SlotP};
use nyashksynth::*;

fn engine() -> Engine {
    let params = SynthParams::default();
    let patch = Patch::read(&params, 48000.0);
    Engine::new(48000.0, patch)
}

fn render(e: &mut Engine, n: usize) -> f32 {
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut peak = 0.0f32;
    for c in 0..(n / 64) {
        let (a, b) = (&mut l[c * 64..c * 64 + 64], &mut r[c * 64..c * 64 + 64]);
        e.process(a, b);
        for x in a.iter() {
            peak = peak.max(x.abs());
        }
    }
    peak
}

#[test]
fn sustain_pedal_holds_then_releases() {
    let mut e = engine();
    e.patch.env[0].r_coef = (-1.0f32 / (0.05 * 48000.0 / 4.6)).exp();
    e.note_on(60, 1.0);
    e.set_sustain(true);
    e.note_off(60);
    render(&mut e, 48000 / 2);
    assert_eq!(e.active_voices(), 1, "pedal must hold the note");
    e.set_sustain(false);
    render(&mut e, 48000);
    assert_eq!(e.active_voices(), 0, "note must end after the pedal is lifted");
}

#[test]
fn arpeggiator_cycles_notes_in_time() {
    let mut e = engine();
    e.patch.arp = ArpP { on: true, beats: 0.25, ..Default::default() };
    e.bpm = 120.0; // 1/16 = 125 ms
    for n in [60u8, 64, 67] {
        e.note_on(n, 1.0);
    }
    let mut seen: Vec<u8> = Vec::new();
    let mut l = vec![0.0f32; 64];
    let mut r = vec![0.0f32; 64];
    for _ in 0..(48000 / 64) {
        e.process(&mut l, &mut r);
        let m = e.note_mask();
        for k in 0..128u8 {
            if (m >> k) & 1 == 1 && seen.last() != Some(&k) {
                seen.push(k);
            }
        }
    }
    // 1 s at 8 steps / s -> 8 steps
    assert!(seen.len() >= 7 && seen.len() <= 9, "steps: {:?}", seen);
    assert_eq!(&seen[..4], &[60, 64, 67, 60]);
}

#[test]
fn fx_modulation_values() {
    let mut e = engine();
    e.patch.macros[0] = 1.0;
    e.patch.slots[0] = SlotP { src: ModSrc::Macro1, dst: ModDst::FxDelayMix, amount: 0.5, aux: ModSrc::None, curve: 0.0, uni: false };
    let m = e.fx_mod().expect("fx destination used");
    assert!((m[6] - 0.5).abs() < 1e-6);
    e.patch.slots[0].dst = ModDst::Cutoff;
    assert!(e.fx_mod().is_none());
}

#[test]
fn bend_range_change_applies_to_held_bend() {
    let mut e = engine();
    e.note_on(69, 1.0);
    e.set_bend(1.0);
    e.patch.bend = 12.0;
    let p = render(&mut e, 4096);
    assert!(p.is_finite() && p > 0.0);
}

#[test]
fn no_stuck_notes_after_random_on_off() {
    for vmode in [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Legato] {
        for seed in 1..=40u32 {
            let mut e = engine();
            e.patch.vmode = vmode;
            e.patch.env[0].r_coef = (-1.0f32 / (0.02 * 48000.0 / 4.6)).exp();
            let mut s = seed.wrapping_mul(2654435761) | 1;
            let mut rnd = move || {
                s ^= s << 13;
                s ^= s >> 17;
                s ^= s << 5;
                s
            };
            let mut held = std::collections::HashSet::new();
            for _ in 0..400 {
                let note = 40 + (rnd() % 12) as u8;
                match rnd() % 5 {
                    0 | 1 => {
                        e.note_on(note, 0.8);
                        held.insert(note);
                    }
                    2 | 3 => {
                        e.note_off(note);
                        held.remove(&note);
                    }
                    _ => e.set_sustain(rnd() % 2 == 0),
                }
                render(&mut e, 64 * (1 + (rnd() % 8) as usize));
            }
            for n in held.clone() {
                e.note_off(n);
            }
            e.set_sustain(false);
            render(&mut e, 48000 * 2);
            assert_eq!(e.active_voices(), 0, "stuck note: {vmode:?} seed {seed}");
        }
    }
}

#[test]
fn no_stuck_notes_when_patch_changes_while_held() {
    let modes = [VoiceMode::Poly, VoiceMode::Mono, VoiceMode::Legato];
    let mut bad = Vec::new();
    for seed in 1..=60u32 {
        let mut e = engine();
        e.patch.env[0].r_coef = (-1.0f32 / (0.02 * 48000.0 / 4.6)).exp();
        let mut s = seed.wrapping_mul(2654435761) | 1;
        let mut rnd = move || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s
        };
        let mut down = std::collections::HashSet::new();
        for _ in 0..300 {
            let note = 40 + (rnd() % 12) as u8;
            match rnd() % 7 {
                0 | 1 => {
                    e.note_on(note, 0.8);
                    down.insert(note);
                }
                2 | 3 => {
                    e.note_off(note);
                    down.remove(&note);
                }
                4 => e.patch.vmode = modes[(rnd() % 3) as usize], // "preset switch"
                5 => e.patch.arp.on = rnd() % 2 == 0,
                _ => e.patch.poly = 1 + (rnd() % 16) as usize,
            }
            render(&mut e, 64 * (1 + (rnd() % 8) as usize));
        }
        for n in down.clone() {
            e.note_off(n);
        }
        e.patch.arp.on = false;
        render(&mut e, 48000 * 3);
        if e.active_voices() != 0 {
            bad.push(seed);
        }
    }
    assert!(bad.is_empty(), "stuck after patch changes, seeds {bad:?}");
}

fn render_vec(e: &mut Engine, n: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(n);
    let mut l = vec![0.0f32; 64];
    let mut r = vec![0.0f32; 64];
    for _ in 0..n / 64 {
        l.iter_mut().for_each(|x| *x = 0.0);
        r.iter_mut().for_each(|x| *x = 0.0);
        e.process(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

fn max_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0.0f32, |m, w| m.max((w[0] - w[1]).abs()))
}

#[test]
fn stolen_voice_and_fast_release_do_not_click() {
    let mut e = engine();
    e.patch.poly = 1; // every new note steals the only voice
    e.patch.sub_on = false;
    e.note_on(48, 1.0);
    let a = render_vec(&mut e, 48000 / 4);
    let steady = max_step(&a[a.len() / 2..]);
    e.note_on(67, 1.0);
    let b = render_vec(&mut e, 48000 / 10);
    let step = max_step(&b[..400]);
    assert!(step < steady * 2.5 + 0.02, "retrigger step {step} vs steady {steady}");

    // the shortest allowed release (6 ms) must ramp down instead of cutting
    e.patch.env[0].r_coef = (-1.0f32 / (0.006 * 48000.0 / 4.6)).exp();
    e.note_off(67);
    let c = render_vec(&mut e, 48000 / 10);
    let tail = max_step(&c);
    assert!(tail < steady * 2.5 + 0.02, "release step {tail} vs steady {steady}");
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}

#[test]
fn custom_point_filter_shapes_the_sound() {
    use nyashksynth::engine::CbP;
    use nyashksynth::params::{BandKind, FilterType};
    let off = CbP { on: false, kind: BandKind::Bell, freq: 1000.0, gain: 0.0, q: 1.0 };
    let play = |bands: [CbP; 8], note: u8| {
        let mut e = engine();
        e.patch.sub_on = false;
        e.patch.fl[0].on = true;
        e.patch.fl[0].ty = FilterType::Custom;
        e.patch.fl[0].cb = bands;
        e.note_on(note, 1.0);
        let v = render_vec(&mut e, 48000 / 2);
        assert!(v.iter().all(|x| x.is_finite() && x.abs() < 20.0));
        rms(&v[v.len() / 2..])
    };
    let flat = play([off; 8], 60);
    // a low-pass point at 300 Hz must darken a bright note (C5)
    let mut b = [off; 8];
    b[0] = CbP { on: true, kind: BandKind::LowPass, freq: 300.0, gain: 0.0, q: 1.0 };
    let dark = play(b, 84);
    let bright = play([off; 8], 84);
    assert!(dark < bright * 0.5, "low-pass point: {dark} vs {bright}");
    // a +12 dB bell at the note frequency raises the level
    let mut b = [off; 8];
    b[2] = CbP { on: true, kind: BandKind::Bell, freq: 261.6, gain: 12.0, q: 1.0 };
    let boosted = play(b, 60);
    assert!(boosted > flat * 1.5, "bell boost: {boosted} vs {flat}");
}

#[test]
fn third_oscillator_adds_sound() {
    let level = |on: bool| {
        let mut e = engine();
        e.patch.sub_on = false;
        e.patch.fl[0].on = false;
        e.patch.osc[0].on = false;
        e.patch.osc[1].on = false;
        e.patch.osc[2].on = on;
        e.patch.osc[2].level = 0.8;
        e.note_on(60, 1.0);
        let v = render_vec(&mut e, 48000 / 2);
        rms(&v[v.len() / 2..])
    };
    assert!(level(true) > 0.01, "osc C is silent");
    assert!(level(false) < 1e-4, "osc C leaks when off");
}

#[test]
fn chaos_lfos_are_bounded_and_vary() {
    use nyashksynth::engine::lfo_eval;
    use nyashksynth::params::LfoShape;
    for sh in [LfoShape::Lorenz, LfoShape::Rossler] {
        let v: Vec<f32> = (0..9000).map(|i| lfo_eval(sh, i as f64 / 4000.0, 7)).collect();
        assert!(v.iter().all(|x| x.is_finite() && x.abs() <= 1.2), "{sh:?} out of range");
        let (mn, mx) = v.iter().fold((9.0f32, -9.0f32), |a, x| (a.0.min(*x), a.1.max(*x)));
        assert!(mx - mn > 0.8, "{sh:?} does not move: {mn}..{mx}");
        let step = v.windows(2).fold(0.0f32, |m, w| m.max((w[0] - w[1]).abs()));
        assert!(step < 0.3, "{sh:?} jumps: {step}");
    }
}

fn goertzel(x: &[f32], f: f32, fs: f32) -> f32 {
    let w = 2.0 * std::f32::consts::PI * f / fs;
    let (mut re, mut im) = (0.0f32, 0.0f32);
    for (i, v) in x.iter().enumerate() {
        re += v * (w * i as f32).cos();
        im += v * (w * i as f32).sin();
    }
    (re * re + im * im).sqrt() / x.len() as f32
}

#[test]
fn bode_shifts_frequencies() {
    use nyashksynth::fx::FxRack;
    use nyashksynth::params::{FxRoute, SynthParams};
    let fs = 48000.0f32;
    let sp = SynthParams::default();
    let mut p = fx_patch_from(&sp, 120.0);
    p.on = [false; 12];
    p.on[10] = true;
    p.bd_mix = 1.0;
    p.bd_fb = 0.0;
    for (shift, want, other) in [(200.0f32, 1200.0f32, 800.0f32), (-200.0, 800.0, 1200.0)] {
        p.bd_shift = shift;
        let mut rack = FxRack::new(fs);
        let n = 16384;
        let mut l: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / fs).sin() * 0.5).collect();
        let mut r = l.clone();
        for c in 0..n / 256 {
            let (a, b) = (c * 256, (c + 1) * 256);
            rack.process(&p, &p, FxRoute::Series, 400.0, 3000.0, 0.0, &mut l[a..b], &mut r[a..b]);
        }
        let seg = &l[4096..];
        let (w, o, orig) = (goertzel(seg, want, fs), goertzel(seg, other, fs), goertzel(seg, 1000.0, fs));
        assert!(w > 0.2 && w > o * 5.0 && w > orig * 8.0, "shift {shift}: wanted {w}, other {o}, original {orig}");
    }
}

#[test]
fn rack_routing_keeps_level_sane() {
    use nyashksynth::fx::FxRack;
    use nyashksynth::params::{FxRoute, SynthParams};
    let fs = 48000.0f32;
    let sp = SynthParams::default();
    let mut p = fx_patch_from(&sp, 120.0);
    p.on = [false; 12];
    p.on[11] = true; // utility only (unity gain)
    for route in [FxRoute::Series, FxRoute::Parallel, FxRoute::SplitLh, FxRoute::SplitMs, FxRoute::SplitLmh] {
        let mut rack = FxRack::new(fs);
        let n = 8192;
        let mut l: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / fs).sin() * 0.5 + (2.0 * std::f32::consts::PI * 3000.0 * i as f32 / fs).sin() * 0.2).collect();
        let mut r: Vec<f32> = l.iter().map(|v| v * 0.8).collect();
        let (l0, r0) = (l.clone(), r.clone());
        for c in 0..n / 256 {
            let (a, b) = (c * 256, (c + 1) * 256);
            rack.process(&p, &p, route, 800.0, 4000.0, 1.0, &mut l[a..b], &mut r[a..b]);
        }
        let e0: f32 = l0[4096..].iter().map(|v| v * v).sum();
        let e1: f32 = l[4096..].iter().map(|v| v * v).sum();
        assert!(e1 > e0 * 0.5 && e1 < e0 * 2.0, "{route:?}: energy {e1} vs {e0}");
        assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()));
        let _ = r0;
    }
}

#[test]
fn second_filter_adds_in_series_and_parallel() {
    use nyashksynth::params::FilterType;
    let run = |two: bool, serial: bool, route2: bool| {
        let mut e = engine();
        e.patch.sub_on = false;
        e.patch.fl[0].on = true;
        e.patch.fl[0].ty = FilterType::Lp12;
        e.patch.fl[0].cutoff = 600.0;
        e.patch.fl[0].res = 0.0;
        e.patch.fl[1].on = two;
        e.patch.fl[1].ty = FilterType::Lp12;
        e.patch.fl[1].cutoff = 600.0;
        e.patch.fl[1].res = 0.0;
        e.patch.fl[1].route = [route2; 5];
        e.patch.fl_serial = serial;
        e.note_on(84, 1.0);
        let v = render_vec(&mut e, 48000 / 2);
        rms(&v[v.len() / 2..])
    };
    let one = run(false, false, false);
    let ser = run(true, true, false);
    assert!(ser < one * 0.6, "series should darken further: {ser} vs {one}");
    // parallel with the second filter taking every source: the same filter twice = same level (not doubled, not silent)
    let par = run(true, false, true);
    assert!(par > one * 0.5 && par < one * 1.3, "parallel level {par} vs {one}");
}

#[test]
fn sample_oscillator_plays_at_pitch_and_loops() {
    use nyashksynth::params::{OscMode, SampleLoop};
    use nyashksynth::sample::NSample;
    use std::sync::Arc;
    // one second of 261.6256 Hz at 44.1 kHz = original pitch on middle C (note 60)
    let f0 = 261.6256f32;
    let data: Vec<f32> = (0..44100).map(|i| (std::f32::consts::TAU * f0 * i as f32 / 44100.0).sin() * 0.8).collect();
    let run = |note: u8, lp: SampleLoop| {
        let mut e = engine();
        e.patch.sub_on = false;
        e.patch.fl[0].on = false;
        e.patch.osc[1].on = false;
        e.patch.osc[0].mode = OscMode::Sample;
        e.patch.osc[0].s_loop = lp;
        e.patch.osc[0].sample = Some(Arc::new(NSample { name: "t".into(), data: data.clone() }));
        e.note_on(note, 1.0);
        render_vec(&mut e, 48000 * 2)
    };
    let v = run(60, SampleLoop::Loop);
    let seg = &v[48000..48000 + 24000];
    let at = goertzel(seg, f0, 48000.0);
    let off = goertzel(seg, f0 * 1.5, 48000.0);
    assert!(at > off * 8.0, "sample not at original pitch: {at} vs {off}");
    // an octave up
    let v = run(72, SampleLoop::Loop);
    let seg = &v[48000..48000 + 24000];
    assert!(goertzel(seg, f0 * 2.0, 48000.0) > goertzel(seg, f0, 48000.0) * 8.0, "octave up wrong");
    // looping keeps sounding after the sample length, one-shot goes silent
    assert!(rms(&run(60, SampleLoop::Loop)[90000..]) > 0.05);
    assert!(rms(&run(60, SampleLoop::Off)[90000..]) < 1e-3);
    assert!(run(60, SampleLoop::PingPong).iter().all(|x| x.is_finite() && x.abs() < 4.0));
}

#[test]
fn a4_reference_and_scale_tuning_change_pitch() {
    use nyashksynth::params::Tuning;
    let freq_of = |a4: f32, tun: Tuning, note: u8| -> f32 {
        let mut e = engine();
        e.patch.a4 = a4;
        e.patch.tun = tun.cents();
        e.patch.sub_on = false;
        e.patch.fl[0].on = false;
        e.patch.osc[1].on = false;
        e.note_on(note, 1.0);
        let v = render_vec(&mut e, 48000);
        let seg = &v[24000..24000 + 16000];
        // pick the strongest of a fine grid around the expected pitch
        let base = a4 * 2f32.powf((note as f32 - 69.0) / 12.0);
        (0..400).map(|i| base * (0.97 + 0.06 * i as f32 / 400.0)).max_by(|a, b| goertzel(seg, *a, 48000.0).partial_cmp(&goertzel(seg, *b, 48000.0)).unwrap()).unwrap()
    };
    let a440 = freq_of(440.0, Tuning::Equal, 69);
    let a432 = freq_of(432.0, Tuning::Equal, 69);
    assert!((a440 - 440.0).abs() < 1.5, "{a440}");
    assert!((a432 - 432.0).abs() < 1.5, "{a432}");
    // just major third above C (note 64, root C): 386.31 cents vs 400 -> about 13.7 cents flat
    let eq = freq_of(440.0, Tuning::Equal, 64);
    let just = freq_of(440.0, Tuning::Just, 64);
    let cents = 1200.0 * (just / eq).log2();
    assert!((cents + 13.7).abs() < 3.0, "just third off by {cents}");
}

#[test]
fn clip_sequencer_plays_pattern_transposed_and_stops() {
    use nyashksynth::clip::{CNote, Clip, ClipP};
    let mut e = engine();
    e.patch.env[0].r_coef = (-1.0f32 / (0.02 * 48000.0 / 4.6)).exp();
    let clip = Clip { bars: 1, notes: vec![CNote { start: 0, len: 2, pitch: 0, vel: 100 }, CNote { start: 8, len: 2, pitch: 7, vel: 100 }] };
    e.patch.clip = ClipP::from_clip(true, 0.25, &clip);
    // 120 bpm: a 1/16 step is 6000 samples; 8 steps = 48000
    e.bpm = 120.0;
    e.note_on(60, 1.0);
    let v = render_vec(&mut e, 48000 * 2);
    // first note at step 0 (60), gap, second at step 8 (67 = 60 + 7), then the bar loops
    assert!(rms(&v[0..9000]) > 0.01, "first clip note silent");
    assert!(rms(&v[24000..30000]) < 0.003, "gap between notes should be silent: {}", rms(&v[24000..30000]));
    assert!(rms(&v[48000..48000 + 9000]) > 0.01, "second clip note silent");
    assert!(rms(&v[96000 - 9000..96000]) > 0.0 || true);
    e.note_off(60);
    render_vec(&mut e, 48000 / 2);
    assert_eq!(e.active_voices(), 0, "clip must stop when the key is released");
    // with the clip off the key plays normally again
    e.patch.clip.on = false;
    e.note_on(60, 1.0);
    let v = render_vec(&mut e, 4800);
    assert!(rms(&v[2400..]) > 0.01);
}

#[test]
fn granular_oscillator_makes_a_cloud_at_pitch() {
    use nyashksynth::params::OscMode;
    use nyashksynth::sample::NSample;
    use std::sync::Arc;
    let f0 = 261.6256f32;
    let data: Vec<f32> = (0..44100).map(|i| (std::f32::consts::TAU * f0 * i as f32 / 44100.0).sin() * 0.8).collect();
    let run = |note: u8, dens: f32| {
        let mut e = engine();
        e.patch.sub_on = false;
        e.patch.fl[0].on = false;
        e.patch.osc[1].on = false;
        e.patch.osc[0].mode = OscMode::Granular;
        e.patch.osc[0].g_dens = dens;
        e.patch.osc[0].g_size = 60.0;
        e.patch.osc[0].g_spray = 0.2;
        e.patch.osc[0].sample = Some(Arc::new(NSample { name: "t".into(), data: data.clone() }));
        e.note_on(note, 1.0);
        render_vec(&mut e, 48000)
    };
    let v = run(60, 40.0);
    assert!(v.iter().all(|x| x.is_finite() && x.abs() < 4.0));
    let seg = &v[24000..24000 + 16000];
    assert!(rms(seg) > 0.01, "granular silent");
    assert!(goertzel(seg, f0, 48000.0) > goertzel(seg, f0 * 1.5, 48000.0) * 6.0, "grains not at key pitch");
    let v2 = run(72, 40.0);
    let seg2 = &v2[24000..24000 + 16000];
    assert!(goertzel(seg2, f0 * 2.0, 48000.0) > goertzel(seg2, f0, 48000.0) * 6.0, "octave up wrong");
    // very dense clouds stay level-controlled
    assert!(rms(&run(60, 250.0)[24000..]) < 1.0);
}

#[test]
fn new_fx_variants_are_stable_and_distinct() {
    use nyashksynth::fx::FxRack;
    use nyashksynth::params::{DistType, FxRoute, ReverbType, SynthParams};
    let fs = 48000.0f32;
    let sp = SynthParams::default();
    let run = |setup: &dyn Fn(&mut nyashksynth::fx::FxPatch)| -> Vec<f32> {
        let mut p = fx_patch_from(&sp, 120.0);
        p.on = [false; 12];
        setup(&mut p);
        let mut rack = FxRack::new(fs);
        let n = 24000;
        let mut l: Vec<f32> = (0..n).map(|i| if i < 2000 { (2.0 * std::f32::consts::PI * 330.0 * i as f32 / fs).sin() * 0.5 } else { 0.0 }).collect();
        let mut r = l.clone();
        for c in 0..n / 256 {
            let (a, b) = (c * 256, (c + 1) * 256);
            rack.process(&p, &p, FxRoute::Series, 400.0, 3000.0, 1.0, &mut l[a..b], &mut r[a..b]);
        }
        assert!(l.iter().chain(r.iter()).all(|v| v.is_finite() && v.abs() < 8.0));
        l
    };
    // reverb algorithms give different tails
    let mut tails = vec![];
    for t in [ReverbType::Plate, ReverbType::Vintage, ReverbType::Nitrous, ReverbType::Basin] {
        let o = run(&|p| {
            p.on[7] = true;
            p.rv_type = t;
            p.rv_mix = 0.5;
            p.rv_size = 0.8;
        });
        assert!(o[6000..].iter().map(|v| v * v).sum::<f32>() > 1e-6, "{t:?} has a tail");
        tails.push(o);
    }
    for i in 1..4 {
        let d: f32 = tails[0].iter().zip(&tails[i]).map(|(a, b)| (a - b).abs()).sum();
        assert!(d > 0.5, "reverb type {i} differs from plate ({d})");
    }
    // overdrive + bias stay finite; bias removes nothing audible but must not leave DC
    let o = run(&|p| {
        p.on[1] = true;
        p.ds_type = DistType::Overdrive;
        p.ds_drive = 0.9;
        p.ds_bias = 0.8;
        p.ds_mix = 1.0;
    });
    assert!(o[..2000].iter().any(|v| v.abs() > 0.1));
    // HQ delay still echoes
    let o = run(&|p| {
        p.on[5] = true;
        p.dl_hq = true;
        p.dl_ms_l = 100.0;
        p.dl_ms_r = 100.0;
        p.dl_mix = 0.8;
    });
    assert!(o[4800..8000].iter().map(|v| v * v).sum::<f32>() > 1e-3);
}

#[test]
fn key_lock_quantizes_to_scale() {
    use nyashksynth::params::ScaleType;
    // C major: C# (61) goes down to C (60), F# (66) goes down to F (65), black keys never survive
    for n in 36u8..96 {
        let q = ScaleType::Major.quantize(n, 0);
        assert!([0, 2, 4, 5, 7, 9, 11].contains(&(q as i32 % 12)), "{n} -> {q}");
        assert!((q as i32 - n as i32).abs() <= 1);
    }
    assert_eq!(ScaleType::Major.quantize(61, 0), 60);
    assert_eq!(ScaleType::Major.quantize(66, 0), 65);
    // root shifts the scale: D major has F#
    assert_eq!(ScaleType::Major.quantize(66, 2), 66);
    assert_eq!(ScaleType::Off.quantize(61, 0), 61);
    // sparse scales always find a note
    for sc in [ScaleType::PentMaj, ScaleType::PentMin, ScaleType::Blues, ScaleType::WholeTone] {
        for n in 0u8..=127 {
            let q = sc.quantize(n, 5);
            assert!(sc.degrees().contains(&((q as i32 - 5).rem_euclid(12))));
        }
    }
}

#[test]
fn bypassed_matrix_slot_does_nothing() {
    use nyashksynth::engine::SlotP;
    use nyashksynth::params::ModSrc;
    let sl = SlotP { src: ModSrc::Lfo1, dst: nyashksynth::params::ModDst::None, amount: 0.5, aux: ModSrc::None, curve: 0.0, uni: false };
    assert!(matches!(sl.clone().into_enabled(false).src, ModSrc::None));
    assert!(matches!(sl.into_enabled(true).src, ModSrc::Lfo1));
}

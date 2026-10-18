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

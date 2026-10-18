use nyashksynth::engine::{Engine, Patch};
use nyashksynth::fx::Fx;
use nyashksynth::{DistType, FilterType, ModDst, ModSrc, NoiseType, SynthParams, WarpMode};
use std::time::Instant;

fn rms_db(x: &[f32]) -> f32 {
    20.0 * ((x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt() + 1e-9).log10()
}

fn render(e: &mut Engine, secs: f32, fs: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (secs * fs) as usize;
    let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
    for c in 0..(n / 256) {
        let a = c * 256;
        e.process(&mut l[a..a + 256], &mut r[a..a + 256]);
    }
    (l, r)
}

// crude spectrum magnitude via Goertzel at one frequency
fn goertzel(x: &[f32], f: f32, fs: f32) -> f32 {
    let w = 2.0 * std::f32::consts::PI * f / fs;
    let (mut s1, mut s2) = (0.0f32, 0.0f32);
    let c = 2.0 * w.cos();
    for v in x {
        let s = v + c * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    ((s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0)).sqrt() / x.len() as f32 * 2.0
}

fn main() {
    let fs = 48000.0;
    let params = SynthParams::default();
    let base = Patch::read(&params, fs);

    // 1. default patch: A4 sine
    let mut e = Engine::new(fs, base.clone());
    e.note_on(69, 1.0);
    let (l, _r) = render(&mut e, 0.5, fs);
    let s = &l[12000..];
    println!("default A4: rms {:.1} dB, fundamental {:.3}, 2nd {:.4}, 3rd {:.4}", rms_db(s), goertzel(s, 440.0, fs), goertzel(s, 880.0, fs), goertzel(s, 1320.0, fs));

    // 2. alias check: saw at high pitch, A table pos 0.5 = saw. measure energy at non-harmonic frequencies
    let mut p = base.clone();
    p.osc[0].pos = 0.5;
    p.fl_on = false;
    let mut e = Engine::new(fs, p);
    e.note_on(96, 1.0); // C7 ~ 2093 Hz
    let (l, _) = render(&mut e, 0.5, fs);
    let s = &l[12000..];
    let f0 = 440.0 * 2f32.powf((96.0 - 69.0) / 12.0);
    let h1 = goertzel(s, f0, fs);
    let inharm: f32 = [0.5, 1.5, 2.5, 3.5, 5.5, 7.5, 9.5].iter().map(|k| goertzel(s, f0 * k, fs)).fold(0.0, f32::max);
    println!("saw C7: f0 {:.1} Hz amp {:.3}; strongest non-harmonic bin {:.5} ({:.0} dB below)", f0, h1, inharm, 20.0 * (h1 / inharm.max(1e-9)).log10());

    // 3. every table x warp mode: finite & bounded
    let mut worst = 0.0f32;
    let mut bad = 0;
    for t in 0..8 {
        for w in [WarpMode::Off, WarpMode::BendPlus, WarpMode::BendMinus, WarpMode::BendBoth, WarpMode::Pwm, WarpMode::Sync, WarpMode::Mirror, WarpMode::Quantize, WarpMode::Fm, WarpMode::Am, WarpMode::Rm, WarpMode::SineBend, WarpMode::Asym] {
            let mut p = base.clone();
            p.osc[0].tbl = nyashksynth::wavetable::tables().t[t].clone();
            p.osc[0].pos = 0.37;
            p.osc[0].wmode = w;
            p.osc[0].warp = 0.8;
            p.osc[0].uni = 7;
            p.osc[1].on = true;
            p.osc[1].tbl = nyashksynth::wavetable::tables().t[(t + 3) % 8].clone();
            p.osc[1].wmode = w;
            p.osc[1].warp = 0.6;
            p.sub_on = true;
            p.noise = 0.2;
            let mut e = Engine::new(fs, p);
            for n in [36u8, 60, 84, 108] {
                e.note_on(n, 0.8);
            }
            let (l, r) = render(&mut e, 0.3, fs);
            for v in l.iter().chain(r.iter()) {
                if !v.is_finite() {
                    bad += 1;
                }
                worst = worst.max(v.abs());
            }
        }
    }
    println!("tables x warps: non-finite samples {}, worst peak {:.2}", bad, worst);

    // 4. filter sweep: cutoff modulated by Env2
    let mut p = base.clone();
    p.osc[0].pos = 0.5;
    p.cutoff = 500.0;
    p.res = 0.6;
    p.slots[0] = nyashksynth::engine::SlotP { src: ModSrc::Env2, dst: ModDst::Cutoff, amount: 0.5, aux: ModSrc::None, curve: 0.0, uni: false };
    p.env[1].s = 0.0;
    let mut e = Engine::new(fs, p);
    e.note_on(48, 1.0);
    let (l, _) = render(&mut e, 1.0, fs);
    let f0 = 440.0 * 2f32.powf((48.0 - 69.0) / 12.0);
    println!("filter env sweep: 15th harmonic early {:.4}, late {:.4}", goertzel(&l[480..2880], f0 * 15.0, fs), goertzel(&l[24000..26400], f0 * 15.0, fs));

    // 5. unison 16 x 16 voices CPU
    let mut p = base.clone();
    p.osc[0].uni = 16;
    p.osc[1].on = true;
    p.osc[1].uni = 16;
    p.sub_on = true;
    let mut e = Engine::new(fs, p);
    for n in 40..56u8 {
        e.note_on(n, 1.0);
    }
    let t0 = Instant::now();
    let secs = 2.0;
    let _ = render(&mut e, secs, fs);
    let el = t0.elapsed().as_secs_f32();
    println!("CPU worst case (16 voices x 2 osc x 16 unison): {:.1}% of one core", el / secs * 100.0);

    // 6. FX modules: every module alone and the whole chain
    {
        let mut e = Engine::new(fs, base.clone());
        e.note_on(60, 1.0);
        e.note_on(67, 1.0);
        let (l0, r0) = render(&mut e, 1.0, fs);
        for m in 0..10usize {
            let mut fx = Fx::new(fs);
            let mut fp = nyashksynth::fx_patch_from(&params, 120.0);
            fp.on = [false; 10];
            fp.on[m] = true;
            fp.dl_mix = 0.4;
            fp.rv_mix = 0.4;
            fp.fg_mix = 0.5;
            fp.ph_mix = 0.5;
            fp.eq_lg = 9.0;
            let (mut l, mut r) = (l0.clone(), r0.clone());
            fx.process(&fp, &mut l, &mut r);
            let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, b| a.max(b.abs()));
            println!("fx {:<8} finite {} peak {:.2} rms {:.1} dB", nyashksynth::FX_NAMES[m], l.iter().chain(r.iter()).all(|v| v.is_finite()), pk, rms_db(&l));
        }
        let mut fx = Fx::new(fs);
        let mut fp = nyashksynth::fx_patch_from(&params, 120.0);
        fp.on = [true; 10];
        fp.ds_type = DistType::Fold;
        let (mut l, mut r) = (l0.clone(), r0.clone());
        fx.process(&fp, &mut l, &mut r);
        println!("all fx on: finite {} peak {:.2}", l.iter().all(|v| v.is_finite()), l.iter().fold(0.0f32, |a, b| a.max(b.abs())));
        // reverb tail rings after the input stops
        let mut fx = Fx::new(fs);
        let mut fp = nyashksynth::fx_patch_from(&params, 120.0);
        fp.on = [false; 10];
        fp.on[7] = true;
        let (mut a, mut b) = (l0[..9600].to_vec(), r0[..9600].to_vec());
        fx.process(&fp, &mut a, &mut b);
        let (mut a, mut b) = (vec![0.0; 24000], vec![0.0; 24000]);
        fx.process(&fp, &mut a, &mut b);
        println!("reverb tail 0-0.25 s after input: {:.1} dB", rms_db(&a[..12000]));
    }
    // 6b. every filter type and noise type renders finite sound
    for ty in 0..<FilterType as nih_plug::prelude::Enum>::variants().len() {
        let mut p = base.clone();
        p.osc[0].pos = 0.5;
        p.fl_type = <FilterType as nih_plug::prelude::Enum>::from_index(ty);
        p.cutoff = 1200.0;
        p.res = 0.6;
        let mut e = Engine::new(fs, p);
        e.note_on(48, 1.0);
        let (l, _) = render(&mut e, 0.3, fs);
        println!("filter {:<10} finite {} rms {:.1} dB peak {:.2}", <FilterType as nih_plug::prelude::Enum>::variants()[ty], l.iter().all(|v| v.is_finite()), rms_db(&l[4000..]), l.iter().fold(0.0f32, |a, b| a.max(b.abs())));
    }
    for ty in 0..<NoiseType as nih_plug::prelude::Enum>::variants().len() {
        let mut p = base.clone();
        p.osc[0].on = false;
        p.noise = 0.8;
        p.noise_type = <NoiseType as nih_plug::prelude::Enum>::from_index(ty);
        let mut e = Engine::new(fs, p);
        e.note_on(60, 1.0);
        let (l, _) = render(&mut e, 0.3, fs);
        println!("noise {:<14} finite {} rms {:.1} dB peak {:.2}", <NoiseType as nih_plug::prelude::Enum>::variants()[ty], l.iter().all(|v| v.is_finite()), rms_db(&l[4000..]), l.iter().fold(0.0f32, |a, b| a.max(b.abs())));
    }
    // 7. mono / legato + glide
    let mut p = base.clone();
    p.vmode = nyashksynth::VoiceMode::Legato;
    p.glide_ms = 200.0;
    let mut e = Engine::new(fs, p);
    e.note_on(60, 1.0);
    let _ = render(&mut e, 0.2, fs);
    e.note_on(72, 1.0);
    let (l, _) = render(&mut e, 0.1, fs);
    println!("legato glide: voices {} first-sample rms {:.1} dB", e.active_voices(), rms_db(&l[..2000]));
}

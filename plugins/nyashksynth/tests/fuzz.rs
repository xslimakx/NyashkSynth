//! Robustness: random patches, random notes, odd buffer sizes and sample rates must never produce NaN / inf,
//! explode, or panic.
use nih_plug::prelude::Enum;
use nyashksynth::arp::ArpP;
use nyashksynth::engine::{Engine, Patch, SlotP};
use nyashksynth::fx::{Fx, FxPatch};
use nyashksynth::wavetable::tables;
use nyashksynth::*;

struct R(u64);
impl R {
    fn f(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }
    fn r(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    fn i(&mut self, n: usize) -> usize {
        ((self.f() * n as f32) as usize).min(n - 1)
    }
    fn e<E: Enum>(&mut self) -> E {
        E::from_index(self.i(E::variants().len()))
    }
    fn b(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

fn randomize(p: &mut Patch, r: &mut R, fs: f32) {
    for o in p.osc.iter_mut() {
        o.on = r.b(0.8);
        o.tbl = tables().t[r.i(tables().t.len())].clone();
        o.pos = r.f();
        o.wmode = r.e();
        o.warp = r.f();
        o.wmode2 = r.e();
        o.warp2 = r.f();
        o.level = r.f();
        o.pan = r.r(-1.0, 1.0);
        o.oct = r.i(9) as i32 - 4;
        o.semi = r.i(25) as i32 - 12;
        o.fine = r.r(-100.0, 100.0);
        o.uni = 1 + r.i(16);
        o.detune = r.f();
        o.blend = r.f();
        o.umode = r.e();
        o.spread = r.f();
        o.phase = r.f();
        o.rand = r.f();
    }
    p.sub_on = r.b(0.5);
    p.sub_wave = r.e();
    p.sub_oct = -(r.i(4) as i32);
    p.sub_level = r.f();
    p.noise = r.f();
    p.noise_type = r.e();
    p.noise_pitch = r.r(-48.0, 48.0);
    p.noise_key = r.b(0.5);
    p.noise_loop = r.b(0.5);
    p.fl_on = r.b(0.9);
    p.fl_type = r.e();
    p.cutoff = 20.0 * (1000.0f32).powf(r.f());
    p.res = r.f();
    p.drive = r.f();
    p.keytrack = r.f();
    p.fl_mix = r.f();
    p.route = [r.b(0.8), r.b(0.8), r.b(0.8), r.b(0.8)];
    for e in p.env.iter_mut() {
        let ms = |v: f32| v * 0.001 * fs;
        e.delay_n = if r.b(0.2) { ms(r.r(1.0, 500.0)) } else { 0.0 };
        e.a_inc = 1.0 / ms(r.r(0.5, 3000.0)).max(1.0);
        e.hold_n = if r.b(0.2) { ms(r.r(1.0, 500.0)) } else { 0.0 };
        e.d_coef = (-1.0 / (ms(r.r(1.0, 4000.0)) / 4.6).max(1.0)).exp();
        e.s = r.f();
        e.r_coef = (-1.0 / (ms(r.r(1.0, 4000.0)) / 4.6).max(1.0)).exp();
        e.ka = 2f32.powf(r.r(-2.0, 2.0));
        e.kd = 2f32.powf(r.r(-2.0, 2.0));
        e.kr = 2f32.powf(r.r(-2.0, 2.0));
    }
    for l in p.lfo.iter_mut() {
        l.shape = r.e();
        l.rate = r.r(0.02, 40.0);
        l.sync = r.b(0.4);
        l.div = r.e();
        l.trig = r.b(0.5);
        l.rise_s = if r.b(0.3) { r.r(0.0, 3.0) } else { 0.0 };
        l.smooth = r.f();
        l.delay_s = if r.b(0.3) { r.r(0.0, 2.0) } else { 0.0 };
        l.oneshot = r.b(0.3);
        l.phase0 = r.f();
    }
    for s in p.slots.iter_mut() {
        *s = SlotP { src: r.e(), dst: r.e(), amount: r.r(-1.0, 1.0), aux: r.e(), curve: r.r(-1.0, 1.0), uni: r.b(0.5) };
    }
    for m in p.macros.iter_mut() {
        *m = r.f();
    }
    p.vmode = r.e();
    p.glide_ms = if r.b(0.5) { r.r(0.0, 2000.0) } else { 0.0 };
    p.bend = r.i(25) as f32;
    p.poly = 1 + r.i(16);
    p.tune = r.r(-100.0, 100.0);
    p.arp = ArpP { on: r.b(0.35), mode: r.e(), beats: [0.125, 0.25, 0.5, 1.0][r.i(4)], octs: 1 + r.i(4), gate: r.r(0.05, 1.0), swing: r.f(), hold: r.b(0.3) };
}

fn randomize_fx(f: &mut FxPatch, r: &mut R) {
    for i in 0..10 {
        f.on[i] = r.b(0.7);
    }
    let mut order: Vec<usize> = (0..10).collect();
    for i in (1..10).rev() {
        order.swap(i, r.i(i + 1));
    }
    f.order.copy_from_slice(&order);
    f.hy_rate = r.r(0.05, 8.0);
    f.hy_detune = r.f();
    f.hy_uni = 2 + r.i(7);
    f.hy_dim = r.f();
    f.hy_mix = r.f();
    f.ds_type = r.e();
    f.ds_drive = r.f();
    f.ds_tone = r.r(200.0, 20000.0);
    f.ds_mix = r.f();
    f.fg_rate = r.r(0.02, 8.0);
    f.fg_depth = r.f();
    f.fg_delay = r.r(0.1, 10.0);
    f.fg_fb = r.r(-1.0, 1.0);
    f.fg_phase = r.f();
    f.fg_mix = r.f();
    f.ph_rate = r.r(0.02, 8.0);
    f.ph_depth = r.f();
    f.ph_freq = r.r(100.0, 8000.0);
    f.ph_fb = r.f();
    f.ph_stages = 2 + r.i(11);
    f.ph_phase = r.f();
    f.ph_mix = r.f();
    f.ch_rate = r.r(0.05, 5.0);
    f.ch_depth = r.f();
    f.ch_delay = r.r(2.0, 30.0);
    f.ch_fb = r.f();
    f.ch_filter = r.r(400.0, 20000.0);
    f.ch_mix = r.f();
    f.dl_ms_l = r.r(10.0, 1500.0);
    f.dl_ms_r = r.r(10.0, 1500.0);
    f.dl_fb = r.f();
    f.dl_pp = r.b(0.5);
    f.dl_lp = r.r(500.0, 20000.0);
    f.dl_hp = r.r(20.0, 2000.0);
    f.dl_width = r.f();
    f.dl_mix = r.f();
    f.cp_thresh = r.r(-60.0, 0.0);
    f.cp_ratio = r.r(1.0, 20.0);
    f.cp_attack = r.r(0.1, 200.0);
    f.cp_release = r.r(5.0, 1000.0);
    f.cp_gain = r.r(0.0, 24.0);
    f.cp_multi = r.b(0.5);
    f.cp_f1 = r.r(60.0, 500.0);
    f.cp_f2 = r.r(800.0, 8000.0);
    f.cp_mix = r.f();
    f.rv_size = r.f();
    f.rv_damp = r.f();
    f.rv_pre = r.r(0.5, 250.0);
    f.rv_lowcut = r.r(20.0, 1000.0);
    f.rv_highcut = r.r(1000.0, 20000.0);
    f.rv_width = r.f();
    f.rv_mix = r.f();
    f.eq_lf = r.r(20.0, 1000.0);
    f.eq_lg = r.r(-18.0, 18.0);
    f.eq_mf = r.r(100.0, 10000.0);
    f.eq_mq = r.r(0.2, 10.0);
    f.eq_mg = r.r(-18.0, 18.0);
    f.eq_hf = r.r(1000.0, 20000.0);
    f.eq_hg = r.r(-18.0, 18.0);
    f.ff_type = r.e();
    f.ff_cut = r.r(20.0, 20000.0);
    f.ff_res = r.f();
    f.ff_drive = r.f();
    f.ff_mix = r.f();
}

#[test]
fn random_patches_stay_finite_and_bounded() {
    let params = SynthParams::default();
    let mut worst = 0.0f32;
    let only: Option<u64> = std::env::var("NY_SEED").ok().and_then(|v| v.parse().ok());
    let off: Option<usize> = std::env::var("NY_FXOFF").ok().and_then(|v| v.parse().ok());
    for seed in 1..=120u64 {
        if only.is_some() && only != Some(seed) {
            continue;
        }
        let mut r = R(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let fs = [22050.0f32, 44100.0, 48000.0, 88200.0, 96000.0, 192000.0][r.i(6)];
        let mut patch = Patch::read(&params, fs);
        randomize(&mut patch, &mut r, fs);
        if std::env::var("NY_NOFILT").is_ok() { patch.fl_on = false; }
        if std::env::var("NY_NOWARP").is_ok() { for o in patch.osc.iter_mut() { o.wmode = nyashksynth::WarpMode::Off; o.wmode2 = nyashksynth::WarpMode::Off; } }
        if std::env::var("NY_LP").is_ok() { patch.fl_type = nyashksynth::FilterType::Lp12; }
        let mut eng = Engine::new(fs, patch);
        let mut fx = Fx::new(fs);
        let mut fxp = nyashksynth::fx_patch_from(&params, 120.0);
        randomize_fx(&mut fxp, &mut r);
        if let Some(o) = off {
            for k in 0..10 {
                fxp.on[k] = k == o;
            }
        }
        if only.is_some() {
            println!("seed {seed} fs {fs} on {:?} order {:?}", fxp.on, fxp.order);
        }
        eng.bpm = r.r(20.0, 300.0);
        let total = (fs * 3.0) as usize;
        let mut done = 0;
        let mut held: Vec<u8> = Vec::new();
        while done < total {
            let n = [1usize, 2, 3, 17, 64, 100, 256, 1000, 4096][r.i(9)];
            // random MIDI traffic
            for _ in 0..r.i(4) {
                match r.i(9) {
                    0..=3 => {
                        let note = 24 + r.i(90) as u8;
                        held.push(note);
                        eng.note_on(note, r.r(0.05, 1.0));
                    }
                    4 | 5 => {
                        if !held.is_empty() {
                            let k = r.i(held.len());
                            eng.note_off(held.swap_remove(k));
                        }
                    }
                    6 => eng.set_bend(r.f()),
                    7 => eng.set_sustain(r.b(0.5)),
                    _ => {
                        if r.b(0.2) {
                            eng.all_notes_off();
                        } else {
                            eng.set_modwheel(r.f());
                            eng.set_aftertouch(r.f());
                        }
                    }
                }
            }
            if r.b(0.02) && std::env::var("NY_NOLIVE").is_err() {
                randomize(&mut eng.patch, &mut r, fs); // live patch change while playing
                if std::env::var("NY_NOFILT").is_ok() { eng.patch.fl_on = false; }
                if std::env::var("NY_NOWARP").is_ok() { for o in eng.patch.osc.iter_mut() { o.wmode = nyashksynth::WarpMode::Off; o.wmode2 = nyashksynth::WarpMode::Off; } }
                if std::env::var("NY_NOSUB").is_ok() { eng.patch.sub_on = false; }
                if std::env::var("NY_NOARP").is_ok() { eng.patch.arp.on = false; }
            }
            let mut l = vec![0.0f32; n];
            let mut rr = vec![0.0f32; n];
            eng.process(&mut l, &mut rr);
            for i in 0..n {
                assert!(l[i].is_finite() && rr[i].is_finite(), "seed {seed} fs {fs}: ENGINE NaN/inf (arp {} vmode {:?})", eng.patch.arp.on, eng.patch.vmode);
                assert!(l[i].abs() < 5000.0, "seed {seed}: ENGINE exploded {} at sample {} (voices {} unison {} {} filter {:?} on {} osc {:?}/{:?} {:?}/{:?} sub {} noise {:?})", l[i], done + i, eng.patch.arp.on as u8, eng.patch.osc[0].uni, eng.patch.osc[1].uni, eng.patch.fl_type, eng.patch.fl_on, eng.patch.osc[0].wmode, eng.patch.osc[0].wmode2, eng.patch.osc[1].wmode, eng.patch.osc[1].wmode2, eng.patch.sub_on, eng.patch.noise_type);
            }
            match eng.fx_mod() {
                Some(m) => {
                    let mut f = fxp.clone();
                    f.apply_mod(&m);
                    fx.process(&f, &mut l, &mut rr);
                }
                None => fx.process(&fxp, &mut l, &mut rr),
            }
            for i in 0..n {
                assert!(l[i].is_finite() && rr[i].is_finite(), "seed {seed} fs {fs}: NaN/inf");
                worst = worst.max(l[i].abs()).max(rr[i].abs());
            }
            done += n;
        }
        assert!(worst < 1000.0, "seed {seed}: output exploded ({worst})");
    }
    println!("worst peak over all random patches: {worst}");
}

fn scenario(seed: u64, tweak: &dyn Fn(&mut Patch)) -> bool {
    let params = SynthParams::default();
    let mut r = R(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let fs = [22050.0f32, 44100.0, 48000.0, 88200.0, 96000.0, 192000.0][r.i(6)];
    let mut patch = Patch::read(&params, fs);
    randomize(&mut patch, &mut r, fs);
    tweak(&mut patch);
    let mut eng = Engine::new(fs, patch);
    eng.bpm = r.r(20.0, 300.0);
    let total = (fs * 3.0) as usize;
    let mut done = 0;
    let mut held: Vec<u8> = Vec::new();
    while done < total {
        let n = [1usize, 2, 3, 17, 64, 100, 256, 1000, 4096][r.i(9)];
        for _ in 0..r.i(4) {
            match r.i(9) {
                0..=3 => {
                    let note = 24 + r.i(90) as u8;
                    held.push(note);
                    eng.note_on(note, r.r(0.05, 1.0));
                }
                4 | 5 => {
                    if !held.is_empty() {
                        let k = r.i(held.len());
                        eng.note_off(held.swap_remove(k));
                    }
                }
                6 => eng.set_bend(r.f()),
                7 => eng.set_sustain(r.b(0.5)),
                _ => {
                    if r.b(0.2) {
                        eng.all_notes_off();
                    } else {
                        eng.set_modwheel(r.f());
                        eng.set_aftertouch(r.f());
                    }
                }
            }
        }
        if r.b(0.02) {
            randomize(&mut eng.patch, &mut r, fs);
            tweak(&mut eng.patch);
        }
        let mut l = vec![0.0f32; n];
        let mut rr = vec![0.0f32; n];
        eng.process(&mut l, &mut rr);
        if l.iter().chain(rr.iter()).any(|x| !x.is_finite()) {
            return false;
        }
        done += n;
    }
    true
}

#[test]
#[ignore]
fn diagnose() {
    use nyashksynth::{FilterType, ModDst, ModSrc, NoiseType, UniMode, WarpMode};
    let _ = (ModDst::None, ModSrc::None);
    for seed in 1..=60u64 {
        if scenario(seed, &|_| {}) {
            continue;
        }
        let mut culprits = Vec::new();
        if scenario(seed, &|p| for o in p.osc.iter_mut() { o.wmode = WarpMode::Off; o.wmode2 = WarpMode::Off }) { culprits.push("warp"); }
        if scenario(seed, &|p| for o in p.osc.iter_mut() { o.uni = 1 }) { culprits.push("unison"); }
        if scenario(seed, &|p| for o in p.osc.iter_mut() { o.umode = UniMode::Linear }) { culprits.push("unimode"); }
        if scenario(seed, &|p| p.fl_on = false) { culprits.push("filter"); }
        if scenario(seed, &|p| p.fl_type = FilterType::Lp12) { culprits.push("filtertype"); }
        if scenario(seed, &|p| p.noise_type = NoiseType::White) { culprits.push("noise"); }
        if scenario(seed, &|p| for s in p.slots.iter_mut() { s.amount = 0.0 }) { culprits.push("matrix"); }
        if scenario(seed, &|p| p.arp.on = false) { culprits.push("arp"); }
        if scenario(seed, &|p| p.sub_on = false) { culprits.push("sub"); }
        if scenario(seed, &|p| for l in p.lfo.iter_mut() { l.smooth = 0.0; l.trig = false }) { culprits.push("lfo"); }
        println!("seed {seed}: fixed by {:?}", culprits);
    }
}

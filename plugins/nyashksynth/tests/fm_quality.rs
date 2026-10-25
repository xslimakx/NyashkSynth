//! FM / warp aliasing: oversampling must clean up the inharmonic junk.
use nyashksynth::engine::{Engine, Patch};
use nyashksynth::params::{FmSrc, OscQuality, WarpMode};
use nyashksynth::*;

fn render_note(q: OscQuality, note: u8, ratio_semi: i32, amt: f32, src: FmSrc) -> Vec<f32> {
    let fs = 48000.0f32;
    let mut patch = Patch::read(&SynthParams::default(), fs);
    patch.quality = q;
    // A (carrier): FM from B; B is the modulator (muted: its level must not matter any more)
    patch.osc[0].on = true;
    patch.osc[0].tbl = nyashksynth::wavetable::tables().t[2].clone(); // bright saw sweep
    patch.osc[0].pos = 0.0;
    patch.osc[0].wmode = WarpMode::Fm;
    patch.osc[0].warp = amt;
    patch.osc[0].fm_src = src;
    patch.osc[1].on = true;
    patch.osc[1].level = 0.0;
    patch.osc[1].semi = ratio_semi;
    let mut e = Engine::new(fs, patch);
    e.note_on(note, 1.0);
    let n = 48000;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for c in 0..n / 64 {
        let (a, b) = (&mut l[c * 64..c * 64 + 64], &mut r[c * 64..c * 64 + 64]);
        e.process(a, b);
    }
    l[12000..].to_vec()
}

/// share of the energy that is not on a multiple of f0 (+-1.5%): aliasing / noise
fn inharmonic(x: &[f32], f0: f32, fs: f32) -> f32 {
    let n = 1 << 15;
    let mut re = vec![0.0f32; n];
    for (i, v) in x.iter().take(n).enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
        re[i] = v * w;
    }
    // naive DFT on a coarse grid would be slow: use a simple radix-2 FFT
    let mut im = vec![0.0f32; n];
    fft(&mut re, &mut im);
    let (mut tot, mut bad) = (0.0f64, 0.0f64);
    for k in 1..n / 2 {
        let f = k as f32 * fs / n as f32;
        if f > 20000.0 {
            break;
        }
        let e = (re[k] * re[k] + im[k] * im[k]) as f64;
        tot += e;
        let h = (f / f0).round().max(1.0);
        if (f - h * f0).abs() > f0 * 0.015 + 2.0 * fs / n as f32 {
            bad += e;
        }
    }
    (bad / tot.max(1e-20)) as f32
}

fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (a, b) = (i + k, i + k + len / 2);
                let (tr, ti) = (re[b] * cr - im[b] * ci, re[b] * ci + im[b] * cr);
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
            i += len;
        }
        len <<= 1;
    }
}

#[test]
fn oversampling_reduces_fm_aliasing() {
    let f0 = 440.0f32 * 2f32.powf((84.0 - 69.0) / 12.0); // C6
    let mut res = Vec::new();
    for q in [OscQuality::X1, OscQuality::X2, OscQuality::X4, OscQuality::X8] {
        let x = render_note(q, 84, 12, 0.8, FmSrc::B);
        assert!(x.iter().all(|v| v.is_finite()));
        let rms = (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        assert!(rms > 0.02, "FM from a muted oscillator B must still sound ({q:?} rms {rms})");
        let a = inharmonic(&x, f0, 48000.0);
        println!("{q:?}: inharmonic share {:.2e}  rms {rms:.3}", a);
        res.push(a);
    }
    assert!(res[1] < res[0] * 0.8 || res[0] < 1e-4, "2x not cleaner than 1x: {res:?}");
    assert!(res[2] <= res[1] * 1.05 + 1e-6, "4x worse than 2x: {res:?}");
}

#[test]
fn fm_sources_all_work() {
    for src in [FmSrc::Auto, FmSrc::A, FmSrc::B, FmSrc::C, FmSrc::Sub, FmSrc::Noise] {
        let x = render_note(OscQuality::Auto, 60, 7, 0.5, src);
        assert!(x.iter().all(|v| v.is_finite() && v.abs() < 4.0), "{src:?}");
    }
}

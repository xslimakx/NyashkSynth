//! Small FFT spectrum analyser used by the FX page and the filter graph.

pub const SPEC_N: usize = 2048;
pub const BINS: usize = SPEC_N / 2;

/// In-place radix-2 FFT (`re`, `im` length must be a power of two).
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
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

/// Magnitude in dB (about -120..0 for a full-scale sine) of the first `BINS` bins.
pub fn magnitudes_db(samples: &[f32]) -> Vec<f32> {
    let mut re = vec![0.0f32; SPEC_N];
    let mut im = vec![0.0f32; SPEC_N];
    let n = samples.len().min(SPEC_N);
    for i in 0..n {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / (SPEC_N - 1) as f32).cos();
        re[i] = samples[i] * w;
    }
    fft(&mut re, &mut im);
    // a full-scale sine with a Hann window peaks at about N/4
    let norm = 4.0 / SPEC_N as f32;
    (0..BINS).map(|k| 20.0 * ((re[k] * re[k] + im[k] * im[k]).sqrt() * norm).max(1e-6).log10()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_sine() {
        let fs = 48000.0;
        let f = 3000.0;
        let x: Vec<f32> = (0..SPEC_N).map(|i| (std::f32::consts::TAU * f * i as f32 / fs).sin() * 0.5).collect();
        let m = magnitudes_db(&x);
        let peak = m.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap()).unwrap().0;
        let pf = peak as f32 * fs / SPEC_N as f32;
        assert!((pf - f).abs() < 30.0, "peak at {pf}");
        assert!((m[peak] - -6.0).abs() < 2.0, "level {}", m[peak]);
    }
}

//! Partitioned FFT convolution (the "Convolve" reverb algorithm: load any .wav as an impulse response).
//! Left and right are packed into one complex FFT (L = real, R = imaginary); the IR is mono.

use std::f32::consts::PI;

/// Block size; the wet signal is delayed by this many samples.
pub const B: usize = 256;
const N: usize = 2 * B;
/// Longest impulse response in blocks (65536 samples, about 1.4 s at 48 kHz).
pub const MAXP: usize = 256;

type C = (f32, f32);

pub struct Fft {
    tw: Vec<C>,
    rev: Vec<usize>,
}

impl Fft {
    pub fn new() -> Fft {
        let bits = N.trailing_zeros();
        let rev = (0..N).map(|i| (i.reverse_bits() >> (usize::BITS - bits)) as usize).collect();
        let tw = (0..N / 2).map(|k| ((-2.0 * PI * k as f32 / N as f32).cos(), (-2.0 * PI * k as f32 / N as f32).sin())).collect();
        Fft { tw, rev }
    }

    pub fn run(&self, x: &mut [C], inverse: bool) {
        for i in 0..N {
            let j = self.rev[i];
            if j > i {
                x.swap(i, j);
            }
        }
        let mut len = 2;
        while len <= N {
            let step = N / len;
            for s in (0..N).step_by(len) {
                for k in 0..len / 2 {
                    let (wr, mut wi) = self.tw[k * step];
                    if inverse {
                        wi = -wi;
                    }
                    let a = x[s + k];
                    let b = x[s + k + len / 2];
                    let t = (b.0 * wr - b.1 * wi, b.0 * wi + b.1 * wr);
                    x[s + k] = (a.0 + t.0, a.1 + t.1);
                    x[s + k + len / 2] = (a.0 - t.0, a.1 - t.1);
                }
            }
            len <<= 1;
        }
    }
}

/// Impulse response split into spectra of 2B points.
pub struct ConvIr {
    pub parts: Vec<Vec<C>>,
    pub name: String,
}

impl ConvIr {
    /// `data` is mono at `src_rate`; it is resampled to `fs`, trimmed and normalised.
    pub fn build(data: &[f32], src_rate: f32, fs: f32, name: &str) -> ConvIr {
        let ratio = src_rate / fs.max(8000.0);
        let n = ((data.len() as f32 / ratio) as usize).min(B * MAXP).max(1);
        let mut h: Vec<f32> = (0..n)
            .map(|i| {
                let p = i as f32 * ratio;
                let k = p as usize;
                let f = p - k as f32;
                let a = data.get(k).copied().unwrap_or(0.0);
                let b = data.get(k + 1).copied().unwrap_or(0.0);
                a + (b - a) * f
            })
            .collect();
        // short fade-out so a trimmed tail does not click
        let fade = (n / 16).min(2048).max(1);
        for i in 0..fade {
            h[n - 1 - i] *= i as f32 / fade as f32;
        }
        let e: f32 = h.iter().map(|v| v * v).sum::<f32>().max(1e-9);
        let g = 1.0 / e.sqrt();
        let fft = Fft::new();
        let np = (n + B - 1) / B;
        let mut parts = Vec::with_capacity(np);
        for p in 0..np {
            let mut buf = vec![(0.0f32, 0.0f32); N];
            for i in 0..B {
                if let Some(v) = h.get(p * B + i) {
                    buf[i] = (v * g, 0.0);
                }
            }
            fft.run(&mut buf, false);
            parts.push(buf);
        }
        ConvIr { parts, name: name.to_string() }
    }
}

pub struct Conv {
    fft: Fft,
    fdl: Vec<Vec<C>>,
    pos: usize,
    inb: Vec<C>,
    acc: Vec<C>,
    out: Vec<C>,
    fill: usize,
}

impl Conv {
    pub fn new() -> Conv {
        Conv { fft: Fft::new(), fdl: vec![vec![(0.0, 0.0); N]; MAXP], pos: 0, inb: vec![(0.0, 0.0); N], acc: vec![(0.0, 0.0); N], out: vec![(0.0, 0.0); B], fill: 0 }
    }

    /// One stereo sample in, one (delayed by `B`) convolved sample out. `np` = blocks of the IR to use.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, ir: &ConvIr, np: usize) -> (f32, f32) {
        self.inb[B + self.fill] = (l, r);
        let o = self.out[self.fill];
        self.fill += 1;
        if self.fill == B {
            self.block(ir, np);
            self.fill = 0;
        }
        o
    }

    fn block(&mut self, ir: &ConvIr, np: usize) {
        let np = np.min(ir.parts.len()).min(MAXP).max(1);
        let pos = self.pos;
        self.fdl[pos].copy_from_slice(&self.inb);
        self.fft.run(&mut self.fdl[pos], false);
        for a in self.acc.iter_mut() {
            *a = (0.0, 0.0);
        }
        for p in 0..np {
            let x = &self.fdl[(pos + MAXP - p) % MAXP];
            let h = &ir.parts[p];
            for k in 0..N {
                let (xr, xi) = x[k];
                let (hr, hi) = h[k];
                self.acc[k].0 += xr * hr - xi * hi;
                self.acc[k].1 += xr * hi + xi * hr;
            }
        }
        self.fft.run(&mut self.acc, true);
        let s = 1.0 / N as f32;
        for i in 0..B {
            self.out[i] = (self.acc[B + i].0 * s, self.acc[B + i].1 * s);
        }
        self.inb.copy_within(B..N, 0);
        self.pos = (pos + 1) % MAXP;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_direct_convolution() {
        let h: Vec<f32> = (0..700).map(|i| ((i * 37 % 101) as f32 / 101.0 - 0.5) * (-(i as f32) / 300.0).exp()).collect();
        let ir = ConvIr::build(&h, 48000.0, 48000.0, "t");
        // rebuild the normalised IR the same way for the reference
        let e: f32 = h.iter().map(|v| v * v).sum::<f32>();
        let fade = (h.len() / 16).min(2048).max(1);
        let mut hn = h.clone();
        let n = hn.len();
        for i in 0..fade {
            hn[n - 1 - i] *= i as f32 / fade as f32;
        }
        let _ = e;
        let e2: f32 = h.iter().map(|v| v * v).sum::<f32>();
        let g = 1.0 / e2.sqrt();
        let x: Vec<f32> = (0..3000).map(|i| ((i * 91 % 57) as f32 / 57.0 - 0.5)).collect();
        let mut c = Conv::new();
        let y: Vec<f32> = x.iter().map(|v| c.tick(*v, -*v, &ir, 100).0).collect();
        for t in [B + 10, 1000, 2000, 2900] {
            let mut want = 0.0f32;
            for (k, hv) in hn.iter().enumerate() {
                if t >= B + k {
                    want += hv * g * x[t - B - k];
                }
            }
            assert!((y[t] - want).abs() < 1e-3, "t={t} got {} want {want}", y[t]);
        }
    }
}

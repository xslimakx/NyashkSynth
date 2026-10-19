//! Band-limited wavetables, generated at start-up or built from user data.
//!
//! Every frame is stored for 10 mip levels. Mip `m` keeps the first `(1 << (10 - m)) - 1`
//! harmonics of the frame (cut with an FFT / inverse FFT), so reading the right mip for the
//! played pitch never aliases. Tables have a variable number of frames (2..=256).

use std::f64::consts::PI;
use std::sync::{Arc, OnceLock};

pub const N: usize = 2048;
pub const BUILTIN_FRAMES: usize = 64;
pub const MAX_FRAMES: usize = 256;
pub const MIPS: usize = 10;
pub const TABLE_COUNT: usize = 8;
const STRIDE: usize = N + 1; // one guard sample for interpolation
/// Partials kept per frame for the spectral oscillator.
pub const SH: usize = 64;

pub const NAMES: [&str; TABLE_COUNT] = [
    "Basic Shapes",
    "Analog PWM",
    "Saw Sweep",
    "Vowels",
    "Comb Sweep",
    "FM Bells",
    "Organ",
    "Rough Noise",
];

/// Highest harmonic kept by each mip level.
#[inline]
pub fn mip_harmonics(m: usize) -> usize {
    (1usize << (MIPS - m)) - 1
}

/// Harmonic spectrum of one cycle: `x = sum a_k cos(k t) + b_k sin(k t)`, k = 1..1023 (index k).
pub type Spec = Vec<(f64, f64)>;

#[derive(Clone)]
pub struct Table {
    data: Vec<f32>, // [mip][frame][STRIDE]
    frames: usize,
    /// first `SH` harmonics of every frame as (a_k, b_k) pairs, for the spectral oscillator
    spec: Vec<f32>,
}

impl Table {
    #[inline]
    fn frame(&self, mip: usize, frame: usize) -> &[f32] {
        let o = (mip * self.frames + frame) * STRIDE;
        &self.data[o..o + STRIDE]
    }

    #[inline]
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Read the wave at `pos` (0..1 across the frames) and `phase` (0..1) with the given mip level.
    #[inline]
    pub fn sample(&self, mip: usize, pos: f32, phase: f32) -> f32 {
        let (f0, ff) = self.frames_at(pos);
        let x = phase * N as f32;
        let i = (x as usize) & (N - 1);
        let fr = x - x.floor();
        let a = self.frame(mip, f0);
        let b = self.frame(mip, f0 + 1);
        let sa = a[i] + (a[i + 1] - a[i]) * fr;
        let sb = b[i] + (b[i + 1] - b[i]) * fr;
        sa + (sb - sa) * ff
    }

    /// Frame indices and blend for a position (compute once per control block).
    #[inline]
    pub fn frames_at(&self, pos: f32) -> (usize, f32) {
        let fpos = pos.clamp(0.0, 1.0) * (self.frames - 1) as f32;
        let f0 = (fpos as usize).min(self.frames - 2);
        (f0, fpos - f0 as f32)
    }

    /// Fast read with precomputed frame pair.
    #[inline]
    pub fn sample_fr(&self, mip: usize, f0: usize, ff: f32, phase: f32) -> f32 {
        let x = phase * N as f32;
        let i = (x as usize) & (N - 1);
        let fr = x - i as f32;
        let a = self.frame(mip, f0);
        let b = self.frame(mip, f0 + 1);
        let sa = a[i] + (a[i + 1] - a[i]) * fr;
        let sb = b[i] + (b[i + 1] - b[i]) * fr;
        sa + (sb - sa) * ff
    }

    /// Mip level to use for a given highest playback frequency.
    #[inline]
    pub fn mip_for(&self, freq: f32, fs: f32) -> usize {
        let allowed = (0.5 * fs / freq.max(1.0)) as usize;
        for m in 0..MIPS {
            if mip_harmonics(m) <= allowed {
                return m;
            }
        }
        MIPS - 1
    }

    /// Blend of the first `SH` harmonics of two frames: `a[k-1]`, `b[k-1]` for harmonic k.
    pub fn spectral_at(&self, f0: usize, ff: f32, a: &mut [f32; SH], b: &mut [f32; SH]) {
        let f1 = (f0 + 1).min(self.frames - 1);
        for k in 0..SH {
            let (a0, b0) = (self.spec[(f0 * SH + k) * 2], self.spec[(f0 * SH + k) * 2 + 1]);
            let (a1, b1) = (self.spec[(f1 * SH + k) * 2], self.spec[(f1 * SH + k) * 2 + 1]);
            a[k] = a0 + (a1 - a0) * ff;
            b[k] = b0 + (b1 - b0) * ff;
        }
    }

    /// Full-bandwidth frame for drawing (mip 0).
    pub fn display_frame(&self, pos: f32, out: &mut [f32]) {
        let n = out.len();
        for (i, o) in out.iter_mut().enumerate() {
            *o = self.sample(0, pos, i as f32 / n as f32);
        }
    }

    /// Time-domain samples (N) of one frame at full bandwidth.
    pub fn frame_time(&self, i: usize) -> Vec<f32> {
        self.frame(0, i.min(self.frames - 1))[..N].to_vec()
    }

    pub fn to_frames(&self) -> Vec<Vec<f32>> {
        (0..self.frames).map(|i| self.frame_time(i)).collect()
    }

    fn alloc(frames: usize) -> Table {
        Table { data: vec![0.0; MIPS * frames * STRIDE], frames, spec: vec![0.0; frames * SH * 2] }
    }

    /// Write all mip levels of one frame from its spectrum. `scale` is applied to the output.
    fn fill(&mut self, fr: usize, spec: &Spec, scale: f64) {
        for k in 1..=SH {
            let (a, b) = spec.get(k).copied().unwrap_or((0.0, 0.0));
            self.spec[(fr * SH + k - 1) * 2] = (a * scale) as f32;
            self.spec[(fr * SH + k - 1) * 2 + 1] = (b * scale) as f32;
        }
        let mut re = vec![0.0f64; N];
        let mut im = vec![0.0f64; N];
        for m in 0..MIPS {
            let maxh = mip_harmonics(m);
            re.iter_mut().for_each(|v| *v = 0.0);
            im.iter_mut().for_each(|v| *v = 0.0);
            for k in 1..=maxh.min(N / 2 - 1) {
                let (a, b) = spec[k];
                re[k] = a / 2.0;
                im[k] = b / 2.0;
                re[N - k] = a / 2.0;
                im[N - k] = -b / 2.0;
            }
            fft(&mut re, &mut im);
            let o = (m * self.frames + fr) * STRIDE;
            for n in 0..N {
                self.data[o + n] = (re[n] * scale) as f32;
            }
            self.data[o + N] = self.data[o];
        }
    }

    /// Build from spectra; each frame is normalised on its own (built-in tables).
    pub fn from_specs_normalized(specs: &[Spec]) -> Table {
        let mut t = Table::alloc(specs.len());
        for (fr, spec) in specs.iter().enumerate() {
            let mut re = vec![0.0f64; N];
            let mut im = vec![0.0f64; N];
            for k in 1..N / 2 {
                let (a, b) = spec[k];
                re[k] = a / 2.0;
                im[k] = b / 2.0;
                re[N - k] = a / 2.0;
                im[N - k] = -b / 2.0;
            }
            fft(&mut re, &mut im);
            let peak = re.iter().fold(0.0f64, |p, v| p.max(v.abs())).max(1e-9);
            let rms = (re.iter().map(|v| v * v).sum::<f64>() / N as f64).sqrt().max(1e-9);
            let scale = (0.95 / peak).min(0.5 / rms);
            t.fill(fr, spec, scale);
        }
        t
    }

    /// Build from time-domain frames (each `N` samples). No normalisation is applied.
    pub fn from_frames(frames: &[Vec<f32>]) -> Table {
        let n = frames.len().clamp(2, MAX_FRAMES);
        let mut t = Table::alloc(n);
        for fr in 0..n {
            let src = frames.get(fr).or_else(|| frames.last());
            let spec = match src {
                Some(f) => spectrum_of(f),
                None => vec![(0.0, 0.0); N / 2],
            };
            t.fill(fr, &spec, 1.0);
        }
        t
    }

    /// Replace one frame with new time-domain samples (rebuilds that frame's mips only).
    pub fn set_frame(&mut self, i: usize, time: &[f32]) {
        if i < self.frames {
            let spec = spectrum_of(time);
            self.fill(i, &spec, 1.0);
        }
    }

    /// Resample the number of frames by interpolating between neighbours.
    pub fn resized(&self, n: usize, spectral: bool) -> Table {
        let n = n.clamp(2, MAX_FRAMES);
        let src = self.to_frames();
        let specs: Vec<Spec> = if spectral { src.iter().map(|f| spectrum_of(f)).collect() } else { Vec::new() };
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(n);
        for i in 0..n {
            let x = i as f64 / (n - 1) as f64 * (self.frames - 1) as f64;
            let a = (x.floor() as usize).min(self.frames - 2);
            let f = x - a as f64;
            if spectral {
                out.push(time_of(&morph_spec(&specs[a], &specs[a + 1], f)));
            } else {
                out.push(src[a].iter().zip(&src[a + 1]).map(|(p, q)| p + (q - p) * f as f32).collect());
            }
        }
        Table::from_frames(&out)
    }

    /// Scale a frame so its peak is `target` (or all frames to a common peak).
    pub fn peak(&self) -> f32 {
        let mut p = 0.0f32;
        for i in 0..self.frames {
            for v in &self.frame(0, i)[..N] {
                p = p.max(v.abs());
            }
        }
        p
    }
}

/// Spectrum (a_k, b_k) of one time-domain frame.
pub fn spectrum_of(time: &[f32]) -> Spec {
    let mut re: Vec<f64> = (0..N).map(|i| *time.get(i).unwrap_or(&0.0) as f64).collect();
    let mut im = vec![0.0f64; N];
    fft(&mut re, &mut im);
    let mut s = vec![(0.0, 0.0); N / 2];
    for k in 1..N / 2 {
        s[k] = (2.0 * re[k] / N as f64, -2.0 * im[k] / N as f64);
    }
    s
}

/// Time-domain frame (N samples) for a spectrum.
pub fn time_of(spec: &Spec) -> Vec<f32> {
    let mut re = vec![0.0f64; N];
    let mut im = vec![0.0f64; N];
    for k in 1..N / 2 {
        let (a, b) = spec[k];
        re[k] = a / 2.0;
        im[k] = b / 2.0;
        re[N - k] = a / 2.0;
        im[N - k] = -b / 2.0;
    }
    fft(&mut re, &mut im);
    re.iter().map(|v| *v as f32).collect()
}

/// Interpolate two spectra using magnitude and (shortest-path) phase.
pub fn morph_spec(a: &Spec, b: &Spec, t: f64) -> Spec {
    a.iter()
        .zip(b)
        .map(|(x, y)| {
            let (ma, mb) = ((x.0 * x.0 + x.1 * x.1).sqrt(), (y.0 * y.0 + y.1 * y.1).sqrt());
            // tiny components have meaningless phase: fall back to the other side's
            let (pa, pb) = (x.1.atan2(x.0), y.1.atan2(y.0));
            let (pa, pb) = if ma < 1e-7 { (pb, pb) } else if mb < 1e-7 { (pa, pa) } else { (pa, pb) };
            let mut d = pb - pa;
            while d > PI {
                d -= 2.0 * PI;
            }
            while d < -PI {
                d += 2.0 * PI;
            }
            let m = ma + (mb - ma) * t;
            let p = pa + d * t;
            (m * p.cos(), m * p.sin())
        })
        .collect()
}

/// Amplitude and phase of harmonic `k` for editors.
pub fn amp_phase(s: &Spec, k: usize) -> (f32, f32) {
    let (a, b) = s.get(k).copied().unwrap_or((0.0, 0.0));
    (((a * a + b * b).sqrt()) as f32, b.atan2(a) as f32)
}

pub fn from_amp_phase(amp: f32, ph: f32) -> (f64, f64) {
    (amp as f64 * (ph as f64).cos(), amp as f64 * (ph as f64).sin())
}

pub struct Tables {
    pub t: Vec<Arc<Table>>,
}

static TABLES: OnceLock<Tables> = OnceLock::new();

pub fn tables() -> &'static Tables {
    TABLES.get_or_init(build_all)
}

// ---------------------------------------------------------------- FFT

fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0usize;
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
        let ang = -2.0 * PI / len as f64;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f64, 0.0f64);
            for k in 0..len / 2 {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (xr, xi) = (re[i + k + len / 2], im[i + k + len / 2]);
                let (vr, vi) = (xr * cr - xi * ci, xr * ci + xi * cr);
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + len / 2] = ur - vr;
                im[i + k + len / 2] = ui - vi;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
}

// ---------------------------------------------------------------- spectra

fn blank() -> Spec {
    vec![(0.0, 0.0); N / 2]
}

fn lerp_spec(a: &Spec, b: &Spec, t: f64) -> Spec {
    a.iter().zip(b).map(|(x, y)| (x.0 + (y.0 - x.0) * t, x.1 + (y.1 - x.1) * t)).collect()
}

fn sgn(k: usize) -> f64 {
    if k % 2 == 1 {
        1.0
    } else {
        -1.0
    }
}

fn shape_sine() -> Spec {
    let mut s = blank();
    s[1].1 = 1.0;
    s
}
fn shape_tri() -> Spec {
    let mut s = blank();
    for k in (1..N / 2).step_by(2) {
        let sign = if ((k - 1) / 2) % 2 == 0 { 1.0 } else { -1.0 };
        s[k].1 = 8.0 / (PI * PI) * sign / (k * k) as f64;
    }
    s
}
fn shape_saw() -> Spec {
    let mut s = blank();
    for k in 1..N / 2 {
        s[k].1 = 2.0 / PI * sgn(k) / k as f64;
    }
    s
}
fn shape_square() -> Spec {
    let mut s = blank();
    for k in (1..N / 2).step_by(2) {
        s[k].1 = 4.0 / (PI * k as f64);
    }
    s
}
fn shape_pulse(w: f64) -> Spec {
    let mut s = blank();
    for k in 1..N / 2 {
        s[k].0 = 2.0 / (PI * k as f64) * (k as f64 * PI * w).sin();
    }
    s
}

fn lcg(seed: &mut u32) -> f64 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    (*seed >> 8) as f64 / (1u32 << 24) as f64
}

fn gen_spec(table: usize, t: f64) -> Spec {
    match table {
        // 0: sine -> triangle -> saw -> square -> narrow pulse
        0 => {
            let shapes = [shape_sine(), shape_tri(), shape_saw(), shape_square(), shape_pulse(0.18)];
            let s = (t * 4.0).min(3.9999);
            let i = s.floor() as usize;
            lerp_spec(&shapes[i], &shapes[i + 1], s - i as f64)
        }
        // 1: pulse width sweep
        1 => shape_pulse(0.5 - 0.46 * t),
        // 2: saw through a resonant low-pass sweep
        2 => {
            let fc = 1.5 * 2f64.powf(t * 5.5);
            let mut s = blank();
            for k in 1..N / 2 {
                let kf = k as f64;
                let lp = 1.0 / (1.0 + (kf / fc).powi(8)).sqrt();
                let peak = 2.2 * (-((kf / fc).ln() / 0.16).powi(2)).exp();
                s[k].1 = sgn(k) / kf * (lp + peak * lp);
            }
            s
        }
        // 3: vowels a e i o u
        3 => {
            let v = [
                [800.0, 1200.0, 2500.0],
                [400.0, 2000.0, 2600.0],
                [300.0, 2300.0, 3000.0],
                [500.0, 900.0, 2500.0],
                [350.0, 600.0, 2400.0],
            ];
            let x = (t * 4.0).min(3.9999);
            let i = x.floor() as usize;
            let f = x - i as f64;
            let form: Vec<f64> = (0..3).map(|j| v[i][j] + (v[i + 1][j] - v[i][j]) * f).collect();
            let lv = [1.0, 0.55, 0.3];
            let mut s = blank();
            for k in 1..N / 2 {
                let kf = k as f64;
                let mut g = 0.015;
                for j in 0..3 {
                    g += lv[j] * (-((kf / (form[j] / 110.0)).ln() / 0.2).powi(2)).exp();
                }
                s[k].1 = sgn(k) / kf.powf(0.75) * g;
            }
            s
        }
        // 4: comb sweep
        4 => {
            let m = 1.0 + 7.0 * t;
            let mut s = blank();
            for k in 1..N / 2 {
                let kf = k as f64;
                let w = 0.5 + 0.5 * (2.0 * PI * kf / m).cos();
                s[k].1 = sgn(k) / kf.powf(0.7) * w.powi(2);
            }
            s
        }
        // 5: two-operator FM, computed in the time domain
        5 => {
            let idx = t * 6.0;
            let ratio = 3.0;
            let mut re: Vec<f64> = (0..N)
                .map(|n| {
                    let th = 2.0 * PI * n as f64 / N as f64;
                    (th + idx * (ratio * th).sin()).sin()
                })
                .collect();
            let mut im = vec![0.0; N];
            fft(&mut re, &mut im);
            let mut s = blank();
            for k in 1..N / 2 {
                s[k] = (2.0 * re[k] / N as f64, -2.0 * im[k] / N as f64);
            }
            s
        }
        // 6: organ registrations
        6 => {
            let hs = [1usize, 2, 3, 4, 6, 8, 10, 12, 16];
            let regs = [
                [1.0, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                [1.0, 0.8, 0.0, 0.5, 0.0, 0.3, 0.0, 0.0, 0.0],
                [1.0, 0.0, 0.9, 0.0, 0.7, 0.0, 0.5, 0.0, 0.0],
                [1.0, 0.7, 0.6, 0.5, 0.45, 0.4, 0.3, 0.25, 0.2],
                [0.6, 0.0, 0.4, 0.0, 0.8, 0.0, 1.0, 0.0, 0.9],
            ];
            let x = (t * 4.0).min(3.9999);
            let i = x.floor() as usize;
            let f = x - i as f64;
            let mut s = blank();
            for (j, h) in hs.iter().enumerate() {
                s[*h].1 = regs[i][j] + (regs[i + 1][j] - regs[i][j]) * f;
            }
            s
        }
        // 7: rough noise -> tone
        _ => {
            let p = 2.0 - 1.85 * t;
            let mut seed = 12345u32;
            let mut s = blank();
            for k in 1..N / 2 {
                let ph = lcg(&mut seed) * 2.0 * PI;
                let a = 1.0 / (k as f64).powf(p);
                s[k] = (a * ph.cos(), a * ph.sin());
            }
            s
        }
    }
}

fn build_all() -> Tables {
    let mut out = Vec::new();
    for tb in 0..TABLE_COUNT {
        let specs: Vec<Spec> = (0..BUILTIN_FRAMES).map(|fr| gen_spec(tb, fr as f64 / (BUILTIN_FRAMES - 1) as f64)).collect();
        out.push(Arc::new(Table::from_specs_normalized(&specs)));
    }
    Tables { t: out }
}

// ---------------------------------------------------------------- text encoding (presets / host state)

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        s.push(B64[(n >> 18) as usize & 63] as char);
        s.push(B64[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 { B64[(n >> 6) as usize & 63] as char } else { '=' });
        s.push(if c.len() > 2 { B64[n as usize & 63] as char } else { '=' });
    }
    s
}

pub fn b64_decode(txt: &str) -> Option<Vec<u8>> {
    let mut lut = [255u8; 256];
    for (i, c) in B64.iter().enumerate() {
        lut[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(txt.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in txt.bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' || c == b' ' {
            continue;
        }
        let v = lut[c as usize];
        if v == 255 {
            return None;
        }
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

const Q: f32 = 16384.0;

/// `NWT1|name|frames|base64(i16 samples)`.
pub fn encode_table(name: &str, t: &Table) -> String {
    let mut bytes = Vec::with_capacity(t.frames * N * 2);
    for i in 0..t.frames {
        for v in &t.frame(0, i)[..N] {
            bytes.extend_from_slice(&((v * Q).round().clamp(-32767.0, 32767.0) as i16).to_le_bytes());
        }
    }
    format!("NWT1|{}|{}|{}", name.replace('|', "/"), t.frames, b64_encode(&bytes))
}

/// Returns (name, table).
pub fn decode_table(s: &str) -> Option<(String, Table)> {
    let mut it = s.splitn(4, '|');
    if it.next()? != "NWT1" {
        return None;
    }
    let name = it.next()?.to_string();
    let frames: usize = it.next()?.parse().ok()?;
    let bytes = b64_decode(it.next()?)?;
    if frames < 2 || frames > MAX_FRAMES || bytes.len() < frames * N * 2 {
        return None;
    }
    let mut fr = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut v = Vec::with_capacity(N);
        for i in 0..N {
            let o = (f * N + i) * 2;
            v.push(i16::from_le_bytes([bytes[o], bytes[o + 1]]) as f32 / Q);
        }
        fr.push(v);
    }
    Some((name, Table::from_frames(&fr)))
}

/// Name stored in an encoded table, without decoding it.
pub fn encoded_name(s: &str) -> String {
    s.split('|').nth(1).unwrap_or("").to_string()
}

// ---------------------------------------------------------------- import from audio files

/// Turn mono audio into a wavetable. `frame_size` forces the cycle length (0 = detect).
pub fn from_audio(x: &[f32], clm: Option<usize>, frame_size: usize) -> Option<Table> {
    if x.len() < 16 {
        return None;
    }
    let fsz = if frame_size > 0 {
        frame_size
    } else if let Some(c) = clm.filter(|c| *c >= 64 && x.len() % *c == 0) {
        c
    } else if x.len() >= N && x.len() % N == 0 {
        N
    } else {
        x.len() // a single cycle
    };
    let nfr = (x.len() / fsz).max(1);
    let mut frames: Vec<Vec<f32>> = (0..nfr).map(|i| cycle_to_n(&x[i * fsz..(i + 1) * fsz])).collect();
    if frames.len() == 1 {
        frames.push(frames[0].clone());
    }
    let mut t = Table::from_frames(&frames);
    if t.frames > MAX_FRAMES {
        t = t.resized(MAX_FRAMES, true);
    }
    // common peak normalisation keeps the relative levels of the frames
    let pk = t.peak();
    if pk > 1e-6 {
        let g = 0.95 / pk;
        let scaled: Vec<Vec<f32>> = t.to_frames().into_iter().map(|f| f.into_iter().map(|v| v * g).collect()).collect();
        t = Table::from_frames(&scaled);
    }
    Some(t)
}

/// Resample one cycle to `N` samples (FFT based when the length is a power of two).
fn cycle_to_n(x: &[f32]) -> Vec<f32> {
    let n = x.len();
    if n == N {
        return x.to_vec();
    }
    if n.is_power_of_two() && n >= 8 {
        let mut re: Vec<f64> = x.iter().map(|v| *v as f64).collect();
        let mut im = vec![0.0; n];
        fft(&mut re, &mut im);
        let mut s = vec![(0.0, 0.0); N / 2];
        let kmax = (n / 2 - 1).min(N / 2 - 1);
        for k in 1..=kmax {
            s[k] = (2.0 * re[k] / n as f64, -2.0 * im[k] / n as f64);
        }
        return time_of(&s);
    }
    (0..N)
        .map(|i| {
            let p = i as f32 / N as f32 * n as f32;
            let a = p as usize % n;
            let b = (a + 1) % n;
            let f = p - p.floor();
            x[a] + (x[b] - x[a]) * f
        })
        .collect()
}

/// Time-domain data of a table as one long mono signal (for exporting WAV files).
pub fn table_to_audio(t: &Table) -> Vec<f32> {
    let mut v = Vec::with_capacity(t.frames * N);
    for i in 0..t.frames {
        v.extend_from_slice(&t.frame(0, i)[..N]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_roundtrip() {
        let t = &tables().t[0];
        let s = encode_table("Test", t);
        let (name, d) = decode_table(&s).unwrap();
        assert_eq!(name, "Test");
        assert_eq!(d.frames(), t.frames());
        let a = t.sample(0, 0.4, 0.3);
        let b = d.sample(0, 0.4, 0.3);
        assert!((a - b).abs() < 0.01, "{a} vs {b}");
    }

    #[test]
    fn import_detects_frames() {
        let mut x = Vec::new();
        for f in 0..8 {
            for i in 0..2048 {
                x.push(((i as f32 / 2048.0 * std::f32::consts::TAU * (f + 1) as f32).sin()) * 0.5);
            }
        }
        let t = from_audio(&x, None, 0).unwrap();
        assert_eq!(t.frames(), 8);
        let one = from_audio(&x[..1000], None, 0).unwrap();
        assert_eq!(one.frames(), 2);
    }

    #[test]
    fn resize_and_edit() {
        let t = tables().t[1].resized(16, true);
        assert_eq!(t.frames(), 16);
        let mut t2 = t.clone();
        let z = vec![0.0f32; N];
        t2.set_frame(3, &z);
        assert!(t2.frame_time(3).iter().all(|v| v.abs() < 1e-6));
    }
}

#[cfg(test)]
mod spectral_tests {
    use super::*;

    #[test]
    fn stored_harmonics_rebuild_the_frame() {
        let t = &tables().t[0];
        let (mut a, mut b) = ([0.0f32; SH], [0.0f32; SH]);
        t.spectral_at(10, 0.0, &mut a, &mut b);
        // basic shapes have few harmonics below 64, so the sum matches the mip-0 frame closely at most points
        let fr = t.frame_time(10);
        let mut err = 0.0f32;
        let mut pw = 0.0f32;
        for i in (0..N).step_by(37) {
            let th = std::f32::consts::TAU * i as f32 / N as f32;
            let s: f32 = (0..SH).map(|k| a[k] * ((k + 1) as f32 * th).cos() + b[k] * ((k + 1) as f32 * th).sin()).sum();
            err += (s - fr[i]).abs();
            pw += fr[i].abs();
        }
        assert!(err < pw * 0.35, "partials do not match the frame: err {err} vs {pw}");
    }
}

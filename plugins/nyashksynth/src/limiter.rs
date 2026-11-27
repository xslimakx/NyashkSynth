//! Output limiter: keeps the synth below the ceiling (−0.3 dBFS) so loud chords never clip the
//! DAW / sound card (clipping there is what crackles and hisses).
//!
//! Look-ahead design, transparent: for every sample the gain it needs is computed; the minimum
//! over the look-ahead window is held and then averaged over the same window, so the gain is
//! already down when the loud sample comes out of the delay and moves smoothly (no clicks, no
//! hard clipping). Release is a ~80 ms glide. All memory is allocated in `new` (sized for any
//! sample rate up to 384 kHz), nothing on the audio thread.

const MAXN: usize = 1024;

pub struct Limiter {
    /// look-ahead (samples); the audio is delayed by `n - 1`
    n: usize,
    dl: [Vec<f32>; 2],
    w: usize,
    /// monotonic queue for the minimum of the needed gains over the window
    q: Vec<(usize, f32)>,
    qh: usize,
    qt: usize,
    t: usize,
    /// box average of the held minimum
    hold: Vec<f32>,
    hsum: f64,
    g: f32,
    rel: f32,
    pub ceiling: f32,
}

impl Limiter {
    pub fn new(fs: f32) -> Self {
        let n = ((fs * 0.0015).round() as usize).clamp(8, MAXN - 1);
        Limiter {
            n,
            dl: [vec![0.0; MAXN], vec![0.0; MAXN]],
            w: 0,
            q: vec![(0, 1.0); MAXN * 2],
            qh: 0,
            qt: 0,
            t: 0,
            hold: vec![1.0; MAXN],
            hsum: n as f64,
            g: 1.0,
            rel: 1.0 - (-1.0 / (0.08 * fs)).exp(),
            ceiling: 10f32.powf(-0.3 / 20.0),
        }
    }

    /// Delay of the audio in samples (reported to the host as latency).
    pub fn latency(&self) -> usize {
        self.n - 1
    }

    pub fn reset(&mut self) {
        for d in self.dl.iter_mut() {
            d.fill(0.0);
        }
        self.hold.fill(1.0);
        self.hsum = self.n as f64;
        self.qh = 0;
        self.qt = 0;
        self.t = 0;
        self.w = 0;
        self.g = 1.0;
    }

    /// One stereo sample. `on` false: only the delay (the latency stays the same either way).
    /// Returns the output and the gain applied.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, on: bool) -> (f32, f32, f32) {
        let n = self.n;
        // delay line
        let ri = (self.w + MAXN - (n - 1)) % MAXN;
        let (dl0, dr0) = (self.dl[0][ri], self.dl[1][ri]);
        self.dl[0][self.w] = l;
        self.dl[1][self.w] = r;
        self.w = (self.w + 1) % MAXN;
        if !on {
            self.g = 1.0;
            return (dl0, dr0, 1.0);
        }
        // gain this sample needs
        let pk = l.abs().max(r.abs());
        let need = if pk > self.ceiling { self.ceiling / pk } else { 1.0 };
        // running minimum over the last n samples (monotonic queue)
        let t = self.t;
        self.t = self.t.wrapping_add(1);
        let qn = self.q.len();
        while self.qt != self.qh && self.q[(self.qt + qn - 1) % qn].1 >= need {
            self.qt = (self.qt + qn - 1) % qn;
        }
        self.q[self.qt] = (t, need);
        self.qt = (self.qt + 1) % qn;
        while self.q[self.qh].0.wrapping_add(n) <= t {
            self.qh = (self.qh + 1) % qn;
        }
        let held = self.q[self.qh].1;
        // average of the held minimum over n samples
        let hi = t % n;
        self.hsum += held as f64 - self.hold[hi] as f64;
        self.hold[hi] = held;
        let avg = (self.hsum / n as f64) as f32;
        // down at once (already smooth), up with the release glide
        self.g = if avg < self.g { avg } else { self.g + (avg - self.g) * self.rel };
        let g = self.g;
        // the look-ahead makes the gain exact; the clamp only catches rounding
        let c = self.ceiling;
        ((dl0 * g).clamp(-c, c), (dr0 * g).clamp(-c, c), g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_over_the_ceiling_and_quiet_audio_untouched() {
        let fs = 48000.0;
        let mut lim = Limiter::new(fs);
        let d = lim.latency();
        let mut worst = 0.0f32;
        let mut quiet_err = 0.0f32;
        let mut x = Vec::new();
        for i in 0..96000 {
            let t = i as f32 / fs;
            // a quiet tone, then bursts up to +14 dB, then quiet again
            let amp = if (24000..60000).contains(&i) { if (i / 3000) % 2 == 0 { 5.0 } else { 0.4 } } else { 0.5 };
            x.push((std::f32::consts::TAU * 220.0 * t).sin() * amp);
        }
        for (i, &s) in x.iter().enumerate() {
            let (l, r, _) = lim.tick(s, -s, true);
            worst = worst.max(l.abs()).max(r.abs());
            if i > d && i < 24000 {
                quiet_err = quiet_err.max((l - x[i - d]).abs());
            }
        }
        assert!(worst <= lim.ceiling + 1e-6, "over the ceiling: {worst}");
        assert!(quiet_err < 1e-6, "quiet audio changed by {quiet_err}");
    }

    #[test]
    fn gain_moves_smoothly() {
        // a sudden +20 dB step: the gain never jumps by more than a small amount per sample
        let fs = 48000.0;
        let mut lim = Limiter::new(fs);
        let mut last = 1.0f32;
        let mut jump = 0.0f32;
        for i in 0..48000 {
            let s = if i > 10000 { 10.0 } else { 0.1 } * if i % 2 == 0 { 1.0 } else { -1.0 };
            let (_, _, g) = lim.tick(s, s, true);
            jump = jump.max((g - last).abs());
            last = g;
        }
        assert!(jump < 0.02, "gain jumped by {jump} in one sample");
    }

    #[test]
    fn off_is_a_pure_delay() {
        let mut lim = Limiter::new(44100.0);
        let d = lim.latency();
        let xs: Vec<f32> = (0..2000).map(|i| (i as f32 * 0.37).sin() * 8.0).collect();
        for (i, &s) in xs.iter().enumerate() {
            let (l, _, _) = lim.tick(s, s, false);
            if i >= d {
                assert_eq!(l, xs[i - d]);
            }
        }
    }
}

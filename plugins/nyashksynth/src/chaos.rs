//! Real-time chaotic LFOs: the Lorenz and Rössler systems are integrated sample block by sample block
//! (RK4), so the modulation never repeats. The GUI shows the attractor with the live position.

/// 0 = Lorenz, 1 = Rössler.
#[derive(Clone, Copy, Debug)]
pub struct Chaos {
    pub s: [f64; 3],
    pub kind: i8,
    /// cycles travelled since the start (for one-shot)
    pub cyc: f64,
}

/// Simulated time units per LFO cycle (about one lobe / loop of the attractor).
fn t_cycle(kind: i8) -> f64 {
    if kind == 0 {
        0.9
    } else {
        6.0
    }
}

fn deriv(kind: i8, s: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = s;
    if kind == 0 {
        [10.0 * (y - x), x * (28.0 - z) - y, x * y - 8.0 / 3.0 * z]
    } else {
        [-y - z, x + 0.2 * y, 0.2 + z * (x - 5.7)]
    }
}

fn rk4(kind: i8, s: [f64; 3], h: f64) -> [f64; 3] {
    let add = |a: [f64; 3], b: [f64; 3], k: f64| [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k];
    let k1 = deriv(kind, s);
    let k2 = deriv(kind, add(s, k1, h * 0.5));
    let k3 = deriv(kind, add(s, k2, h * 0.5));
    let k4 = deriv(kind, add(s, k3, h));
    [
        s[0] + h / 6.0 * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]),
        s[1] + h / 6.0 * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]),
        s[2] + h / 6.0 * (k1[2] + 2.0 * k2[2] + 2.0 * k3[2] + k4[2]),
    ]
}

/// A point on the attractor (after a warm-up), computed once per system.
fn warm(kind: i8) -> [f64; 3] {
    use std::sync::OnceLock;
    static W: OnceLock<[[f64; 3]; 2]> = OnceLock::new();
    W.get_or_init(|| {
        let mk = |k: i8| {
            let mut s = if k == 0 { [1.0, 1.0, 1.0] } else { [0.1, 0.0, 0.0] };
            for _ in 0..8000 {
                s = rk4(k, s, 0.005);
            }
            s
        };
        [mk(0), mk(1)]
    })[kind.clamp(0, 1) as usize]
}

impl Chaos {
    pub fn new(kind: i8, seed: u32) -> Self {
        let mut s = warm(kind);
        // every voice / LFO starts a hair away from the others and drifts apart
        let h = |n: u32| ((seed.wrapping_mul(2654435761).wrapping_add(n.wrapping_mul(40503)) >> 8) & 0xFFFF) as f64 / 65535.0 - 0.5;
        s[0] += h(1) * 0.05;
        s[1] += h(2) * 0.05;
        s[2] += h(3) * 0.05;
        Self { s, kind, cyc: 0.0 }
    }

    pub fn idle() -> Self {
        Self { s: [0.0; 3], kind: -1, cyc: 0.0 }
    }

    /// Make sure the state belongs to `kind` (re-seed after a shape change).
    pub fn ensure(&mut self, kind: i8, seed: u32) {
        if self.kind != kind {
            *self = Chaos::new(kind, seed);
        }
    }

    /// Move forward by `cycles` LFO cycles.
    pub fn advance(&mut self, cycles: f64) {
        if self.kind < 0 || !cycles.is_finite() || cycles <= 0.0 {
            return;
        }
        self.cyc += cycles;
        let dt = (cycles * t_cycle(self.kind)).min(2.0);
        let n = ((dt / 0.006).ceil() as usize).clamp(1, 400);
        let h = dt / n as f64;
        for _ in 0..n {
            self.s = rk4(self.kind, self.s, h);
        }
        if !(self.s[0].is_finite() && self.s[1].is_finite() && self.s[2].is_finite()) || self.s.iter().any(|v| v.abs() > 200.0) {
            *self = Chaos::new(self.kind, 7);
        }
    }

    /// Modulation value in -1..1 (the x variable).
    pub fn out(&self) -> f32 {
        view(self.kind, self.s)[2]
    }

    /// Position on the 2-D picture of the attractor, each in -1..1.
    pub fn xy(&self) -> (f32, f32) {
        let v = view(self.kind, self.s);
        (v[0], v[1])
    }
}

/// (picture x, picture y, modulation value), all in -1..1.
fn view(kind: i8, s: [f64; 3]) -> [f32; 3] {
    let c = |v: f64| v.clamp(-1.0, 1.0) as f32;
    if kind == 0 {
        // Lorenz butterfly: x against z
        [c(s[0] / 20.0), c((s[2] - 25.0) / 25.0), c(s[0] / 19.0)]
    } else {
        // Rössler spiral: x against y
        [c((s[0] - 1.0) / 11.0), c((s[1] + 1.0) / 10.0), c((s[0] - 1.0) / 10.5)]
    }
}

/// A long orbit of the attractor for the GUI backdrop: picture coordinates in -1..1.
pub fn orbit(kind: i8, n: usize) -> Vec<(f32, f32)> {
    let mut c = Chaos::new(kind, 99);
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        c.advance(0.02);
        v.push(c.xy());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_systems_stay_bounded_and_move() {
        for kind in [0i8, 1] {
            let mut c = Chaos::new(kind, 5);
            let mut mn = 9.0f32;
            let mut mx = -9.0f32;
            let mut maxstep = 0.0f32;
            let mut last = c.out();
            for _ in 0..40000 {
                c.advance(0.01);
                let o = c.out();
                assert!(o.is_finite() && o.abs() <= 1.0);
                mn = mn.min(o);
                mx = mx.max(o);
                maxstep = maxstep.max((o - last).abs());
                last = o;
            }
            assert!(mx - mn > 0.9, "kind {kind} does not move: {mn}..{mx}");
            assert!(maxstep < 0.35, "kind {kind} jumps: {maxstep}");
        }
    }

    #[test]
    fn different_seeds_drift_apart() {
        let mut a = Chaos::new(0, 1);
        let mut b = Chaos::new(0, 2);
        for _ in 0..3000 {
            a.advance(0.01);
            b.advance(0.01);
        }
        assert!((a.out() - b.out()).abs() > 0.05, "real chaos: tiny differences grow");
    }
}

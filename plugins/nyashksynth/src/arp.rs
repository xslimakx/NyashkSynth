//! Arpeggiator note logic (no audio, no allocation after construction).
use crate::params::ArpMode;

#[derive(Clone, Copy)]
pub struct ArpP {
    pub on: bool,
    pub mode: ArpMode,
    pub beats: f32,
    pub octs: usize,
    pub gate: f32,
    pub swing: f32,
    pub hold: bool,
}

impl Default for ArpP {
    fn default() -> Self {
        Self { on: false, mode: ArpMode::Up, beats: 0.25, octs: 1, gate: 0.6, swing: 0.0, hold: false }
    }
}

pub struct Arp {
    pub held: Vec<(u8, f32)>,
    pub down: Vec<u8>,
    seq: Vec<(u8, f32)>,
    pub out: Vec<(u8, f32)>,
    pub playing: Vec<u8>,
    step: usize,
    pub timer: f64,
    pub gate: f64,
    pub odd: bool,
    latched: bool,
    rng: u32,
}

impl Default for Arp {
    fn default() -> Self {
        Self::new()
    }
}

impl Arp {
    pub fn new() -> Self {
        Self {
            held: Vec::with_capacity(128),
            down: Vec::with_capacity(128),
            seq: Vec::with_capacity(512),
            out: Vec::with_capacity(512),
            playing: Vec::with_capacity(512),
            step: 0,
            timer: f64::INFINITY,
            gate: f64::INFINITY,
            odd: false,
            latched: false,
            rng: 0x9E37_79B9,
        }
    }

    pub fn idle(&self) -> bool {
        self.timer.is_infinite()
    }

    pub fn clear(&mut self) {
        self.held.clear();
        self.down.clear();
        self.playing.clear();
        self.timer = f64::INFINITY;
        self.gate = f64::INFINITY;
        self.latched = false;
        self.step = 0;
        self.odd = false;
    }

    pub fn press(&mut self, note: u8, vel: f32, hold: bool) {
        if hold && self.latched && self.down.is_empty() {
            self.held.clear();
        }
        self.latched = false;
        if self.held.len() >= 120 {
            self.held.remove(0);
        }
        self.down.retain(|n| *n != note);
        self.down.push(note);
        self.held.retain(|(n, _)| *n != note);
        self.held.push((note, vel));
        if self.timer.is_infinite() {
            self.timer = 0.0;
            self.step = 0;
            self.odd = false;
        }
    }

    pub fn release(&mut self, note: u8, hold: bool) {
        self.down.retain(|n| *n != note);
        if hold {
            if self.down.is_empty() {
                self.latched = true;
            }
        } else {
            self.held.retain(|(n, _)| *n != note);
        }
    }

    fn rand(&mut self) -> usize {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as usize
    }

    /// Fills `out` with the notes of the next step.
    pub fn advance(&mut self, p: &ArpP) {
        self.out.clear();
        if self.held.is_empty() {
            return;
        }
        self.seq.clear();
        for &x in self.held.iter() {
            self.seq.push(x);
        }
        if p.mode != ArpMode::AsPlayed {
            self.seq.sort_unstable_by_key(|x| x.0);
        }
        let base = self.seq.len();
        for o in 1..p.octs.clamp(1, 4) {
            for k in 0..base {
                let (n, v) = self.seq[k];
                let nn = n as usize + 12 * o;
                if nn <= 127 && self.seq.len() < 500 {
                    self.seq.push((nn as u8, v));
                }
            }
        }
        let n = self.seq.len();
        let st = self.step;
        let bounce = |st: usize| {
            let period = (2 * n).saturating_sub(2).max(1);
            let k = st % period;
            if k < n {
                k
            } else {
                period - k
            }
        };
        let idx = match p.mode {
            ArpMode::Up | ArpMode::AsPlayed => st % n,
            ArpMode::Down => n - 1 - (st % n),
            ArpMode::UpDown => bounce(st),
            ArpMode::DownUp => n - 1 - bounce(st),
            ArpMode::Random => self.rand() % n,
            ArpMode::Chord => 0,
        };
        if p.mode == ArpMode::Chord {
            for i in 0..n {
                let x = self.seq[i];
                self.out.push(x);
            }
        } else {
            let x = self.seq[idx];
            self.out.push(x);
        }
        self.step = self.step.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(mode: ArpMode, octs: usize, steps: usize, notes: &[u8]) -> Vec<u8> {
        let mut a = Arp::new();
        for n in notes {
            a.press(*n, 1.0, false);
        }
        let p = ArpP { mode, octs, ..Default::default() };
        let mut v = Vec::new();
        for _ in 0..steps {
            a.advance(&p);
            v.push(a.out[0].0);
        }
        v
    }

    #[test]
    fn up_down_patterns() {
        assert_eq!(run(ArpMode::Up, 1, 5, &[60, 64, 67]), vec![60, 64, 67, 60, 64]);
        assert_eq!(run(ArpMode::Down, 1, 4, &[60, 64, 67]), vec![67, 64, 60, 67]);
        assert_eq!(run(ArpMode::UpDown, 1, 6, &[60, 64, 67]), vec![60, 64, 67, 64, 60, 64]);
        assert_eq!(run(ArpMode::Up, 2, 6, &[60, 64]), vec![60, 64, 72, 76, 60, 64]);
        assert_eq!(run(ArpMode::AsPlayed, 1, 3, &[67, 60, 64]), vec![67, 60, 64]);
    }

    #[test]
    fn hold_latches_until_next_press() {
        let mut a = Arp::new();
        a.press(60, 1.0, true);
        a.release(60, true);
        assert_eq!(a.held.len(), 1);
        a.press(64, 1.0, true);
        assert_eq!(a.held.len(), 1);
        assert_eq!(a.held[0].0, 64);
    }
}

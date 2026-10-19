//! Arpeggiator note logic (no audio, no allocation after construction).
use crate::params::ArpMode;

/// One step of the pattern editor (mode "Pattern").
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ArpStep {
    pub on: bool,
    /// Which held note (0 = lowest, wraps round the chord).
    pub idx: u8,
    /// Extra transposition in semitones.
    pub tr: i8,
    pub vel: f32,
}

pub const PAT_STEPS: usize = 16;

impl ArpStep {
    pub fn default_for(i: usize) -> ArpStep {
        ArpStep { on: true, idx: (i % 4) as u8, tr: 0, vel: 1.0 }
    }
}

pub fn default_pattern() -> ([ArpStep; PAT_STEPS], usize) {
    (std::array::from_fn(ArpStep::default_for), 8)
}

/// `len|on,idx,tr,vel;...` (vel 0..100).
pub fn encode_pattern(p: &[ArpStep; PAT_STEPS], len: usize) -> String {
    let mut s = format!("{len}|");
    for (i, st) in p.iter().enumerate() {
        if i > 0 {
            s.push(';');
        }
        s.push_str(&format!("{},{},{},{}", st.on as u8, st.idx, st.tr, (st.vel * 100.0).round() as i32));
    }
    s
}

pub fn parse_pattern(s: &str) -> ([ArpStep; PAT_STEPS], usize) {
    let (mut p, mut len) = default_pattern();
    if s.is_empty() {
        return (p, len);
    }
    let Some((l, rest)) = s.split_once('|') else { return (p, len) };
    len = l.trim().parse::<usize>().unwrap_or(8).clamp(1, PAT_STEPS);
    for (i, part) in rest.split(';').take(PAT_STEPS).enumerate() {
        let mut it = part.split(',');
        let on = it.next().map_or(true, |x| x.trim() != "0");
        let idx = it.next().and_then(|x| x.trim().parse::<u8>().ok()).unwrap_or(0).min(15);
        let tr = it.next().and_then(|x| x.trim().parse::<i8>().ok()).unwrap_or(0).clamp(-24, 24);
        let vel = it.next().and_then(|x| x.trim().parse::<f32>().ok()).unwrap_or(100.0).clamp(0.0, 100.0) / 100.0;
        p[i] = ArpStep { on, idx, tr, vel };
    }
    (p, len)
}

#[derive(Clone, Copy)]
pub struct ArpP {
    pub pat: [ArpStep; PAT_STEPS],
    pub plen: usize,
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
        let (pat, plen) = default_pattern();
        Self { pat, plen, on: false, mode: ArpMode::Up, beats: 0.25, octs: 1, gate: 0.6, swing: 0.0, hold: false }
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
    /// The last step produced no notes on purpose (pattern rest): keep the clock running.
    pub rested: bool,
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
            rested: false,
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

    /// HOLD was switched off while a chord was latched: let it go.
    pub fn unlatch(&mut self) {
        if self.latched && self.down.is_empty() {
            self.held.clear();
        }
        self.latched = false;
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
        self.rested = false;
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
        if p.mode == ArpMode::Pattern {
            let ps = p.pat[st % p.plen.clamp(1, PAT_STEPS)];
            self.step = self.step.wrapping_add(1);
            if !ps.on {
                self.rested = true;
                return;
            }
            let (nt, v) = self.seq[ps.idx as usize % n];
            let nn = (nt as i32 + ps.tr as i32).clamp(0, 127) as u8;
            self.out.push((nn, (v * ps.vel).clamp(0.02, 1.0)));
            return;
        }
        let idx = match p.mode {
            ArpMode::Pattern => 0,
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
    fn pattern_mode_uses_steps() {
        let mut a = Arp::new();
        for n in [60u8, 64, 67] {
            a.press(n, 1.0, false);
        }
        let mut p = ArpP { mode: ArpMode::Pattern, plen: 4, ..Default::default() };
        p.pat[0] = ArpStep { on: true, idx: 0, tr: 0, vel: 1.0 };
        p.pat[1] = ArpStep { on: false, idx: 0, tr: 0, vel: 1.0 };
        p.pat[2] = ArpStep { on: true, idx: 2, tr: 12, vel: 0.5 };
        p.pat[3] = ArpStep { on: true, idx: 4, tr: -12, vel: 1.0 }; // idx wraps: 4 % 3 = 1 -> 64
        a.advance(&p);
        assert_eq!(a.out, vec![(60, 1.0)]);
        a.advance(&p);
        assert!(a.out.is_empty() && a.rested);
        a.advance(&p);
        assert_eq!(a.out, vec![(79, 0.5)]);
        a.advance(&p);
        assert_eq!(a.out[0].0, 52);
        a.advance(&p);
        assert_eq!(a.out[0].0, 60, "pattern must loop");
        let (pt, l) = parse_pattern(&encode_pattern(&p.pat, 4));
        assert_eq!(l, 4);
        assert_eq!(pt[2].tr, 12);
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

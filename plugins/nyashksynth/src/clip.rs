//! Clip sequencer data: 12 clips of up to 4 bars, notes on a step grid (pitch relative to the held key).

pub const NCLIPS: usize = 12;
pub const STEPS_PER_BAR: usize = 16;
pub const MAX_BARS: usize = 4;
pub const MAX_STEPS: usize = STEPS_PER_BAR * MAX_BARS;
pub const MAX_NOTES: usize = 128;
/// Lowest / highest pitch offset (semitones above the key that is held).
pub const PITCH_MIN: i8 = -12;
pub const PITCH_MAX: i8 = 11;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct CNote {
    pub start: u8,
    pub len: u8,
    pub pitch: i8,
    /// 1..=100
    pub vel: u8,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Clip {
    pub bars: u8,
    pub notes: Vec<CNote>,
}

impl Default for Clip {
    fn default() -> Self {
        Clip { bars: 1, notes: Vec::new() }
    }
}

impl Clip {
    pub fn steps(&self) -> usize {
        self.bars.clamp(1, MAX_BARS as u8) as usize * STEPS_PER_BAR
    }

    pub fn encode(&self) -> String {
        let mut s = format!("{}|", self.bars);
        for (i, n) in self.notes.iter().enumerate() {
            if i > 0 {
                s.push(';');
            }
            s.push_str(&format!("{},{},{},{}", n.start, n.len, n.pitch, n.vel));
        }
        s
    }

    pub fn decode(s: &str) -> Clip {
        let Some((b, rest)) = s.split_once('|') else { return Clip::default() };
        let bars = b.trim().parse::<u8>().unwrap_or(1).clamp(1, MAX_BARS as u8);
        let mut c = Clip { bars, notes: Vec::new() };
        let max = bars as usize * STEPS_PER_BAR;
        for part in rest.split(';') {
            let v: Vec<&str> = part.split(',').collect();
            if v.len() < 4 || c.notes.len() >= MAX_NOTES {
                continue;
            }
            let (Ok(st), Ok(len), Ok(pi), Ok(ve)) = (v[0].trim().parse::<u8>(), v[1].trim().parse::<u8>(), v[2].trim().parse::<i8>(), v[3].trim().parse::<u8>()) else { continue };
            if (st as usize) < max {
                c.notes.push(CNote { start: st, len: len.clamp(1, MAX_STEPS as u8), pitch: pi.clamp(PITCH_MIN, PITCH_MAX), vel: ve.clamp(1, 100) });
            }
        }
        c
    }
}

pub fn decode_all(s: &str) -> Vec<Clip> {
    let mut v: Vec<Clip> = s.split('#').take(NCLIPS).map(Clip::decode).collect();
    v.resize(NCLIPS, Clip::default());
    v
}

pub fn encode_all(c: &[Clip]) -> String {
    c.iter().map(|x| x.encode()).collect::<Vec<_>>().join("#")
}

/// The clip as the audio thread sees it.
#[derive(Clone, Copy)]
pub struct ClipP {
    pub on: bool,
    pub n: usize,
    pub steps: usize,
    pub beats: f32,
    pub swing: f32,
    pub notes: [CNote; MAX_NOTES],
}

impl Default for ClipP {
    fn default() -> Self {
        ClipP { on: false, n: 0, steps: STEPS_PER_BAR, beats: 0.25, swing: 0.0, notes: [CNote { start: 0, len: 1, pitch: 0, vel: 100 }; MAX_NOTES] }
    }
}

impl ClipP {
    pub fn from_clip(on: bool, beats: f32, c: &Clip) -> ClipP {
        let mut p = ClipP { on, beats, steps: c.steps(), ..Default::default() };
        for (i, n) in c.notes.iter().take(MAX_NOTES).enumerate() {
            p.notes[i] = *n;
            p.n = i + 1;
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut v = vec![Clip::default(); NCLIPS];
        v[2] = Clip { bars: 2, notes: vec![CNote { start: 3, len: 4, pitch: -5, vel: 80 }, CNote { start: 31, len: 1, pitch: 11, vel: 1 }] };
        let d = decode_all(&encode_all(&v));
        assert_eq!(d, v);
        assert_eq!(decode_all("").len(), NCLIPS);
        assert!(Clip::decode("9|1,1,0,50;999,1,0,50").notes.len() == 1);
    }
}

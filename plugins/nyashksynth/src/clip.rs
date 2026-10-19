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
    /// automation lane: one value per step, -100..100 (empty = none). Drives the CLIP LANE mod source.
    pub lane: Vec<i8>,
}

impl Default for Clip {
    fn default() -> Self {
        Clip { bars: 1, notes: Vec::new(), lane: Vec::new() }
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
        if !self.lane.is_empty() && self.lane.iter().any(|v| *v != 0) {
            s.push('|');
            s.push_str(&self.lane.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(","));
        }
        s
    }

    pub fn decode(s: &str) -> Clip {
        let Some((b, rest)) = s.split_once('|') else { return Clip::default() };
        let (rest, lane_txt) = rest.split_once('|').unwrap_or((rest, ""));
        let bars = b.trim().parse::<u8>().unwrap_or(1).clamp(1, MAX_BARS as u8);
        let mut c = Clip { bars, notes: Vec::new(), lane: Vec::new() };
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
        c.lane = lane_txt.split(',').filter_map(|t| t.trim().parse::<i8>().ok()).map(|v| v.clamp(-100, 100)).take(max).collect();
        c
    }
}

/// Turn recorded MIDI events `(is_on, note, vel 0..1, time in beats)` into a clip.
/// The first note on starts the clip (step 0) and is the key the clip is transposed from.
pub fn clip_from_events(ev: &[(bool, u8, f32, f64)], bars: u8) -> Option<Clip> {
    let first = ev.iter().find(|e| e.0)?;
    let (root, t0) = (first.1 as i32, first.3);
    let bars = bars.clamp(1, MAX_BARS as u8);
    let steps = bars as usize * STEPS_PER_BAR;
    let mut clip = Clip { bars, notes: Vec::new(), lane: Vec::new() };
    for (i, e) in ev.iter().enumerate() {
        if !e.0 || e.3 < t0 {
            continue;
        }
        let st = ((e.3 - t0) * 4.0).round() as i64;
        if st < 0 || st as usize >= steps || clip.notes.len() >= MAX_NOTES {
            continue;
        }
        // length: until the matching note off
        let off = ev[i + 1..].iter().find(|x| !x.0 && x.1 == e.1).map(|x| x.3).unwrap_or(e.3 + 0.25);
        let len = (((off - e.3) * 4.0).round() as i64).clamp(1, (steps as i64 - st).max(1));
        let pitch = (e.1 as i32 - root).clamp(PITCH_MIN as i32, PITCH_MAX as i32) as i8;
        clip.notes.push(CNote { start: st as u8, len: len as u8, pitch, vel: (e.2 * 100.0).round().clamp(1.0, 100.0) as u8 });
    }
    Some(clip)
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
    pub sync: bool,
    pub notes: [CNote; MAX_NOTES],
    pub lane: [i8; MAX_STEPS],
    pub has_lane: bool,
}

impl Default for ClipP {
    fn default() -> Self {
        ClipP { on: false, n: 0, steps: STEPS_PER_BAR, beats: 0.25, swing: 0.0, sync: false, notes: [CNote { start: 0, len: 1, pitch: 0, vel: 100 }; MAX_NOTES], lane: [0; MAX_STEPS], has_lane: false }
    }
}

impl ClipP {
    pub fn from_clip(on: bool, beats: f32, c: &Clip) -> ClipP {
        let mut p = ClipP { on, beats, steps: c.steps(), ..Default::default() };
        for (i, n) in c.notes.iter().take(MAX_NOTES).enumerate() {
            p.notes[i] = *n;
            p.n = i + 1;
        }
        for (i, v) in c.lane.iter().take(MAX_STEPS).enumerate() {
            p.lane[i] = *v;
            p.has_lane = true;
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_quantizes_events_to_steps() {
        // quarter notes at beats 3.0, 3.5 (8th), 4.0: first note is the root
        let ev = vec![(true, 60u8, 0.8f32, 3.0f64), (false, 60, 0.0, 3.4), (true, 64, 1.0, 3.5), (false, 64, 0.0, 3.9), (true, 67, 0.5, 4.0)];
        let c = clip_from_events(&ev, 1).unwrap();
        assert_eq!(c.notes.len(), 3);
        assert_eq!((c.notes[0].start, c.notes[0].pitch, c.notes[0].len), (0, 0, 2));
        assert_eq!((c.notes[1].start, c.notes[1].pitch), (2, 4));
        assert_eq!((c.notes[2].start, c.notes[2].pitch), (4, 7));
        assert_eq!(c.notes[2].vel, 50);
        assert!(clip_from_events(&[(false, 60, 0.0, 1.0)], 1).is_none());
    }

    #[test]
    fn lane_round_trips_and_old_clips_still_load() {
        let mut c = Clip { bars: 1, notes: vec![CNote { start: 0, len: 2, pitch: 3, vel: 90 }], lane: vec![0; 16] };
        c.lane[3] = 50;
        c.lane[7] = -100;
        let back = Clip::decode(&c.encode());
        assert_eq!(back, c);
        let old = Clip::decode("1|0,2,3,90");
        assert!(old.lane.is_empty());
        assert_eq!(old.notes.len(), 1);
    }

    #[test]
    fn round_trip() {
        let mut v = vec![Clip::default(); NCLIPS];
        v[2] = Clip { bars: 2, notes: vec![CNote { start: 3, len: 4, pitch: -5, vel: 80 }, CNote { start: 31, len: 1, pitch: 11, vel: 1 }], lane: vec![] };
        let d = decode_all(&encode_all(&v));
        assert_eq!(d, v);
        assert_eq!(decode_all("").len(), NCLIPS);
        assert!(Clip::decode("9|1,1,0,50;999,1,0,50").notes.len() == 1);
    }
}

//! Noise / sample source data (mono, 44.1 kHz) and custom LFO curves.

use crate::wav;
use crate::wavetable::{b64_decode, b64_encode};

pub const SAMPLE_RATE: u32 = 44100;
pub const MAX_SECONDS: usize = 4;

pub struct NSample {
    pub name: String,
    pub data: Vec<f32>,
}

impl NSample {
    pub fn from_file(path: &std::path::Path) -> Result<NSample, String> {
        let w = wav::read(path)?;
        let mut d = wav::resample(&w.mono, w.rate, SAMPLE_RATE);
        d.truncate(SAMPLE_RATE as usize * MAX_SECONDS);
        if d.len() < 64 {
            return Err("sample is too short".into());
        }
        let pk = d.iter().fold(0.0f32, |p, v| p.max(v.abs()));
        if pk > 1e-6 {
            let g = 0.9 / pk;
            d.iter_mut().for_each(|v| *v *= g);
        }
        let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("sample").to_string();
        Ok(NSample { name, data: d })
    }

    /// `NNS1|name|len|base64(i16)`.
    pub fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(self.data.len() * 2);
        for v in &self.data {
            bytes.extend_from_slice(&((v * 32767.0).round().clamp(-32767.0, 32767.0) as i16).to_le_bytes());
        }
        format!("NNS1|{}|{}|{}", self.name.replace('|', "/"), self.data.len(), b64_encode(&bytes))
    }

    pub fn decode(s: &str) -> Option<NSample> {
        let mut it = s.splitn(4, '|');
        if it.next()? != "NNS1" {
            return None;
        }
        let name = it.next()?.to_string();
        let len: usize = it.next()?.parse().ok()?;
        let bytes = b64_decode(it.next()?)?;
        if bytes.len() < len * 2 || len < 16 {
            return None;
        }
        let data = (0..len).map(|i| i16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]) as f32 / 32767.0).collect();
        Some(NSample { name, data })
    }
}

pub fn encoded_name(s: &str) -> String {
    s.split('|').nth(1).unwrap_or("").to_string()
}

/// Drawn LFO shape: 64 points over one cycle, -1..1.
pub const LFO_POINTS: usize = 64;

pub struct LfoCurve {
    pub pts: [f32; LFO_POINTS],
}

impl LfoCurve {
    pub fn default_curve() -> LfoCurve {
        let mut pts = [0.0; LFO_POINTS];
        for (i, p) in pts.iter_mut().enumerate() {
            *p = (i as f32 / LFO_POINTS as f32 * std::f32::consts::TAU).sin();
        }
        LfoCurve { pts }
    }

    #[inline]
    pub fn eval(&self, f: f32) -> f32 {
        let x = f * LFO_POINTS as f32;
        let i = (x as usize) % LFO_POINTS;
        let fr = x - x.floor();
        let a = self.pts[i];
        let b = self.pts[(i + 1) % LFO_POINTS];
        a + (b - a) * fr
    }

    pub fn encode(&self) -> String {
        let bytes: Vec<u8> = self.pts.iter().map(|v| ((v.clamp(-1.0, 1.0) * 127.0).round() as i8) as u8).collect();
        format!("NLC1|{}", b64_encode(&bytes))
    }

    pub fn decode(s: &str) -> Option<LfoCurve> {
        let rest = s.strip_prefix("NLC1|")?;
        let b = b64_decode(rest)?;
        if b.len() < LFO_POINTS {
            return None;
        }
        let mut pts = [0.0; LFO_POINTS];
        for i in 0..LFO_POINTS {
            pts[i] = (b[i] as i8) as f32 / 127.0;
        }
        Some(LfoCurve { pts })
    }
}

/// One key range of a multisample instrument.
#[derive(Clone)]
pub struct Region {
    pub lo: u8,
    pub hi: u8,
    pub root: u8,
    /// velocity range 1..=127 (SFZ lovel/hivel)
    pub lv: u8,
    pub hv: u8,
    pub smp: std::sync::Arc<NSample>,
}

/// A sampled instrument: several recordings, each for its own range of keys (loaded from .sfz).
pub struct MultiS {
    pub name: String,
    pub regions: Vec<Region>,
}

const MULTI_MAX_REGIONS: usize = 48;
const MULTI_MAX_SAMPLES: usize = SAMPLE_RATE as usize * 24;

fn note_number(s: &str) -> Option<u8> {
    let t = s.trim();
    if let Ok(n) = t.parse::<i32>() {
        return Some(n.clamp(0, 127) as u8);
    }
    let mut ch = t.chars();
    let base = match ch.next()?.to_ascii_lowercase() {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest: String = ch.collect();
    let (acc, oct) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let o: i32 = oct.parse().ok()?;
    Some(((o + 1) * 12 + base + acc).clamp(0, 127) as u8)
}

impl MultiS {
    /// Minimal SFZ reader: `<control> default_path`, `<group>` / `<region>` with
    /// `sample`, `lokey`, `hikey`, `key`, `pitch_keycenter`.
    pub fn from_sfz(path: &std::path::Path) -> Result<MultiS, String> {
        let txt = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        #[derive(Clone, Default)]
        struct Ops {
            sample: Option<String>,
            lo: Option<u8>,
            hi: Option<u8>,
            root: Option<u8>,
            lv: Option<u8>,
            hv: Option<u8>,
        }
        let mut default_path = String::new();
        let mut group = Ops::default();
        let mut cur: Option<Ops> = None;
        let mut regions: Vec<Ops> = Vec::new();
        // tokenise: header tags and key=value pairs (a value may contain spaces up to the next key=)
        let clean: String = txt.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
        let mut toks: Vec<String> = Vec::new();
        let mut rest = clean.as_str();
        while !rest.trim().is_empty() {
            rest = rest.trim_start();
            if rest.starts_with('<') {
                let e = rest.find('>').ok_or("bad sfz")?;
                toks.push(rest[..=e].to_string());
                rest = &rest[e + 1..];
            } else {
                // key=value ; value runs until whitespace followed by "word=" or "<"
                let eq = match rest.find('=') {
                    Some(e) => e,
                    None => break,
                };
                let key = rest[..eq].trim().to_string();
                let after = &rest[eq + 1..];
                let mut end = after.len();
                let b = after.as_bytes();
                let mut i = 0;
                while i < b.len() {
                    if b[i] == b'<' {
                        end = i;
                        break;
                    }
                    if b[i].is_ascii_whitespace() {
                        let tail = after[i..].trim_start();
                        let wlen = tail.find(|c: char| c.is_whitespace() || c == '=' || c == '<').unwrap_or(tail.len());
                        if tail[wlen..].starts_with('=') {
                            end = i;
                            break;
                        }
                    }
                    i += 1;
                }
                toks.push(format!("{}={}", key, after[..end].trim()));
                rest = &after[end..];
            }
        }
        let mut section = String::new();
        let apply = |o: &mut Ops, k: &str, v: &str| match k {
            "sample" => o.sample = Some(v.to_string()),
            "lokey" => o.lo = note_number(v),
            "hikey" => o.hi = note_number(v),
            "key" => {
                let n = note_number(v);
                o.lo = n;
                o.hi = n;
                o.root = n;
            }
            "pitch_keycenter" => o.root = note_number(v),
            "lovel" => o.lv = v.trim().parse::<u8>().ok(),
            "hivel" => o.hv = v.trim().parse::<u8>().ok(),
            _ => {}
        };
        for t in toks {
            if t.starts_with('<') {
                if section == "region" {
                    if let Some(c) = cur.take() {
                        regions.push(c);
                    }
                }
                section = t.trim_matches(|c| c == '<' || c == '>').to_string();
                match section.as_str() {
                    "group" => group = Ops::default(),
                    "region" => cur = Some(group.clone()),
                    _ => {}
                }
            } else if let Some((k, v)) = t.split_once('=') {
                match section.as_str() {
                    "control" => {
                        if k == "default_path" {
                            default_path = v.to_string();
                        }
                    }
                    "group" | "global" | "master" => apply(&mut group, k, v),
                    "region" => {
                        if let Some(c) = cur.as_mut() {
                            apply(c, k, v);
                        }
                    }
                    _ => {}
                }
            }
        }
        if let Some(c) = cur.take() {
            regions.push(c);
        }
        if regions.is_empty() {
            return Err("no <region> found in the .sfz".into());
        }
        let mut out: Vec<Region> = Vec::new();
        let mut total = 0usize;
        let mut cache: std::collections::HashMap<String, std::sync::Arc<NSample>> = std::collections::HashMap::new();
        for r in regions.iter().take(MULTI_MAX_REGIONS * 4) {
            if out.len() >= MULTI_MAX_REGIONS {
                break;
            }
            let Some(sp) = &r.sample else { continue };
            let rel = format!("{}{}", default_path, sp).replace('\\', "/");
            let smp = if let Some(c) = cache.get(&rel) {
                c.clone()
            } else {
                let w = match wav::read(&dir.join(&rel)) {
                    Ok(w) => w,
                    Err(_) => continue,
                };
                let mut d = wav::resample(&w.mono, w.rate, SAMPLE_RATE);
                d.truncate(SAMPLE_RATE as usize * 2);
                if d.len() < 64 {
                    continue;
                }
                if total + d.len() > MULTI_MAX_SAMPLES {
                    let keep = MULTI_MAX_SAMPLES.saturating_sub(total);
                    if keep < 2048 {
                        break;
                    }
                    d.truncate(keep);
                }
                total += d.len();
                let a = std::sync::Arc::new(NSample { name: rel.clone(), data: d });
                cache.insert(rel.clone(), a.clone());
                a
            };
            let root = r.root.or(r.lo).unwrap_or(60);
            out.push(Region { lo: r.lo.unwrap_or(0), hi: r.hi.unwrap_or(127), root, lv: r.lv.unwrap_or(1).clamp(1, 127), hv: r.hv.unwrap_or(127).clamp(1, 127), smp });
        }
        if out.is_empty() {
            return Err("could not load any sample of the .sfz".into());
        }
        let name = path.file_stem().and_then(|x| x.to_str()).unwrap_or("multisample").to_string();
        Ok(MultiS { name, regions: out })
    }

    /// Region for a note: the one whose range contains it, else the one with the nearest root.
    pub fn pick(&self, note: u8, vel: u8) -> Option<&Region> {
        let vel = vel.clamp(1, 127);
        let near = |r: &&Region| (r.root as i32 - note as i32).abs();
        self.regions
            .iter()
            .filter(|r| note >= r.lo && note <= r.hi && vel >= r.lv && vel <= r.hv)
            .min_by_key(near)
            .or_else(|| self.regions.iter().filter(|r| note >= r.lo && note <= r.hi).min_by_key(near))
            .or_else(|| self.regions.iter().min_by_key(near))
    }

    /// `NMS1|name|` then regions joined by `;`, each `lo,hi,root,base64(i16)`.
    pub fn encode(&self) -> String {
        let mut s = format!("NMS2|{}|", self.name.replace('|', "/"));
        for (i, r) in self.regions.iter().enumerate() {
            if i > 0 {
                s.push(';');
            }
            let mut bytes = Vec::with_capacity(r.smp.data.len() * 2);
            for v in &r.smp.data {
                bytes.extend_from_slice(&((v * 32767.0).round().clamp(-32767.0, 32767.0) as i16).to_le_bytes());
            }
            s.push_str(&format!("{},{},{},{},{},{}", r.lo, r.hi, r.root, r.lv, r.hv, b64_encode(&bytes)));
        }
        s
    }

    pub fn decode(s: &str) -> Option<MultiS> {
        let v2 = s.starts_with("NMS2|");
        let rest = s.strip_prefix("NMS2|").or_else(|| s.strip_prefix("NMS1|"))?;
        let (name, body) = rest.split_once('|')?;
        let mut regions = Vec::new();
        for part in body.split(';') {
            let mut it = part.splitn(if v2 { 6 } else { 4 }, ',');
            let lo: u8 = it.next()?.parse().ok()?;
            let hi: u8 = it.next()?.parse().ok()?;
            let root: u8 = it.next()?.parse().ok()?;
            let (lv, hv): (u8, u8) = if v2 { (it.next()?.parse().ok()?, it.next()?.parse().ok()?) } else { (1, 127) };
            let bytes = b64_decode(it.next()?)?;
            if bytes.len() < 32 {
                return None;
            }
            let data: Vec<f32> = bytes.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32767.0).collect();
            regions.push(Region { lo, hi, root, lv, hv, smp: std::sync::Arc::new(NSample { name: String::new(), data }) });
        }
        if regions.is_empty() {
            return None;
        }
        Some(MultiS { name: name.to_string(), regions })
    }
}

pub fn encoded_multi_name(s: &str) -> String {
    s.split('|').nth(1).unwrap_or("").to_string()
}

#[cfg(test)]
mod multi_tests {
    use super::*;

    #[test]
    fn note_names_and_pick() {
        assert_eq!(note_number("c4"), Some(60));
        assert_eq!(note_number("C#3"), Some(49));
        assert_eq!(note_number("db3"), Some(49));
        assert_eq!(note_number("72"), Some(72));
        let mk = |lo, hi, root| Region { lo, hi, root, lv: 1, hv: 127, smp: std::sync::Arc::new(NSample { name: String::new(), data: vec![0.1; 64] }) };
        let m = MultiS { name: "t".into(), regions: vec![mk(0, 59, 48), mk(60, 127, 72)] };
        assert_eq!(m.pick(50, 100).unwrap().root, 48);
        assert_eq!(m.pick(100, 100).unwrap().root, 72);
        // velocity layers
        let mut soft = mk(0, 127, 60);
        soft.hv = 63;
        let mut hard = mk(0, 127, 60);
        hard.lv = 64;
        hard.smp = std::sync::Arc::new(NSample { name: String::new(), data: vec![0.9; 64] });
        let v = MultiS { name: "v".into(), regions: vec![soft, hard] };
        assert!(v.pick(60, 30).unwrap().smp.data[0] < 0.5);
        assert!(v.pick(60, 110).unwrap().smp.data[0] > 0.5);
        let vb = MultiS::decode(&v.encode()).unwrap();
        assert_eq!((vb.regions[1].lv, vb.regions[1].hv), (64, 127));
        let back = MultiS::decode(&m.encode()).unwrap();
        assert_eq!(back.regions.len(), 2);
        assert_eq!(back.regions[1].lo, 60);
        assert_eq!(back.regions[0].smp.data.len(), 64);
    }

    #[test]
    fn reads_an_sfz_with_two_regions() {
        let dir = std::env::temp_dir().join(format!("nyashk_sfz_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let tone = |f: f32| -> Vec<f32> { (0..4410).map(|i| (i as f32 * f * std::f32::consts::TAU / 44100.0).sin() * 0.5).collect() };
        crate::wav::write(&dir.join("lo.wav"), 44100, &tone(200.0), None).unwrap();
        crate::wav::write(&dir.join("hi.wav"), 44100, &tone(800.0), None).unwrap();
        std::fs::write(dir.join("t.sfz"), "<group> loop_mode=no_loop\n<region> sample=lo.wav lokey=c1 hikey=b3 pitch_keycenter=c3\n<region> sample=hi.wav lokey=c4 hikey=c7 pitch_keycenter=c5 // comment\n").unwrap();
        let m = MultiS::from_sfz(&dir.join("t.sfz")).unwrap();
        assert_eq!(m.regions.len(), 2);
        assert_eq!((m.regions[0].lo, m.regions[0].hi, m.regions[0].root), (24, 59, 48));
        assert_eq!((m.regions[1].lo, m.regions[1].root), (60, 72));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

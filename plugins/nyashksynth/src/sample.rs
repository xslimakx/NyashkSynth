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

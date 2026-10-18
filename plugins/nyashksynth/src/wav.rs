//! Minimal WAV reader / writer (PCM 8/16/24/32, float 32/64 in; float32 out).

pub struct Wav {
    pub rate: u32,
    /// Mono mix of all channels.
    pub mono: Vec<f32>,
    /// Frame size announced by a Serum-style `clm ` chunk.
    pub clm: Option<usize>,
}

fn u16le(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32le(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn parse(b: &[u8]) -> Result<Wav, String> {
    if b.len() < 44 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let (mut fmt, mut chans, mut rate, mut bits) = (0u16, 0usize, 0u32, 0u16);
    let mut data: Option<(usize, usize)> = None;
    let mut clm = None;
    let mut o = 12;
    while o + 8 <= b.len() {
        let id = &b[o..o + 4];
        let sz = u32le(b, o + 4) as usize;
        let body = o + 8;
        let end = (body + sz).min(b.len());
        match id {
            b"fmt " if end - body >= 16 => {
                fmt = u16le(b, body);
                chans = u16le(b, body + 2) as usize;
                rate = u32le(b, body + 4);
                bits = u16le(b, body + 14);
                if fmt == 0xFFFE && end - body >= 26 {
                    fmt = u16le(b, body + 24);
                }
            }
            b"data" => data = Some((body, end)),
            b"clm " => {
                let txt = String::from_utf8_lossy(&b[body..end]).to_string();
                if let Some(rest) = txt.strip_prefix("<!>") {
                    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                    clm = digits.parse::<usize>().ok().filter(|v| *v >= 64 && *v <= 16384);
                }
            }
            _ => {}
        }
        o = body + sz + (sz & 1);
    }
    let (ds, de) = data.ok_or("no audio data")?;
    if chans == 0 || rate == 0 {
        return Err("bad format".into());
    }
    let bps = (bits as usize) / 8;
    if bps == 0 {
        return Err("bad bit depth".into());
    }
    let frames = (de - ds) / (bps * chans);
    let mut mono = Vec::with_capacity(frames);
    for f in 0..frames {
        let mut acc = 0.0f32;
        for c in 0..chans {
            let p = ds + (f * chans + c) * bps;
            let v = match (fmt, bits) {
                (1, 8) => (b[p] as f32 - 128.0) / 128.0,
                (1, 16) => i16::from_le_bytes([b[p], b[p + 1]]) as f32 / 32768.0,
                (1, 24) => {
                    let x = (b[p] as i32) << 8 | (b[p + 1] as i32) << 16 | (b[p + 2] as i32) << 24;
                    (x >> 8) as f32 / 8388608.0
                }
                (1, 32) => i32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]) as f32 / 2147483648.0,
                (3, 32) => f32::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3]]),
                (3, 64) => f64::from_le_bytes([b[p], b[p + 1], b[p + 2], b[p + 3], b[p + 4], b[p + 5], b[p + 6], b[p + 7]]) as f32,
                _ => return Err(format!("unsupported WAV format ({fmt}, {bits} bit)")),
            };
            acc += v;
        }
        mono.push(acc / chans as f32);
    }
    Ok(Wav { rate, mono, clm })
}

pub fn read(path: &std::path::Path) -> Result<Wav, String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    parse(&b)
}

/// 32-bit float mono WAV; `clm` adds a Serum-compatible frame-size chunk.
pub fn write(path: &std::path::Path, rate: u32, x: &[f32], clm: Option<usize>) -> Result<(), String> {
    let mut data = Vec::with_capacity(x.len() * 4);
    for v in x {
        data.extend_from_slice(&v.to_le_bytes());
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    if let Some(c) = clm {
        let txt = format!("<!>{c} 10000000 wavetable (www.nyashk.com)");
        let mut t = txt.into_bytes();
        if t.len() % 2 == 1 {
            t.push(0);
        }
        out.extend_from_slice(b"clm ");
        out.extend_from_slice(&(t.len() as u32).to_le_bytes());
        out.extend_from_slice(&t);
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    let total = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&total.to_le_bytes());
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// Linear resampling to a new rate.
pub fn resample(x: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || x.is_empty() {
        return x.to_vec();
    }
    let n = (x.len() as f64 * to as f64 / from as f64) as usize;
    let r = from as f64 / to as f64;
    (0..n)
        .map(|i| {
            let p = i as f64 * r;
            let a = p as usize;
            let f = (p - a as f64) as f32;
            let s0 = x[a.min(x.len() - 1)];
            let s1 = x[(a + 1).min(x.len() - 1)];
            s0 + (s1 - s0) * f
        })
        .collect()
}

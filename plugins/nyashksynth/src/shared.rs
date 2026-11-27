//! Data shared between the GUI and the audio thread: user wavetables, noise sample, LFO curves.
//! The audio thread only ever uses `try_lock` and clones an `Arc`; replaced values are kept
//! alive for a while so the last owner is never dropped on the audio thread.

use crate::params::NLFO;
use crate::sample::{LfoCurve, NSample};
use crate::wavetable::Table;
use std::sync::{Arc, Mutex};

pub struct Slot<T> {
    cur: Mutex<Option<Arc<T>>>,
    retired: Mutex<Vec<Arc<T>>>,
}

impl<T> Slot<T> {
    pub fn new() -> Self {
        Self { cur: Mutex::new(None), retired: Mutex::new(Vec::new()) }
    }

    /// Audio thread: `None` if the GUI is swapping right now (keep the old value).
    pub fn try_get(&self) -> Option<Option<Arc<T>>> {
        self.cur.try_lock().ok().map(|g| g.clone())
    }

    pub fn get(&self) -> Option<Arc<T>> {
        self.cur.lock().ok().and_then(|g| g.clone())
    }

    /// GUI thread.
    pub fn set(&self, v: Option<Arc<T>>) {
        let old = match self.cur.lock() {
            Ok(mut g) => std::mem::replace(&mut *g, v),
            Err(_) => return,
        };
        if let Some(o) = old {
            if let Ok(mut r) = self.retired.lock() {
                r.push(o);
                if r.len() > 8 {
                    r.remove(0);
                }
            }
        }
    }
}

pub struct Shared {
    pub waves: [Slot<Table>; 3],
    pub noise: Slot<NSample>,
    pub ir: Slot<crate::convolve::ConvIr>,
    /// host sample rate (set in `initialize`, used to build impulse responses)
    pub fs: std::sync::atomic::AtomicU32,
    pub smp: [Slot<NSample>; 3],
    pub ms: [Slot<crate::sample::MultiS>; 3],
    pub lfo: [Slot<LfoCurve>; NLFO],
    /// Bumped whenever a slot is changed from state/preset (lets the GUI notice host state loads).
    pub version: std::sync::atomic::AtomicU32,
    /// Things the audio thread let go of whose memory it should not free itself (a replaced
    /// wavetable, sample or impulse response can be megabytes): the GUI drops them. The vectors
    /// have room reserved up front, the audio thread only pushes with `try_lock`.
    pub trash: std::sync::Mutex<Trash>,
}

pub struct Trash {
    pub patches: Vec<crate::engine::Patch>,
    pub irs: Vec<std::sync::Arc<crate::convolve::ConvIr>>,
}

impl Shared {
    /// Hands `x` to the GUI thread to drop, if there is room; otherwise drops it here.
    pub fn retire_patch(&self, p: crate::engine::Patch) {
        if let Ok(mut t) = self.trash.try_lock() {
            if t.patches.len() < t.patches.capacity() {
                t.patches.push(p);
            }
        }
    }
    pub fn retire_ir(&self, ir: std::sync::Arc<crate::convolve::ConvIr>) {
        if let Ok(mut t) = self.trash.try_lock() {
            if t.irs.len() < t.irs.capacity() {
                t.irs.push(ir);
            }
        }
    }
    /// GUI side: free what the audio thread retired (keeps the reserved room).
    pub fn empty_trash(&self) {
        let (p, i): (Vec<_>, Vec<_>) = {
            let Ok(mut t) = self.trash.lock() else { return };
            (t.patches.drain(..).collect(), t.irs.drain(..).collect())
        };
        drop(p);
        drop(i);
    }
}

impl Shared {
    pub fn new() -> Self {
        Self { waves: [Slot::new(), Slot::new(), Slot::new()], noise: Slot::new(), ir: Slot::new(), fs: std::sync::atomic::AtomicU32::new(44100), smp: [Slot::new(), Slot::new(), Slot::new()], ms: [Slot::new(), Slot::new(), Slot::new()], lfo: std::array::from_fn(|_| Slot::new()), version: std::sync::atomic::AtomicU32::new(0), trash: std::sync::Mutex::new(Trash { patches: Vec::with_capacity(32), irs: Vec::with_capacity(8) }) }
    }

    pub fn build_ir(&self, n: &NSample) -> crate::convolve::ConvIr {
        crate::convolve::ConvIr::build(&n.data, crate::sample::SAMPLE_RATE as f32, self.fs.load(std::sync::atomic::Ordering::Relaxed) as f32, &n.name)
    }

    /// Rebuild every slot from the strings stored in the parameters (state load).
    pub fn sync_from_params(&self, p: &crate::SynthParams) {
        for (i, op) in [&p.a, &p.b, &p.c].iter().enumerate() {
            let s = op.wave.lock().map(|g| g.clone()).unwrap_or_default();
            let sm = op.smp.lock().map(|g| g.clone()).unwrap_or_default();
            self.smp[i].set(if sm.is_empty() { None } else { NSample::decode(&sm).map(Arc::new) });
            let mm = op.msmp.lock().map(|g| g.clone()).unwrap_or_default();
            self.ms[i].set(if mm.is_empty() { None } else { crate::sample::MultiS::decode(&mm).map(Arc::new) });
            self.waves[i].set(if s.is_empty() { None } else { crate::wavetable::decode_table(&s).map(|(_, t)| Arc::new(t)) });
        }
        let s = p.noise_sample.lock().map(|g| g.clone()).unwrap_or_default();
        self.noise.set(if s.is_empty() { None } else { NSample::decode(&s).map(Arc::new) });
        let s = p.ir.lock().map(|g| g.clone()).unwrap_or_default();
        self.ir.set(if s.is_empty() { None } else { NSample::decode(&s).map(|n| Arc::new(self.build_ir(&n))) });
        for i in 0..NLFO {
            let s = p.lfo[i].draw.lock().map(|g| g.clone()).unwrap_or_default();
            self.lfo[i].set(if s.is_empty() { None } else { LfoCurve::decode(&s).map(Arc::new) });
        }
        self.version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

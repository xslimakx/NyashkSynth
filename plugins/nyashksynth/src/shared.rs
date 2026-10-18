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
    pub smp: [Slot<NSample>; 3],
    pub lfo: [Slot<LfoCurve>; NLFO],
    /// Bumped whenever a slot is changed from state/preset (lets the GUI notice host state loads).
    pub version: std::sync::atomic::AtomicU32,
}

impl Shared {
    pub fn new() -> Self {
        Self { waves: [Slot::new(), Slot::new(), Slot::new()], noise: Slot::new(), smp: [Slot::new(), Slot::new(), Slot::new()], lfo: std::array::from_fn(|_| Slot::new()), version: std::sync::atomic::AtomicU32::new(0) }
    }

    /// Rebuild every slot from the strings stored in the parameters (state load).
    pub fn sync_from_params(&self, p: &crate::SynthParams) {
        for (i, op) in [&p.a, &p.b, &p.c].iter().enumerate() {
            let s = op.wave.lock().map(|g| g.clone()).unwrap_or_default();
            let sm = op.smp.lock().map(|g| g.clone()).unwrap_or_default();
            self.smp[i].set(if sm.is_empty() { None } else { NSample::decode(&sm).map(Arc::new) });
            self.waves[i].set(if s.is_empty() { None } else { crate::wavetable::decode_table(&s).map(|(_, t)| Arc::new(t)) });
        }
        let s = p.noise_sample.lock().map(|g| g.clone()).unwrap_or_default();
        self.noise.set(if s.is_empty() { None } else { NSample::decode(&s).map(Arc::new) });
        for i in 0..NLFO {
            let s = p.lfo[i].draw.lock().map(|g| g.clone()).unwrap_or_default();
            self.lfo[i].set(if s.is_empty() { None } else { LfoCurve::decode(&s).map(Arc::new) });
        }
        self.version.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}

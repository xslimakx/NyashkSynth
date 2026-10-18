use nih_plug::prelude::*;
use nyashksynth::engine::Patch;
use nyashksynth::shared::Shared;
use nyashksynth::wavetable::{self, tables};
use nyashksynth::*;

#[test]
fn custom_wavetable_survives_project_save_and_load() {
    let src = SynthParams::default();
    let t = tables().t[3].clone();
    let enc = wavetable::encode_table("My Table", &t);
    *src.a.wave.lock().unwrap() = enc.clone();
    *src.preset_name.lock().unwrap() = "Roundtrip".into();
    *src.lfo[1].draw.lock().unwrap() = "NLC1|test".into();
    // what the host stores
    let saved = src.serialize_fields();
    assert!(saved.contains_key("a-wave") || saved.keys().any(|k| k.contains("wave")), "keys: {:?}", saved.keys().collect::<Vec<_>>());

    // fresh instance, host restores the state
    let dst = SynthParams::default();
    dst.deserialize_fields(&saved);
    assert_eq!(*dst.a.wave.lock().unwrap(), enc);
    assert_eq!(*dst.preset_name.lock().unwrap(), "Roundtrip");
    assert_eq!(*dst.lfo[1].draw.lock().unwrap(), "NLC1|test");

    // and the audio side rebuilds the same table
    let sh = Shared::new();
    sh.sync_from_params(&dst);
    let patch = Patch::read_with(&dst, 48000.0, Some(&sh), None);
    let (name, orig) = wavetable::decode_table(&enc).unwrap();
    assert_eq!(name, "My Table");
    assert_eq!(patch.osc[0].tbl.frames(), orig.frames());
    let a = patch.osc[0].tbl.frame_time(10);
    let b = orig.frame_time(10);
    let err = a.iter().zip(b.iter()).fold(0.0f32, |m, (x, y)| m.max((x - y).abs()));
    assert!(err < 1e-4, "table changed after restore: {err}");
}

#[test]
fn garbage_state_is_rejected_not_fatal() {
    let p = SynthParams::default();
    for bad in ["", "NWT1", "NWT1|x|3|@@@@", "NWT1|x|999999|AAAA", "NNS1|1|2", "\u{0}\u{1}\u{2}", "NWT1|x|64|AAAA"] {
        *p.a.wave.lock().unwrap() = bad.to_string();
        *p.noise_sample.lock().unwrap() = bad.to_string();
        *p.lfo[0].draw.lock().unwrap() = bad.to_string();
        let sh = Shared::new();
        sh.sync_from_params(&p);
        let patch = Patch::read_with(&p, 48000.0, Some(&sh), None);
        assert!(patch.osc[0].tbl.frames() >= 2);
    }
}

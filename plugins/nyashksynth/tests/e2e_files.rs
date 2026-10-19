//! End to end with real files: an .sfz instrument on disk through the loader into the engine, and
//! preset packs exported and imported through the real folders.
use nyashksynth::engine::{Engine, Patch};
use nyashksynth::params::OscMode;
use nyashksynth::presets;
use nyashksynth::*;
use std::sync::Arc;

fn goertzel(x: &[f32], f: f32, fs: f32) -> f32 {
    let w = std::f32::consts::TAU * f / fs;
    let (mut re, mut im) = (0.0f32, 0.0f32);
    for (i, v) in x.iter().enumerate() {
        re += v * (w * i as f32).cos();
        im += v * (w * i as f32).sin();
    }
    (re * re + im * im).sqrt() / x.len() as f32
}

fn tone(f: f32) -> Vec<f32> {
    (0..44100 * 2).map(|i| (std::f32::consts::TAU * f * i as f32 / 44100.0).sin() * 0.7).collect()
}

fn play(multi: &Arc<nyashksynth::sample::MultiS>, note: u8, vel: f32) -> Vec<f32> {
    let params = SynthParams::default();
    let mut e = Engine::new(48000.0, Patch::read(&params, 48000.0));
    e.patch.sub_on = false;
    e.patch.fl[0].on = false;
    e.patch.osc[1].on = false;
    e.patch.osc[0].mode = OscMode::Multisample;
    e.patch.osc[0].multi = Some(multi.clone());
    e.note_on(note, vel);
    let mut out = Vec::new();
    for _ in 0..(48000 * 3 / 2 / 64) {
        let (mut l, mut r) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        e.process(&mut l, &mut r);
        out.extend_from_slice(&l);
    }
    out
}

#[test]
fn sfz_on_disk_plays_the_right_recording_and_velocity_layer() {
    let dir = std::env::temp_dir().join(format!("nyashk_e2e_sfz_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("samples")).unwrap();
    for (n, f) in [("low.wav", 220.0), ("soft.wav", 300.0), ("hard.wav", 600.0)] {
        wav::write(&dir.join("samples").join(n), 44100, &tone(f), None).unwrap();
    }
    std::fs::write(
        dir.join("inst.sfz"),
        "<control> default_path=samples/\n<group>\n<region> sample=low.wav lokey=c-1 hikey=b3 pitch_keycenter=a3\n<region> sample=soft.wav lokey=c4 hikey=g9 pitch_keycenter=c4 lovel=1 hivel=63\n<region> sample=hard.wav lokey=c4 hikey=g9 pitch_keycenter=c4 lovel=64 hivel=127\n",
    )
    .unwrap();
    let m = nyashksynth::sample::MultiS::from_sfz(&dir.join("inst.sfz")).expect("sfz loads");
    assert_eq!(m.regions.len(), 3);
    // survives the preset encoding
    let m = Arc::new(nyashksynth::sample::MultiS::decode(&m.encode()).expect("decodes"));
    let seg = |v: &Vec<f32>| v[24000..24000 + 24000].to_vec();
    let a = seg(&play(&m, 57, 0.9)); // A3 = 220 Hz recording
    assert!(goertzel(&a, 220.0, 48000.0) > goertzel(&a, 600.0, 48000.0) * 8.0, "low region");
    let soft = seg(&play(&m, 60, 0.2));
    assert!(goertzel(&soft, 300.0, 48000.0) > goertzel(&soft, 600.0, 48000.0) * 8.0, "soft layer");
    let hard = seg(&play(&m, 60, 0.95));
    assert!(goertzel(&hard, 600.0, 48000.0) > goertzel(&hard, 300.0, 48000.0) * 8.0, "hard layer");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn preset_pack_exports_and_imports_through_real_folders() {
    let cfg = std::env::temp_dir().join(format!("nyashk_e2e_cfg_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cfg);
    std::env::set_var("XDG_CONFIG_HOME", &cfg);
    std::env::set_var("APPDATA", &cfg);
    let p = SynthParams::default();
    presets::save_user(&p, "Pack Test One", "Bass").unwrap();
    presets::save_user(&p, "Pack Test Two", "Lead").unwrap();
    let (path, n) = presets::export_pack("E2E Pack", None).unwrap();
    assert_eq!(n, 2);
    assert!(path.exists() && path.extension().unwrap() == "nypack");
    // wipe the presets and bring them back from the pack
    let pdir = files::presets_dir().unwrap();
    std::fs::remove_dir_all(&pdir).unwrap();
    let got = presets::import_packs().unwrap();
    assert_eq!(got, 2);
    assert!(pdir.join("Bass").join("Pack Test One.nyp").exists());
    assert!(pdir.join("Lead").join("Pack Test Two.nyp").exists());
    // importing again must not overwrite or duplicate
    assert_eq!(presets::import_packs().unwrap(), 0);
    let _ = std::fs::remove_dir_all(&cfg);
}

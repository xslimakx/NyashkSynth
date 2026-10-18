## NyashkSynth v1.1.1

- Wavetable editor rewritten: frame strip with range selection, APPLY TO (frame / selection / all), strength, fade, soft edges, vary and drift, drawing on many frames at once, MORPH, redo, auto-fit wave view (no more clipped waves), SAVE TO LIBRARY.
- RANDOM builds fuller patches (3-10 matrix routes with varied sources/targets, all LFOs, envelope 3, macros, FX) and resets arp/FX first.
- Master level shows its value in dB.
- Drag and drop of `.wav` (wavetable / noise) and `.nyp` presets into the plugin window; SINGLE WAVE button.
- Fewer clicks: notes that cut in on a sounding voice (mono / voice steal) now cross over smoothly, attack and release have a short minimum ramp.
- Stuck-note fixes (GUI watchdog, panic button), faster Vulkan / CPU renderer.

## NyashkSynth v1.0.0

NyashkSynth is a Serum-style wavetable synthesizer VST3 plugin.

Downloads: `NyashkSynth-<OS>-VST3.zip`. Unzip and copy `NyashkSynth.vst3` into your VST3 folder (Linux `~/.vst3/`, Windows `C:\\Program Files\\Common Files\\VST3\\`, macOS `~/Library/Audio/Plug-Ins/VST3/`; macOS builds are unsigned: `xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/NyashkSynth.vst3`).

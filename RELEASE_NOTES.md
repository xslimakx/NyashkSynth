## NyashkSynth v1.1.1

Downloads: `NyashkSynth-Linux-VST3.zip`, `NyashkSynth-Windows-VST3.zip` (unzip and copy the whole `NyashkSynth.vst3` folder into your VST3 folder).

**Known limits:** drag and drop depends on the host passing file drops to the plugin window (works in the standalone app; some DAWs on Linux do not forward it, use BROWSE / FILES... there). The Windows build is tested under Wine only; macOS builds come from CI.

- **Custom point filter**: choose "Custom (points)" as the filter type, click the graph to add up to 8 points, drag them (frequency + gain), mouse wheel = width, double-click = remove. Point types: Bell, Low/High Shelf, High/Low Pass, Notch, Band Pass. KEY and the CUTOFF modulation destination move all points together. 3 example presets.
- Wavetable editor rewritten: frame strip with range selection, APPLY TO (frame / selection / all), strength, fade, soft edges, vary and drift, drawing on many frames at once, MORPH, redo, auto-fit wave view (no more clipped waves), SAVE TO LIBRARY.
- RANDOM builds fuller patches (3-10 matrix routes with varied sources/targets, all LFOs, envelope 3, macros, FX) and resets arp/FX first.
- Master level shows its value in dB.
- Drag and drop of `.wav` (wavetable / noise) and `.nyp` presets into the plugin window; SINGLE WAVE button.
- Fewer clicks: notes that cut in on a sounding voice (mono / voice steal) now cross over smoothly, attack and release have a short minimum ramp.
- Stuck-note fixes (GUI watchdog, panic button), faster Vulkan / CPU renderer.

## NyashkSynth v1.0.0

NyashkSynth is a Serum-style wavetable synthesizer VST3 plugin.

Downloads: `NyashkSynth-<OS>-VST3.zip`. Unzip and copy `NyashkSynth.vst3` into your VST3 folder (Linux `~/.vst3/`, Windows `C:\\Program Files\\Common Files\\VST3\\`, macOS `~/Library/Audio/Plug-Ins/VST3/`; macOS builds are unsigned: `xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/NyashkSynth.vst3`).

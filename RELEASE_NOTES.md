# NyashkSynth v1.0.7

- **Wheels back to standard:** the on-screen pitch and mod wheels cannot be MIDI-learned any more (v1.0.6 allowed it for the mod wheel). They simply follow your keyboard: pitch bend moves the pitch wheel, CC 1 moves the mod wheel. A wheel binding saved by v1.0.6 is dropped automatically.

# v1.0.6 (withdrawn)

- Mod wheel MIDI-learn; removed again in 1.0.7.

# v1.0.5

- **MIDI knobs now bind in VST3 hosts (Bitwig, FL Studio …):** the plugin used to declare "basic" MIDI, so hosts never sent it controller (CC) messages and MIDI LEARN saw nothing. It now asks for full MIDI CCs.
- **Preset browser star folders:** the list had only "4+ stars"; now there is a folder for every rating (5, 4, 3, 2, 1 stars) next to Favorites.
- **FX tab rebuilt.** Graph, signal flow and spectrum no longer overlap; BUS / split knobs sit in the header line. Every effect has a live visualisation (compressor curve, delay taps, reverb decay, EQ with spectrum).
- **Filter points like the EQ:** the FILTER "Points (EQ)" mode and the FX filter have draggable nodes (frequency / gain, scroll = Q); point GAIN / WIDTH are modulation targets.
- Combo boxes truncate instead of overflowing; MULTISAMPLE region editor, matrix rows, preset label and toasts re-laid out.
- **MIDI learn everywhere:** spinners and sliders show the learn ring and `CC` tag; right-click unbinds.
- **Engine fixes (~30 from a full code review):** granular with 1 grain, serial filter routing, LFO smoothing, clip / arpeggiator stop on all-notes-off, fresh envelope on voice steal, sub pan modulation, DC / NaN guards, key-lock / transpose note refcount (no stuck notes), SFZ `<global>/<master>/<group>` inheritance, preset parsing of non-finite values, disabled mod slots no longer drive anything.
- 4 new regression tests (33 engine tests); full suite, pluginval strictness 5 and GUI fuzzing pass.

# NyashkSynth v1.0.4

- **OCT / SEMI spinners and pitch / mod wheels are back left of the keyboard (on by default)**, like Serum. `WHEELS` in SETTINGS can still hide them.
- **OCT and SEMI can now be MIDI-learned** (MIDI LEARN, click the spinner, press your keyboard's transpose control); a bound spinner shows its `CC` tag.
- Interface pass over every page: SUB / NOISE knobs on one grid, VOICE strip (GLIDE / BEND / VOICES / TUNE) no longer overlaps, FX BUS 2 / split knobs moved into the header line, MATRIX rows use the full page height.

# v1.0.3

- **On-screen wheels and OCT / SEMI are hidden by default**, so the keyboard gets the full width. SETTINGS > MIDI / PLAY has a `WHEELS: HIDDEN / SHOWN` switch, and OCT / SEMI spinners now also live there.
- **Docs:** manual and screenshots updated for the new layout.
- **Interface alignment pass:** the FILTER title no longer collides with the 1 / 2 tabs, SUB knobs sit on an even grid, the INV.LEGATO pills are no longer clipped, LFO panels use the free space (taller scope, SYNC / TRIG / 1-SHOT lined up with the knob row), the bottom strip fits the window (TUNE was cut off) and its voice knobs are evenly spaced.

# v1.0.2

- **Step tuning mode** for oscillators (click the label under SEMI): moves by steps of the KEY LOCK scale.
- **More modulation sources:** `FILT 1` and `FILT 2` (filter outputs).
- **More modulation destinations:** all 12 FX knobs of **bus 2**, and the gain / Q of both point filters.
- **DC blocker** in the engine (5 Hz): pulse waves and distortion no longer leave an offset (Square Pulse Lead had 4 % DC).
- **Fix:** the harmonic editor of the wavetable editor could crash when a drag continued after switching zoom or between the amplitude and phase panes.
- **Factory presets:** HIDE / RESTORE buttons in the preset browser (they are built into the plugin, so they cannot be deleted).
- **Docs:** README and MANUAL brought up to date (status table, new sections for everything added since 1.0.0, all annotated screenshots re-made, "where are the presets" table).
- **Tests:** new audit that plays all 72 factory presets through engine and effects (NaN, DC, silence, level, tails) and a CPU-cost report; GUI fuzzing on every page found no further crash.

- **New modulation sources:** `OSC A / OSC B / OSC C` (the output of an oscillator, usable for cross-modulation of anything) and `LANE` (the automation lane of the active clip).
- **Clip automation lane:** under the piano roll of the clip sequencer; drag up/down on a step to set a value (-100..+100), right-click resets. Send it anywhere with the LANE chip. Stored with the clip; old presets load unchanged.
- **Multisample velocity layers:** `lovel` / `hivel` of `.sfz` files are read; the region is chosen by key *and* velocity.
- **Region mapping editor:** click a bar in the Multisample display, then edit its key range, root note and velocity range. Changes are heard immediately and saved with the preset.
- **MIDI out:** SETTINGS > MIDI / PLAY > `MIDI OUT` sends the notes made by the arpeggiator and the clip sequencer to the host.
- **Fix:** Wind noise could blow up after switching the noise type while playing (state is now kept in range).
- Mod source chips re-arranged (15 per row) to fit the four new sources.

---

# Development history

These builds were numbered 2.x / 3.x during development; the numbering was reset so the current release is **1.0.2**.

## NyashkSynth v3.3.0

- **Spectral oscillator**: oscillator mode list (WT / SMP / GRN / MS / Spectral) is now a drop-down. Spectral resynthesises the wavetable frame from its first 64 partials with TILT (darker / brighter), SHIFT (stretch partials: harmonic -> bell-like) and EVEN/ODD balance. One voice per note (no unison).
- **Clip sequencer**: play-head moves over the piano roll; CAPTURE turns the notes you just played into the clip (quantised to 1/16, first note = key); SYNC starts the clip on the host's step grid.
- **FX page**: SIGNAL FLOW strip shows the chain order of the selected bus, which modules are on, and how the routing ends.
- **Preset packs**: EXPORT PACK bundles all your presets into one .nypack file, IMPORT PACKS unpacks every .nypack from the Packs folder (nothing is overwritten).

## NyashkSynth v3.2.0

- **Multisample oscillator (MS)**: LOAD SFZ opens .sfz instruments from the Noises folder; every key plays the recording of its own range (key ranges, root keys, shared .wav files). Saved with presets and projects.
- **LFO draw grid and shapes**: in Custom LFO mode, GRID (4 / 8 / 16 / 32 steps) turns drawing into a step sequencer, SHAPES > fills the drawing with a ready-made shape (sine, triangle, saws, square, pulse, stairs, bounce, exp decay, random steps).
- **HYBRID** button (SOUND page, tools): mixes the current sound with a random preset, section by section (oscillators, filters, envelopes, LFOs, matrix rows, effects).

## NyashkSynth v3.1.0

- **Convolve reverb**: choose "Convolve" in the REVERB algorithm list and load any .wav as an impulse response (room, hall, cabinet, anything). Partitioned FFT convolution, up to ~1.4 s of IR; SIZE trims the tail. The IR is saved with presets and projects.
- **Oscillator tuning modes**: click the SEMI label of an oscillator to switch between SEMI (semitones), HARM (harmonic number 1-32) and RATIO (free frequency ratio 0.25-16).
- Oscillator header: COPY / PASTE are now compact `C` / `P` buttons so they no longer overlap the WT / SMP / GRN buttons.

## NyashkSynth v3.0.0

- **Arp pattern banks**: 12 pattern slots (BANK 1-12) on the SEQ page, saved with the preset.
- **Clip swing**: delays every second step of the clip sequencer.
- **Key lock**: SETTINGS -> KEY LOCK moves every incoming note to the nearest note of a scale (12 scales + root). Also feeds the arp and clips.
- **Matrix**: every row has an on/bypass button (the row number) and ^ / v buttons to reorder rows.
- **Oscillator COPY / PASTE** in every oscillator header: copies all settings including wavetable and sample.

## NyashkSynth v2.9.0

- **Split Low / Mid / High routing**: two crossovers (LOW/MID, MID/HI); lows go to FX bus 1, mids to bus 2, highs stay dry.
- **Distortion**: new **Overdrive** type and **DC BIAS** knob (asymmetry / even harmonics).
- **Reverb algorithms**: Plate, Vintage, Nitrous, Basin.
- **Delay HQ**: cubic interpolation and smooth time changes (tape-like glides).

## NyashkSynth v2.8.0

- **Granular oscillator mode** (GRN): position, grain size, density (up to 256 grains/s), spray; works on a loaded sample.
- **Spectrum analyser** on the FX page and behind the filter graph.
- **Preset ratings (stars) and tags** in the preset browser, "4+ stars" filter, tag filters and tag search.

## NyashkSynth v2.7.0

- **Pages like Serum**: SOUND, MOD, MATRIX, FX, SEQ, SETTINGS instead of one small tab block. All 8 LFOs are visible together, the matrix shows all 16 routes, arp and clip sequencer share the SEQ page. Window is a little taller (1458 x 902).

## NyashkSynth v2.6.0

- **Clip sequencer**: 12 clips of 1-4 bars with a piano roll; a held key plays the clip transposed from that key.
- **SETTINGS tab**: A4 reference (400-480 Hz), scale tunings (Just, Pythagorean, Meantone, Werckmeister III) with root, MIDI channel filter, velocity curve, learned-CC list.
- LFO and envelope title chips can be dragged onto any knob.

## NyashkSynth v2.5.0

Downloads: `NyashkSynth-Linux-VST3.zip`, `NyashkSynth-Windows-VST3.zip` (unzip and copy the whole `NyashkSynth.vst3` folder into your VST3 folder). Illustrated guide: `docs/MANUAL.md`.

- **MIDI learn**: MIDI LEARN button, click any knob, move a CC on your keyboard. Right-click to unbind, CLEAR removes all. Saved with the project.
- **OCT / SEMI transpose spinners** (click, drag, mouse wheel, double-click reset) and on-screen **pitch and mod wheels**; the wheels follow hardware wheels.
- **Third oscillator (OSC C)**, with its own filter routing and mod destinations; COPY OSC between oscillators.
- **Two filters** (F1/F2) in series or parallel, **Custom (points)** filter type.
- **8 LFOs** (Lorenz and Rössler chaos shapes, rate up to 1000 Hz), **4 envelopes** with INV.LEGATO, **8 macros**.
- **Two FX buses** with series / parallel / low-high / mid-side routing, plus **Bode** frequency shifter and **Utility**.
- Master level can be typed in dB; deleting a user preset also removes empty folders.
- **Sample oscillator mode** (WT / SMP switch per oscillator): start / end markers, one-shot / loop / ping-pong, loop crossfade, ROOT, modulatable start and end.
- **Arp Pattern mode**: 16-step editor (on / rest, velocity, chord note, transpose, length).
- **Undo / redo** (UNDO / REDO buttons, Ctrl+Z / Ctrl+Y).
- Not yet (planned): Granular / Spectral / SFZ oscillator modes, clip sequencer, arp banks, Convolve.

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

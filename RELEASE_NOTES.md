# NyashkSynth v1.0.5

- **Alt-tab fix (Linux):** after alt-tab, minimising or covering the window, parts of the picture (backgrounds, knobs, images) could stay blank until something on them moved. The window is now drawn again in full whenever the system throws its contents away, and pictures are no longer lost while the window is hidden or a frame is skipped.
- Smoother images in the CPU renderer (used when Vulkan is not available): pictures drawn smaller than their real size, such as knobs and photos, are now scaled with mipmaps instead of picking single pixels, so they no longer look grainy or jagged.
- **Display fix:** the built-in CPU renderer (used when Vulkan is not available) drew pixels on the shared edges of triangles twice, which showed as brighter seams and specks in see-through fills (spectra, curves, bars). It now uses an exact fill rule like a GPU: every pixel is drawn once, without holes.
- Phase: typing a value in degrees works (typing 1 gave 360 degrees).
- Modulation amount: typed values are read as percent (typing -99 gave -100).
- These parameters can now be typed in the host as they are shown: velocity curve, MIDI channel ("Omni", "Ch 3"), compressor ratio ("4.0:1"), utility width, arp gate and all effect knobs.
- Times from 999.5 ms show as seconds, so typing "1.00 s" keeps showing "1.00 s".
- Value boxes have a small gap between them, and a value that would not fit its box is drawn smaller instead of running over the frame.
- Clicking a value box and pressing Enter no longer changes a value whose unit matters (for example "1.03 s" was read back as 1.03 ms).
- No more "-0%" / "-0.00" read-outs (pan, curves, modulation).
- New automatic test: every parameter's text is typed back over its whole range and must give the same value.

# NyashkSynth v1.0.4

A big update: real chaos, clean FM, a modulation system you can read, an effect rack with repeatable effects, a new silver look and 75 new presets.

**New look and handling**
- **Monochrome silver design** (black, white, silver-grey): one dark style for every list, field and button, flat light "on" buttons with animated hover. Icon buttons in the header (save, MIDI learn, clear, undo, redo, previous / next preset). Envelopes in four separate cards; the unison MODE box and the master value box match the knob value boxes.
- **146 factory presets** (75 new): leads, pads, keys, plucks, arps, bells, synth chords and FX / textures, mostly mid and high register with a wide choice of effects.
- **Mono mode** restarts the sound when you go back to a key that is still held (Legato only changes the pitch, as on hardware synths).

**Polish round (Oct 25)**
- **FM / AM / RM source in the warp list** (like Serum): `FM (B)`, `FM (C)`, `FM (Sub)`, `FM (Noise)` and the same for AM and RM; an oscillator can not modulate itself. **Each warp slot has its own source**, so FM from B and AM from C work together. Older presets keep their source.
- **Noise type `None`** (first in the list) switches the noise off completely.
- **Chaos LFO picture:** the trail now follows the attractor itself, so it stays smooth at any rate (no more criss-cross lines); every LFO sits on its own card.
- **Animated effect pictures:** Flanger (moving comb filter), Phaser (sweeping notches), Chorus (delay time with a live dot), Hyper (unison copies drifting, stereo dimension) and Utility (stereo field: width, balance, gain, mono, polarity), next to the existing graphs of EQ, filter, compressor, delay, reverb, distortion and Bode.
- **Unison row:** VOICES, DETUNE, BLEND, SPREAD and MODE sit in their own row; **noise is off by default** (type `None`; old presets keep their white noise).
- **Layout pass:** LFO cards have inner padding (SYNC / TRIG / 1-SHOT no longer touch the frame); FX tab has a header card (BUS 1 / BUS 2, routing, `BUS 2 LEVEL` next to it, signal path) and cards for the chain list and the effect detail; switched-off oscillators and the SUB step back (dimmed).
- **Settings in three cards** (sound and tuning / MIDI and play / bindings); **MATRIX** shows empty slots dimmed so the used routes stand out.
- **Wavetable editor re-laid out in four blocks:** FRAMES (insert, delete, copy, count, interpolation, save), DRAW / SHAPES / FORMULA, APPLY TO (scope, fade, strength, soft edges, vary, drift) and PROCESS. Bigger wave and harmonic views.

**Oscillators and sound quality**
- **FM / AM / RM source** is chosen in the warp list: Osc A / B / C / Sub / Noise (Auto keeps the old A<B, B<A, C<B for older presets).
- **Clean FM:** the wavetable oscillators now oversample (x2 / x4 / x8) with a long Kaiser-sinc decimator, FM runs with zero-delay routing and cubic table interpolation. In the test the aliasing of a bright FM patch fell by roughly a factor of 1000 (x1 2e-2 -> x4 1e-7).
- **Settings -> SOUND QUALITY -> OSC OVERSAMPLING**: Auto (x2 for light warps, x4 for FM / sync / fold), Draft x1, High x2, Ultra x4, Max x8.

- **Unison modes Exp and Inv** (voices bunched in the centre / at the edges).

**LFOs**
- **Chaos LFOs are real:** Lorenz and Rössler are integrated in real time (RK4), per voice or free running, so they never repeat. The LFO page draws the attractor with a trail and a live position circle, like Serum. Every other LFO shape shows its live position as a circle on the curve too.

**Modulation**
- **One ring per source** on a knob (several sources no longer sit on top of each other); drag a ring to change that source only.
- **Right-click a modulated knob:** list of its sources with amount slider, BI (bipolar) toggle and a remove button for each; `Remove all`.
- Drag and drop: the highlighted knob is the one under the pointer, the drag can no longer get stuck (no more flickering circle), drops use the complete knob list of the frame.

- Right-click menu of a modulated knob: `off` bypasses one source (it stays listed as bypassed and can be switched `on`), `Reset control` returns the knob to default.

- **Lock parameter:** right-click any knob -> `Lock parameter`; loading a preset leaves locked controls untouched (saved with the project).

**Effects**
- **Effect rack:** up to 10 effects per bus, any order, and the **same effect can be added as many times as you like** (3 delays, 2 reverbs ...). `+ ADD EFFECT` menu, drag rows to reorder, dot = on/off, `x` = remove, DUPLICATE, RESET. The detail view builds its knobs from the effect description, with live graphs (EQ and filter nodes, compressor curve, delay taps, reverb decay, distortion curve).
- **Old presets and projects convert automatically** into rack slots (checked against all factory presets).
- **Bode frequency shifter is complete:** modes Shift / Ring mod / Spread (left up, right down), SHIFT +-5 kHz and FINE, an LFO on the shift (rate, depth), feedback with a tone filter in the loop, mix, and a map showing where every input frequency lands.

**Interface and platform**
- macOS: resizing the window no longer jumps or collapses into a small square (window size is now handled in host points, also in NyashkTune and NyashkOpto).
- Computer keyboard plays notes (Ableton layout: the A S D F G H J K L row = white keys, W E T Y U O P = black keys, Z / X = octave) and keeps working after you click the title bar.
- Tooltips and the SETTINGS help text describe the new right-click menu.

**Tests**
- New: chaos bounds and divergence, FM aliasing by quality and by source, Bode modes and LFO, repeated effects, old-preset conversion on all factory presets, effect description round trips. Full suite, 13-minute GUI-parameter fuzz and the CPU audit pass.


# v1.0.3

**MIDI**
- MIDI knobs now bind in VST3 hosts (Bitwig, FL Studio ...): the plugin now asks the host for full MIDI CCs, so MIDI LEARN sees your controller.
- Every knob, slider and the OCT / SEMI spinners are MIDI-learnable; a bound control shows a `CC` tag, right-click unbinds. The on-screen wheels follow your keyboard (pitch bend and CC 1) and are not re-assignable.

**Interface**
- OCT / SEMI spinners and the pitch / mod wheels are back left of the keyboard (on by default, `WHEELS` in SETTINGS can hide them); the wheels show their values (bend in semitones, mod 0-127).
- FX tab rebuilt: graph, signal flow and spectrum no longer overlap, every effect has a live visualisation, effects without a graph give the room to a taller spectrum.
- Filter "Points (EQ)" mode and the FX filter have draggable nodes like the EQ; the node only grows when the pointer is on it.
- Alignment pass over every page: drop-downs fill exactly their space, oscillator knob grid spans the panel, UNI MODE on the same rows as the knobs, SUB knobs on the NOISE grid, MASTER header lined up, MATRIX rows use the page height.
- Preset browser: a star folder for every rating (5, 4, 3, 2, 1) next to Favorites.
- Names, categories, tags and search accept any language and symbols (Ukrainian, crosses, stars ...): a DejaVu Sans font is built in, CJK uses a system font when present; AltGr types on Windows.

**Engine and fixes** (~30 from a full code review)
- Granular with 1 grain, serial filter routing, LFO smoothing, arpeggiator / clip stop on all-notes-off, fresh envelope on voice steal, sub pan modulation, DC / NaN guards, key-lock / transpose note refcount (no stuck notes), SFZ `<global>/<master>/<group>` inheritance, preset parsing of non-finite values, disabled mod slots no longer drive anything.
- 4 new regression tests; full suite, pluginval (strictness 5) and GUI fuzzing pass.

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

# NyashkSynth manual

Quick visual guide. Every numbered badge in a picture is explained under it.

## 0. Pages

Like Serum, the plugin is split into pages (buttons under the header):

| Page | What is on it |
|---|---|
| **SOUND** | 3 oscillators, 2 filters, sub / noise, 4 envelopes |
| **MOD** | all 8 LFOs at once |
| **MATRIX** | all 16 modulation routes in two columns |
| **FX** | effects chain, two buses, routing |
| **SEQ** | arpeggiator (with pattern editor) and clip sequencer |
| **SETTINGS** | tuning, MIDI, learned CCs |

The mod-source chips, macros, voice settings, keyboard and wheels stay visible on every page.

## 1. Overview

![Overview](img/manual_overview.png)

1. **Oscillators A / B / C**: each has a mode box (**Wavetable, Sample, Granular, Multisample, Spectral**), a wavetable view, BROWSE, two warp slots, POS, WARP, WARP 2, LEVEL, PAN, OCT, SEMI, FINE, UNISON, DETUNE, BLEND, SPREAD, PHASE, RAND. `EDIT WAVE` opens the wavetable editor; the small `C` / `P` buttons copy and paste a whole oscillator.
2. **Tuning mode**: click the small label under the SEMI knob to switch SEMI -> HARM (harmonic number) -> RATIO (frequency ratio) -> STEP (steps of the KEY LOCK scale).
3. **Filters**: two filters (tabs **1** and **2**), `SER` toggles series / parallel. 16 types plus **Custom (points)**: drag up to 8 EQ-style points on the graph. `ROUTE` picks which sources feed the selected filter.
4. **MIDI LEARN**: switch it on, click any knob, then move a knob/fader on your MIDI keyboard. The knob shows `CCn`. Right-click a bound knob to unbind, `CLEAR` removes all bindings. Bindings are saved with the project.
5. **Presets**: browse (`<` `>` or the list), SAVE, HIDE factory presets, DELETE your own, export and import packs.
6. **TOOLS** (under SUB / NOISE): INIT, RANDOM (a whole new patch), COPY and **HYBRID** (mix the current sound with another preset).
7. **Mod sources**: drag a chip onto any knob; drag vertically on the knob to set the depth. Right-click the modulation ring to remove it. Besides envelopes, LFOs, macros, velocity, key, wheel, aftertouch and random there are `OSC A/B/C`, `FILT 1/2` (outputs) and `LANE` (clip automation).
8. **8 macros**: assign them like any other source.
9. **OCT / SEMI spinners and wheels**: click the arrows, drag, use the mouse wheel; double-click resets. They transpose both MIDI input and the on-screen keys. The pitch wheel springs back, the mod wheel stays where you leave it; both follow your hardware wheels.
10. **Master**: drag, or double-click and type a dB value.

## 2. Effects

![FX](img/manual_fx.png)

1. **Chain slot**: click to edit, drag onto another slot to reorder. Twelve effects: Hyper, Dist, Flanger, Phaser, Chorus, Delay, Comp, Reverb, EQ, Filter, Bode (frequency shifter), Utility (gain, pan, width, mono, invert).
2. **Dot** = on/off.
3. **BUS 1 / BUS 2**: two identical chains, so you can use an effect twice. Drag a mod-source chip onto an effect knob while the bus tab is open: the route targets that bus.
4. **Routing**: Bus 1 > Bus 2 (series), parallel, split low/high, split low/mid/high, split mid/side.
5. **BUS 2** sets the level of the second bus.
6. **Signal flow**: the order of the selected bus and a text line for the routing.
7. **Output spectrum** (live).

The reverb has algorithms Plate, Vintage, Nitrous, Basin and **Convolve** (press LOAD IR and pick a `.wav` impulse response). Distortion has an Overdrive type and a DC BIAS knob; Delay has an HQ switch.

## 3. LFOs

![LFO](img/manual_lfo.png)

1. Switch between LFO 1-4 and LFO 5-8.
2. Shapes include **Lorenz** and **Rössler** chaos, drawable curve, random and more.
3. SYNC (tempo), TRIG (restart on note), 1-SHOT.
4. Rate goes up to 1000 Hz; RISE, SMOOTH, DELAY and PHASE shape the output.

## 4. Playing with a MIDI keyboard

* Notes, velocity, pitch bend, mod wheel, aftertouch and sustain pedal are handled.
* OCT / SEMI shift incoming notes; notes already held are released correctly when you change the shift.
* MIDI learn works for every parameter: oscillator, filter, envelope, LFO, FX and macro knobs.

## 5. Modulation matrix

![Matrix](img/manual_matrix.png)

1. **Row number**: click to bypass a route (it turns off); the route keeps its settings.
2. **Source**: 33 sources, including oscillator outputs, filter outputs and the clip LANE.
3. **Destination**: every knob, both FX buses (`FX2 ...` entries are bus 2) and the point filter (`Filter 1 Points Gain`, `Filter 1 Points Q`, ...).
4. **Amount** (bipolar) and a **curve** slider next to it.
5. **UNI**: make a bipolar source unipolar (0..1). The **SCALED BY** box multiplies the route by a second source.
6. `^` `v` move the row up / down, `x` clears it.
7. The source chips at the bottom are the quick way to add a route: drag one onto a knob and the route appears here by itself.

## 6. Sample oscillator

![Sample oscillator](img/manual_sample.png)

Each oscillator can be switched between **WT** (wavetable) and **SMP** (sample) with the two buttons next to ON.

1. Drag the start / end markers on the waveform, or use the **START / END** knobs (both can be modulated, so you can scan through a sample).
2. **LOAD SAMPLE** opens the sample browser (your `Noises` folder); you can also drop a `.wav` on the oscillator. The first 4 seconds are used.
3. **One-shot / Loop / Ping-pong**; **XFADE** crossfades the loop seam.
4. **ROOT** shifts the pitch the sample was recorded at. A sample plays at its original speed on middle C, follows the keyboard, OCT / SEMI / FINE and unison.
5. Sample oscillators go through the same filters, envelopes and FX as wavetable ones.

## 7. Granular oscillator

![Granular](img/manual_granular.png)

Press **GRN** next to ON. The oscillator plays a cloud of short grains read from a loaded sample.

* **POS** where in the sample the grains start; modulate it with an LFO or envelope to scan through the sample. The marker on the waveform shows it (drag it), the shaded band is the **SPRAY** range.
* **SIZE** grain length (5-500 ms), **DENSITY** grains per second (up to 256), **SPRAY** random spread of the grain positions.
* Grains play at the key pitch (ROOT shifts it). Overall level is kept steady when you raise density.

## 8. Arpeggiator pattern editor

![Arp pattern](img/manual_arp.png)

Set **MODE = Pattern** in the ARPEGGIATOR panel of the SEQ page. 16 steps, each with on / rest, velocity, which held note it plays (1 = lowest, wraps round the chord) and a transposition in semitones. Right-click a step number to end the pattern there. The pattern is saved with presets and projects.

## 9. Clip sequencer

![Clip sequencer](img/manual_clip.png)

Open the **SEQ** page, go to the CLIP SEQUENCER panel and switch **CLIP ON**. Hold a key: the selected clip plays and loops, transposed from the key you hold (the **KEY** row of the piano roll is that key). Releasing the key stops it.

1. **CLIP ON** replaces the arpeggiator while it is on.
2. **12 clip slots**; a dot marks slots that contain notes. Clips are saved with the preset.
3. **BARS** (1-4) and **STEP** (grid size, 1/16 by default).
4. **SWING** delays every second step.
5. **CAPTURE**: play something on your keyboard, press CAPTURE and the notes become the clip (the first note you played is step 1 and the held key).
6. **SYNC**: start the clip on the host bar instead of at the moment you press a key.
7. **LANE**: one automation value per step (-100..+100). Drag up / down on a step, right-click resets. Use the **LANE** chip as a modulation source.
8. Piano roll: click = add a note, drag right = longer, drag a note = move it (also up / down), right-click = delete, mouse wheel over a note = velocity (brighter = louder). A white play-head runs while the clip plays.
9. **KEY** row = the key you hold. Range is one octave below to 11 semitones above it.

## 10. Settings tab

![Settings](img/manual_settings.png)

1. **A4 reference** from 400 to 480 Hz with quick buttons for 432 / 440 / 442 / 444, and a fine tune in cents.
2. **Scale tuning**: Equal, Just intonation, Pythagorean, Meantone 1/4, Werckmeister III, with a **root** note. The bars show the deviation from equal temperament in cents.
3. **KEY LOCK**: choose a scale and a root; every incoming note snaps to the nearest scale note (also feeds the arpeggiator and clips, and the STEP tuning mode of the oscillators).
4. **MIDI channel** (0 = omni), **velocity curve**, **pitch-bend range**, glide.
5. **MIDI OUT**: send arpeggiator and clip notes to the host.
6. **MIDI bindings**: every learned CC with its parameter name and an x to remove it.

LFOs and envelopes can be dragged onto any knob straight from their title chips in the LFO / ENV tabs, like in Serum.

## 11. Spectrum analyser

![FX page with spectrum](img/manual_fx.png)

The FX page shows a live output spectrum (20 Hz - 20 kHz, log scale). A dim copy of the same spectrum also runs behind the filter graph, so you can see what you are cutting.

## 12. Preset browser

![Preset browser](img/manual_presets.png)

1. **Stars** on the left of a row rate a preset (click the same star again to clear it).
2. **Favorite** star on the right.
3. Left list: all, favorites, "4+ stars", one entry per category and one per tag. Select a preset, type comma separated tags in the TAGS box and press SET.
4. **Search** also matches tags.
5. **HIDE** removes a factory preset from the list (they are built into the plugin and cannot be deleted); **DELETE** appears for your own presets; **RESTORE** brings hidden ones back.
6. **EXPORT PACK / IMPORT PACKS**: share your presets as one `.nypack` file.

Ratings and tags live in `meta.txt` next to your presets, so they are not lost when presets are re-installed.

## 13. Undo / redo

UNDO and REDO (header, or Ctrl+Z / Ctrl+Y) step through your parameter changes, including preset loads. Wavetable-editor edits have their own undo inside the editor.

## 14. Feature status compared to Serum 2

| Area | Status |
|---|---|
| 3 oscillators: wavetable, sample, granular, **multisample (SFZ)**, **spectral** | done |
| Dual warp, unison, sub, noise, tuning modes Semi / Harm / Ratio / Step | done |
| 2 filters, series/parallel, custom point filter | done |
| 8 LFOs (grid, shapes, Lorenz/Rössler), 4 envelopes, 8 macros | done |
| 2 FX buses, Bode, Utility, split routing (L/H and L/M/H, M/S), Convolve reverb | done |
| 33 modulation sources (incl. oscillator and filter outputs, clip lane) | done |
| FX bus 1 / bus 2 and point-filter modulation destinations | done |
| MIDI learn, transpose, wheels, key lock | done |
| Arp pattern editor with 12 banks, clip sequencer (swing, sync, capture, lane) | done |
| MIDI out of arp / clip notes | done |
| Region mapping editor with velocity layers | done |
| Preset browser: stars, tags, favorites, packs, HYBRID | done |
| Undo / redo, spectrum analyser, settings (tuning, MIDI channel, velocity curve) | done |
| Not done | per-region loop points in Multisample, SFZ round-robin / release samples, per-point modulation of individual filter points |

## What is new in 1.0.2

### Sequencing, keys and matrix

- **Arp BANK 1-12** (SEQ page, Pattern mode): twelve pattern slots; click a number to edit another pattern. A dot marks an edited slot.
- **Clip SWING** slider next to the clip STEP selector.
- **KEY LOCK** (SETTINGS page): choose a scale and root; played notes snap to the nearest scale note.
- **Matrix rows**: click the row number to bypass the row, `^` / `v` move it up or down, `x` clears it.
- **COPY / PASTE** buttons in each oscillator header.

### Reverb and tuning modes

- **Convolve reverb** (FX page, REVERB, ALGORITHM = Convolve): press LOAD IR and pick a .wav. SIZE shortens the tail, MIX sets the wet level, LOW/HIGH CUT shape it.
- **Tuning modes**: click the label under the SEMI knob of an oscillator: SEMI -> HARM (harmonic number) -> RATIO (frequency ratio).

### Oscillators, clips and packs

- **Multisample oscillator** (`.sfz`): choose *Multisample* in the mode box, press LOAD SFZ and pick an `.sfz` file from the Noises folder. Every key plays the recording whose key range contains it (`lokey`, `hikey`, `key`, `pitch_keycenter`). Up to 48 regions.
- **Spectral oscillator**: 64 additive partials per wavetable frame. Knobs: POS (frame), TILT (bright/dark), SHIFT (move all partials), EVEN/ODD (balance of even and odd harmonics). Unison is off in this mode.
- **Clip SYNC**: the clip starts on the host bar instead of when you press the key. **CAPTURE** turns the notes you just played into the selected clip. A white play-head runs over the piano roll.
- **Preset packs**: `EXPORT PACK` writes all your presets into one `.nypack` file (Packs folder), `IMPORT PACKS` unpacks every pack found there.

### LFO grid, HYBRID and FX flow

- **LFO grid and shapes**: in *Custom* LFO mode the GRID pill snaps the drawn curve to a grid and SHAPES inserts a preset curve (sine, triangle, saw, square, stairs, bounce, exp decay, random steps).
- **HYBRID**: press it in the TOOLS strip, pick another preset and the sound is mixed from both presets (every oscillator, filter, LFO, matrix row and effect comes from one or the other, by chance).
- **FX signal flow** strip on the FX page shows the effect order of the selected bus and the routing text.

### Modulation, MIDI out and fixes

- **New modulation sources**: `OSC A`, `OSC B`, `OSC C` (oscillator output, for cross-modulation), `FILT 1`, `FILT 2` (filter output) and `LANE` (the automation lane of the active clip).
- **Clip automation lane**: under the piano roll, one value per step (-100 to +100). Drag up / down on a step to set it, right-click resets. Drag the LANE chip onto any knob.
- **Multisample velocity layers** (`lovel` / `hivel`) and a **region map editor**: click a bar in the Multisample display, then edit its key range, root note and velocity range on the right. Changes are heard at once and saved with the preset.
- **Step tuning**: click the label under the SEMI knob until it reads STEP. The knob then moves the oscillator by steps of the KEY LOCK scale (+2 in C major = E).
- **New modulation destinations**: all twelve FX knobs of **bus 2** (drag a chip onto a knob while the BUS 2 tab is open) and the **gain** and **Q** of the point filter (matrix only: `Filter 1 Points Gain` and so on).
- **MIDI OUT** (SETTINGS > MIDI / PLAY): sends the notes made by the arpeggiator and the clip sequencer to your host.
- **DC blocker** (5 Hz) in the engine: pulse waves and distortion no longer leave an offset; fixes a rare blow-up of the *Wind* noise after switching the noise type while playing.
- **Factory presets**: HIDE / RESTORE buttons in the preset browser.

## Where are the presets, and can I delete them?

| What | Where | Delete? |
|---|---|---|
| 72 factory presets | built into the plugin (not files) | no, but **HIDE** removes one from the browser and **RESTORE n** brings them all back (the list of hidden ones is `hidden.txt` in the data folder) |
| Your presets | `~/.config/NyashkSynth/Presets/<Category>/<Name>.nyp` (Windows `%APPDATA%\NyashkSynth\Presets`) | yes, **DELETE** in the browser or delete the file |
| Packs | `.../NyashkSynth/Packs/*.nypack` | yes, plain files |
| Tables, noises, `.sfz` | `.../NyashkSynth/Tables`, `.../Noises` | yes, plain files |

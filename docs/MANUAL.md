# NyashkSynth manual

Quick visual guide. Every numbered badge in a picture is explained under it.

## 1. Overview

![Overview](img/manual_overview.png)

1. **Oscillators A / B / C**: each has a wavetable view, BROWSE, two warp slots, POS, WARP, WARP 2, LEVEL, PAN, OCT, SEMI, FINE, UNISON, DETUNE, BLEND, SPREAD, PHASE, RAND. `EDIT WAVE` opens the wavetable editor. COPY OSC (bottom left) copies an oscillator to another.
2. **Filter**: two filters (tabs **1** and **2**), `SER` toggles series / parallel. 16 types plus **Custom (points)**: drag up to 8 EQ-style points on the graph. `ROUTE` picks which sources feed the selected filter.
3. **MIDI LEARN**: switch it on, click any knob, then move a knob/fader on your MIDI keyboard. The knob shows `CCn`. Right-click a bound knob to unbind, `CLEAR` removes all bindings. Bindings are saved with the project.
4. **SUB / NOISE** with INIT, RANDOM and COPY OSC tools.
5. **Tabs**: ENV (4 envelopes, INV.LEGATO), LFO (8 LFOs on two pages), MATRIX (16 slots), FX, ARP.
6. **Mod sources**: drag a chip onto any knob; drag vertically on the knob to set the depth. Right-click the modulation ring to remove it.
7. **8 macros**: assign them like any other source.
8. **OCT / SEMI spinners and wheels**: click the arrows, drag, use the mouse wheel; double-click resets. They transpose both MIDI input and the on-screen keys. The pitch wheel springs back, the mod wheel stays where you leave it; both follow your hardware wheels.
9. **Master**: drag, or double-click and type a dB value.
10. **Presets**: browse, SAVE; the delete button also removes empty category folders.

## 2. Effects

![FX](img/manual_fx.png)

1. **Chain slot**: click to edit, drag onto another slot to reorder.
2. **Dot** = on/off.
3. **Bode** (frequency shifter) and **Utility** (gain, pan, width, mono, invert).
4. **Routing**: Bus 1 > Bus 2 (series), parallel, split low/high, split mid/side. Buses 1 and 2 hold the same effects, so you can use an effect twice.
5. **BUS 2** sets the level of the second bus.

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

## 5. Feature status compared to Serum 2

| Area | Status |
|---|---|
| 3 wavetable oscillators, dual warp, unison, sub, noise | done |
| 2 filters, series/parallel, custom point filter | done |
| 8 LFOs (with Lorenz/Rössler), 4 envelopes (invert legato), 8 macros | done |
| 2 FX buses, Bode, Utility, split routing | done |
| MIDI learn, transpose, wheels | done |
| Sample / Multisample / Granular / Spectral oscillator modes | not yet |
| Clip sequencer, arp pattern banks, oscillator mapping editor | not yet |
| Convolve, reverb types, preset tags and ratings | not yet |

# NyashkSynth v1.0.5

- **MIDI knobs now bind in VST3 hosts (Bitwig, FL Studio …):** the plugin used to declare "basic" MIDI, so hosts never sent it controller (CC) messages and MIDI LEARN saw nothing. It now asks for full MIDI CCs.
- **Preset browser star folders:** the list had only "4+ stars"; now there is a folder for every rating (5, 4, 3, 2, 1 stars) next to Favorites.
- **FX tab rebuilt.** Graph, signal flow and spectrum no longer overlap; BUS / split knobs sit in the header line. Every effect has a live visualisation (compressor curve, delay taps, reverb decay, EQ with spectrum).
- **Filter points like the EQ:** the FILTER "Points (EQ)" mode and the FX filter have draggable nodes (frequency / gain, scroll = Q); point GAIN / WIDTH are modulation targets.
- Combo boxes truncate instead of overflowing; MULTISAMPLE region editor, matrix rows, preset label and toasts re-laid out.
- **MIDI learn everywhere:** spinners and sliders show the learn ring and `CC` tag; right-click unbinds.
- **Engine fixes (~30 from a full code review):** granular with 1 grain, serial filter routing, LFO smoothing, clip / arpeggiator stop on all-notes-off, fresh envelope on voice steal, sub pan modulation, DC / NaN guards, key-lock / transpose note refcount (no stuck notes), SFZ `<global>/<master>/<group>` inheritance, preset parsing of non-finite values, disabled mod slots no longer drive anything.
- 4 new regression tests (33 engine tests); full suite, pluginval strictness 5 and GUI fuzzing pass.


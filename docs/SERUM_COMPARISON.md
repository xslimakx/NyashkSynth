# NyashkSynth 1.0.4 vs Serum 1 / Serum 2 (analysis)

| Area | Serum | NyashkSynth 1.0.4 | Status |
|---|---|---|---|
| Wavetable osc, warp, FM/AM/RM, sync | yes, FM source = other osc | A/B/C/Sub/Noise selectable, oversampled x1-x8 | done |
| Quality setting | Oversampling / quality in global | Settings -> SOUND QUALITY | done |
| Unison | Linear/Super/Exp/Inv/Random, Stack, Width, Range, Blend, up to 16 | Linear/Super/Random/Stack, width, detune, blend | done in 1.0.4: Exp and Inv added (Stack and detune range already existed) |
| Chaos LFO | Lorenz/Rossler, live position | RK4 real time, attractor + live dot | done |
| Mod UX | ring per source, right-click list, bipolar, remove, reset | same, plus Reset control and per-source bypass (1.0.4) | done |
| FX rack | 15 modules, reorder, repeat | 12 kinds, repeat, reorder, Bode full | Hyper/Dimension, Flanger, Utility, Bode present; only Splitter L/H, L/M/H, M/S missing |
| Voice control / MTS-ESP / tuning files | yes | scale + key lock only | gap |
| Lock parameter on preset load, MIDI learn | yes | both (right-click -> Lock parameter, 1.0.4) | done |
| Undo/redo | yes | yes (Ctrl/Cmd+Z, +Shift+Z / Y) | done |
| Resize UI | yes | yes, mac fixed | done |

Gaps are listed for the next updates; nothing here was published to GitHub.

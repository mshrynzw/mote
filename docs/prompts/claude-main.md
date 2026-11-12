# Claude への依頼（Main の設計図）

このファイルは依頼文であり、設計の正本ではない。正本は `docs/synth.md` と `docs/synth.mmd` である。

以下を、そのまま Claude に貼る。

---

You are designing a static HTML/CSS mock of the Main screen for a software synthesizer named Mote. This is a picture to view in a browser. It will not be embedded in the plugin. Do not write a synth engine, JavaScript audio, presets, or a settings screen.

All visible text is English.

## Default size and layout

The default artboard is 1000px wide and landscape, similar to a compact desktop synth. When the window is narrower, sections wrap instead of shrinking into unreadability. Do not design a separate phone app. Responsive means the layout follows the window width.

Use round knobs, not sliders. Repeat one Oscillator panel four times. Do not design four different oscillator layouts.

## Colors

Use only these RGB colors:

- Background: 15, 24, 57
- Text and panel fill: 241, 241, 241
- Frames and secondary lines: 164, 182, 183
- Active or selected values: 159, 197, 233

## What to draw

Four parallel voice chains. Each chain is one Oscillator, its own Filter, its own Filter Env, and its own Amp Env. The four chains meet at one Master.

Mod Env 1–4 and LFO 1–4 are separate panels. Do not draw wires from every modulator to every knob. There is no Matrix, no Part A/B, and no effects rack.

Inside each Oscillator, draw a small Unison display. The horizontal axis is pan. The vertical axis is detune. Each unison voice is one dot.

Do not print numeric ranges or default values. Those are not decided. Show the control name only.

## Controls

Every Oscillator has the same controls:

- Pitch: Octave, Note, Fine
- Output: Volume, Pan, Stereo
- Wave: Wave, Phase, Inv, Retrigger
- Unison: Voices, Detune

Every Filter has the same controls, plus a Sync toggle that links Filter 1–4 settings together: Type, Amount, Drive, Cutoff, Resonance.

Every Filter Env has the same controls, plus a Sync toggle. Filter Env 1 moves only Filter 1 Cutoff. Filter Env 2, 3, and 4 follow the same pairing. Controls: Attack, Decay, Sustain, Release, Amount.

Every Amp Env has the same controls, plus a Sync toggle: Attack, Decay, Sustain, Release. Do not add Cutoff to Amp Env.

Every Mod Env has the same controls, plus a Sync toggle: Attack, Decay, Sustain, Release, Amount.

Every LFO has the same controls, plus a Sync toggle that links LFO 1–4 settings together. The LFO control named Sync means tempo sync. Controls: Wave, Rate, Sync, Phase, Retrigger, Amount.

Master controls: Master Volume, Transpose, Bend Range, Mode (Poly / Mono), Mono Legato, Portamento.

## Do not add

Do not add parameters, sections, logos of other products, or feature ideas that are not listed above. If something does not fit, leave space rather than inventing a control.

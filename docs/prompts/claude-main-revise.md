# Claude への依頼（Main の見た目を直す）

このファイルは依頼文であり、設計の正本ではない。正本は `docs/synth.md` と `docs/synth.mmd` である。

対象は、すでにある `design-mock/main/index.html` である。以下を、そのまま Claude に貼る。

---

You are an excellent designer. Push beyond the limits of your own capabilities. Take as much time as you need to create your work.

Revise the existing Mote Main screen at design-mock/main/index.html. You are an excellent designer. Go beyond your usual limits. Taking a long time is fine.

This file is a browser mock only. It will not be embedded in the plugin. Do not write a synth engine or JavaScript audio. Do not add Preset or Setting screens.

Push the look toward something contemporary and dreamlike. The background and the edges of the panels may be fantastical. If it helps, use GSAP, Three.js, or a shader for the background. Animate the Unison dots slowly. Load libraries from a CDN if you need them, and keep the result as this one HTML file plus files in design-mock/main/.

The controls stay readable. Knob names stay in the light color rgb(241, 241, 241) or the active color rgb(159, 197, 233), on the dark background rgb(15, 24, 57). Do not hide a label inside the effect. Frames may still use rgb(164, 182, 183).

Do not add, remove, or rename controls. Do not add Matrix, Part A/B, or an effects rack. Do not print numeric ranges. Keep every existing English label, including both LFO Sync controls. Keep four voice chains, Filter Env paired to the same-numbered Filter Cutoff, Mod Env 1–4, LFO 1–4, Master, and the Unison display inside each Oscillator.

The default width stays about 1000px. Narrower windows may wrap sections. Do not turn this into a phone app.

# シンセのイメージ

信号の流れの正本は [synth.mmd](synth.mmd)、ノブの名前の正本はこの文書である。この文書には図を写さない。

Phase の範囲、完了条件、計算方式は [design.md](design.md) を優先する。

## 信号の流れ

Oscillator 1〜4 は、それぞれ自分の Filter と Amp Env を通り、Master で合流する。Filter Env 1〜4 は、同じ番号の Filter の Cutoff だけを動かす。Mod Env 1〜4 と LFO 1〜4 は、Oscillator、Filter、Amp Env のすべてに割り当てられる。

## Oscillator（1〜4 共通）

4基とも同じ項目を持つ。

### Pitch

- Octave
- Note
- Fine

### Output

- Volume
- Pan
- Stereo

### Wave

- Wave
- Phase
- Inv（Waveの反転）
- Retrigger

### Unison

- Voices
- Detune

## Filter（1〜4 共通）

4基とも同じ項目を持つ。各 Filter の設定を、同期の ON/OFF で揃えられる。

- Type
- Amount
- Drive
- Cutoff
- Resonance

## Amp Env（1〜4 共通）

4基とも同じ項目を持つ。各 Amp Env の設定を、同期の ON/OFF で揃えられる。

- Attack
- Decay
- Sustain
- Release

## Filter Env（1〜4 共通）

4基とも同じ項目を持つ。各 Filter Env の設定を、同期の ON/OFF で揃えられる。Filter Env 1 は Filter 1 の Cutoff だけを動かし、2、3、4 も同じ対応にする。Amount は、そのエンベロープで Cutoff をどれだけ動かすかである。

- Attack
- Decay
- Sustain
- Release
- Amount

## Mod Env（1〜4 共通）

4基とも同じ項目を持つ。各 Mod Env の設定を、同期の ON/OFF で揃えられる。割り当て先は Oscillator、Filter、Amp Env の各設定である。

- Attack
- Decay
- Sustain
- Release
- Amount

## LFO（1〜4 共通）

4基とも同じ項目を持つ。各 LFO の設定を、同期の ON/OFF で揃えられる。割り当て先は Oscillator、Filter、Amp Env の各設定である。ここでの Sync は、テンポに合わせる設定である。

- Wave
- Rate
- Sync（テンポに合わせる）
- Phase
- Retrigger
- Amount

## Master

以下の項目を持。

- Master Vlolume
- Transpose
- Bend Range
- Mode (Poly / Mono)
- Mono Legato
- Portamento

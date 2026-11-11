# シンセのイメージ

信号の流れの正本は [synth.mmd](synth.mmd)、ノブの名前の正本はこの文書である。この文書には図を写さない。

Phase の範囲、完了条件、計算方式は [design.md](design.md) を優先する。

## 信号の流れ

Oscillator 1〜4 は、それぞれ自分の Filter と Amp Env を通り、Master で合流する。Filter Env 1〜4 は、同じ番号の Filter の Cutoff だけを動かす。Mod Env 1〜4 と LFO 1〜4 は、Oscillator、Filter、Amp Env のすべてに割り当てられる。

## Main のタブ

画面上部に ALL、OSC 1、OSC 2、OSC 3、OSC 4 を置く。

ALL は、4 本の列を一覧する。大きな図は置かない。

OSC 1 は、Oscillator 1 とその Filter、Filter Env、Amp Env だけを表示する。波形、Unison の点、エンベロープの曲線を大きく描く。OSC 2、OSC 3、OSC 4 も同じ対応にする。

Master、Mod Env、LFO はタブの外に残し、どのタブでも見える。

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

Amp Env、Filter Env、Mod Env は、Attack、Decay、Sustain、Release のそれぞれに Curve を持つ。中央の初期値は直線で、片方は最初が急で後から緩やかになり、もう片方は最初が緩やかで後から急になる。LFO には付けない。音への反映は Phase 2 の各 Env で行い、画面にノブが並ぶのは Phase 3 である。

## Amp Env（1〜4 共通）

4基とも同じ項目を持つ。各 Amp Env の設定を、同期の ON/OFF で揃えられる。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve

## Filter Env（1〜4 共通）

4基とも同じ項目を持つ。各 Filter Env の設定を、同期の ON/OFF で揃えられる。Filter Env 1 は Filter 1 の Cutoff だけを動かし、2、3、4 も同じ対応にする。Amount は、そのエンベロープで Cutoff をどれだけ動かすかである。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve
- Amount

## Mod Env（1〜4 共通）

4基とも同じ項目を持つ。各 Mod Env の設定を、同期の ON/OFF で揃えられる。割り当て先は Oscillator、Filter、Amp Env の各設定である。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve
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

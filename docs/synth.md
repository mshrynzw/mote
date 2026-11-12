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

4基とも同じ項目を持つ。初期状態でオンなのは Oscillator 1 だけである。2、3、4 はオフで、オフのあいだはその基を計算しない。

範囲と初期値は、Phase 3 の Setting 画面で変えられるようにする。それまでは、ここに書いた値を本体の初期値にする。

- On。Oscillator 1 の初期値はオン。2、3、4 はオフ

### Pitch

- Octave。初期値 0。範囲は -2 から +2
- Note。初期値 0。範囲は -24 から +24
- Fine。単位はセント。初期値 0。範囲は -100 から +100

### Output

- Volume。単位は dB。初期値 0 dB。範囲は -60 dB から +12 dB
- Pan
- Stereo

### Wave

最初に実装するのは Classic だけである。Classic は、サイン、三角、ノコギリ、パルスを計算で出す。Stacked Saw と Wavetable は名前だけ残し、Classic が鳴ってから別の作業にする。Stacked Saw は 1 基の中でノコギリを重ねる。Wavetable は、作っておいた 1 周期の波を読み替える。

- Wave
- Phase
- Inv（Waveの反転）
- Retrigger

### Unison

Detune の単位はセント。初期値 0。範囲は 0 から 100。最大のとき、いちばん外側の Voice が ±100 セントになる。

配置は十字が基準である。横軸が Pan、縦軸が Detune、交点が中央である。各 Voice は別の横位置に、等間隔で置く。いちばん外側でも軸の端には載せない。8 Voice のとき、左端に 4 つ、右端に 4 つとはしない。Detune も同じ順で、上下が鏡になるように振る。音程と定位の中心は交点のままである。

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

# シンセのイメージ

信号の流れの正本は [synth.mmd](synth.mmd)、ノブの名前の正本はこの文書である。この文書には図を写さない。

Phase の範囲、完了条件、計算方式は [design.md](design.md) を優先する。

## 信号の流れ

Oscillator 1〜4 は、それぞれ自分の Filter と Amp Env を通り、Master で合流する。Filter Env 1〜4 は、同じ番号の Filter の Cutoff だけを動かす。Mod Env 1〜4 と LFO 1〜4 は、Oscillator、Filter、Amp Env のすべてに割り当てられる。

## Main のタブ

画面上部に ALL、A、B、C、D を置く。A は 1 本目の列、B、C、D も同じ順である。昔の「2 基でフィルターを共有するパート」ではない。

ALL は、4 本の列を一覧する。大きな図は置かない。

A は、その列の Oscillator、Filter、Filter Env、Amp Env だけを表示する。波形、Unison の点、エンベロープの曲線を大きく描く。B、C、D も同じ対応にする。

Master、Mod Env、LFO はタブの外に残し、どのタブでも見える。

## Oscillator（1〜4 共通）

4基とも同じ項目を持つ。表示名は A On、B Octave のように列の文字を付ける。初期状態でオンなのは A だけである。B、C、D はオフで、オフのあいだはその基を計算しない。

範囲と初期値は、Phase 3 の Setting 画面で変えられるようにする。それまでは、ここに書いた値を本体の初期値にする。

- On。A の初期値はオン。B、C、D はオフ

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
- Retrigger。初期値はオフ。オンのときだけ、ノートオンで Phase の位置から打ち直す

### Unison

Detune の単位はセント。初期値 0。範囲は 0 から 100。最大のとき、いちばん外側の Voice が ±100 セントになる。

配置は十字が基準である。横軸が Pan、縦軸が Detune、交点が中央である。各 Voice は別の横位置に、等間隔で置く。いちばん外側でも軸の端には載せない。8 Voice のとき、左端に 4 つ、右端に 4 つとはしない。Detune も同じ順で、上下が鏡になるように振る。音程と定位の中心は交点のままである。

- Voices
- Detune

## Filter（1〜4 共通）

4基とも音の項目は同じである。揃えるスイッチは A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列のノブは A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。

Type の初期値は Lowpass。Lowpass、Highpass、Bandpass、Notch から選ぶ。Cutoff は 20 Hz から 20 kHz、初期値は 20 kHz。低い周波数の方がノブの動きが大きい。Resonance、Drive、Amount は 0% から 100%、初期値は 0%。Resonance を上げると Cutoff 付近の山が立つ。自己発振はしない。Drive はフィルターのあとで tanh により歪ませる。Amount は MIDI ノート 60 を基準に、鍵盤の高さで Cutoff を追う。100% のとき、1 オクターブ上がると Cutoff は 2 倍になる。この計算はノブへ書き戻さない。フィルターと Drive は 2 倍のオーバーサンプリングで計算する。

- Type
- Amount
- Drive
- Cutoff
- Resonance
- Sync
- Sync B
- Sync C
- Sync D

Amp Env、Filter Env、Mod Env は、Attack、Decay、Sustain、Release のそれぞれに Curve を持つ。中央の初期値は直線で、片方は最初が急で後から緩やかになり、もう片方は最初が緩やかで後から急になる。LFO には付けない。音への反映は Phase 2 の各 Env で行い、画面にノブが並ぶのは Phase 3 である。

## Amp Env（1〜4 共通）

4基とも音の項目は同じである。揃えるスイッチは A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列のノブは A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。

Attack、Decay、Release の初期値は 0 秒、0.3 秒、0.05 秒。範囲は 0 秒から 10 秒。Sustain の初期値は 1、範囲は 0 から 1。Curve の初期値は 0（直線）、範囲は -1 から +1。-1 は最初が急、+1 は最初が緩やか。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve
- Sync
- Sync B
- Sync C
- Sync D

## Filter Env（1〜4 共通）

4基とも音の項目は同じである。揃えるスイッチは A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列のノブは A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。

Filter Env A は Filter A の Cutoff だけを動かし、B、C、D も同じ対応にする。Amount は、そのエンベロープで Cutoff をどれだけ動かすかである。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve
- Amount
- Sync
- Sync B
- Sync C
- Sync D

## Mod Env（1〜4 共通）

4基とも音の項目は同じである。揃えるスイッチは A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列のノブは A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。

割り当て先は Oscillator、Filter、Amp Env の各設定である。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve
- Amount
- Sync
- Sync B
- Sync C
- Sync D

## LFO（1〜4 共通）

4基とも音の項目は同じである。揃えるスイッチは A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列のノブは A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。テンポに合わせるノブは Tempo と表示し、揃える Sync と混同しない。割り当て先は Oscillator、Filter、Amp Env の各設定である。

- Wave
- Rate
- Tempo（テンポに合わせる）
- Phase
- Retrigger
- Amount
- Sync
- Sync B
- Sync C
- Sync D

## Master

以下の項目を持。

- Master Vlolume
- Transpose
- Bend Range
- Mode (Poly / Mono)
- Mono Legato
- Portamento

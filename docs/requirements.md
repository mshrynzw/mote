# 要件

Phase が終わったと判断する条件だけを書く。ノブの名前は [synth.md](synth.md)、信号の流れは [synth.mmd](synth.mmd) を参照し、ここへは写さない。

[ideas.md](ideas.md) にしか無い機能は、どの Phase の完了にも含めない。発音中にメモリを確保しない、というルールはすべての Phase で守る。

## Phase 1

完了している。FL Studio で、MIDI のノートオンからその音程のサイン波が出て、ノートオフで止まった。

## Phase 2

[synth.mmd](synth.mmd) の流れのうち、Matrix を除いたものが鳴る。[synth.md](synth.md) に書かれた設定項目が、その音に反映される。

- Oscillator 4 基が、それぞれ自分の Filter と Amp Env を通り、Master で合流する
- Filter Env 4 基が、同じ番号の Filter の Cutoff だけを動かす
- Mod Env 4 基と LFO 4 基を、Oscillator、Filter、Amp Env のすべてに割り当てられる
- 画面はホスト標準のノブでよい。独自の画面と設定の保存は Phase 3 まで入れない
- Matrix は入れない

## Phase 3

Main、Preset、Setting の見た目を、ブラウザの HTML/CSS で確認してから、プラグインの画面として描き直す。HTML と CSS はプラグインの実装には使わない。ウィンドウサイズにレイアウトが追従し、[synth.md](synth.md) の設定を操作でき、その設定を保存して呼び出せる。Main は ALL と A、B、C、D のタブを持ち、A のタブではその列だけを、波形、Unison の点、エンベロープの曲線とともに大きく表示する。B、C、D も同じである。

## Phase 4

Matrix を追加する。行は 1〜4。1 行につきソースを 1 つ、ターゲットを 2 つ持つ。ソースは Mod Env、LFO、Modulation Wheel から選ぶ。ターゲットは Oscillator、Filter、Amp Env、Mod Env、LFO の各設定。

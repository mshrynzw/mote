# Mote の設計

Phase の大枠はここに書く。信号の流れとノブのイメージは [synth.md](synth.md) と [synth.mmd](synth.mmd) を参照し、この文書へは写さない。

完了条件は [requirements.md](requirements.md)、作業の区切りと目安は [schedule.md](schedule.md) に書く。予定の日付は、作業できる時期が分かってから埋める。いまの Phase より先の実装はしない。

## Phase

### Phase 1

MIDI でサイン波が鳴って止まる VST3。FL Studio で鳴った。完了条件は次のとおり。

- Cubase 13 か FL Studio に VST3 として読み込める
- MIDI のノートオンで、その音程のサイン波が出る
- ノートオフで音が止まる
- 独自の画面、Unison、フィルター、エンベロープは入れない

発音中にメモリを確保しない、というルールはこの範囲でも守る。ビルド手順は `README.md` にある。

### Phase 2

[synth.md](synth.md) と [synth.mmd](synth.mmd) のうち、Matrix を除いたもの。Oscillator、Filter、Filter Env、Amp Env、Mod Env、LFO、Master までの流れと、そこに書かれた設定項目。各 Env の Curve も含む。

信号の流れとノブの名前が「最終的な姿」と食い違うときは、[synth.mmd](synth.mmd) と [synth.md](synth.md) を優先する。範囲が広いので、着手するときに中を分けてよい。

### Phase 3

ブラウザで見た Main、Preset、Setting の見た目を、プラグインの画面として描き、設定を保存する。HTML と CSS は実装に使わない。Main のタブは [synth.md](synth.md) に従う。Unison の点は、A、B、C、D のタブで大きく描く。

### Phase 4

Matrix を追加する。行は 1〜4。1 行につきソースを 1 つ、ターゲットを 2 つ持つ。ソースは Mod Env、LFO、Modulation Wheel から選ぶ。ターゲットは Oscillator、Filter、Amp Env、Mod Env、LFO の各設定。

## 最終的な姿

中心はウェーブテーブル・シンセサイザーにする。音の厚みと軽さは Sylenth1 と Viper、音の広がりと現代的な Voice 数は Serum 2 を方向の参考にする。
画面、Wave ROM、プリセットはコピーしない。ここは未実装である。

Voice の数には余白を残す。エンジンの上限は、同時発音 128、各OscillatorのUnison 16 とし、その分のメモリは起動時に確保する。
日常のパッチで軽く保つ目安は、同時発音 16、Oscillator 4、Unison 8 である。使っていない Voice は計算しない。上限まで鳴らしたときだけ負荷が増える。

- Oscillator は 4 基。それぞれが自分の Filter と Amp Env を通り、Master で合流する。パートという階層は持たない
- Filter Env は 4 基。Filter Env 1 は Filter 1 の Cutoff だけを動かし、2、3、4 も同じ対応にする
- Mod Env は 4 基、LFO は 4 基。どちらも Oscillator、Filter、Amp Env のすべてに割り当てられる
- 各OscillatorのUnisonは 1〜16。音程のずれ、左右の広がり、定位、音量、リトリガーと位相
- 同時発音は 1〜128
- ノブの名前は [synth.md](synth.md) を参照する。Filter の計算は、2 極のゼロディレイ・フィードバック（ローパス、ハイパス、バンドパス、ノッチ）に、そのあとの tanh によるドライブを加えたものとする。この経路だけ 2 倍でオーバーサンプリングする。Amount は、鍵盤の高さで Cutoff を追う量である

なお、実装過程で最終的な姿の方針が変更する可能性ある。

### Oscillatorのモード

出荷するWaveは、自前の計算と自作のウェーブテーブルで用意する。
実機の録音は耳の基準と、利用者が自分で読み込む wav に限る。出荷データには入れない。

JP-8000 や Virus のような厚いノコギリは、録音した 1 周期ではなく Stacked Saw の計算で作る。機種名は音の方向の呼び名にとどめ、商品名にはしない。

| モード | 音 |
| --- | --- |
| Classic | サイン、三角、ノコギリ、パルス幅付き矩形 |
| Stacked Saw | 1 基の中でノコギリを最大 9 枚重ねる |
| Wavetable | 自作テーブルと、利用者が読み込む wav |

Unison（Voice を複製して左右と音程を広げる）と Stacked Saw（1 基の中でノコギリを重ねる）は別のノブにする。

### Unisonの配置図

各Oscillatorに、Voice の位置を描く。Phase 3 の画面で描く。Phase 1 には入れない。

- 横軸は Pan、縦軸は Detune（セント）。交点が中央である
- 各 Voice は別の位置に等間隔で置く。軸の端には載せない。左端と右端にまとめない
- 1 Voice が 1 点。音量で大きさが変わる
- わずかな揺らぎで点が動く
- 選んでいるWaveを、同じパネルに小さく描く
- Stacked Saw の各ノコギリは、薄い点で重ねる

描画はオーディオスレッドの外で行う。

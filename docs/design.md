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

[synth.md](synth.md) と [synth.mmd](synth.mmd) のうち、Matrix と、各列の Filter の 2 基目を除いたもの。Oscillator、Filter、Filter Env、Amp Env、Mod Env、LFO、Master までの流れと、そこに書かれた設定項目。各 Env の Curve も含む。Filter は列ごとに 1 基が鳴る。

信号の流れとノブの名前が「最終的な姿」と食い違うときは、[synth.mmd](synth.mmd) と [synth.md](synth.md) を優先する。範囲が広いので、着手するときに中を分けてよい。

### Phase 3

ブラウザで見た Main、Preset、Setting の見た目を、プラグインの画面として描き、設定を保存する。HTML と CSS はプラグインの実装に使わない。見た目は [design-mock](../design-mock) と完全一致させる。背景の光と粒子、そして動きも含み完全一致させる。Filter の 2 基目だけは、Phase 5 の音が入ってからプラグインの画面に描く。Main のタブ、Preset の項目、Setting の項目は [synth.md](synth.md) に従う。Unison の点は、A、B、C、D のタブで大きく描く。

### Phase 4

Matrix を追加する。行は 1〜4。1 行につきソースを 1 つ、ターゲットを 2 つ持つ。ソースは Mod Env、LFO、Modulation Wheel から選ぶ。ターゲットは Oscillator、Filter、Amp Env、Mod Env、LFO の各設定。

### Phase 5

各列の Filter を 2 基にする。ノブとつなぎ方は [synth.md](synth.md)、流れは [synth.mmd](synth.mmd) に書く。ブラウザの Main には、その並びを先に描いておく。プラグインの画面は、Phase 5 の音が入ってから 2 基目を描く。

## 最終的な姿

中心はウェーブテーブル・シンセサイザーにする。音の厚みと軽さは Sylenth1 と Viper、音の広がりと現代的な Voice 数は Serum 2 を方向の参考にする。
画面、Wave ROM、プリセットはコピーしない。ここは未実装である。

MIDI ノートは Polyphony で 1〜16 に限り、初期値は 4 である。その分のメモリは起動時に確保する。各 Oscillator の Unison は 1〜16。同時に計算するオシレーターの本数は、Polyphony × オンのオシレーター数 × Unison で、最大 1024 である。フィルターは Unison を混ぜたあとに、列ごとに 2 基なので、ノートとオシレーターと基の組で最大 128 である。エンベロープは列ごとに 1 つなので、最大 64 である。使っていないノート、オフのオシレーター、オフのフィルターは計算しない。初期状態では 2 基目がオフなので、厚いパッチの目安は、Polyphony 16、Oscillator 4、Unison 8 でオシレーター 512 本、フィルターはオンの 1 基だけである。

- Oscillator は 4 基。それぞれが自分の Filter 2 基と Amp Env を通り、Master で合流する。パートという階層は持たない。2 基のつなぎと配分は [synth.md](synth.md) に書く
- Filter Env は列ごとに 2 基。それぞれ自分の Filter の Cutoff だけを動かす。2 基を同じだけ動かすときは、Mod Env か LFO の Cutoff を使う
- Mod Env は 4 基、LFO は 4 基。どちらも Oscillator、Filter、Amp Env のすべてに割り当てられる。Cutoff と Resonance は、届いた列の 2 基へ同じ量を足す
- 各OscillatorのUnisonは 1〜16。音程のずれ、左右の広がり、定位、音量、リトリガーと位相
- Polyphony は 1〜16。初期値は 4。Mono のときは鳴るノートを 1 つにする
- ノブの名前は [synth.md](synth.md) を参照する。Filter の計算は、2 極のゼロディレイ・フィードバック（ローパス、ハイパス、バンドパス、ノッチ）に、そのあとの tanh によるドライブを加えたものとする。列の 2 基がオンのときは、この経路だけをまとめて 2 倍でオーバーサンプリングする。Amount は、鍵盤の高さで、その基の Cutoff を追う量である

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

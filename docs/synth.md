# シンセのイメージ

信号の流れの正本は [synth.mmd](synth.mmd)、ノブの名前の正本はこの文書である。この文書には図を写さない。

Phase の範囲、完了条件、計算方式は [design.md](design.md) を優先する。

## 信号の流れ

Oscillator 1〜4 は、Unison を混ぜたあと、自分の Filter を 2 基通って Amp Env に入り、Master で合流する。2 基は直列か並列でつなぎ、Oscillator の Balance がどちらへどれだけ出すかを決める。初期状態では 2 基目がオフなので、1 基だけを通る。Filter Env は列ごとに 2 基で、それぞれ自分の Filter の Cutoff だけを動かす。2 基を同じだけ動かすときは、Mod Env か LFO の Cutoff を使う。Mod Env 1〜4 と LFO 1〜4 は、Oscillator、Filter、Amp Env のすべてに割り当てられる。Cutoff と Resonance は、届いた列の 2 基を同じだけ動かす。この 2 基目の音は Phase 5 で鳴らし、それまでは 1 基目だけが鳴る。

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

### Balance

Filter が 2 基ともオンのとき、Oscillator がどちらへどれだけ出すかを決める。初期値は -100%。範囲は -100% から +100%。マイナス側は 1 基目、プラス側は 2 基目で、0 は半分ずつである。片方だけがオンのときは、このノブに関係なく、オンの方へ全量を入れる。両方オフのときは、Filter を通さず Amp Env へ進む。Filter Route は並列か直列かで、初期値は並列である。どちらも Filter の Sync ではコピーしない。

- Balance
- Filter Route

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

## Filter（各列に 2 基）

各 Oscillator は、Unison を混ぜたあとに Filter を 2 つ持つ。表示は A1 Type、A2 Cutoff のように、列の文字と基の番号を付ける。1 基目の On の初期値はオン、2 基目はオフである。オフの基は計算しない。Filter Route は Oscillator の Output に置く。

並列では、Balance の割合で 2 基へ分け、出た音を足す。0 のときは半分ずつなので、両方を素通りした音量は 1 基のときと変わらない。直列では、Oscillator は必ず 1 基目に入り、その出力が 2 基目に入る。Balance は、1 基目を出た音と 2 基目を出た音のどちらを聴くかを決める。マイナス側の端では 2 基目を計算しない。

Type、Cutoff、Resonance、Drive、Amount は基ごとに持つ。Type の初期値は Lowpass。Lowpass、Highpass、Bandpass、Notch から選ぶ。Cutoff は 20 Hz から 20 kHz、初期値は 20 kHz。低い周波数の方がノブの動きが大きい。Resonance、Drive、Amount は 0% から 100%、初期値は 0%。Resonance を上げると Cutoff 付近の山が立つ。自己発振はしない。Drive は、その基のフィルターのあとで tanh により歪ませる。Amount は MIDI ノート 60 を基準に、鍵盤の高さでその基の Cutoff を追う。100% のとき、1 オクターブ上がると Cutoff は 2 倍になる。この計算はノブへ書き戻さない。2 基がオンのときは、分けたあとのフィルターと Drive をまとめて 2 倍でオーバーサンプリングする。

揃えるスイッチは、列の A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列の 2 基は A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。Balance と Filter Route は Oscillator 側なので、ここには含まない。

- On
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

Attack の初期値は 0.001 秒である。0 秒だと無音から一気に立ち上がり、発音の頭でクリックが聞こえる。Decay、Release の初期値は 0.3 秒、0.05 秒。範囲は 0 秒から 10 秒。Sustain の初期値は 1、範囲は 0 から 1。Curve の初期値は 0（直線）、範囲は -1 から +1。-1 は最初が急、+1 は最初が緩やか。

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

## Filter Env（各列に 2 基）

各列に 2 基置く。表示は A1 Attack、A2 Amount のように、列の文字と Filter の番号を付ける。Filter Env A1 は Filter A1 の Cutoff だけを動かし、A2 は Filter A2 の Cutoff だけを動かす。B、C、D も同じ対応にする。2 基の Cutoff を同じだけ動かすときは、Mod Env か LFO の Cutoff を使う。

揃えるスイッチは、列の A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列の 2 基は A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。

時間と Curve の範囲は Amp Env と同じである。Attack の初期値は 0 秒、Decay は 0.3 秒、Sustain は 1、Release は 0.05 秒、Curve は 0 である。Amount は -100% から +100%、初期値は 0%。0% のあいだ Cutoff は動かない。プラスではエンベロープが上がるほど Cutoff が上がり、100% で頂点のとき 4 オクターブ上がる。マイナスでは同じ幅だけ下がる。動いた Cutoff はノブへ書き戻さない。

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

割り当て先は Oscillator、Filter、Amp Env の各設定である。時間と Curve の範囲は Amp Env と同じである。Attack の初期値は 0 秒、Decay は 0.3 秒、Sustain は 1、Release は 0.05 秒、Curve は 0 である。Amount は -100% から +100%、初期値は 0%。0% では割り当て先を動かさない。各基は Target と Chain を持つ。Target の初期値は Off。Chain の初期値は All で、All、A、B、C、D から選ぶ。Off のあいだ音は変わらない。Amount 100% のとき、Pitch は ±24 半音、Volume は ±12 dB、Pan は左右いっぱい、Cutoff は ±4 オクターブ、Resonance は ±100%、Amp は Amp Env のあとで無音から 2 倍まで。Cutoff と Resonance は、届いた列の 2 基へ同じ量を足す。Chain が All のときは、そのノートでオンの列すべてに届く。複数の Mod Env と LFO が同じ行き先に重なったときは足す。On、Wave、Sync、Retrigger、Unison の本数、Filter Route、Balance、Master は動かさない。結果はノブへ書き戻さない。

- Attack
- Attack Curve
- Decay
- Decay Curve
- Sustain
- Sustain Curve
- Release
- Release Curve
- Amount
- Target
- Chain
- Sync
- Sync B
- Sync C
- Sync D

## LFO（1〜4 共通）

4基とも音の項目は同じである。揃えるスイッチは A にだけ置く。A の Sync をオンにしたあと、Sync B、Sync C、Sync D で揃える列を選ぶ。複数を同時に選べる。ある列を対象にした瞬間、その列のノブは A の値で上書きされる。揃っているあいだ、A と対象の列のどちらを動かしても、揃っている列は同じ値になる。Sync をオフにすると揃えるのをやめ、対象だった列には、そのとき A に出ていた値を残す。再びオンにしたとき、まだ選ばれている列はもう一度 A の値で上書きされる。テンポに合わせるノブは Tempo と表示し、揃える Sync と混同しない。割り当て先は Oscillator、Filter、Amp Env の各設定である。割り当ては Mod Env と同じ Target と Chain を使う。Target の初期値は Off、Chain の初期値は All である。

Wave は Sine、Triangle、Saw、Square、Sample & Hold。初期値は Sine。Rate は 0.01 Hz から 20 Hz、初期値は 1 Hz。遅い方が細かく動く。Tempo の初期値はオフ。オフのとき Rate は Hz。オンのとき同じノブ位置が音符の長さになり、一覧の表示も Hz ではなく 1/4 のような音符名になる。初期位置の 1 Hz は 4 分音符になる。長さは遅い順に、1 小節、付点 2 分、2 分、付点 4 分、4 分、付点 8 分、4 分三連、8 分、8 分三連、16 分、16 分三連、32 分。テンポは DAW の BPM を使い、取れないときは 120。1 小節の拍数は拍子の分子、無いときは 4。Phase は 0 から 1、初期値は 0。Retrigger の初期値はオフ。オフでは 4 基が自由に回り、鳴っているノートは同じ位相を読む。オンではノートごとに Phase から打ち直す。Amount は -100% から +100%、初期値は 0%。マイナスは波の上下を逆にする。Curve は付けない。

- Wave
- Rate
- Tempo（テンポに合わせる）
- Phase
- Retrigger
- Amount
- Target
- Chain
- Sync
- Sync B
- Sync C
- Sync D

## Master

4 本の列を混ぜたあとに通す。

Master Volume は -60 dB から +12 dB、初期値は 0 dB。Transpose は ±24 半音、初期値は 0。全ノートの音程に足し、オシレーターの Note には書き戻さない。Bend Range は 0 から 12 半音、初期値は 2。MIDI のピッチベンドが、中央から上下その幅まで音程を動かす。0 ではベンドは効かない。Transpose とベンドは、Portamento とは別に音程へ足す。

Polyphony は同時に押せる MIDI ノートの数。1 から 16、初期値は 4。Mono のときは鳴るノートを 1 つにし、数字は Poly に戻したときへ残す。超えたときは、空いているスロット、次に Release 中でいちばん小さい音、最後にいちばん古く押されたキーの順で明け渡す。ノブを小さくしたときは、超えた分を古いノートから Release にする。

Mode の初期値は Poly。Poly は押したキーごとに 1 ノートを使う。Mono は押した順にキーを覚え、新しいキーの音程になる。そのキーを離してほかが残っていれば直前のキーへ戻り、最後のキーを離したときだけ Release に入る。Poly から Mono へ切り替えるときは、いちばん新しいノートを残す。

Mono Legato の初期値はオフで、Mono のときだけ効く。オフでは音程が替わるたびに Amp Env と Filter Env を Attack からやり直す。オンでは、すでにキーを押しているあいだの移動ではエンベロープを続ける。どのキーも押していない状態から弾いたときだけ Attack から始まる。オシレーターの Retrigger は別で、位相を Phase から打ち直すのは Retrigger がオンのときだけである。フィルターの内部状態は、ノートが替わってもゼロには戻さない。

Portamento は 0 秒から 10 秒、初期値は 0 秒。短い時間の方が細かく動く。0 秒では音程がすぐに替わる。すでにキーを押しているときに新しいノートが来たら、直前の音程からその時間をかけて移る。どのキーも押していないときに弾いたノートは、すぐにその音程になる。

- Master Volume
- Transpose
- Bend Range
- Mode (Poly / Mono)
- Mono Legato
- Portamento
- Polyphony

## Preset

プリセットは、そのときのノブとスイッチをまとめて保存したデータである。選ぶ、保存する、名前を変える。他のシンセのプリセットは持ってこない。マクロのノブは置かない。

ライブラリは、プリセットを入れる外側のまとまりである。製品に付くものと、利用者が保存したものに分かれる。1 つのプリセットが所属するライブラリは 1 つである。

ライブラリが持つのは、名前と ID である。ID はデータに持ち、画面には出さない。

プリセットの必須は名前だけである。名前は重なることがあるので、区別は ID でする。ID はデータに持ち、画面には出さない。ほかの項目は空でよい。

- 名前
- ジャンル。曲の種類
- サブジャンル。そのジャンルの中の細かい種類
- 作者。保存するときに入る
- 所属するライブラリ
- カテゴリー。音の役割。Bass、Lead、Pad など
- 説明
- タグ。役割やジャンルを横切る短い言葉で、いくつでも付けられる
- Rating。0 から 5。未評価は 0 とは別にし、並べるときは末尾に置く
- 作成日。保存するときに入る

名前の文字列で検索する。タグを選んで絞り込む。両方を使ったときは、名前が合い、選んだタグをすべて持つものだけを出す。並べ替えは、名前、カテゴリー、作成日、Rating、作者でできる。カテゴリーやジャンルで絞り込むことは、この画面の必須にしない。

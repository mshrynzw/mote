# Mote のエージェント向け決定

このリポジトリで実装するときは、次を守る。

## 製品

- 商品名は Mote。クレート名とフォルダ名は `mote`
- 対象は Windows 11。確認用 DAW は Cubase 13 と FL Studio
- プラグイン形式は VST3。枠は nice-plug。CLAP も一緒に出す
- Audio Unit は Mac が手に入ってから、clap-wrapper で CLAP を包む
- 言語は Rust。C++ や VST2 SDK では書かない

## 音の処理

- Voice のメモリは起動時に確保する。発音中は数値の更新だけにする
- オーディオスレッドではメモリ確保、ロック、ログ出力、画面描画をしない
- ノコギリと矩形は、サンプルごとに倍音を積まず、周波数帯ごとのウェーブテーブルを読む
- オーバーサンプリングはフィルターと歪みに限る
- 使っていないOscillatorとエフェクトは、そのバッファでは計算しない
- エンジンの上限は同時発音 128、Unison 16。そのメモリは起動時に確保する
- 軽さの目安は、16 音、Oscillator 4、Unison 8 でも 48 kHz でコアを占有しきらないこと。使っていない Voice は計算しない

## 参照の範囲

- Synth1 作者の解説から持っていくのは、ウェーブテーブルと整数位相という計算の考え方だけ
- VST SDK 2.4 の雛形、クラス構成、`new` によるバッファ確保は移植しない
- 方向の参考は Sylenth1、Viper、Serum 2。既存シンセのWave ROM、画面、プリセットはコピーしない
- 出荷するWaveは自前のアルゴリズムと自作テーブルで用意する。実機の録音は耳の基準と、利用者の wav 読み込みに限る
- 厚いノコギリは、録音した 1 周期ではなく Stacked Saw の計算で作る

## 進め方

- Phase の大枠は [docs/design.md](docs/design.md) に従う。いまの Phase より先は実装しない
- 完了条件は [docs/requirements.md](docs/requirements.md)、作業の区切りは [docs/schedule.md](docs/schedule.md) に従う。予定の日付が空でも、いまの Phase より先は実装しない
- 音の骨格を変えるときは [docs/design.md](docs/design.md) も更新する
- [docs/ideas.md](docs/ideas.md) は思いつきのメモである。設計と要件より優先しない。実装の根拠にしない
- 資料の間で矛盾があったら、推測で埋めずに確認する
- ライブラリの API は Context7 で今のドキュメントを確認してから書く

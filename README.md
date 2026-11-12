# Mote

Windows 11 向けのウェーブテーブル・シンセサイザーです。DAW に読み込ませる形式は VST3 です。

軽い動作を仕様にします。MIDI ノートは最大 16、Unison は最大 16 です。
日常の目安として、Polyphony 16、Oscillator 4 基、Unison 8 でも 48 kHz で CPU のコアを占有しきらないことを目標にします。

## いまの段階

Phase 2 の途中です。オシレーターは 4 基で、Classic（サイン、三角、ノコギリ、パルス）と Unison が鳴ります。初期状態でオンなのは 1 基目だけです。Polyphony の初期値は 4 で、Mode の初期値は Poly です。Amp Env と Filter Env が鳴り、各オシレーターは自分のフィルターを通る。Master では音量、移調、ベンド、Mono、Legato、Portamento を扱います。Mod Env と LFO のノブはある。行き先はまだ割り当てていないので、音は変わらない。Stacked Saw、Wavetable、独自の画面はまだない。

## ビルド

Rust 1.88 以降が必要です。初回だけ、バンドル用のコマンドを入れます。

```sh
cargo install cargo-nice-plug --version 0.1.1 --locked
```

テストと、VST3 / CLAP の作成は次です。

```sh
cargo test
cargo nice-plug bundle mote --release
```

成果物は `target/bundled/mote.vst3` と `target/bundled/mote.clap` です。Cubase 13 か FL Studio のプラグイン検索パスに `target/bundled` を追加するか、`mote.vst3` を VST3 フォルダへコピーして読み込みます。MIDI のノートオンでサイン波が出て、そのノートのノートオフで止まります。

Phase 2 以降の範囲、日程、要件、設計の詳細は、Phase 1 が鳴ったあとに [docs/design.md](docs/design.md) の方針どおり `docs` へ足します。

Audio Unit は、Mac が手に入ってから対応します。

## 開発の決定

新しいチャットでも守る事項は [AGENTS.md](AGENTS.md)、音の骨格は [docs/design.md](docs/design.md) にあります。
思いつきは [docs/ideas.md](docs/ideas.md) に残します。設計や要件ではありません。

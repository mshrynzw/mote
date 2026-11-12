# Mote

Windows 11 向けのウェーブテーブル・シンセサイザーです。DAW に読み込ませる形式は VST3 です。

軽い動作を仕様にします。MIDI ノートは最大 16、Unison は最大 16 です。
日常の目安として、Polyphony 16、Oscillator 4 基、Unison 8 でも 48 kHz で CPU のコアを占有しきらないことを目標にします。

## いまの段階

Phase 2 の途中です。オシレーターは 4 基で、Classic（サイン、三角、ノコギリ、パルス）と Unison が鳴ります。初期状態でオンなのは 1 基目だけです。Polyphony の初期値は 4 で、Mode の初期値は Poly です。Amp Env と Filter Env が鳴り、各オシレーターは自分のフィルターを 1 基通る。各列の 2 基目は仕様だけ決まり、音にはまだ入っていない。Master では音量、移調、ベンド、Mono、Legato、Portamento を扱います。Mod Env と LFO は Target と Chain で、音程、音量、定位、Cutoff、Resonance、Amp に割り当てられる。初期値は Off なので、選ぶまで音は変わらない。Stacked Saw、Wavetable、独自の画面はまだない。

## ビルド

画面つきの VST3 は、`third_party/wrac-plugin-template` で作る。

```sh
cargo xtask build -p mote_plugin --release --target=vst3
```

成果物は `target/wrac-plugins/mote/wrac/plugins/release/Mote.vst3` である。

開発中は、FL Studio のプラグイン一覧を、この中の `Contents\x86_64-win\Mote.vst3` へ直接向ける。VST3 フォルダへはコピーしない。ビルドする前に、Mote を開いた FL Studio は閉じる。開いたままだとファイルを掴んで書き込めない。

開発が終わったら、`Mote.vst3` を `C:\Program Files\Common Files\VST3` へ置き、一覧の次の3か所にある `ps_file_filename_0` を、置いたパスへ戻す。

- `%USERPROFILE%\Documents\Image-Line\FL Studio\Presets\Plugin database\Installed\.Plugins.ini`
- `%USERPROFILE%\Documents\Image-Line\FL Studio\Presets\Plugin database\Installed\Generators\VST3\Mote.nfo`
- `%USERPROFILE%\Documents\Image-Line\FL Studio\Presets\Plugin database\Installed\Generators\New\Mote.nfo`

Phase 2 以降の範囲、日程、要件、設計の詳細は、Phase 1 が鳴ったあとに [docs/design.md](docs/design.md) の方針どおり `docs` へ足します。

Audio Unit は、Mac が手に入ってから対応します。

## 開発の決定

新しいチャットでも守る事項は [AGENTS.md](AGENTS.md)、音の骨格は [docs/design.md](docs/design.md) にあります。
思いつきは [docs/ideas.md](docs/ideas.md) に残します。設計や要件ではありません。

# コードの地図

音の骨格は [design.md](design.md)、信号の流れは [synth.mmd](synth.mmd) を見る。
この文書は、どのフォルダが何を担当するかと、読む順だけを書く。

## 全体の流れ

いま DAW で鳴る経路は WRAC である。

`third_party/wrac-plugin-template/plugins/mote`
→ `mote::SharedEngine` / `mote::Engine`（ルートの `src/`）
→ `voice` / `oscillator` など

- `src/` … 音のエンジン（クレート名 `mote`）
- `third_party/wrac-plugin-template` … VST3・画面の枠。`mote` を `default-features = false` でライブラリとして使う
- `src/lib.rs` の `Plugin` 実装と `editor.rs` … nice-plug 用の別ルート。WRAC のバッファ処理からは呼ばない

## 読み順（WRAC 前提）

1. この文書と [synth.mmd](synth.mmd)
2. `src/lib.rs` … モジュール一覧と `Engine` / `SharedEngine` の公開。`impl Plugin for Mote` は深入りしなくてよい
3. `src/engine.rs` … 画面とオーディオの橋
4. `src/voice.rs` → `src/oscillator.rs`
5. 必要に応じて `params` / `filter` / `envelope` など
6. WRAC 側は `plugins/mote/src-plugin` の `plugin.rs` → `audio.rs` → `gui.rs`

## `src/` の役割

| ファイル | 役割 |
| --- | --- |
| `lib.rs` | クレート入口。モジュール宣言と `Engine` の再公開。nice-plug の `Mote` プラグイン実装も含む |
| `engine.rs` | WRAC が使う音の入口。`SharedEngine`（共有）と `Engine`（オーディオ側） |
| `voice.rs` | ノート枠と 1 サンプル分の合成 |
| `oscillator.rs` | Classic オシレーターと Unison |
| `wavetable.rs` | ノコギリ・パルス・三角の周波数帯テーブル |
| `unison.rs` | Unison の配置計算 |
| `filter.rs` | 列ごとのフィルター |
| `filter_env.rs` | Filter Env |
| `envelope.rs` | Amp Env |
| `mod_env.rs` | Mod Env |
| `lfo.rs` | LFO |
| `assign.rs` | Mod Env / LFO の Target と Chain |
| `master.rs` | Master（音量・移調・ベンド・Mono など） |
| `params.rs` | ホストへ出すノブ定義 |
| `editor.rs` | nice-plug 用 egui 画面。WRAC の画面ではない |

## WRAC 側（Mote が触る入口）

パスは `third_party/wrac-plugin-template/plugins/mote/src-plugin/src/`。

| ファイル | 役割 |
| --- | --- |
| `plugin.rs` | プラグイン本体。`SharedEngine` を作り、activate で `Engine` を渡す |
| `audio.rs` | オーディオスレッド。`Engine` で発音する |
| `gui.rs` / `gui/` | 画面。ノートやノブを `SharedEngine` へ渡す |
| `commands.rs` | 画面からのコマンド |

テンプレートの他クレート（`wrac_clap_adapter` など）は、枠の内部実装である。普段の音の追跡では読まない。

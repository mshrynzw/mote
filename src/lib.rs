//! Mote のプラグイン入口。
//!
//! `lib.rs` はライブラリのルートで、DAW が最初に読み込むファイルになる。
//! 音の計算は `voice.rs` と `oscillator.rs` にあり、ここでは MIDI を受け取って左右の Output へ書き込む。

// `prelude` は、プラグインを書くときによく使う型やマクロをまとめて読み込む。
use nice_plug::prelude::*;
// 0 を許さない整数。Outputチャンネル数「2」を、未設定の 0 と区別するために使う。
use std::num::NonZeroU32;
// `Arc` は複数の場所から同じ値を共有するポインタ。パラメータをホストとプラグインで共有する。
use std::sync::Arc;

mod assign;
mod editor;
mod envelope;
mod filter;
mod filter_env;
mod lfo;
mod master;
mod mod_env;
mod oscillator;
mod params;
mod unison;
mod voice;
mod wavetable;

use params::MoteParams;
use voice::Voices;
use editor::MoteEditor;

/// DAW に見えるプラグイン本体。
///
/// `pub` は他のクレートからも見える、という意味。書き出しマクロがこの型を必要とする。
pub struct Mote {
    /// ホストへ渡すパラメータ一覧。オシレーター 4 基分のノブ。
    params: Arc<MoteParams>,
    /// 最大 16 ノート。鳴っていないスロットは計算しない。
    voices: Voices,
    /// 画面の大きさ。ウィンドウを閉じても覚えておく。
    editor_state: Arc<nice_plug_egui::EguiEditorState>,
}

// `Default` を実装すると `Mote::default()` で初期状態を作れる。
// ホストはプラグインを読み込むとき、このメソッドでインスタンスを作る。
impl Default for Mote {
    fn default() -> Self {
        // `Arc::new` で共有用のポインタにする。中身のコピーはまだ発生しない。
        // そのあとで Sync のコピー先を結ぶ。結ぶ前は、4 基の住所がまだ決まらない。
        let params = Arc::new(MoteParams::default());
        params.bind_amp_sync();
        params.bind_filter_sync();
        params.bind_filter_env_sync();
        params.bind_mod_env_sync();
        params.bind_lfo_sync();
        Self {
            params,
            voices: Voices::new(),
            editor_state: editor::initial_state(),
        }
    }
}

impl Mote {
    /// MIDI イベントを 1 つ、サイン波へ伝える。
    ///
    /// `NoteEvent<()>` の `()` は「SysEx（機種固有の長い MIDI）は使わない」という印。
    fn handle_event(&mut self, event: NoteEvent<()>) {
        // イベントの種類で分岐する。`..` は、いま使わないフィールドを省略する書き方。
        match event {
            NoteEvent::NoteOn { key, velocity, .. } => {
                // キー番号が無いイベント（どのキーでもよい、という指定）は無視する。
                // `if let Some(note)` は、値があるときだけ中に入る。
                if let Some(note) = key.number() {
                    self.voices.note_on(
                        note,
                        velocity,
                        &self.params.oscillators(),
                        &self.params.lfos(),
                        &self.params.master,
                    );
                }
            }
            // `|` は「この形でも、あの形でも」。どちらも Release に入る。
            // `key.number()` が None のときは、鳴っている音をすべて Release にする。
            NoteEvent::NoteOff { key, .. } | NoteEvent::Choke { key, .. } => {
                self.voices.note_off(
                    key.number(),
                    &self.params.master,
                    &self.params.oscillators(),
                    &self.params.lfos(),
                );
            }
            // `value` の 0.5 が中央。上下は Bend Range の半音数まで。
            NoteEvent::MidiPitchBend { value, .. } => {
                self.voices.set_bend(value);
            }
            _ => {}
        }
    }
}

// `Plugin` は nice-plug が要求する共通の振る舞い。
// `impl トレイト for 型` で、「Mote はこのトレイトを満たす」と宣言する。
impl Plugin for Mote {
    // `const` はコンパイル時に決まる値。DAW のプラグイン一覧に出る名前。
    const NAME: &'static str = "Mote";
    const VENDOR: &'static str = "mshrynzw";
    const URL: &'static str = "https://github.com/mshrynzw/mote";
    const EMAIL: &'static str = "";
    // `env!` はビルド時に Cargo.toml の version を文字列として埋め込む。
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // 入力は無し（楽器なので）、Outputはステレオ 2 チャンネル。
    // `..AudioIOLayout::const_default()` は、書いていない項目を既定値にする。
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    // ノートとピッチベンドを受け取る。VST3 ではベンドを届けるために CC も含む。
    const MIDI_INPUT: MidiConfig = MidiConfig::MidiCCs;
    // バッファの途中にある MIDI も、そのサンプル位置で処理する。
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // 関連型。SysEx とバックグラウンド処理はまだ使わない。画面は Main。
    type Editor = MoteEditor;
    type SysExMessage = ();
    type BackgroundTask = ();

    /// パラメータ一覧をホストへ渡す。`Arc::clone` は中身ではなく共有ポインタを増やす。
    fn params(&self) -> Arc<dyn Params> {
        // `dyn Params` は「Params トレイトを実装した何か」。中身の型をホストから隠す。
        self.params.clone()
    }

    /// プラグインの画面。ホストがウィンドウを開くときに、この中身を埋め込む。
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.editor_state.clone())
    }

    /// 音を出し始める直前に 1 回呼ばれる。サンプルレートをここで覚える。
    /// 重い準備やメモリ確保をしてよいのは、このタイミングまで。
    /// 引数名の `_` は「受け取ったが、この関数では使わない」という印。
    fn activate(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl ActivateContext<Self>,
    ) -> bool {
        self.voices.set_sample_rate(buffer_config.sample_rate);
        // false を返すと、プラグインの起動失敗になる。
        true
    }

    /// 再生位置のリセット。オーディオ処理の途中でも呼ばれることがあるので、確保はしない。
    fn reset(&mut self) {
        self.voices.reset();
    }

    /// 数十〜数百サンプルずつ繰り返し呼ばれる、音の本体。
    /// ここではメモリ確保、ロック、ログ出力をしない。
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // このバッファ内の、次に処理する MIDI。まだ無ければ None。
        let mut next_event = context.next_event();
        let transport = context.transport();
        self.voices
            .set_transport(transport.tempo, transport.time_sig_numerator);

        // `iter_samples` は「サンプル時刻ごとに、全チャンネル」を順に渡す。
        // `sample_id` はバッファ先頭からの番号。0 が最初のサンプル。
        for (sample_id, channel_samples) in buffer.iter_samples().enumerate() {
            // このサンプル時刻までに届いているイベントを、先に全部適用する。
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    // まだ未来のイベントなので、次のサンプルまで取っておく。
                    break;
                }
                self.handle_event(event);
                next_event = context.next_event();
            }

            let oscs = self.params.oscillators();
            let filters = self.params.filters();
            let filter_envs = self.params.filter_envelopes();
            let mod_envs = self.params.mod_envelopes();
            let lfos = self.params.lfos();
            let amps = self.params.amp_envelopes();
            let chain = voice::ChainParams {
                oscs: &oscs,
                filters: &filters,
                filter_envs: &filter_envs,
                mod_envs: &mod_envs,
                lfos: &lfos,
                amps: &amps,
            };
            let (left, right) = self.voices.next_sample(&chain, &self.params.master);
            // 左右で値が違う。`enumerate` の 0 が左、1 が右。
            for (channel, channel_sample) in channel_samples.into_iter().enumerate() {
                *channel_sample = if channel == 0 { left } else { right };
            }
        }

        // 音が残っているあいだは、ホストに処理を続けてもらう。
        // 無音なら通常終了。次のノートオンは、そのときの process で受け取る。
        if self.voices.is_sounding() {
            ProcessStatus::KeepAlive
        } else {
            ProcessStatus::Normal
        }
    }
}

// CLAP 形式で書き出すときに必要な識別子。
impl ClapPlugin for Mote {
    const CLAP_ID: &'static str = "com.mshrynzw.mote";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Classic oscillators with multimode filters");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    // 楽器であり、シンセであり、ステレオOutputである、とホストへ伝える。
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
    ];
}

// VST3 形式で書き出すときに必要な識別子。
impl Vst3Plugin for Mote {
    // 16 バイトちょうど。プラグインを一意に識別する ID で、あとから変えない。
    const VST3_CLASS_ID: [u8; 16] = *b"Mote.Phase1.Sine";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

// これらのマクロが、DAW から読める CLAP と VST3 の入口関数を生成する。
nice_export_clap!(Mote);
nice_export_vst3!(Mote);

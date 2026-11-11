//! モノフォニックのサイン波。
//!
//! このファイルはプラグインの枠（VST3 や CLAP）を知らない。
//! 「いまどのキーが鳴っているか」と「次のサンプルの値」だけを担当する。
//! 発音中は位相の加算だけを行う。メモリ確保はしない。

use nice_plug::util::midi_key_to_freq;
// TAU は 2π。サイン波の 1 周期を 0.0〜1.0 の位相で表すときに使う。
use std::f32::consts::TAU;

/// クリップしない固定音量。エンベロープは Phase 1 では持たない。
/// 1.0 は最大音量なので、余裕を見て 0.2 にしている。
const AMPLITUDE: f32 = 0.2;

/// 1 音だけのサイン波。
///
/// `struct` は、関連する値を 1 つにまとめた型。
/// フィールドは外から直接触らせず、下の `impl` のメソッド経由で更新する。
#[derive(Debug)]
pub struct SineVoice {
    /// 1 秒あたりのサンプル数。48_000 なら、1 秒で 48000 回 `next_sample` が呼ばれる。
    sample_rate: f32,
    /// Waveの中の今の位置。0.0 以上 1.0 未満で、1.0 は 1 周期。
    phase: f32,
    /// いま鳴らしている周波数。鳴っていないときは 0。
    frequency_hz: f32,
    /// 押されている MIDI ノート番号。
    /// `Option<u8>` は「ある（Some）か、ない（None）か」を表す。None は無音。
    active_key: Option<u8>,
}

// `impl 型名` の中に、その型のメソッドを書く。
impl SineVoice {
    /// 無音の Voice を作る。
    /// `Self` はこの impl の型、つまり `SineVoice` 自身を指す。
    pub fn new() -> Self {
        Self {
            // サンプルレート未設定のあいだ、0 除算を避けるための仮の値。
            // 実際の値はプラグインの activate で上書きする。
            sample_rate: 1.0,
            phase: 0.0,
            frequency_hz: 0.0,
            active_key: None,
        }
    }

    /// DAW が選んだサンプルレートを覚える。
    /// `&mut self` は「この Voice の中身を書き換えてよい」という引数。
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    /// 位相とノートを消して無音に戻す。サンプルレートは残す。
    /// ホストが再生を止めたり、プラグインを初期化し直したりしたときに呼ばれる。
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.frequency_hz = 0.0;
        self.active_key = None;
    }

    /// いま音が出ているか。`&self` は中身を読むだけで、書き換えない。
    pub fn is_sounding(&self) -> bool {
        // `is_some` は Option が Some のとき true。
        self.active_key.is_some()
    }

    /// ノートオン。ベロシティ 0 は、MIDI の慣習に従ってノートオフとして扱う。
    /// `key` は 0〜127。69 が A4（440 Hz）。
    pub fn note_on(&mut self, key: u8, velocity: f32) {
        if velocity <= 0.0 {
            // `Some(key)` は「このキーだけ止める」という意味の Option。
            self.note_off(Some(key));
            return;
        }

        self.active_key = Some(key);
        // ノート番号から周波数へ。計算式は nice-plug の関数に任せている。
        self.frequency_hz = midi_key_to_freq(key);
        // 位相 0 のサインは 0 なので、鳴り始めがクリックになりにくい。
        self.phase = 0.0;
    }

    /// ノートオフ。
    /// `Some(番号)` はそのキーだけ止める。`None` は、どのキーの音も止める。
    /// 別のキーのノートオフでは、いま鳴っている音を止めない。
    pub fn note_off(&mut self, key: Option<u8>) {
        // `match` は値の組み合わせで分岐する。`_` は「それ以外」。
        let stop = match (self.active_key, key) {
            // 何か鳴っていて、止めるキーが指定されていない。
            (Some(_), None) => true,
            // 鳴っているキーと、離されたキーが同じ。
            (Some(active), Some(released)) => active == released,
            // 無音のときや、別のキーが離されたときは何もしない。
            _ => false,
        };
        if stop {
            self.active_key = None;
            self.frequency_hz = 0.0;
        }
    }

    /// サンプルを 1 つ進めて、その値を返す。範囲はおおよそ -0.2〜0.2。
    pub fn next_sample(&mut self) -> f32 {
        if self.active_key.is_none() {
            return 0.0;
        }

        // sin の引数はラジアン。位相 0〜1 を 0〜2π に直してから掛ける。
        let sample = (self.phase * TAU).sin() * AMPLITUDE;
        // 1 サンプルで進む位相は「周波数 ÷ サンプルレート」。
        self.phase += self.frequency_hz / self.sample_rate;
        // 位相が 1 周期を超えたら、整数部を捨てて 0〜1 に戻す。
        if self.phase >= 1.0 {
            self.phase -= self.phase.floor();
        }
        sample
    }
}

// `Default` は `SineVoice::default()` で値を作れるようにするトレイト。
// 中身は `new` と同じ。
impl Default for SineVoice {
    fn default() -> Self {
        Self::new()
    }
}

// このモジュールは `cargo test` のときだけコンパイルされる。
#[cfg(test)]
mod tests {
    // 親モジュールの公開・非公開をまとめて使う。AMPLITUDE もテストから見える。
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    /// A4（ノート 69）を押した直後の Voice。複数のテストで同じ準備を繰り返さないため。
    fn sounding_a4() -> SineVoice {
        let mut voice = SineVoice::new();
        voice.set_sample_rate(SAMPLE_RATE);
        voice.note_on(69, 1.0);
        voice
    }

    #[test]
    fn silence_until_a_note_starts() {
        let mut voice = SineVoice::new();
        voice.set_sample_rate(SAMPLE_RATE);
        // `assert!` は条件が false ならテスト失敗。`assert_eq!` は左右が違うと失敗。
        assert!(!voice.is_sounding());
        assert_eq!(voice.next_sample(), 0.0);
    }

    #[test]
    fn note_on_starts_at_zero_then_follows_the_sine() {
        let mut voice = sounding_a4();
        // 位相 0 の最初のサンプルは 0。
        assert_eq!(voice.next_sample(), 0.0);

        // 2 つ目は、440 Hz を 48000 Hz で 1 サンプル進めたサインと一致する。
        let expected = (TAU * 440.0 / SAMPLE_RATE).sin() * AMPLITUDE;
        let actual = voice.next_sample();
        assert!(
            (actual - expected).abs() < 1.0e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn higher_note_advances_phase_faster() {
        let mut low = SineVoice::new();
        let mut high = SineVoice::new();
        low.set_sample_rate(SAMPLE_RATE);
        high.set_sample_rate(SAMPLE_RATE);
        // 57 は A3、81 は A5。高い方が同じ時間で位相を多く進める。
        low.note_on(57, 1.0);
        high.note_on(81, 1.0);

        // 最初のサンプルはどちらも位相 0 なので 0。2 つ目で差が出る。
        let _ = low.next_sample();
        let _ = high.next_sample();
        let low_sample = low.next_sample().abs();
        let high_sample = high.next_sample().abs();
        assert!(high_sample > low_sample);
    }

    #[test]
    fn note_off_stops_only_the_held_key() {
        let mut voice = sounding_a4();
        let _ = voice.next_sample();

        // 押していないキーを離しても、A4 は鳴り続ける。
        voice.note_off(Some(70));
        assert!(voice.is_sounding());
        assert_ne!(voice.next_sample(), 0.0);

        voice.note_off(Some(69));
        assert!(!voice.is_sounding());
        assert_eq!(voice.next_sample(), 0.0);
    }

    #[test]
    fn zero_velocity_note_on_stops_the_held_key() {
        let mut voice = sounding_a4();
        voice.note_on(69, 0.0);
        assert!(!voice.is_sounding());
        assert_eq!(voice.next_sample(), 0.0);
    }

    #[test]
    fn wildcard_note_off_stops_any_key() {
        let mut voice = sounding_a4();
        // None は「キーを指定しないノートオフ」。
        voice.note_off(None);
        assert!(!voice.is_sounding());
        assert_eq!(voice.next_sample(), 0.0);
    }

    #[test]
    fn reset_returns_to_silence() {
        let mut voice = sounding_a4();
        let _ = voice.next_sample();
        voice.reset();
        assert!(!voice.is_sounding());
        assert_eq!(voice.next_sample(), 0.0);
    }
}

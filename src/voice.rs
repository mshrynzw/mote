//! 1 つの MIDI ノートと、Classic のオシレーター 4 基。
//!
//! 同時に鳴るノートは 1 つ。最後に押したキーが残る。
//! 4 基の合成は、0 dB を 4 本足しても割れにくいように 1/4 している。

use crate::oscillator::{OscParams, Oscillator};
use crate::wavetable::WaveBank;

/// プラグインが発音中に持つ状態。ノブは `MoteParams` 側にある。
pub struct NoteVoice {
    oscillators: [Oscillator; 4],
    bank: WaveBank,
    sample_rate: f32,
    note: Option<u8>,
}

impl NoteVoice {
    pub fn new() -> Self {
        Self {
            oscillators: std::array::from_fn(|_| Oscillator::new()),
            bank: WaveBank::new(48_000.0),
            sample_rate: 48_000.0,
            note: None,
        }
    }

    /// DAW のサンプルレートが決まったときに呼ぶ。波形表をここで作り直す。
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.bank = WaveBank::new(sample_rate);
    }

    pub fn reset(&mut self) {
        for oscillator in &mut self.oscillators {
            oscillator.reset();
        }
        self.note = None;
    }

    pub fn is_sounding(&self) -> bool {
        self.note.is_some()
    }

    /// ベロシティ 0 のノートオンは、ノートオフとして扱う。
    pub fn note_on(&mut self, key: u8, velocity: f32, params: &[&OscParams; 4]) {
        if velocity <= 0.0 {
            self.note_off(Some(key));
            return;
        }
        self.note = Some(key);
        for (oscillator, osc_params) in self.oscillators.iter_mut().zip(params.iter()) {
            oscillator.note_on(osc_params);
        }
    }

    /// `None` は、今鳴っているキーを問わず止める。
    pub fn note_off(&mut self, key: Option<u8>) {
        let stop = match (self.note, key) {
            (Some(_), None) => true,
            (Some(active), Some(released)) => active == released,
            _ => false,
        };
        if stop {
            self.note = None;
        }
    }

    /// 左右 1 サンプル。オンの基だけ足し、その本数で割る。
    pub fn next_sample(&mut self, params: &[&OscParams; 4]) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;
        let mut active = 0u32;
        for (oscillator, osc_params) in self.oscillators.iter_mut().zip(params.iter()) {
            // オフの基は波形も位相も進まない。
            if !osc_params.on.value() {
                continue;
            }
            active += 1;
            let (osc_left, osc_right) =
                oscillator.next_sample(osc_params, &self.bank, self.note, self.sample_rate);
            left += osc_left;
            right += osc_right;
        }
        if active == 0 {
            return (0.0, 0.0);
        }
        let scale = 1.0 / active as f32;
        (left * scale, right * scale)
    }
}

impl Default for NoteVoice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::MoteParams;

    fn params() -> MoteParams {
        MoteParams::default()
    }

    #[test]
    fn note_on_then_off_returns_to_silence() {
        let mut voice = NoteVoice::new();
        voice.set_sample_rate(48_000.0);
        let knobs = params();
        let oscs = knobs.oscillators();
        voice.note_on(69, 1.0, &oscs);

        let mut heard = false;
        for _ in 0..32 {
            let (left, right) = voice.next_sample(&oscs);
            if left.abs() > 0.0001 || right.abs() > 0.0001 {
                heard = true;
            }
        }
        assert!(heard);

        voice.note_off(Some(69));
        let (left, right) = voice.next_sample(&oscs);
        assert_eq!(left, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn only_oscillator_1_is_on_by_default() {
        let knobs = params();
        assert!(knobs.osc1.on.value());
        assert!(!knobs.osc2.on.value());
        assert!(!knobs.osc3.on.value());
        assert!(!knobs.osc4.on.value());
    }
}

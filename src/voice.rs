//! 1 つの MIDI ノートと、Classic のオシレーター 4 基。
//!
//! 同時に鳴るノートは 1 つ。最後に押したキーが残る。
//! オンの基は、自分のフィルターを通ってから Amp Env を掛ける。
//! Filter Env は、同じ番号の Cutoff だけをオクターブで上下する。
//! 4 基の合成は、オンの本数で割って音量を揃える。
//! ノートオフのあとは Amp Env の Release が終わるまで音が残る。

use crate::envelope::{synced_amp, AmpEnvelope, AmpParams};
use crate::filter::{synced_filter, Filter, FilterParams};
use crate::filter_env::{synced_filter_env, FilterEnv, FilterEnvParams};
use crate::oscillator::{OscParams, Oscillator};
use crate::wavetable::WaveBank;

/// プラグインが発音中に持つ状態。ノブは `MoteParams` 側にある。
pub struct NoteVoice {
    oscillators: [Oscillator; 4],
    filters: [Filter; 4],
    filter_envs: [FilterEnv; 4],
    amps: [AmpEnvelope; 4],
    bank: WaveBank,
    sample_rate: f32,
    /// 今の音程。Release 中も、消えるまで残す。
    note: Option<u8>,
    /// キーを押しているあいだ true。離すと Release に入る。
    held: bool,
}

impl NoteVoice {
    pub fn new() -> Self {
        Self {
            oscillators: std::array::from_fn(|_| Oscillator::new()),
            filters: std::array::from_fn(|_| Filter::new()),
            filter_envs: std::array::from_fn(|_| FilterEnv::new()),
            amps: std::array::from_fn(|_| AmpEnvelope::new()),
            bank: WaveBank::new(48_000.0),
            sample_rate: 48_000.0,
            note: None,
            held: false,
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
        for filter in &mut self.filters {
            filter.reset();
        }
        for filter_env in &mut self.filter_envs {
            filter_env.reset();
        }
        for amp in &mut self.amps {
            amp.reset();
        }
        self.note = None;
        self.held = false;
    }

    pub fn is_sounding(&self) -> bool {
        self.held || self.amps.iter().any(|amp| amp.is_busy())
    }

    /// ベロシティ 0 のノートオンは、ノートオフとして扱う。
    pub fn note_on(&mut self, key: u8, velocity: f32, params: &[&OscParams; 4]) {
        if velocity <= 0.0 {
            self.note_off(Some(key));
            return;
        }
        self.note = Some(key);
        self.held = true;
        for (index, (oscillator, osc_params)) in
            self.oscillators.iter_mut().zip(params.iter()).enumerate()
        {
            if osc_params.on.value() {
                oscillator.note_on(osc_params);
                self.filter_envs[index].start();
                self.amps[index].start();
            } else {
                self.filter_envs[index].silence();
                self.amps[index].reset();
            }
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
            self.held = false;
            for filter_env in &mut self.filter_envs {
                filter_env.release();
            }
            for amp in &mut self.amps {
                amp.release();
            }
        }
    }

    /// 左右 1 サンプル。オンの基だけ足し、その本数で割る。
    pub fn next_sample(
        &mut self,
        oscs: &[&OscParams; 4],
        filters: &[&FilterParams; 4],
        filter_envs: &[&FilterEnvParams; 4],
        amps: &[&AmpParams; 4],
    ) -> (f32, f32) {
        let mut left = 0.0;
        let mut right = 0.0;
        let mut active = 0u32;
        for (index, osc_params) in oscs.iter().enumerate() {
            // オフの基は、波形もフィルターもエンベロープも進まない。
            if !osc_params.on.value() {
                self.filters[index].silence();
                self.filter_envs[index].silence();
                self.amps[index].reset();
                continue;
            }
            active += 1;
            let (osc_left, osc_right) = self.oscillators[index].next_sample(
                osc_params,
                &self.bank,
                self.note,
                self.sample_rate,
            );
            let env_params = synced_filter_env(index, filter_envs);
            let env_octaves = self.filter_envs[index].next_octaves(env_params, self.sample_rate);
            let (filtered_left, filtered_right) = self.filters[index].next_sample(
                synced_filter(index, filters),
                osc_left,
                osc_right,
                self.note,
                self.sample_rate,
                env_octaves,
            );
            let env = self.amps[index].next(synced_amp(index, amps).settings(), self.sample_rate);
            left += filtered_left * env;
            right += filtered_right * env;
        }
        if !self.held && self.amps.iter().all(|amp| !amp.is_busy()) {
            self.note = None;
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
        let filters = knobs.filters();
        let filter_envs = knobs.filter_envelopes();
        let amps = knobs.amp_envelopes();
        voice.note_on(69, 1.0, &oscs);

        let mut heard = false;
        for _ in 0..32 {
            let (left, right) = voice.next_sample(&oscs, &filters, &filter_envs, &amps);
            if left.abs() > 0.0001 || right.abs() > 0.0001 {
                heard = true;
            }
        }
        assert!(heard);

        voice.note_off(Some(69));
        assert!(voice.is_sounding());
        // Release の初期値は 0.05 秒。48 kHz なら 2400 サンプルより少し多く回せば消える。
        for _ in 0..4_000 {
            voice.next_sample(&oscs, &filters, &filter_envs, &amps);
        }
        let (left, right) = voice.next_sample(&oscs, &filters, &filter_envs, &amps);
        assert!(!voice.is_sounding());
        assert_eq!(left, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn low_cutoff_quiets_a_high_note() {
        use nice_plug::params::InternalParamMut;

        fn energy(cutoff: f32) -> f32 {
            let mut voice = NoteVoice::new();
            voice.set_sample_rate(48_000.0);
            let knobs = params();
            unsafe {
                let _ = InternalParamMut::_internal_set_plain_value(&knobs.filter1.cutoff, cutoff);
            }
            let oscs = knobs.oscillators();
            let filters = knobs.filters();
            let filter_envs = knobs.filter_envelopes();
            let amps = knobs.amp_envelopes();
            voice.note_on(90, 1.0, &oscs);
            let mut sum = 0.0;
            for i in 0..4_000 {
                let (left, right) = voice.next_sample(&oscs, &filters, &filter_envs, &amps);
                if i > 2_000 {
                    sum += left * left + right * right;
                }
            }
            sum
        }

        let open = energy(20_000.0);
        let closed = energy(200.0);
        assert!(open > closed * 4.0, "open {open}, closed {closed}");
    }

    #[test]
    fn filter_env_amount_opens_a_low_cutoff() {
        use nice_plug::params::InternalParamMut;

        fn energy(amount: f32) -> f32 {
            let mut voice = NoteVoice::new();
            voice.set_sample_rate(48_000.0);
            let knobs = params();
            unsafe {
                let _ = InternalParamMut::_internal_set_plain_value(&knobs.filter1.cutoff, 200.0);
                let _ =
                    InternalParamMut::_internal_set_plain_value(&knobs.filter_env1.amount, amount);
            }
            let oscs = knobs.oscillators();
            let filters = knobs.filters();
            let filter_envs = knobs.filter_envelopes();
            let amps = knobs.amp_envelopes();
            voice.note_on(69, 1.0, &oscs);
            let mut sum = 0.0;
            for i in 0..4_000 {
                let (left, right) = voice.next_sample(&oscs, &filters, &filter_envs, &amps);
                if i > 2_000 {
                    sum += left * left + right * right;
                }
            }
            sum
        }

        let shut = energy(0.0);
        let open = energy(100.0);
        assert!(open > shut * 2.0, "open {open}, shut {shut}");
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

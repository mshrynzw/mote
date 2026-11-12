//! Classic のオシレーター 1 基。
//!
//! Unison は最大 16 Voice。その位相は起動時に配列で持っておく。
//! ノコギリ、パルス、三角は波形表を読む。サインだけその場で計算する。

use nice_plug::prelude::*;
use std::f32::consts::TAU;

use crate::unison::{unison_cents, unison_offset};
use crate::wavetable::{TableShape, WaveBank};

pub const MAX_UNISON: usize = 16;

/// Classic で選べる波形。並びを変えると、保存した音色の番号がずれる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ClassicWave {
    #[id = "sine"]
    Sine,
    #[id = "triangle"]
    Triangle,
    #[id = "saw"]
    Saw,
    #[id = "pulse"]
    Pulse,
}

/// 1 基分のノブ。ID は `o1_octave` のように、外側で番号を付ける。
#[derive(Params)]
pub struct OscParams {
    /// オフの基は、そのサンプルでは波形を計算しない。
    #[id = "on"]
    pub on: BoolParam,
    #[id = "octave"]
    pub octave: IntParam,
    #[id = "note"]
    pub note: IntParam,
    #[id = "fine"]
    pub fine: FloatParam,
    #[id = "volume"]
    pub volume: FloatParam,
    #[id = "pan"]
    pub pan: FloatParam,
    #[id = "stereo"]
    pub stereo: FloatParam,
    #[id = "wave"]
    pub wave: EnumParam<ClassicWave>,
    #[id = "phase"]
    pub phase: FloatParam,
    #[id = "inv"]
    pub inv: BoolParam,
    #[id = "retrigger"]
    pub retrigger: BoolParam,
    #[id = "voices"]
    pub voices: IntParam,
    #[id = "detune"]
    pub detune: FloatParam,
}

impl OscParams {
    /// `part` は A、B、C、D。一覧にグループ名が出ないホストでも、どの列か分かる。
    pub fn new(part: char, on: bool) -> Self {
        let name = |label: &str| format!("{part} {label}");
        Self {
            on: BoolParam::new(name("On"), on),
            octave: IntParam::new(name("Octave"), 0, IntRange::Linear { min: -2, max: 2 }),
            note: IntParam::new(name("Note"), 0, IntRange::Linear { min: -24, max: 24 }),
            fine: FloatParam::new(
                name("Fine"),
                0.0,
                FloatRange::Linear {
                    min: -100.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" cents"),
            volume: FloatParam::new(
                name("Volume"),
                0.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" dB"),
            pan: FloatParam::new(
                name("Pan"),
                0.0,
                FloatRange::Linear {
                    min: -1.0,
                    max: 1.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0)),
            stereo: FloatParam::new(
                name("Stereo"),
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(10.0)),
            wave: EnumParam::new(name("Wave"), ClassicWave::Sine),
            phase: FloatParam::new(
                name("Phase"),
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            inv: BoolParam::new(name("Inv"), false),
            retrigger: BoolParam::new(name("Retrigger"), false),
            voices: IntParam::new(name("Voices"), 1, IntRange::Linear { min: 1, max: 16 }),
            detune: FloatParam::new(
                name("Detune"),
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(10.0))
            .with_unit(" cents"),
        }
    }
}

impl Default for OscParams {
    fn default() -> Self {
        Self::new('A', false)
    }
}

/// 1 基の、発音中に書き換える状態。ノブ自体はここには置かない。
pub struct Oscillator {
    /// 各 Unison Voice の位相。1 周が 2^32。
    phases: [u32; MAX_UNISON],
    /// 音程ノブをまとめた値。急に半音が変わっても、少しずつ追う。
    pitch_cents: Smoother<f32>,
    last_pitch_target: f32,
}

impl Oscillator {
    pub fn new() -> Self {
        Self {
            phases: [0; MAX_UNISON],
            pitch_cents: Smoother::new(SmoothingStyle::Linear(5.0)),
            last_pitch_target: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.phases = [0; MAX_UNISON];
        self.pitch_cents.reset(0.0);
        self.last_pitch_target = 0.0;
    }

    /// ノートオン。Retrigger がオンのときだけ、Phase ノブの位置から打ち直す。
    pub fn note_on(&mut self, params: &OscParams) {
        if params.retrigger.value() {
            let start = phase_from_unit(params.phase.value());
            self.phases = [start; MAX_UNISON];
        }
    }

    /// 左右のサンプルを 1 つ返す。鳴っていないときは `(0, 0)` だが、ノブの追従は進める。
    pub fn next_sample(
        &mut self,
        params: &OscParams,
        bank: &WaveBank,
        midi_note: Option<u8>,
        sample_rate: f32,
    ) -> (f32, f32) {
        // `smoothed.next` は 1 サンプルにつき 1 回だけ呼ぶ。呼ばないとノブの動きが遅れる。
        let fine = params.fine.smoothed.next();
        let volume_db = params.volume.smoothed.next();
        let pan = params.pan.smoothed.next();
        let stereo = params.stereo.smoothed.next();
        let detune = params.detune.smoothed.next();
        let pitch_target =
            params.octave.value() as f32 * 1200.0 + params.note.value() as f32 * 100.0 + fine;
        if (pitch_target - self.last_pitch_target).abs() > 0.01 {
            self.pitch_cents.set_target(sample_rate, pitch_target);
            self.last_pitch_target = pitch_target;
        }
        let pitch_cents = self.pitch_cents.next();

        let Some(note) = midi_note else {
            return (0.0, 0.0);
        };

        let voices = (params.voices.value() as usize).clamp(1, MAX_UNISON);
        let wave = params.wave.value();
        let invert = if params.inv.value() { -1.0 } else { 1.0 };
        let gain = nice_plug::util::db_to_gain(volume_db) / voices as f32;
        let base_hz = note_frequency(note, pitch_cents);

        let mut left = 0.0;
        let mut right = 0.0;
        for index in 0..voices {
            let cents = unison_cents(index, voices, detune);
            let frequency = base_hz * cents_to_ratio(cents);
            let sample = read_wave(wave, bank, self.phases[index], frequency) * invert;
            self.phases[index] =
                self.phases[index].wrapping_add(phase_step(frequency, sample_rate));

            let spread = unison_offset(index, voices);
            let voice_pan = (pan + stereo * spread).clamp(-1.0, 1.0);
            let (pan_left, pan_right) = equal_power_pan(voice_pan);
            left += sample * pan_left;
            right += sample * pan_right;
        }

        let left = left * gain;
        let right = right * gain;
        (finite_or_zero(left), finite_or_zero(right))
    }
}

impl Default for Oscillator {
    fn default() -> Self {
        Self::new()
    }
}

fn note_frequency(note: u8, pitch_cents: f32) -> f32 {
    nice_plug::util::midi_key_to_freq(note) * cents_to_ratio(pitch_cents)
}

fn cents_to_ratio(cents: f32) -> f32 {
    2.0f32.powf(cents / 1200.0)
}

fn phase_step(frequency_hz: f32, sample_rate: f32) -> u32 {
    if sample_rate <= 0.0 || !frequency_hz.is_finite() {
        return 0;
    }
    (frequency_hz / sample_rate * u32::MAX as f32) as u32
}

fn phase_from_unit(phase: f32) -> u32 {
    (phase.clamp(0.0, 1.0) * u32::MAX as f32) as u32
}

fn read_wave(wave: ClassicWave, bank: &WaveBank, phase: u32, frequency_hz: f32) -> f32 {
    match wave {
        ClassicWave::Sine => (phase as f32 / u32::MAX as f32 * TAU).sin(),
        ClassicWave::Triangle => bank.read(TableShape::Triangle, phase, frequency_hz),
        ClassicWave::Saw => bank.read(TableShape::Saw, phase, frequency_hz),
        ClassicWave::Pulse => bank.read(TableShape::Pulse, phase, frequency_hz),
    }
}

/// Pan の -1 が左、0 が中央、1 が右。左右のエネルギーがほぼ一定になる分け方。
fn equal_power_pan(pan: f32) -> (f32, f32) {
    let angle = (pan + 1.0) * 0.25 * std::f32::consts::PI;
    (angle.cos(), angle.sin())
}

fn finite_or_zero(sample: f32) -> f32 {
    if sample.is_finite() {
        sample
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_pitch_is_midi_note() {
        let hz = note_frequency(69, 0.0);
        assert!((hz - 440.0).abs() < 0.01);
    }

    #[test]
    fn one_octave_doubles_frequency() {
        let root = note_frequency(69, 0.0);
        let up = note_frequency(69, 1200.0);
        assert!((up / root - 2.0).abs() < 0.001);
    }

    #[test]
    fn fine_cent_is_a_hundredth_of_a_semitone() {
        let root = note_frequency(69, 0.0);
        let fine = note_frequency(69, 100.0);
        let semitone = 2.0f32.powf(1.0 / 12.0);
        assert!((fine / root - semitone).abs() < 0.001);
    }
}

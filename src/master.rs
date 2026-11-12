//! Master。4 本の列を混ぜたあとの音量と、全ノートに共通の音程。
//!
//! Polyphony は MIDI ノートの数。オシレーターの Unison とは別である。

use nice_plug::prelude::*;

/// 同時に持てる MIDI ノートの上限。Polyphony の最大と同じ。
pub const MAX_NOTES: usize = 16;

/// Poly はノートを重ねる。Mono は 1 ノートだけ鳴らす。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum PlayMode {
    #[id = "poly"]
    Poly,
    #[id = "mono"]
    Mono,
}

/// ホストに出す Master のノブ。ID は `master_volume` のようになる。
#[derive(Params)]
pub struct MasterParams {
    #[id = "volume"]
    pub volume: FloatParam,
    #[id = "transpose"]
    pub transpose: IntParam,
    #[id = "bend"]
    pub bend: IntParam,
    #[id = "mode"]
    pub mode: EnumParam<PlayMode>,
    #[id = "legato"]
    pub legato: BoolParam,
    #[id = "portamento"]
    pub portamento: FloatParam,
    #[id = "polyphony"]
    pub polyphony: IntParam,
}

impl MasterParams {
    pub fn new() -> Self {
        let time = FloatRange::Skewed {
            min: 0.0,
            max: 10.0,
            factor: FloatRange::skew_factor(-2.0),
        };
        Self {
            volume: FloatParam::new(
                "Master Volume",
                0.0,
                FloatRange::Linear {
                    min: -60.0,
                    max: 12.0,
                },
            )
            .with_unit(" dB"),
            transpose: IntParam::new("Transpose", 0, IntRange::Linear { min: -24, max: 24 }),
            bend: IntParam::new("Bend Range", 2, IntRange::Linear { min: 0, max: 12 }),
            mode: EnumParam::new("Mode", PlayMode::Poly),
            legato: BoolParam::new("Mono Legato", false),
            portamento: FloatParam::new("Portamento", 0.0, time).with_unit(" s"),
            polyphony: IntParam::new("Polyphony", 4, IntRange::Linear { min: 1, max: 16 }),
        }
    }
}

impl Default for MasterParams {
    fn default() -> Self {
        Self::new()
    }
}

/// ピッチベンドの 0.5 が中央。Bend Range が 2 なら、上端は +2 半音。
pub fn bend_semitones(bend: f32, range: f32) -> f32 {
    (bend.clamp(0.0, 1.0) - 0.5) * 2.0 * range.max(0.0)
}

/// Poly のとき Polyphony の数、Mono のとき 1。
pub fn note_limit(mode: PlayMode, polyphony: i32) -> usize {
    match mode {
        PlayMode::Mono => 1,
        PlayMode::Poly => (polyphony as usize).clamp(1, MAX_NOTES),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_bend_is_zero_and_full_bend_uses_the_range() {
        assert!(bend_semitones(0.5, 2.0).abs() < 1.0e-5);
        assert!((bend_semitones(1.0, 2.0) - 2.0).abs() < 1.0e-5);
        assert!((bend_semitones(0.0, 2.0) + 2.0).abs() < 1.0e-5);
        assert!(bend_semitones(1.0, 0.0).abs() < 1.0e-5);
    }

    #[test]
    fn mono_limits_notes_to_one() {
        assert_eq!(note_limit(PlayMode::Mono, 16), 1);
        assert_eq!(note_limit(PlayMode::Poly, 4), 4);
    }
}

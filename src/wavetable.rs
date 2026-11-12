//! ノコギリ、パルス、三角を、周波数帯ごとの表で読む。
//!
//! 表は `activate` で一度だけ作る。発音中は表を読むだけ。
//! サインは倍音が無いので、この表には入れない。

use std::f32::consts::{PI, TAU};

const LEN: usize = 2048;
const LEN_MASK: usize = LEN - 1;

/// 1 周期分の波形と、その表に入れた倍音の数。
struct Mip {
    harmonics: u32,
    samples: Vec<f32>,
}

/// サンプルレートに合わせて作った Classic の波形表。
pub struct WaveBank {
    saw: Vec<Mip>,
    pulse: Vec<Mip>,
    triangle: Vec<Mip>,
    sample_rate: f32,
}

impl WaveBank {
    pub fn new(sample_rate: f32) -> Self {
        let mut saw = Vec::new();
        let mut pulse = Vec::new();
        let mut triangle = Vec::new();
        // 1, 2, 4, … 1024。表の長さが 2048 なので、これより多い倍音は入れられない。
        let mut harmonics = 1u32;
        while harmonics <= 1024 {
            saw.push(Mip {
                harmonics,
                samples: bandlimited(harmonics, Shape::Saw),
            });
            pulse.push(Mip {
                harmonics,
                samples: bandlimited(harmonics, Shape::Pulse),
            });
            triangle.push(Mip {
                harmonics,
                samples: bandlimited(harmonics, Shape::Triangle),
            });
            harmonics *= 2;
        }

        Self {
            saw,
            pulse,
            triangle,
            sample_rate,
        }
    }

    /// 位相 `phase`（0 以上 2^32 未満で 1 周）から、その周波数に合う表を読む。
    pub fn read(&self, shape: TableShape, phase: u32, frequency_hz: f32) -> f32 {
        let mips = match shape {
            TableShape::Saw => &self.saw,
            TableShape::Pulse => &self.pulse,
            TableShape::Triangle => &self.triangle,
        };
        let mip = &mips[mip_index(mips, frequency_hz, self.sample_rate)];
        read_interpolated(&mip.samples, phase)
    }
}

#[derive(Clone, Copy)]
pub enum TableShape {
    Saw,
    Pulse,
    Triangle,
}

#[derive(Clone, Copy)]
enum Shape {
    Saw,
    Pulse,
    Triangle,
}

fn bandlimited(harmonics: u32, shape: Shape) -> Vec<f32> {
    let mut samples = vec![0.0; LEN];
    for harmonic in 1..=harmonics {
        let use_harmonic = match shape {
            Shape::Saw => true,
            Shape::Pulse | Shape::Triangle => harmonic % 2 == 1,
        };
        if !use_harmonic {
            continue;
        }

        let weight = match shape {
            Shape::Saw | Shape::Pulse => 1.0 / harmonic as f32,
            Shape::Triangle => {
                let sign = if (harmonic / 2) % 2 == 0 { 1.0 } else { -1.0 };
                sign / (harmonic as f32 * harmonic as f32)
            }
        };
        for (index, sample) in samples.iter_mut().enumerate() {
            let cycle = index as f32 / LEN as f32;
            *sample += (TAU * harmonic as f32 * cycle).sin() * weight;
        }
    }

    let scale = match shape {
        Shape::Saw => 2.0 / PI,
        Shape::Pulse => 4.0 / PI,
        Shape::Triangle => 8.0 / (PI * PI),
    };
    for sample in &mut samples {
        *sample *= scale;
    }
    samples
}

fn mip_index(mips: &[Mip], frequency_hz: f32, sample_rate: f32) -> usize {
    let nyquist = sample_rate * 0.5;
    let allowed = if frequency_hz < 1.0 {
        1024
    } else {
        (nyquist / frequency_hz).floor().clamp(1.0, 1024.0) as u32
    };

    // ナイキストより上の倍音を含まない、いちばん詳しい表を選ぶ。
    let mut chosen = 0;
    for (index, mip) in mips.iter().enumerate() {
        if mip.harmonics <= allowed {
            chosen = index;
        }
    }
    chosen
}

fn read_interpolated(table: &[f32], phase: u32) -> f32 {
    // 上位 11 ビットが表の位置。残りが、隣のサンプルとの割合。
    let shift = 32 - 11;
    let index = (phase >> shift) as usize;
    let fraction = (phase & ((1 << shift) - 1)) as f32 / (1u32 << shift) as f32;
    let current = table[index & LEN_MASK];
    let next = table[(index + 1) & LEN_MASK];
    current + (next - current) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_notes_use_fewer_harmonics() {
        let bank = WaveBank::new(48_000.0);
        let low = mip_index(&bank.saw, 110.0, 48_000.0);
        let high = mip_index(&bank.saw, 4_000.0, 48_000.0);
        assert!(bank.saw[high].harmonics < bank.saw[low].harmonics);
    }

    #[test]
    fn tables_stay_finite() {
        let bank = WaveBank::new(48_000.0);
        let sample = bank.read(TableShape::Saw, 0x1234_5678, 440.0);
        assert!(sample.is_finite());
        assert!(sample.abs() <= 2.0);
    }
}

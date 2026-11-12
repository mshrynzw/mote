//! Mod Env と LFO の割り当て。
//!
//! 各ソースは Target を 1 つ、Chain を 1 つ持つ。結果はノブへ書き戻さない。

use nice_plug::prelude::*;

/// 変調の行き先。Off では音を変えない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ModTarget {
    #[id = "off"]
    Off,
    #[id = "pitch"]
    Pitch,
    #[id = "volume"]
    Volume,
    #[id = "pan"]
    Pan,
    #[id = "cutoff"]
    Cutoff,
    #[id = "resonance"]
    Resonance,
    #[id = "amp"]
    Amp,
}

/// 変調を届ける列。All はオンの列すべて。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum ModChain {
    #[id = "all"]
    All,
    #[id = "a"]
    A,
    #[id = "b"]
    B,
    #[id = "c"]
    C,
    #[id = "d"]
    D,
}

impl ModChain {
    pub fn includes(self, index: usize) -> bool {
        match self {
            ModChain::All => index < 4,
            ModChain::A => index == 0,
            ModChain::B => index == 1,
            ModChain::C => index == 2,
            ModChain::D => index == 3,
        }
    }
}

/// 1 列へ足す変調。Amount 100% の深さは、各項目のフルスケール。
#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnMod {
    /// 半音。±24 がフルスケール。
    pub pitch: f32,
    /// dB。±12 がフルスケール。
    pub volume_db: f32,
    /// ±1 が左右いっぱい。
    pub pan: f32,
    /// オクターブ。±4 がフルスケール。
    pub cutoff_octaves: f32,
    /// 0 から 1 の Resonance へ足す量。±1 が ±100%。
    pub resonance: f32,
    /// Amp Env のあとの倍率は `1 + amp`。±1 が無音から 2 倍。
    pub amp: f32,
}

/// 1 つの Mod Env か LFO。`depth` は -1 から 1。
#[derive(Clone, Copy)]
pub struct ModSource {
    pub target: ModTarget,
    pub chain: ModChain,
    pub depth: f32,
}

/// ソースを 4 列へ振り分ける。同じ行き先は足す。
pub fn fold(sources: &[ModSource]) -> [ColumnMod; 4] {
    let mut columns = [ColumnMod::default(); 4];
    for source in sources {
        let depth = if source.depth.is_finite() {
            source.depth
        } else {
            0.0
        };
        if depth == 0.0 || source.target == ModTarget::Off {
            continue;
        }
        for (index, column) in columns.iter_mut().enumerate() {
            if !source.chain.includes(index) {
                continue;
            }
            match source.target {
                ModTarget::Off => {}
                ModTarget::Pitch => column.pitch += depth * 24.0,
                ModTarget::Volume => column.volume_db += depth * 12.0,
                ModTarget::Pan => column.pan += depth,
                ModTarget::Cutoff => column.cutoff_octaves += depth * 4.0,
                ModTarget::Resonance => column.resonance += depth,
                ModTarget::Amp => column.amp += depth,
            }
        }
    }
    columns
}

/// Amp Env のあとに掛ける倍率。0 が無音、1 がそのまま、2 が 2 倍。
pub fn amp_gain(depth: f32) -> f32 {
    (1.0 + depth).clamp(0.0, 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_and_zero_depth_leave_the_columns_alone() {
        let columns = fold(&[
            ModSource {
                target: ModTarget::Pitch,
                chain: ModChain::All,
                depth: 0.0,
            },
            ModSource {
                target: ModTarget::Off,
                chain: ModChain::A,
                depth: 1.0,
            },
        ]);
        assert_eq!(columns[0].pitch, 0.0);
        assert_eq!(columns[3].pitch, 0.0);
    }

    #[test]
    fn chain_a_does_not_reach_b() {
        let columns = fold(&[ModSource {
            target: ModTarget::Cutoff,
            chain: ModChain::A,
            depth: 1.0,
        }]);
        assert!((columns[0].cutoff_octaves - 4.0).abs() < 1.0e-5);
        assert_eq!(columns[1].cutoff_octaves, 0.0);
    }

    #[test]
    fn two_sources_add() {
        let columns = fold(&[
            ModSource {
                target: ModTarget::Pitch,
                chain: ModChain::All,
                depth: 0.5,
            },
            ModSource {
                target: ModTarget::Pitch,
                chain: ModChain::B,
                depth: -0.5,
            },
        ]);
        assert!((columns[0].pitch - 12.0).abs() < 1.0e-4);
        assert!(columns[1].pitch.abs() < 1.0e-4);
    }

    #[test]
    fn full_amp_depth_reaches_silence_and_double() {
        assert!((amp_gain(0.0) - 1.0).abs() < 1.0e-5);
        assert!((amp_gain(1.0) - 2.0).abs() < 1.0e-5);
        assert!(amp_gain(-1.0).abs() < 1.0e-5);
        assert!((amp_gain(3.0) - 2.0).abs() < 1.0e-5);
    }
}

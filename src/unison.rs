//! Unison の Voice を、十字の内側へ等間隔に置く。
//!
//! 横が Pan、縦が Detune。交点が中央。
//! 8 Voice でも、左端に 4 つ、右端に 4 つとは置かない。

/// `count` 本のうち `index` 番の位置。範囲は -1 より大きく、1 より小さい。
///
/// `index` は 0 から始まる。1 本だけのときは中央の 0。
pub fn unison_offset(index: usize, count: usize) -> f32 {
    if count <= 1 {
        return 0.0;
    }

    let n = count as f32;
    let i = index as f32;
    // 分母を n+1 にすると、両端の Voice が ±1 まで届かない。
    (2.0 * i - (n - 1.0)) / (n + 1.0)
}

/// Detune ノブの値を、その Voice のセントへ分配する。
///
/// いちばん外側が ±`detune_cents` になる。内側はそれに比例する。
pub fn unison_cents(index: usize, count: usize, detune_cents: f32) -> f32 {
    let offset = unison_offset(index, count);
    let edge = unison_offset(0, count).abs();
    if edge == 0.0 {
        0.0
    } else {
        detune_cents * offset / edge
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eight_voices_are_not_piled_on_the_edges() {
        let mut pans = [0.0; 8];
        for (index, pan) in pans.iter_mut().enumerate() {
            *pan = unison_offset(index, 8);
        }

        // 8 本が全部別の位置。左の 4 本が同じ端、にはならない。
        for i in 0..8 {
            for j in (i + 1)..8 {
                assert!((pans[i] - pans[j]).abs() > 0.05);
            }
        }
        assert!(pans[0] > -1.0);
        assert!(pans[7] < 1.0);
        assert!((pans[0] + pans[7]).abs() < 1.0e-5);
    }

    #[test]
    fn outer_voice_reaches_the_detune_knob() {
        let low = unison_cents(0, 8, 100.0);
        let high = unison_cents(7, 8, 100.0);
        assert!((low + 100.0).abs() < 1.0e-4);
        assert!((high - 100.0).abs() < 1.0e-4);
    }
}

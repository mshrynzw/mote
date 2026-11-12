//! ホストに見せるノブ。
//!
//! ID は `o1_octave` のように固定する。一度出したら、名前を変えても ID は変えない。

use nice_plug::prelude::*;

use std::sync::Arc;

use crate::envelope::{bind_amp_sync, AmpParams, AmpSyncHub};
use crate::oscillator::OscParams;

#[derive(Params)]
pub struct MoteParams {
    #[nested(id_prefix = "o1", group = "A Oscillator")]
    pub osc1: OscParams,
    #[nested(id_prefix = "o2", group = "B Oscillator")]
    pub osc2: OscParams,
    #[nested(id_prefix = "o3", group = "C Oscillator")]
    pub osc3: OscParams,
    #[nested(id_prefix = "o4", group = "D Oscillator")]
    pub osc4: OscParams,
    #[nested(id_prefix = "a1", group = "A Amp Env")]
    pub amp1: AmpParams,
    #[nested(id_prefix = "a2", group = "B Amp Env")]
    pub amp2: AmpParams,
    #[nested(id_prefix = "a3", group = "C Amp Env")]
    pub amp3: AmpParams,
    #[nested(id_prefix = "a4", group = "D Amp Env")]
    pub amp4: AmpParams,
}

impl Default for MoteParams {
    fn default() -> Self {
        // 4 基が同じ hub を持つ。`clone` は中身の複製ではなく、共有の数を 1 増やす。
        let hub = Arc::new(AmpSyncHub::new());
        Self {
            osc1: OscParams::new('A', true),
            osc2: OscParams::new('B', false),
            osc3: OscParams::new('C', false),
            osc4: OscParams::new('D', false),
            amp1: AmpParams::new('A', 0, hub.clone()),
            amp2: AmpParams::new('B', 1, hub.clone()),
            amp3: AmpParams::new('C', 2, hub.clone()),
            amp4: AmpParams::new('D', 3, hub),
        }
    }
}

impl MoteParams {
    /// 4 基を同じ順で渡す。`&` はコピーせず、ノブを借りる印。
    pub fn oscillators(&self) -> [&OscParams; 4] {
        [&self.osc1, &self.osc2, &self.osc3, &self.osc4]
    }

    pub fn amp_envelopes(&self) -> [&AmpParams; 4] {
        [&self.amp1, &self.amp2, &self.amp3, &self.amp4]
    }

    /// コピーのコールバックが 4 基を見つけられるようにする。`Arc` に入れてから呼ぶ。
    pub fn bind_amp_sync(&self) {
        bind_amp_sync(self.amp_envelopes());
    }
}

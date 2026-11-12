//! ホストに見せるノブ。
//!
//! ID は `o1_octave` のように固定する。一度出したら、名前を変えても ID は変えない。

use nice_plug::prelude::*;

use crate::oscillator::OscParams;

#[derive(Params)]
pub struct MoteParams {
    #[nested(id_prefix = "o1", group = "Oscillator 1")]
    pub osc1: OscParams,
    #[nested(id_prefix = "o2", group = "Oscillator 2")]
    pub osc2: OscParams,
    #[nested(id_prefix = "o3", group = "Oscillator 3")]
    pub osc3: OscParams,
    #[nested(id_prefix = "o4", group = "Oscillator 4")]
    pub osc4: OscParams,
}

impl Default for MoteParams {
    fn default() -> Self {
        Self {
            osc1: OscParams::new(true),
            osc2: OscParams::new(false),
            osc3: OscParams::new(false),
            osc4: OscParams::new(false),
        }
    }
}

impl MoteParams {
    /// 4 基を同じ順で渡す。`&` はコピーせず、ノブを借りる印。
    pub fn oscillators(&self) -> [&OscParams; 4] {
        [&self.osc1, &self.osc2, &self.osc3, &self.osc4]
    }
}

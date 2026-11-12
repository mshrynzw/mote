//! ホストに見せるノブ。
//!
//! ID は `o1_octave` のように固定する。一度出したら、名前を変えても ID は変えない。

use nice_plug::prelude::*;

use std::sync::Arc;

use crate::envelope::{bind_amp_sync, AmpParams, AmpSyncHub};
use crate::filter::{bind_filter_sync, FilterParams, FilterSyncHub};
use crate::filter_env::{bind_filter_env_sync, FilterEnvParams, FilterEnvSyncHub};
use crate::master::MasterParams;
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
    #[nested(id_prefix = "f1", group = "A Filter")]
    pub filter1: FilterParams,
    #[nested(id_prefix = "f2", group = "B Filter")]
    pub filter2: FilterParams,
    #[nested(id_prefix = "f3", group = "C Filter")]
    pub filter3: FilterParams,
    #[nested(id_prefix = "f4", group = "D Filter")]
    pub filter4: FilterParams,
    #[nested(id_prefix = "fe1", group = "A Filter Env")]
    pub filter_env1: FilterEnvParams,
    #[nested(id_prefix = "fe2", group = "B Filter Env")]
    pub filter_env2: FilterEnvParams,
    #[nested(id_prefix = "fe3", group = "C Filter Env")]
    pub filter_env3: FilterEnvParams,
    #[nested(id_prefix = "fe4", group = "D Filter Env")]
    pub filter_env4: FilterEnvParams,
    #[nested(id_prefix = "a1", group = "A Amp Env")]
    pub amp1: AmpParams,
    #[nested(id_prefix = "a2", group = "B Amp Env")]
    pub amp2: AmpParams,
    #[nested(id_prefix = "a3", group = "C Amp Env")]
    pub amp3: AmpParams,
    #[nested(id_prefix = "a4", group = "D Amp Env")]
    pub amp4: AmpParams,
    #[nested(id_prefix = "master", group = "Master")]
    pub master: MasterParams,
}

impl Default for MoteParams {
    fn default() -> Self {
        // 同じ種類の 4 基が、同じ hub を持つ。`clone` は中身の複製ではなく、共有の数を 1 増やす。
        let amp_hub = Arc::new(AmpSyncHub::new());
        let filter_hub = Arc::new(FilterSyncHub::new());
        let filter_env_hub = Arc::new(FilterEnvSyncHub::new());
        Self {
            osc1: OscParams::new('A', true),
            osc2: OscParams::new('B', false),
            osc3: OscParams::new('C', false),
            osc4: OscParams::new('D', false),
            filter1: FilterParams::new('A', 0, filter_hub.clone()),
            filter2: FilterParams::new('B', 1, filter_hub.clone()),
            filter3: FilterParams::new('C', 2, filter_hub.clone()),
            filter4: FilterParams::new('D', 3, filter_hub),
            filter_env1: FilterEnvParams::new('A', 0, filter_env_hub.clone()),
            filter_env2: FilterEnvParams::new('B', 1, filter_env_hub.clone()),
            filter_env3: FilterEnvParams::new('C', 2, filter_env_hub.clone()),
            filter_env4: FilterEnvParams::new('D', 3, filter_env_hub),
            amp1: AmpParams::new('A', 0, amp_hub.clone()),
            amp2: AmpParams::new('B', 1, amp_hub.clone()),
            amp3: AmpParams::new('C', 2, amp_hub.clone()),
            amp4: AmpParams::new('D', 3, amp_hub),
            master: MasterParams::new(),
        }
    }
}

impl MoteParams {
    /// 4 基を同じ順で渡す。`&` はコピーせず、ノブを借りる印。
    pub fn oscillators(&self) -> [&OscParams; 4] {
        [&self.osc1, &self.osc2, &self.osc3, &self.osc4]
    }

    pub fn filters(&self) -> [&FilterParams; 4] {
        [&self.filter1, &self.filter2, &self.filter3, &self.filter4]
    }

    pub fn filter_envelopes(&self) -> [&FilterEnvParams; 4] {
        [
            &self.filter_env1,
            &self.filter_env2,
            &self.filter_env3,
            &self.filter_env4,
        ]
    }

    pub fn amp_envelopes(&self) -> [&AmpParams; 4] {
        [&self.amp1, &self.amp2, &self.amp3, &self.amp4]
    }

    /// コピーのコールバックが 4 基を見つけられるようにする。`Arc` に入れてから呼ぶ。
    pub fn bind_amp_sync(&self) {
        bind_amp_sync(self.amp_envelopes());
    }

    pub fn bind_filter_sync(&self) {
        bind_filter_sync(self.filters());
    }

    pub fn bind_filter_env_sync(&self) {
        bind_filter_env_sync(self.filter_envelopes());
    }
}

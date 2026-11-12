//! Mod Env。4 基のエンベロープ。
//!
//! 時間の計算は Amp Env と同じ。行き先は、LFO のあとの割り当てで決める。
//! ここではノブと Sync だけを持ち、音の計算にはまだ足さない。

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;

use crate::assign::{ModChain, ModTarget};
use crate::envelope::{AmpEnvelope, AmpSettings};

/// エンベロープの高さと Amount から、割り当て先へ足す割合を返す。100% で高さそのもの。
pub fn scaled_amount(level: f32, amount_percent: f32) -> f32 {
    let level = if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let amount = if amount_percent.is_finite() {
        (amount_percent / 100.0).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    level * amount
}

/// 1 基の Mod Env。発音中は数値だけを進める。
pub struct ModEnv {
    envelope: AmpEnvelope,
}

impl ModEnv {
    pub fn new() -> Self {
        Self {
            envelope: AmpEnvelope::new(),
        }
    }

    pub fn reset(&mut self) {
        self.envelope.reset();
    }

    pub fn start(&mut self) {
        self.envelope.start();
    }

    pub fn release(&mut self) {
        self.envelope.release();
    }

    /// いまの高さ。0 が始まり、Sustain がその高さで留まる。
    pub fn next(&mut self, params: &ModEnvParams, sample_rate: f32) -> f32 {
        self.envelope.next(params.settings(), sample_rate.max(1.0))
    }
}

impl Default for ModEnv {
    fn default() -> Self {
        Self::new()
    }
}

struct CopyGuard<'a>(&'a AtomicBool);

impl Drop for CopyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 4 基を、コピーのコールバックから指すための置き場。
pub struct ModEnvSyncHub {
    envs: [AtomicPtr<ModEnvParams>; 4],
    busy: AtomicBool,
}

impl ModEnvSyncHub {
    pub fn new() -> Self {
        Self {
            envs: std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut())),
            busy: AtomicBool::new(false),
        }
    }

    pub fn bind(&self, envs: [&ModEnvParams; 4]) {
        for (slot, env) in self.envs.iter().zip(envs) {
            slot.store(
                env as *const ModEnvParams as *mut ModEnvParams,
                Ordering::Release,
            );
        }
    }

    fn enter(&self) -> Option<CopyGuard<'_>> {
        if self.busy.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some(CopyGuard(&self.busy))
        }
    }

    fn envs(&self) -> Option<[&ModEnvParams; 4]> {
        let mut raw = [std::ptr::null::<ModEnvParams>(); 4];
        for (slot, ptr) in raw.iter_mut().zip(self.envs.iter()) {
            let loaded = ptr.load(Ordering::Acquire);
            if loaded.is_null() {
                return None;
            }
            *slot = loaded;
        }
        Some(unsafe { raw.map(|ptr| &*ptr) })
    }

    fn on_knob(&self, index: usize, knob: EnvKnob, value: f32) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(envs) = self.envs() else {
            return;
        };
        if !in_link(envs, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(envs, other) {
                continue;
            }
            set_knob(envs[other], knob, value);
        }
    }

    fn on_master(&self, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(envs) = self.envs() else {
            return;
        };
        mirror_selected(envs);
    }

    fn on_target(&self, target: usize, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(envs) = self.envs() else {
            return;
        };
        if !envs[0].sync.value() {
            return;
        }
        copy_env(envs[0], envs[target]);
    }

    fn on_mod_target(&self, index: usize, value: ModTarget) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(envs) = self.envs() else {
            return;
        };
        if !in_link(envs, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(envs, other) {
                continue;
            }
            set_mod_target(envs[other], value);
        }
    }

    fn on_chain(&self, index: usize, value: ModChain) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(envs) = self.envs() else {
            return;
        };
        if !in_link(envs, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(envs, other) {
                continue;
            }
            set_chain(envs[other], value);
        }
    }
}

impl Default for ModEnvSyncHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum EnvKnob {
    Attack,
    AttackCurve,
    Decay,
    DecayCurve,
    Sustain,
    SustainCurve,
    Release,
    ReleaseCurve,
    Amount,
}

fn knob(env: &ModEnvParams, which: EnvKnob) -> &FloatParam {
    match which {
        EnvKnob::Attack => &env.attack,
        EnvKnob::AttackCurve => &env.attack_curve,
        EnvKnob::Decay => &env.decay,
        EnvKnob::DecayCurve => &env.decay_curve,
        EnvKnob::Sustain => &env.sustain,
        EnvKnob::SustainCurve => &env.sustain_curve,
        EnvKnob::Release => &env.release,
        EnvKnob::ReleaseCurve => &env.release_curve,
        EnvKnob::Amount => &env.amount,
    }
}

fn set_knob(env: &ModEnvParams, which: EnvKnob, value: f32) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(knob(env, which), value);
    }
}

fn copy_env(from: &ModEnvParams, to: &ModEnvParams) {
    for which in [
        EnvKnob::Attack,
        EnvKnob::AttackCurve,
        EnvKnob::Decay,
        EnvKnob::DecayCurve,
        EnvKnob::Sustain,
        EnvKnob::SustainCurve,
        EnvKnob::Release,
        EnvKnob::ReleaseCurve,
        EnvKnob::Amount,
    ] {
        set_knob(to, which, knob(from, which).unmodulated_plain_value());
    }
    set_mod_target(to, from.target.value());
    set_chain(to, from.chain.value());
}

fn set_mod_target(env: &ModEnvParams, value: ModTarget) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(&env.target, value);
    }
}

fn set_chain(env: &ModEnvParams, value: ModChain) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(&env.chain, value);
    }
}

fn mirror_selected(envs: [&ModEnvParams; 4]) {
    if envs[0].link_b.value() {
        copy_env(envs[0], envs[1]);
    }
    if envs[0].link_c.value() {
        copy_env(envs[0], envs[2]);
    }
    if envs[0].link_d.value() {
        copy_env(envs[0], envs[3]);
    }
}

fn in_link(envs: [&ModEnvParams; 4], index: usize) -> bool {
    if !envs[0].sync.value() {
        return false;
    }
    match index {
        0 => envs[0].link_b.value() || envs[0].link_c.value() || envs[0].link_d.value(),
        1 => envs[0].link_b.value(),
        2 => envs[0].link_c.value(),
        3 => envs[0].link_d.value(),
        _ => false,
    }
}

fn follow(hub: &Arc<ModEnvSyncHub>, index: usize, which: EnvKnob, param: FloatParam) -> FloatParam {
    let hub = Arc::clone(hub);
    param.with_callback(Arc::new(move |value| hub.on_knob(index, which, value)))
}

/// 1 基分のノブ。ID は `me1_attack` のように外側で番号を付ける。
#[derive(Params)]
pub struct ModEnvParams {
    #[id = "attack"]
    pub attack: FloatParam,
    #[id = "attack_curve"]
    pub attack_curve: FloatParam,
    #[id = "decay"]
    pub decay: FloatParam,
    #[id = "decay_curve"]
    pub decay_curve: FloatParam,
    #[id = "sustain"]
    pub sustain: FloatParam,
    #[id = "sustain_curve"]
    pub sustain_curve: FloatParam,
    #[id = "release"]
    pub release: FloatParam,
    #[id = "release_curve"]
    pub release_curve: FloatParam,
    #[id = "amount"]
    pub amount: FloatParam,
    #[id = "target"]
    pub target: EnumParam<ModTarget>,
    #[id = "chain"]
    pub chain: EnumParam<ModChain>,
    #[id = "sync"]
    pub sync: BoolParam,
    #[id = "link_b"]
    pub link_b: BoolParam,
    #[id = "link_c"]
    pub link_c: BoolParam,
    #[id = "link_d"]
    pub link_d: BoolParam,
    pub hub: Arc<ModEnvSyncHub>,
}

impl ModEnvParams {
    /// `index` は 0 が A。一覧では Amp Env や Filter Env と区別できるよう、Mod を名前に入れる。
    pub fn new(part: char, index: usize, hub: Arc<ModEnvSyncHub>) -> Self {
        let name = |label: &str| format!("{part} Mod {label}");
        let time = FloatRange::Skewed {
            min: 0.0,
            max: 10.0,
            factor: FloatRange::skew_factor(-2.0),
        };
        let curve = FloatRange::Linear {
            min: -1.0,
            max: 1.0,
        };
        let percent = FloatRange::Linear {
            min: -100.0,
            max: 100.0,
        };
        let attack = follow(
            &hub,
            index,
            EnvKnob::Attack,
            FloatParam::new(name("Attack"), 0.0, time).with_unit(" s"),
        );
        let attack_curve = follow(
            &hub,
            index,
            EnvKnob::AttackCurve,
            FloatParam::new(name("Attack Curve"), 0.0, curve),
        );
        let decay = follow(
            &hub,
            index,
            EnvKnob::Decay,
            FloatParam::new(name("Decay"), 0.3, time).with_unit(" s"),
        );
        let decay_curve = follow(
            &hub,
            index,
            EnvKnob::DecayCurve,
            FloatParam::new(name("Decay Curve"), 0.0, curve),
        );
        let sustain = follow(
            &hub,
            index,
            EnvKnob::Sustain,
            FloatParam::new(
                name("Sustain"),
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
        );
        let sustain_curve = follow(
            &hub,
            index,
            EnvKnob::SustainCurve,
            FloatParam::new(name("Sustain Curve"), 0.0, curve),
        );
        let release = follow(
            &hub,
            index,
            EnvKnob::Release,
            FloatParam::new(name("Release"), 0.05, time).with_unit(" s"),
        );
        let release_curve = follow(
            &hub,
            index,
            EnvKnob::ReleaseCurve,
            FloatParam::new(name("Release Curve"), 0.0, curve),
        );
        let amount = follow(
            &hub,
            index,
            EnvKnob::Amount,
            FloatParam::new(name("Amount"), 0.0, percent).with_unit(" %"),
        );
        let hub_target = Arc::clone(&hub);
        let hub_chain = Arc::clone(&hub);
        let target =
            EnumParam::new(name("Target"), ModTarget::Off).with_callback(Arc::new(move |value| {
                hub_target.on_mod_target(index, value)
            }));
        let chain = EnumParam::new(name("Chain"), ModChain::All)
            .with_callback(Arc::new(move |value| hub_chain.on_chain(index, value)));
        let sync = BoolParam::new(name("Sync"), false);
        let link_b = BoolParam::new(name("Sync B"), false);
        let link_c = BoolParam::new(name("Sync C"), false);
        let link_d = BoolParam::new(name("Sync D"), false);
        let (sync, link_b, link_c, link_d) = if index == 0 {
            let hub_master = Arc::clone(&hub);
            let hub_b = Arc::clone(&hub);
            let hub_c = Arc::clone(&hub);
            let hub_d = Arc::clone(&hub);
            (
                sync.with_callback(Arc::new(move |on| hub_master.on_master(on))),
                link_b.with_callback(Arc::new(move |on| hub_b.on_target(1, on))),
                link_c.with_callback(Arc::new(move |on| hub_c.on_target(2, on))),
                link_d.with_callback(Arc::new(move |on| hub_d.on_target(3, on))),
            )
        } else {
            (sync.hide(), link_b.hide(), link_c.hide(), link_d.hide())
        };
        Self {
            attack,
            attack_curve,
            decay,
            decay_curve,
            sustain,
            sustain_curve,
            release,
            release_curve,
            amount,
            target,
            chain,
            sync,
            link_b,
            link_c,
            link_d,
            hub,
        }
    }

    pub fn settings(&self) -> AmpSettings {
        AmpSettings {
            attack: self.attack.value(),
            decay: self.decay.value(),
            sustain: self.sustain.value(),
            release: self.release.value(),
            attack_curve: self.attack_curve.value(),
            decay_curve: self.decay_curve.value(),
            sustain_curve: self.sustain_curve.value(),
            release_curve: self.release_curve.value(),
        }
    }

    pub fn links(&self, index: usize) -> bool {
        if index == 0 || !self.sync.value() {
            return false;
        }
        match index {
            1 => self.link_b.value(),
            2 => self.link_c.value(),
            3 => self.link_d.value(),
            _ => false,
        }
    }
}

impl Default for ModEnvParams {
    fn default() -> Self {
        Self::new('A', 0, Arc::new(ModEnvSyncHub::new()))
    }
}

pub fn bind_mod_env_sync(envs: [&ModEnvParams; 4]) {
    envs[0].hub.bind(envs);
}

/// 揃えている列は A のノブを読む。
pub fn synced_mod_env<'a>(index: usize, envs: &'a [&'a ModEnvParams; 4]) -> &'a ModEnvParams {
    if envs[0].links(index) {
        envs[0]
    } else {
        envs[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_float(param: &FloatParam, value: f32) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, value);
        }
    }

    fn set_bool(param: &BoolParam, value: bool) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, value);
        }
    }

    #[test]
    fn amount_scales_the_level_and_zero_stays_quiet() {
        assert!((scaled_amount(1.0, 100.0) - 1.0).abs() < 1.0e-5);
        assert!((scaled_amount(1.0, -100.0) + 1.0).abs() < 1.0e-5);
        assert!((scaled_amount(0.5, 100.0) - 0.5).abs() < 1.0e-5);
        assert!(scaled_amount(1.0, 0.0).abs() < 1.0e-5);
    }

    #[test]
    fn default_envelope_reaches_sustain() {
        let params = ModEnvParams::default();
        let mut env = ModEnv::new();
        env.start();
        let mut level = 0.0;
        for _ in 0..8 {
            level = env.next(&params, 48_000.0);
        }
        assert!((level - 1.0).abs() < 0.01, "level {level}");
    }

    #[test]
    fn sync_copies_amount_onto_b() {
        let hub = Arc::new(ModEnvSyncHub::new());
        let envs = [
            ModEnvParams::new('A', 0, Arc::clone(&hub)),
            ModEnvParams::new('B', 1, Arc::clone(&hub)),
            ModEnvParams::new('C', 2, Arc::clone(&hub)),
            ModEnvParams::new('D', 3, Arc::clone(&hub)),
        ];
        bind_mod_env_sync([&envs[0], &envs[1], &envs[2], &envs[3]]);
        set_float(&envs[0].amount, -40.0);
        set_bool(&envs[0].sync, true);
        set_bool(&envs[0].link_b, true);
        assert!((envs[1].amount.value() + 40.0).abs() < 0.01);
        set_float(&envs[1].attack, 0.8);
        assert!((envs[0].attack.value() - 0.8).abs() < 0.01);
        set_bool(&envs[0].sync, false);
        set_float(&envs[0].amount, 0.0);
        assert!((envs[1].amount.value() + 40.0).abs() < 0.01);
    }
}

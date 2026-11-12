//! Filter Env。同じ番号の Filter の Cutoff だけを、エンベロープで上下する。
//!
//! 時間の計算は Amp Env と同じ。動かすのは音量ではなく、Cutoff のオクターブ。
//! Amount が 0% のあいだは、Cutoff ノブの位置を変えない。結果はノブへ書き戻さない。

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;

use crate::envelope::{AmpEnvelope, AmpSettings};

/// 100% のとき、エンベロープの頂点で Cutoff が上がるオクターブ数。
const FULL_OCTAVES: f32 = 4.0;

/// エンベロープの高さと Amount から、Cutoff に足すオクターブを返す。
pub fn cutoff_octaves(level: f32, amount_percent: f32) -> f32 {
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
    level * amount * FULL_OCTAVES
}

/// 1 列の Filter Env。エンベロープの状態と、Amount の追従を持つ。
pub struct FilterEnv {
    envelope: AmpEnvelope,
    amount: Smoother<f32>,
    last_amount: f32,
}

impl FilterEnv {
    pub fn new() -> Self {
        let amount = Smoother::new(SmoothingStyle::Linear(10.0));
        amount.reset(0.0);
        Self {
            envelope: AmpEnvelope::new(),
            amount,
            last_amount: 0.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// オフの列で呼ぶ。エンベロープは止める。Amount の追従は残す。
    pub fn silence(&mut self) {
        self.envelope.reset();
    }

    pub fn start(&mut self) {
        self.envelope.start();
    }

    pub fn release(&mut self) {
        self.envelope.release();
    }

    /// このサンプルで Cutoff を何オクターブ動かすか。`&mut self` はエンベロープを進めるため。
    pub fn next_octaves(&mut self, params: &FilterEnvParams, sample_rate: f32) -> f32 {
        let level = self.envelope.next(params.settings(), sample_rate.max(1.0));
        let target = params.amount.value();
        if (self.last_amount - target).abs() > 1.0e-3 {
            self.amount.set_target(sample_rate.max(1.0), target);
            self.last_amount = target;
        }
        cutoff_octaves(level, self.amount.next())
    }
}

impl Default for FilterEnv {
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

/// 4 基の Filter Env を、コピーのコールバックから指すための置き場。
pub struct FilterEnvSyncHub {
    envs: [AtomicPtr<FilterEnvParams>; 4],
    busy: AtomicBool,
}

impl FilterEnvSyncHub {
    pub fn new() -> Self {
        Self {
            envs: std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut())),
            busy: AtomicBool::new(false),
        }
    }

    pub fn bind(&self, envs: [&FilterEnvParams; 4]) {
        for (slot, env) in self.envs.iter().zip(envs) {
            slot.store(
                env as *const FilterEnvParams as *mut FilterEnvParams,
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

    fn envs(&self) -> Option<[&FilterEnvParams; 4]> {
        let mut raw = [std::ptr::null::<FilterEnvParams>(); 4];
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
}

impl Default for FilterEnvSyncHub {
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

fn knob(env: &FilterEnvParams, which: EnvKnob) -> &FloatParam {
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

fn set_knob(env: &FilterEnvParams, which: EnvKnob, value: f32) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(knob(env, which), value);
    }
}

fn copy_env(from: &FilterEnvParams, to: &FilterEnvParams) {
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
}

fn mirror_selected(envs: [&FilterEnvParams; 4]) {
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

fn in_link(envs: [&FilterEnvParams; 4], index: usize) -> bool {
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

fn follow(
    hub: &Arc<FilterEnvSyncHub>,
    index: usize,
    which: EnvKnob,
    param: FloatParam,
) -> FloatParam {
    let hub = Arc::clone(hub);
    param.with_callback(Arc::new(move |value| hub.on_knob(index, which, value)))
}

/// 1 基分の Filter Env ノブ。ID は `fe1_attack` のように外側で番号を付ける。
#[derive(Params)]
pub struct FilterEnvParams {
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
    #[id = "sync"]
    pub sync: BoolParam,
    #[id = "link_b"]
    pub link_b: BoolParam,
    #[id = "link_c"]
    pub link_c: BoolParam,
    #[id = "link_d"]
    pub link_d: BoolParam,
    pub hub: Arc<FilterEnvSyncHub>,
}

impl FilterEnvParams {
    /// `index` は 0 が A。一覧では Amp Env の Attack と区別できるよう、Filter を名前に入れる。
    pub fn new(part: char, index: usize, hub: Arc<FilterEnvSyncHub>) -> Self {
        let name = |label: &str| format!("{part} Filter {label}");
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

impl Default for FilterEnvParams {
    fn default() -> Self {
        Self::new('A', 0, Arc::new(FilterEnvSyncHub::new()))
    }
}

pub fn bind_filter_env_sync(envs: [&FilterEnvParams; 4]) {
    envs[0].hub.bind(envs);
}

/// 揃えている列は A のノブを読む。
pub fn synced_filter_env<'a>(
    index: usize,
    envs: &'a [&'a FilterEnvParams; 4],
) -> &'a FilterEnvParams {
    if envs[0].links(index) {
        envs[0]
    } else {
        envs[index]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{Filter, FilterParams};

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
    fn amount_scales_octaves_and_zero_stays_put() {
        assert!((cutoff_octaves(1.0, 100.0) - 4.0).abs() < 1.0e-5);
        assert!((cutoff_octaves(1.0, -100.0) + 4.0).abs() < 1.0e-5);
        assert!((cutoff_octaves(0.5, 100.0) - 2.0).abs() < 1.0e-5);
        assert!(cutoff_octaves(1.0, 0.0).abs() < 1.0e-5);
        assert!(cutoff_octaves(0.0, 100.0).abs() < 1.0e-5);
    }

    #[test]
    fn positive_envelope_opens_the_lowpass() {
        let params = FilterParams::default();
        set_float(&params.cutoff, 200.0);
        let mut closed = Filter::new();
        let mut open = Filter::new();
        let mut closed_peak: f32 = 0.0;
        let mut open_peak: f32 = 0.0;
        let mut phase = 0.0;
        for i in 0..4_096 {
            let sample = (phase * std::f32::consts::TAU).sin();
            phase = (phase + 800.0 / 48_000.0) % 1.0;
            let (low, _) = closed.next_sample(&params, sample, sample, Some(60), 48_000.0, 0.0);
            let (high, _) = open.next_sample(&params, sample, sample, Some(60), 48_000.0, 4.0);
            if i > 3_000 {
                closed_peak = closed_peak.max(low.abs());
                open_peak = open_peak.max(high.abs());
            }
        }
        assert!(
            open_peak > closed_peak * 2.0,
            "open {open_peak}, closed {closed_peak}"
        );
    }

    #[test]
    fn sync_copies_amount_onto_b() {
        let hub = Arc::new(FilterEnvSyncHub::new());
        let envs = [
            FilterEnvParams::new('A', 0, Arc::clone(&hub)),
            FilterEnvParams::new('B', 1, Arc::clone(&hub)),
            FilterEnvParams::new('C', 2, Arc::clone(&hub)),
            FilterEnvParams::new('D', 3, Arc::clone(&hub)),
        ];
        bind_filter_env_sync([&envs[0], &envs[1], &envs[2], &envs[3]]);
        set_float(&envs[0].amount, 80.0);
        set_bool(&envs[0].sync, true);
        set_bool(&envs[0].link_b, true);
        assert!((envs[1].amount.value() - 80.0).abs() < 0.01);
        set_float(&envs[1].attack, 1.25);
        assert!((envs[0].attack.value() - 1.25).abs() < 0.01);
        set_bool(&envs[0].sync, false);
        set_float(&envs[0].amount, 0.0);
        assert!((envs[1].amount.value() - 80.0).abs() < 0.01);
    }
}

//! アンプ用のエンベロープ。
//!
//! Attack、Decay、Release は時間をかけて値が動く。
//! Sustain は押しているあいだの高さを保つ。ここは平坦なので、Sustain Curve はノブとして残し、音の高さ自体は曲げない。
//! 発音中にメモリは確保しない。
//!
//! Sync は変調の書き戻しではない。利用者が A の Sync をオンにして B、C、D を選んだときだけ、ノブの値をコピーする。

use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;

/// エンベロープの今の区間。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

pub struct AmpEnvelope {
    stage: Stage,
    /// 今の区間の進み。0 が始まり、1 が終わり。
    position: f32,
    level: f32,
    /// Release を始めたときの高さ。そこから 0 へ戻る。
    release_start: f32,
}

impl AmpEnvelope {
    pub fn new() -> Self {
        Self {
            stage: Stage::Idle,
            position: 0.0,
            level: 0.0,
            release_start: 0.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn is_busy(&self) -> bool {
        self.stage != Stage::Idle
    }

    /// いまの高さ。0 が無音、1 が Attack の終わり。
    pub fn level(&self) -> f32 {
        self.level
    }

    /// ノートオン。Attack の頭に戻す。
    pub fn start(&mut self) {
        self.stage = Stage::Attack;
        self.position = 0.0;
        self.level = 0.0;
        self.release_start = 0.0;
    }

    /// ノートオフ。今の高さから Release を始める。
    pub fn release(&mut self) {
        if self.stage == Stage::Idle {
            return;
        }
        self.release_start = self.level;
        self.stage = Stage::Release;
        self.position = 0.0;
    }

    /// 1 サンプル進めて、0 から 1 付近の音量を返す。
    pub fn next(&mut self, settings: AmpSettings, sample_rate: f32) -> f32 {
        // 時間が 0 の区間は、その場で次へ進む。最大でも数回なので、ループは短い。
        for _ in 0..4 {
            match self.stage {
                Stage::Idle => return 0.0,
                Stage::Attack => {
                    if !self.advance(settings.attack, sample_rate) {
                        self.level = shape(self.position, settings.attack_curve);
                        return self.level;
                    }
                    self.level = 1.0;
                    self.enter(Stage::Decay);
                }
                Stage::Decay => {
                    if !self.advance(settings.decay, sample_rate) {
                        let fall = shape(self.position, settings.decay_curve);
                        self.level = 1.0 + (settings.sustain - 1.0) * fall;
                        return self.level;
                    }
                    self.level = settings.sustain;
                    self.enter(Stage::Sustain);
                }
                Stage::Sustain => {
                    // Sustain は高さを保つ区間なので、Curve は音の高さを曲げない。ノブは残してある。
                    let _hold_curve = settings.sustain_curve;
                    self.level = settings.sustain;
                    return self.level;
                }
                Stage::Release => {
                    if !self.advance(settings.release, sample_rate) {
                        let fall = shape(self.position, settings.release_curve);
                        self.level = self.release_start * (1.0 - fall);
                        return self.level;
                    }
                    self.level = 0.0;
                    self.enter(Stage::Idle);
                    return 0.0;
                }
            }
        }
        self.level
    }

    fn enter(&mut self, stage: Stage) {
        self.stage = stage;
        self.position = 0.0;
    }

    /// 区間を 1 サンプル進める。終わったら `true`。
    fn advance(&mut self, seconds: f32, sample_rate: f32) -> bool {
        let length = seconds.max(0.0) * sample_rate;
        if length <= 1.0 {
            self.position = 1.0;
            return true;
        }
        self.position += 1.0 / length;
        self.position >= 1.0
    }
}

impl Default for AmpEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

/// 1 サンプル分のエンベロープ設定。ノブからコピーした値で、発音中にノブへは書き戻さない。
#[derive(Clone, Copy)]
pub struct AmpSettings {
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub attack_curve: f32,
    pub decay_curve: f32,
    pub sustain_curve: f32,
    pub release_curve: f32,
}

/// コピー中かどうか。コールバックから別のノブを書くと、そのノブのコールバックがもう一度走る。
/// `busy` が true のあいだは、その二度目を無視して、A と B が互いを呼び続けるのを止める。
struct CopyGuard<'a>(&'a AtomicBool);

impl Drop for CopyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 4 基の Amp Env を、コピーのコールバックから指すための置き場。
///
/// ノブを作る時点では、4 基はまだ並んでいない。`bind_amp_sync` で、並んだあとの住所を入れる。
/// `AtomicPtr` は、その住所をロックせずに読み書きする印。
pub struct AmpSyncHub {
    amps: [AtomicPtr<AmpParams>; 4],
    busy: AtomicBool,
}

impl AmpSyncHub {
    pub fn new() -> Self {
        Self {
            amps: std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut())),
            busy: AtomicBool::new(false),
        }
    }

    /// `amps` は `Arc` の中に置いた 4 基を指す。`Arc` の中身は動かないので、この住所はプラグインがあるあいだ有効。
    pub fn bind(&self, amps: [&AmpParams; 4]) {
        for (slot, amp) in self.amps.iter().zip(amps) {
            slot.store(amp as *const AmpParams as *mut AmpParams, Ordering::Release);
        }
    }

    fn enter(&self) -> Option<CopyGuard<'_>> {
        if self.busy.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some(CopyGuard(&self.busy))
        }
    }

    /// 住所がまだ無いときは `None`。あるときは 4 基を共有参照で返す。`&` は書き換えの独占ではない。
    fn amps(&self) -> Option<[&AmpParams; 4]> {
        let mut raw = [std::ptr::null::<AmpParams>(); 4];
        for (slot, ptr) in raw.iter_mut().zip(self.amps.iter()) {
            let loaded = ptr.load(Ordering::Acquire);
            if loaded.is_null() {
                return None;
            }
            *slot = loaded;
        }
        // 住所は `bind` で渡した 4 基。呼び出し側は共有参照だけで、コピーが終わるまで捨てない。
        Some(unsafe { raw.map(|ptr| &*ptr) })
    }

    fn on_knob(&self, index: usize, knob: AmpKnob, value: f32) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(amps) = self.amps() else {
            return;
        };
        if !in_link(amps, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(amps, other) {
                continue;
            }
            set_knob(amps[other], knob, value);
        }
    }

    fn on_master(&self, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(amps) = self.amps() else {
            return;
        };
        // オンにした瞬間は、選んである列を A で上書きする。
        // オフにした瞬間も同じコピーをして、聞こえていた値をその列へ残す。
        mirror_selected(amps);
    }

    fn on_target(&self, target: usize, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(amps) = self.amps() else {
            return;
        };
        if !amps[0].sync.value() {
            return;
        }
        copy_amp(amps[0], amps[target]);
    }
}

impl Default for AmpSyncHub {
    fn default() -> Self {
        Self::new()
    }
}

/// コピーする 8 つのノブ。Sync のスイッチ自体はコピーしない。
#[derive(Clone, Copy)]
enum AmpKnob {
    Attack,
    AttackCurve,
    Decay,
    DecayCurve,
    Sustain,
    SustainCurve,
    Release,
    ReleaseCurve,
}

const AMP_KNOBS: [AmpKnob; 8] = [
    AmpKnob::Attack,
    AmpKnob::AttackCurve,
    AmpKnob::Decay,
    AmpKnob::DecayCurve,
    AmpKnob::Sustain,
    AmpKnob::SustainCurve,
    AmpKnob::Release,
    AmpKnob::ReleaseCurve,
];

fn amp_knob(amp: &AmpParams, which: AmpKnob) -> &FloatParam {
    match which {
        AmpKnob::Attack => &amp.attack,
        AmpKnob::AttackCurve => &amp.attack_curve,
        AmpKnob::Decay => &amp.decay,
        AmpKnob::DecayCurve => &amp.decay_curve,
        AmpKnob::Sustain => &amp.sustain,
        AmpKnob::SustainCurve => &amp.sustain_curve,
        AmpKnob::Release => &amp.release,
        AmpKnob::ReleaseCurve => &amp.release_curve,
    }
}

fn set_knob(amp: &AmpParams, which: AmpKnob, value: f32) {
    // 同じ値ならコールバックは走らない。ここでの書き込みは、利用者の Sync によるコピー。
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(amp_knob(amp, which), value);
    }
}

fn copy_amp(from: &AmpParams, to: &AmpParams) {
    for which in AMP_KNOBS {
        // 変調を足したあとの値ではなく、ノブに出ている値をコピーする。
        set_knob(to, which, amp_knob(from, which).unmodulated_plain_value());
    }
}

fn mirror_selected(amps: [&AmpParams; 4]) {
    if amps[0].link_b.value() {
        copy_amp(amps[0], amps[1]);
    }
    if amps[0].link_c.value() {
        copy_amp(amps[0], amps[2]);
    }
    if amps[0].link_d.value() {
        copy_amp(amps[0], amps[3]);
    }
}

/// A の Sync がオンで、その列が対象なら true。0 は A 自身で、対象が 1 列でもあれば true。
fn in_link(amps: [&AmpParams; 4], index: usize) -> bool {
    if !amps[0].sync.value() {
        return false;
    }
    match index {
        0 => amps[0].link_b.value() || amps[0].link_c.value() || amps[0].link_d.value(),
        1 => amps[0].link_b.value(),
        2 => amps[0].link_c.value(),
        3 => amps[0].link_d.value(),
        _ => false,
    }
}

fn follow_float(
    hub: &Arc<AmpSyncHub>,
    index: usize,
    which: AmpKnob,
    param: FloatParam,
) -> FloatParam {
    let hub = Arc::clone(hub);
    // `Arc` はコールバックとノブで同じ hub を共有する。`move` は、その hub をコールバックの中へ持っていく。
    param.with_callback(Arc::new(move |value| hub.on_knob(index, which, value)))
}

/// 1 基分の Amp Env ノブ。ID は `a1_attack` のように外側で番号を付ける。
#[derive(Params)]
pub struct AmpParams {
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
    /// A の揃えるスイッチ。B、C、D では隠す。ID `sync` は残す。
    #[id = "sync"]
    pub sync: BoolParam,
    /// A から見た、B を揃える対象にするスイッチ。
    #[id = "link_b"]
    pub link_b: BoolParam,
    #[id = "link_c"]
    pub link_c: BoolParam,
    #[id = "link_d"]
    pub link_d: BoolParam,
    /// パラメータ一覧には出さない。4 基をコピーでつなぐ。
    pub hub: Arc<AmpSyncHub>,
}

impl AmpParams {
    /// `index` は 0 が A、1 が B、2 が C、3 が D。Sync のスイッチが見えるのは A だけ。
    pub fn new(part: char, index: usize, hub: Arc<AmpSyncHub>) -> Self {
        let name = |label: &str| format!("{part} {label}");
        // 時間は 0 秒から 10 秒。短い時間の方がノブの動きが大きい。
        let time = FloatRange::Skewed {
            min: 0.0,
            max: 10.0,
            factor: FloatRange::skew_factor(-2.0),
        };
        let curve = FloatRange::Linear {
            min: -1.0,
            max: 1.0,
        };
        let attack = follow_float(
            &hub,
            index,
            AmpKnob::Attack,
            FloatParam::new(name("Attack"), 0.0, time).with_unit(" s"),
        );
        let attack_curve = follow_float(
            &hub,
            index,
            AmpKnob::AttackCurve,
            FloatParam::new(name("Attack Curve"), 0.0, curve),
        );
        let decay = follow_float(
            &hub,
            index,
            AmpKnob::Decay,
            FloatParam::new(name("Decay"), 0.3, time).with_unit(" s"),
        );
        let decay_curve = follow_float(
            &hub,
            index,
            AmpKnob::DecayCurve,
            FloatParam::new(name("Decay Curve"), 0.0, curve),
        );
        let sustain = follow_float(
            &hub,
            index,
            AmpKnob::Sustain,
            FloatParam::new(
                name("Sustain"),
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
        );
        let sustain_curve = follow_float(
            &hub,
            index,
            AmpKnob::SustainCurve,
            FloatParam::new(name("Sustain Curve"), 0.0, curve),
        );
        let release = follow_float(
            &hub,
            index,
            AmpKnob::Release,
            FloatParam::new(name("Release"), 0.05, time).with_unit(" s"),
        );
        let release_curve = follow_float(
            &hub,
            index,
            AmpKnob::ReleaseCurve,
            FloatParam::new(name("Release Curve"), 0.0, curve),
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
            sync,
            link_b,
            link_c,
            link_d,
            hub,
        }
    }

    /// A の Sync がオンで、`index` の列が対象なら true。音は、そのあいだ A のノブを読む。
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

impl Default for AmpParams {
    fn default() -> Self {
        Self::new('A', 0, Arc::new(AmpSyncHub::new()))
    }
}

impl AmpParams {
    /// 今のノブ位置を、エンベロープが読む値へコピーする。ノブ自体は変えない。
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
}

/// `Arc` の中に置いた 4 基の住所を、コピー用の hub へ渡す。
pub fn bind_amp_sync(amps: [&AmpParams; 4]) {
    amps[0].hub.bind(amps);
}

/// 揃えている列は A のノブを読む。外れている列は自分のノブを読む。
pub fn synced_amp<'a>(index: usize, amps: &'a [&'a AmpParams; 4]) -> &'a AmpParams {
    if amps[0].links(index) {
        amps[0]
    } else {
        amps[index]
    }
}

/// Curve の -1 は最初が急、0 は直線、+1 は最初が緩やか。
pub fn shape(position: f32, curve: f32) -> f32 {
    let t = position.clamp(0.0, 1.0);
    if curve.abs() < 1.0e-4 {
        return t;
    }
    let bend = 1.0 + curve.abs() * 4.0;
    if curve > 0.0 {
        t.powf(bend)
    } else {
        1.0 - (1.0 - t).powf(bend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear() -> AmpSettings {
        AmpSettings {
            attack: 1.0,
            decay: 1.0,
            sustain: 0.5,
            release: 1.0,
            attack_curve: 0.0,
            decay_curve: 0.0,
            sustain_curve: 0.0,
            release_curve: 0.0,
        }
    }

    #[test]
    fn linear_midpoint_is_halfway() {
        assert!((shape(0.5, 0.0) - 0.5).abs() < 1.0e-5);
    }

    #[test]
    fn positive_curve_starts_slower_than_linear() {
        assert!(shape(0.5, 1.0) < 0.5);
    }

    #[test]
    fn negative_curve_starts_faster_than_linear() {
        assert!(shape(0.5, -1.0) > 0.5);
    }

    #[test]
    fn sync_copies_the_chosen_columns_and_keeps_them_after_off() {
        let hub = Arc::new(AmpSyncHub::new());
        let amps = [
            AmpParams::new('A', 0, Arc::clone(&hub)),
            AmpParams::new('B', 1, Arc::clone(&hub)),
            AmpParams::new('C', 2, Arc::clone(&hub)),
            AmpParams::new('D', 3, Arc::clone(&hub)),
        ];
        bind_amp_sync([&amps[0], &amps[1], &amps[2], &amps[3]]);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].attack, 1.5);
            // Sync がオフのまま B を選んでも、まだ上書きしない。
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].link_b, true);
        }
        assert!((amps[1].attack.value() - 0.0).abs() < 1.0e-5);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].sync, true);
        }
        assert!((amps[1].attack.value() - 1.5).abs() < 1.0e-5);
        assert!((amps[2].attack.value() - 0.0).abs() < 1.0e-5);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[1].decay, 2.5);
        }
        assert!((amps[0].decay.value() - 2.5).abs() < 1.0e-5);
        assert!((amps[2].decay.value() - 0.3).abs() < 1.0e-5);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].link_c, true);
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].sustain, 0.25);
        }
        assert!((amps[1].sustain.value() - 0.25).abs() < 1.0e-5);
        assert!((amps[2].sustain.value() - 0.25).abs() < 1.0e-5);
        assert!((amps[2].attack.value() - 1.5).abs() < 1.0e-5);
        assert!((amps[3].sustain.value() - 1.0).abs() < 1.0e-5);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[1].release, 0.8);
        }
        assert!((amps[0].release.value() - 0.8).abs() < 1.0e-5);
        assert!((amps[2].release.value() - 0.8).abs() < 1.0e-5);
        let view = [&amps[0], &amps[1], &amps[2], &amps[3]];
        assert!((synced_amp(1, &view).settings().release - 0.8).abs() < 1.0e-5);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].sync, false);
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].sustain, 0.9);
        }
        assert!((amps[1].sustain.value() - 0.25).abs() < 1.0e-5);
        assert!((amps[2].sustain.value() - 0.25).abs() < 1.0e-5);

        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(&amps[0].sync, true);
        }
        assert!((amps[1].sustain.value() - 0.9).abs() < 1.0e-5);
        assert!((amps[2].sustain.value() - 0.9).abs() < 1.0e-5);
        assert!((amps[3].sustain.value() - 1.0).abs() < 1.0e-5);
    }

    #[test]
    fn release_does_not_finish_on_the_first_sample() {
        let mut env = AmpEnvelope::new();
        env.start();
        env.level = 1.0;
        env.release();
        let mut settings = linear();
        settings.release = 1.0;
        let first = env.next(settings, 48_000.0);
        assert!(first > 0.5);
        assert!(env.is_busy());
    }
}

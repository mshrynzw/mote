//! LFO。4 基の周期的な変調。
//!
//! Tempo がオフのとき Rate は Hz。オンのとき、同じノブ位置が音符の長さになる。
//! 行き先は次の割り当てで決める。ここでは波を計算するが、音にはまだ足さない。

use std::f32::consts::TAU;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;

use crate::assign::{ModChain, ModTarget};

/// 1 周の波形。並びを変えると、保存した音色の番号がずれる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum LfoWave {
    #[id = "sine"]
    Sine,
    #[id = "triangle"]
    Triangle,
    #[id = "saw"]
    Saw,
    #[id = "square"]
    Square,
    #[id = "sample_hold"]
    SampleHold,
}

const RATE_MIN: f32 = 0.01;
const RATE_MAX: f32 = 20.0;
/// 初期値 1 Hz のノブ位置が、Tempo オンでは 4 分音符の枠に入る。
const RATE_SKEW: f32 = 0.326_368_4;

/// 遅い順。単位は拍。1 小節目は呼び出し側の拍子で置き換える。
const DIVISION_BEATS: [f32; 12] = [
    4.0,
    3.0,
    2.0,
    1.5,
    1.0,
    0.75,
    2.0 / 3.0,
    0.5,
    1.0 / 3.0,
    0.25,
    1.0 / 6.0,
    0.125,
];
const DIVISION_NAMES: [&str; 12] = [
    "1 Bar", "1/2.", "1/2", "1/4.", "1/4", "1/8.", "1/4 T", "1/8", "1/8 T", "1/16", "1/16 T",
    "1/32",
];
pub fn rate_range() -> FloatRange {
    FloatRange::Skewed {
        min: RATE_MIN,
        max: RATE_MAX,
        factor: RATE_SKEW,
    }
}

/// ノブの Hz を 0 から 1 の位置にする。Tempo オンでは、この位置で音符を選ぶ。
pub fn rate_normalized(hz: f32) -> f32 {
    let span = RATE_MAX - RATE_MIN;
    let ratio = (hz.clamp(RATE_MIN, RATE_MAX) - RATE_MIN) / span;
    ratio.powf(RATE_SKEW).clamp(0.0, 1.0)
}

pub fn rate_from_normalized(normalized: f32) -> f32 {
    let span = RATE_MAX - RATE_MIN;
    normalized.clamp(0.0, 1.0).powf(1.0 / RATE_SKEW) * span + RATE_MIN
}

pub fn division_name(normalized: f32) -> &'static str {
    DIVISION_NAMES[division_index(normalized)]
}

pub fn division_index(normalized: f32) -> usize {
    let scaled = normalized.clamp(0.0, 0.999_999) * DIVISION_BEATS.len() as f32;
    scaled as usize
}

/// Tempo オンのとき、1 周に何拍かかるか。1 小節は `beats_per_bar`。
pub fn division_beats(normalized: f32, beats_per_bar: f32) -> f32 {
    let index = division_index(normalized);
    if index == 0 {
        beats_per_bar.max(1.0)
    } else {
        DIVISION_BEATS[index]
    }
}

/// 1 秒あたりの周回数。Tempo オフは Rate の Hz。取れない BPM は 120。
pub fn lfo_hz(rate_hz: f32, tempo: bool, bpm: f32, beats_per_bar: f32) -> f32 {
    if !tempo {
        return rate_hz.clamp(RATE_MIN, RATE_MAX);
    }
    let bpm = if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        120.0
    };
    let beats = division_beats(rate_normalized(rate_hz), beats_per_bar);
    (bpm / 60.0) / beats
}

/// 波の 1 点。-1 から 1。`hold` は Sample & Hold が保っている値。
pub fn sample_wave(wave: LfoWave, phase: f32, hold: f32) -> f32 {
    let phase = phase.rem_euclid(1.0);
    match wave {
        LfoWave::Sine => (phase * TAU).sin(),
        LfoWave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        LfoWave::Saw => phase * 2.0 - 1.0,
        LfoWave::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        LfoWave::SampleHold => hold.clamp(-1.0, 1.0),
    }
}

pub fn apply_amount(sample: f32, amount_percent: f32) -> f32 {
    let amount = if amount_percent.is_finite() {
        (amount_percent / 100.0).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    sample * amount
}

fn next_random(state: &mut u32) -> f32 {
    let mut x = state.wrapping_add(0x9E37_79B9);
    if x == 0 {
        x = 1;
    }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// 1 基分の位相。起動時に確保し、発音中は数値だけ進める。
pub struct LfoPhase {
    phase: f32,
    hold: f32,
    rng: u32,
}

impl LfoPhase {
    pub fn new(seed: u32) -> Self {
        let mut rng = seed | 1;
        let hold = next_random(&mut rng);
        Self {
            phase: 0.0,
            hold,
            rng,
        }
    }

    pub fn reset(&mut self, seed: u32) {
        *self = Self::new(seed);
    }

    /// Retrigger オンのノート開始。Phase ノブの位置から打ち直す。
    pub fn retrigger(&mut self, phase: f32) {
        self.phase = phase.rem_euclid(1.0);
        self.hold = next_random(&mut self.rng);
    }

    /// 1 サンプル進めて、Amount を掛ける前の波を返す。
    pub fn advance(&mut self, hz: f32, sample_rate: f32, wave: LfoWave, phase_offset: f32) -> f32 {
        let rate = if sample_rate > 0.0 { sample_rate } else { 1.0 };
        let step = if hz.is_finite() {
            hz.max(0.0) / rate
        } else {
            0.0
        };
        let before = (self.phase + phase_offset).rem_euclid(1.0);
        self.phase = (self.phase + step).rem_euclid(1.0);
        let after = (self.phase + phase_offset).rem_euclid(1.0);
        if after < before {
            self.hold = next_random(&mut self.rng);
        }
        sample_wave(wave, after, self.hold)
    }
}

impl Default for LfoPhase {
    fn default() -> Self {
        Self::new(1)
    }
}

struct CopyGuard<'a>(&'a AtomicBool);

impl Drop for CopyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub struct LfoSyncHub {
    lfos: [AtomicPtr<LfoParams>; 4],
    busy: AtomicBool,
}

impl LfoSyncHub {
    pub fn new() -> Self {
        Self {
            lfos: std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut())),
            busy: AtomicBool::new(false),
        }
    }

    pub fn bind(&self, lfos: [&LfoParams; 4]) {
        for (slot, lfo) in self.lfos.iter().zip(lfos) {
            slot.store(lfo as *const LfoParams as *mut LfoParams, Ordering::Release);
        }
    }

    fn enter(&self) -> Option<CopyGuard<'_>> {
        if self.busy.swap(true, Ordering::AcqRel) {
            None
        } else {
            Some(CopyGuard(&self.busy))
        }
    }

    fn lfos(&self) -> Option<[&LfoParams; 4]> {
        let mut raw = [std::ptr::null::<LfoParams>(); 4];
        for (slot, ptr) in raw.iter_mut().zip(self.lfos.iter()) {
            let loaded = ptr.load(Ordering::Acquire);
            if loaded.is_null() {
                return None;
            }
            *slot = loaded;
        }
        Some(unsafe { raw.map(|ptr| &*ptr) })
    }

    fn on_float(&self, index: usize, knob: FloatKnob, value: f32) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        if !in_link(lfos, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(lfos, other) {
                continue;
            }
            set_float(lfos[other], knob, value);
        }
    }

    fn on_bool(&self, index: usize, knob: BoolKnob, value: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        if !in_link(lfos, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(lfos, other) {
                continue;
            }
            set_bool(lfos[other], knob, value);
        }
    }

    fn on_wave(&self, index: usize, value: LfoWave) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        if !in_link(lfos, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(lfos, other) {
                continue;
            }
            set_wave(lfos[other], value);
        }
    }

    fn on_master(&self, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        mirror_selected(lfos);
    }

    fn on_target(&self, target: usize, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        if !lfos[0].sync.value() {
            return;
        }
        copy_lfo(lfos[0], lfos[target]);
    }

    fn on_mod_target(&self, index: usize, value: ModTarget) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        if !in_link(lfos, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(lfos, other) {
                continue;
            }
            set_mod_target(lfos[other], value);
        }
    }

    fn on_chain(&self, index: usize, value: ModChain) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(lfos) = self.lfos() else {
            return;
        };
        if !in_link(lfos, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(lfos, other) {
                continue;
            }
            set_chain(lfos[other], value);
        }
    }
}

impl Default for LfoSyncHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum FloatKnob {
    Rate,
    Phase,
    Amount,
}

#[derive(Clone, Copy)]
enum BoolKnob {
    Tempo,
    Retrigger,
}

fn float_param(lfo: &LfoParams, knob: FloatKnob) -> &FloatParam {
    match knob {
        FloatKnob::Rate => &lfo.rate,
        FloatKnob::Phase => &lfo.phase,
        FloatKnob::Amount => &lfo.amount,
    }
}

fn bool_param(lfo: &LfoParams, knob: BoolKnob) -> &BoolParam {
    match knob {
        BoolKnob::Tempo => &lfo.tempo,
        BoolKnob::Retrigger => &lfo.retrigger,
    }
}

fn set_float(lfo: &LfoParams, knob: FloatKnob, value: f32) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(float_param(lfo, knob), value);
    }
}

fn set_bool(lfo: &LfoParams, knob: BoolKnob, value: bool) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(bool_param(lfo, knob), value);
    }
}

fn set_wave(lfo: &LfoParams, value: LfoWave) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(&lfo.wave, value);
    }
}

fn copy_lfo(from: &LfoParams, to: &LfoParams) {
    set_wave(to, from.wave.value());
    for knob in [FloatKnob::Rate, FloatKnob::Phase, FloatKnob::Amount] {
        set_float(to, knob, float_param(from, knob).unmodulated_plain_value());
    }
    for knob in [BoolKnob::Tempo, BoolKnob::Retrigger] {
        set_bool(to, knob, bool_param(from, knob).value());
    }
    set_mod_target(to, from.target.value());
    set_chain(to, from.chain.value());
}

fn set_mod_target(lfo: &LfoParams, value: ModTarget) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(&lfo.target, value);
    }
}

fn set_chain(lfo: &LfoParams, value: ModChain) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(&lfo.chain, value);
    }
}

fn mirror_selected(lfos: [&LfoParams; 4]) {
    if lfos[0].link_b.value() {
        copy_lfo(lfos[0], lfos[1]);
    }
    if lfos[0].link_c.value() {
        copy_lfo(lfos[0], lfos[2]);
    }
    if lfos[0].link_d.value() {
        copy_lfo(lfos[0], lfos[3]);
    }
}

fn in_link(lfos: [&LfoParams; 4], index: usize) -> bool {
    if !lfos[0].sync.value() {
        return false;
    }
    match index {
        0 => lfos[0].link_b.value() || lfos[0].link_c.value() || lfos[0].link_d.value(),
        1 => lfos[0].link_b.value(),
        2 => lfos[0].link_c.value(),
        3 => lfos[0].link_d.value(),
        _ => false,
    }
}

fn follow_float(
    hub: &Arc<LfoSyncHub>,
    index: usize,
    knob: FloatKnob,
    param: FloatParam,
) -> FloatParam {
    let hub = Arc::clone(hub);
    param.with_callback(Arc::new(move |value| hub.on_float(index, knob, value)))
}

fn follow_bool(hub: &Arc<LfoSyncHub>, index: usize, knob: BoolKnob, param: BoolParam) -> BoolParam {
    let hub = Arc::clone(hub);
    param.with_callback(Arc::new(move |value| hub.on_bool(index, knob, value)))
}

/// 1 基分のノブ。ID は `l1_rate` のように外側で番号を付ける。
#[derive(Params)]
pub struct LfoParams {
    #[id = "wave"]
    pub wave: EnumParam<LfoWave>,
    #[id = "rate"]
    pub rate: FloatParam,
    #[id = "tempo"]
    pub tempo: BoolParam,
    #[id = "phase"]
    pub phase: FloatParam,
    #[id = "retrigger"]
    pub retrigger: BoolParam,
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
    pub hub: Arc<LfoSyncHub>,
}

impl LfoParams {
    /// `index` は 0 が A。一覧では他の Phase と区別できるよう、LFO を名前に入れる。
    pub fn new(part: char, index: usize, hub: Arc<LfoSyncHub>) -> Self {
        let name = |label: &str| format!("{part} LFO {label}");
        let percent = FloatRange::Linear {
            min: -100.0,
            max: 100.0,
        };
        let hub_wave = Arc::clone(&hub);
        let wave = EnumParam::new(name("Wave"), LfoWave::Sine)
            .with_callback(Arc::new(move |value| hub_wave.on_wave(index, value)));
        // Tempo のオンオフを、Rate の表示が読む。各 LFO が自分の印を持つ。
        let show_notes = Arc::new(AtomicBool::new(false));
        let show_for_rate = Arc::clone(&show_notes);
        let show_for_parse = Arc::clone(&show_notes);
        let rate = follow_float(
            &hub,
            index,
            FloatKnob::Rate,
            FloatParam::new(name("Rate"), 1.0, rate_range())
                .with_value_to_string(Arc::new(move |hz| {
                    rate_to_string(hz, show_for_rate.load(Ordering::Relaxed))
                }))
                .with_string_to_value(Arc::new(move |text| {
                    parse_rate(text, show_for_parse.load(Ordering::Relaxed))
                })),
        );
        let show_for_tempo = show_notes;
        let hub_tempo = Arc::clone(&hub);
        let tempo = BoolParam::new(name("Tempo"), false).with_callback(Arc::new(move |on| {
            show_for_tempo.store(on, Ordering::Relaxed);
            hub_tempo.on_bool(index, BoolKnob::Tempo, on);
        }));
        let phase = follow_float(
            &hub,
            index,
            FloatKnob::Phase,
            FloatParam::new(
                name("Phase"),
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
        );
        let retrigger = follow_bool(
            &hub,
            index,
            BoolKnob::Retrigger,
            BoolParam::new(name("Retrigger"), false),
        );
        let amount = follow_float(
            &hub,
            index,
            FloatKnob::Amount,
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
            wave,
            rate,
            tempo,
            phase,
            retrigger,
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

impl Default for LfoParams {
    fn default() -> Self {
        Self::new('A', 0, Arc::new(LfoSyncHub::new()))
    }
}

fn rate_to_string(hz: f32, notes: bool) -> String {
    if notes {
        division_name(rate_normalized(hz)).to_string()
    } else {
        let text = format!("{hz:.2}");
        let trimmed = text.trim_end_matches('0').trim_end_matches('.');
        format!("{trimmed} Hz")
    }
}

fn parse_rate(text: &str, notes: bool) -> Option<f32> {
    let trimmed = text.trim();
    if notes {
        let folded = trimmed.to_ascii_lowercase();
        let compact: String = folded.chars().filter(|ch| !ch.is_whitespace()).collect();
        for (index, name) in DIVISION_NAMES.iter().enumerate() {
            let name_folded = name.to_ascii_lowercase();
            let name_compact: String = name_folded
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect();
            if folded == name_folded || compact == name_compact {
                let center = (index as f32 + 0.5) / DIVISION_BEATS.len() as f32;
                return Some(rate_from_normalized(center));
            }
        }
    }
    let without_unit = trimmed
        .trim_end_matches(|ch: char| ch.eq_ignore_ascii_case(&'z') || ch.eq_ignore_ascii_case(&'h'))
        .trim();
    without_unit.parse().ok()
}

pub fn bind_lfo_sync(lfos: [&LfoParams; 4]) {
    lfos[0].hub.bind(lfos);
}

pub fn synced_lfo<'a>(index: usize, lfos: &'a [&'a LfoParams; 4]) -> &'a LfoParams {
    if lfos[0].links(index) {
        lfos[0]
    } else {
        lfos[index]
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

    fn shown(rate: &FloatParam, hz: f32) -> String {
        use nice_plug::prelude::Param;
        rate.normalized_value_to_string(rate.preview_normalized(hz), true)
    }

    #[test]
    fn tempo_on_shows_a_note_name_instead_of_hertz() {
        let params = LfoParams::default();
        assert_eq!(shown(&params.rate, 1.0), "1 Hz");
        set_bool(&params.tempo, true);
        assert_eq!(shown(&params.rate, 1.0), "1/4");
        let eighth = {
            use nice_plug::prelude::Param;
            let normalized = params.rate.string_to_normalized_value("1/8 T").unwrap();
            params.rate.preview_plain(normalized)
        };
        assert_eq!(division_index(rate_normalized(eighth)), 8);
    }

    #[test]
    fn default_rate_is_a_quarter_note_when_tempo_is_on() {
        assert_eq!(division_index(rate_normalized(1.0)), 4);
        let hz = lfo_hz(1.0, true, 120.0, 4.0);
        assert!((hz - 2.0).abs() < 0.01, "hz {hz}");
    }

    #[test]
    fn missing_tempo_uses_120_bpm() {
        let known = lfo_hz(1.0, true, 120.0, 4.0);
        let missing = lfo_hz(1.0, true, f32::NAN, 4.0);
        assert!((known - missing).abs() < 0.01);
    }

    #[test]
    fn free_rate_stays_in_hertz() {
        assert!((lfo_hz(5.0, false, 120.0, 4.0) - 5.0).abs() < 1.0e-4);
    }

    #[test]
    fn sine_zero_crosses_and_amount_flips_it() {
        assert!(sample_wave(LfoWave::Sine, 0.0, 0.0).abs() < 1.0e-5);
        assert!((sample_wave(LfoWave::Sine, 0.25, 0.0) - 1.0).abs() < 1.0e-5);
        assert!((apply_amount(1.0, -100.0) + 1.0).abs() < 1.0e-5);
        assert!(apply_amount(1.0, 0.0).abs() < 1.0e-5);
    }

    #[test]
    fn retrigger_starts_at_the_phase_knob() {
        let mut phase = LfoPhase::new(3);
        phase.retrigger(0.25);
        let sample = phase.advance(0.0, 48_000.0, LfoWave::Sine, 0.0);
        assert!((sample - 1.0).abs() < 1.0e-4, "sample {sample}");
    }

    #[test]
    fn sync_copies_rate_onto_b() {
        let hub = Arc::new(LfoSyncHub::new());
        let lfos = [
            LfoParams::new('A', 0, Arc::clone(&hub)),
            LfoParams::new('B', 1, Arc::clone(&hub)),
            LfoParams::new('C', 2, Arc::clone(&hub)),
            LfoParams::new('D', 3, Arc::clone(&hub)),
        ];
        bind_lfo_sync([&lfos[0], &lfos[1], &lfos[2], &lfos[3]]);
        set_float(&lfos[0].rate, 4.0);
        set_bool(&lfos[0].sync, true);
        set_bool(&lfos[0].link_b, true);
        assert!((lfos[1].rate.value() - 4.0).abs() < 0.01);
        set_bool(&lfos[1].tempo, true);
        assert!(lfos[0].tempo.value());
        set_bool(&lfos[0].sync, false);
        set_float(&lfos[0].rate, 1.0);
        assert!((lfos[1].rate.value() - 4.0).abs() < 0.01);
    }
}

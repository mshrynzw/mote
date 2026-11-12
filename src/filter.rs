//! 1 列分のフィルター。
//!
//! オシレーターの左右を受け取り、Amp Env の前へ渡す。
//! 計算は 2 極のゼロディレイ・フィードバック。Lowpass、Highpass、Bandpass、Notch を、同じ状態から読み分ける。
//! Drive はそのあとに tanh で歪ませる。この経路だけ、サンプルを 2 倍の速さで計算する。
//! Amount は鍵盤が Cutoff を追う量で、結果はノブへ書き戻さない。

use std::f32::consts::{FRAC_1_SQRT_2, PI};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;

use nice_plug::params::InternalParamMut;
use nice_plug::prelude::*;

/// ハーフバンド FIR の係数。中心が 0.5 付近で、1 つおきに 0。直流の合計は 1。
const HALF_BAND: [f32; 23] = [
    -0.002_320_098,
    0.0,
    0.005_424_059,
    0.0,
    -0.015_900_96,
    0.0,
    0.038_630_295,
    0.0,
    -0.089_455_22,
    0.0,
    0.313_069_28,
    0.501_105_3,
    0.313_069_28,
    0.0,
    -0.089_455_22,
    0.0,
    0.038_630_295,
    0.0,
    -0.015_900_96,
    0.0,
    0.005_424_059,
    0.0,
    -0.002_320_098,
];

/// 選べる型。並びを変えると、保存した音色の番号がずれる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum FilterType {
    #[id = "lowpass"]
    Lowpass,
    #[id = "highpass"]
    Highpass,
    #[id = "bandpass"]
    Bandpass,
    #[id = "notch"]
    Notch,
}

/// 2 倍オーバーサンプリング用の、長さ 23 の FIR。履歴は起動時に確保する。
struct Fir {
    hist: [f32; 23],
    pos: usize,
}

impl Fir {
    fn new() -> Self {
        Self {
            hist: [0.0; 23],
            pos: 0,
        }
    }

    fn reset(&mut self) {
        self.hist = [0.0; 23];
        self.pos = 0;
    }

    /// 1 サンプル入れて、フィルタを通した値を返す。`&mut self` は履歴を書き換えるため。
    fn tick(&mut self, sample: f32) -> f32 {
        self.hist[self.pos] = sample;
        let mut acc = 0.0;
        let mut index = self.pos;
        for coef in HALF_BAND {
            acc += coef * self.hist[index];
            index = if index == 0 {
                HALF_BAND.len() - 1
            } else {
                index - 1
            };
        }
        self.pos += 1;
        if self.pos == HALF_BAND.len() {
            self.pos = 0;
        }
        acc
    }
}

/// フィルターの係数。サンプルレートと Cutoff から、発音中に作り直す。
#[derive(Clone, Copy)]
struct Coeffs {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

fn coeffs(cutoff_hz: f32, resonance: f32, sample_rate: f32) -> Coeffs {
    let fc = cutoff_hz.clamp(20.0, sample_rate * 0.45);
    let g = (PI * fc / sample_rate).tan();
    // 0% は山のない 2 極。100% でも Q は 8 で止め、自己発振しない。
    let shaped = resonance.clamp(0.0, 1.0);
    let q = FRAC_1_SQRT_2 + shaped * shaped * (8.0 - FRAC_1_SQRT_2);
    let k = 1.0 / q;
    let a1 = 1.0 / (1.0 + g * (g + k));
    if !a1.is_finite() {
        return Coeffs {
            k: 1.0,
            a1: 0.0,
            a2: 0.0,
            a3: 0.0,
        };
    }
    let a2 = g * a1;
    let a3 = g * a2;
    Coeffs { k, a1, a2, a3 }
}

/// 積分器 2 つの状態。Lowpass などは、この 2 つと入力から読む。
struct Svf {
    ic1eq: f32,
    ic2eq: f32,
}

impl Svf {
    fn new() -> Self {
        Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    fn clear(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    fn tick(&mut self, input: f32, coeffs: Coeffs, kind: FilterType) -> f32 {
        let v3 = input - self.ic2eq;
        let v1 = coeffs.a1 * self.ic1eq + coeffs.a2 * v3;
        let v2 = self.ic2eq + coeffs.a2 * self.ic1eq + coeffs.a3 * v3;
        self.ic1eq = flush(2.0 * v1 - self.ic1eq);
        self.ic2eq = flush(2.0 * v2 - self.ic2eq);
        let mixed = match kind {
            FilterType::Lowpass => v2,
            FilterType::Bandpass => v1,
            FilterType::Highpass => input - coeffs.k * v1 - v2,
            FilterType::Notch => input - coeffs.k * v1,
        };
        if mixed.is_finite() {
            mixed
        } else {
            self.clear();
            0.0
        }
    }
}

fn flush(value: f32) -> f32 {
    if value.abs() < 1.0e-20 {
        0.0
    } else {
        value
    }
}

/// MIDI ノート 60 を基準に、Amount の割合だけ Cutoff を上下する。
pub fn tracked_cutoff(cutoff_hz: f32, note: u8, amount: f32) -> f32 {
    let octaves = (note as f32 - 60.0) / 12.0 * amount;
    cutoff_hz * 2.0f32.powf(octaves)
}

fn apply_drive(sample: f32, drive: f32) -> f32 {
    if drive <= 1.0e-5 {
        return sample;
    }
    let gain = 1.0 + drive * drive * 24.0;
    (sample * gain).tanh() / gain.tanh()
}

/// ノブが急に動いても、10 ミリ秒で目標へ着く。`set_target` は目標が変わったときだけ呼ぶ。
fn glide(smoother: &Smoother<f32>, last: &mut f32, target: f32, sample_rate: f32) -> f32 {
    if (*last - target).abs() > 1.0e-3 {
        smoother.set_target(sample_rate, target);
        *last = target;
    }
    smoother.next()
}

/// 1 列の、発音中に書き換える状態。ノブ自体はここには置かない。
pub struct Filter {
    left: Svf,
    right: Svf,
    up_left: Fir,
    up_right: Fir,
    down_left: Fir,
    down_right: Fir,
    cutoff: Smoother<f32>,
    last_cutoff: f32,
    resonance: Smoother<f32>,
    last_resonance: f32,
    drive: Smoother<f32>,
    last_drive: f32,
}

impl Filter {
    pub fn new() -> Self {
        let cutoff = Smoother::new(SmoothingStyle::Logarithmic(10.0));
        cutoff.reset(20_000.0);
        let resonance = Smoother::new(SmoothingStyle::Linear(10.0));
        resonance.reset(0.0);
        let drive = Smoother::new(SmoothingStyle::Linear(10.0));
        drive.reset(0.0);
        Self {
            left: Svf::new(),
            right: Svf::new(),
            up_left: Fir::new(),
            up_right: Fir::new(),
            down_left: Fir::new(),
            down_right: Fir::new(),
            cutoff,
            last_cutoff: 20_000.0,
            resonance,
            last_resonance: 0.0,
            drive,
            last_drive: 0.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// オフの列で呼ぶ。状態は消すが、ノブの追従はリセットしない。
    pub fn silence(&mut self) {
        self.left.clear();
        self.right.clear();
        self.up_left.reset();
        self.up_right.reset();
        self.down_left.reset();
        self.down_right.reset();
    }

    /// 左右 1 サンプル。`note` が `None` のときは、基準のノート 60 として Amount を計算する。
    pub fn next_sample(
        &mut self,
        params: &FilterParams,
        left: f32,
        right: f32,
        note: Option<u8>,
        sample_rate: f32,
    ) -> (f32, f32) {
        let sample_rate = sample_rate.max(1.0);
        let amount = (params.amount.value() / 100.0).clamp(0.0, 1.0);
        let target_cutoff = tracked_cutoff(params.cutoff.value(), note.unwrap_or(60), amount)
            .clamp(20.0, sample_rate * 0.9);
        let cutoff = glide(
            &self.cutoff,
            &mut self.last_cutoff,
            target_cutoff,
            sample_rate,
        )
        .clamp(20.0, sample_rate * 0.9);
        let resonance = glide(
            &self.resonance,
            &mut self.last_resonance,
            (params.resonance.value() / 100.0).clamp(0.0, 1.0),
            sample_rate,
        );
        let drive = glide(
            &self.drive,
            &mut self.last_drive,
            (params.drive.value() / 100.0).clamp(0.0, 1.0),
            sample_rate,
        );
        let kind = params.kind.value();
        // 2 倍の速さで計算するので、係数のサンプルレートも 2 倍。
        let oversampled = coeffs(cutoff, resonance, sample_rate * 2.0);
        let left = process_channel(
            &mut self.left,
            &mut self.up_left,
            &mut self.down_left,
            finite_or_zero(left),
            oversampled,
            kind,
            drive,
        );
        let right = process_channel(
            &mut self.right,
            &mut self.up_right,
            &mut self.down_right,
            finite_or_zero(right),
            oversampled,
            kind,
            drive,
        );
        (left, right)
    }
}

fn process_channel(
    svf: &mut Svf,
    up: &mut Fir,
    down: &mut Fir,
    sample: f32,
    coeffs: Coeffs,
    kind: FilterType,
    drive: f32,
) -> f32 {
    // 0 を 1 つ挟むとスペクトルが半分に薄まるので、FIR のあとで 2 倍して戻す。
    let first = apply_drive(svf.tick(up.tick(sample) * 2.0, coeffs, kind), drive);
    let second = apply_drive(svf.tick(up.tick(0.0) * 2.0, coeffs, kind), drive);
    let _discarded = down.tick(first);
    let output = down.tick(second);
    if output.is_finite() {
        output
    } else {
        svf.clear();
        0.0
    }
}

impl Default for Filter {
    fn default() -> Self {
        Self::new()
    }
}

fn finite_or_zero(sample: f32) -> f32 {
    if sample.is_finite() {
        sample
    } else {
        0.0
    }
}

/// コピー中は、相手のコールバックからもう一度コピーしない。
struct CopyGuard<'a>(&'a AtomicBool);

impl Drop for CopyGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// 4 基の Filter を、コピーのコールバックから指すための置き場。
pub struct FilterSyncHub {
    filters: [AtomicPtr<FilterParams>; 4],
    busy: AtomicBool,
}

impl FilterSyncHub {
    pub fn new() -> Self {
        Self {
            filters: std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut())),
            busy: AtomicBool::new(false),
        }
    }

    pub fn bind(&self, filters: [&FilterParams; 4]) {
        for (slot, filter) in self.filters.iter().zip(filters) {
            slot.store(
                filter as *const FilterParams as *mut FilterParams,
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

    fn filters(&self) -> Option<[&FilterParams; 4]> {
        let mut raw = [std::ptr::null::<FilterParams>(); 4];
        for (slot, ptr) in raw.iter_mut().zip(self.filters.iter()) {
            let loaded = ptr.load(Ordering::Acquire);
            if loaded.is_null() {
                return None;
            }
            *slot = loaded;
        }
        // 住所は `bind` で渡した 4 基。共有参照だけで読む。
        Some(unsafe { raw.map(|ptr| &*ptr) })
    }

    fn on_float(&self, index: usize, knob: FloatKnob, value: f32) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(filters) = self.filters() else {
            return;
        };
        if !in_link(filters, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(filters, other) {
                continue;
            }
            set_float(filters[other], knob, value);
        }
    }

    fn on_type(&self, index: usize, value: FilterType) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(filters) = self.filters() else {
            return;
        };
        if !in_link(filters, index) {
            return;
        }
        for other in 0..4 {
            if other == index || !in_link(filters, other) {
                continue;
            }
            set_type(filters[other], value);
        }
    }

    fn on_master(&self, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(filters) = self.filters() else {
            return;
        };
        mirror_selected(filters);
    }

    fn on_target(&self, target: usize, _enabled: bool) {
        let Some(_guard) = self.enter() else {
            return;
        };
        let Some(filters) = self.filters() else {
            return;
        };
        if !filters[0].sync.value() {
            return;
        }
        copy_filter(filters[0], filters[target]);
    }
}

impl Default for FilterSyncHub {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
enum FloatKnob {
    Amount,
    Drive,
    Cutoff,
    Resonance,
}

fn float_param(filter: &FilterParams, knob: FloatKnob) -> &FloatParam {
    match knob {
        FloatKnob::Amount => &filter.amount,
        FloatKnob::Drive => &filter.drive,
        FloatKnob::Cutoff => &filter.cutoff,
        FloatKnob::Resonance => &filter.resonance,
    }
}

fn set_float(filter: &FilterParams, knob: FloatKnob, value: f32) {
    unsafe {
        let _changed =
            InternalParamMut::_internal_set_plain_value(float_param(filter, knob), value);
    }
}

fn set_type(filter: &FilterParams, value: FilterType) {
    unsafe {
        let _changed = InternalParamMut::_internal_set_plain_value(&filter.kind, value);
    }
}

fn copy_filter(from: &FilterParams, to: &FilterParams) {
    set_type(to, from.kind.value());
    for knob in [
        FloatKnob::Amount,
        FloatKnob::Drive,
        FloatKnob::Cutoff,
        FloatKnob::Resonance,
    ] {
        set_float(to, knob, float_param(from, knob).unmodulated_plain_value());
    }
}

fn mirror_selected(filters: [&FilterParams; 4]) {
    if filters[0].link_b.value() {
        copy_filter(filters[0], filters[1]);
    }
    if filters[0].link_c.value() {
        copy_filter(filters[0], filters[2]);
    }
    if filters[0].link_d.value() {
        copy_filter(filters[0], filters[3]);
    }
}

fn in_link(filters: [&FilterParams; 4], index: usize) -> bool {
    if !filters[0].sync.value() {
        return false;
    }
    match index {
        0 => filters[0].link_b.value() || filters[0].link_c.value() || filters[0].link_d.value(),
        1 => filters[0].link_b.value(),
        2 => filters[0].link_c.value(),
        3 => filters[0].link_d.value(),
        _ => false,
    }
}

fn follow_float(
    hub: &Arc<FilterSyncHub>,
    index: usize,
    knob: FloatKnob,
    param: FloatParam,
) -> FloatParam {
    let hub = Arc::clone(hub);
    param.with_callback(Arc::new(move |value| hub.on_float(index, knob, value)))
}

/// 1 基分の Filter ノブ。ID は `f1_cutoff` のように外側で番号を付ける。
#[derive(Params)]
pub struct FilterParams {
    #[id = "type"]
    pub kind: EnumParam<FilterType>,
    #[id = "amount"]
    pub amount: FloatParam,
    #[id = "drive"]
    pub drive: FloatParam,
    #[id = "cutoff"]
    pub cutoff: FloatParam,
    #[id = "resonance"]
    pub resonance: FloatParam,
    #[id = "sync"]
    pub sync: BoolParam,
    #[id = "link_b"]
    pub link_b: BoolParam,
    #[id = "link_c"]
    pub link_c: BoolParam,
    #[id = "link_d"]
    pub link_d: BoolParam,
    /// パラメータ一覧には出さない。4 基をコピーでつなぐ。
    pub hub: Arc<FilterSyncHub>,
}

impl FilterParams {
    /// `index` は 0 が A、1 が B、2 が C、3 が D。Sync のスイッチが見えるのは A だけ。
    pub fn new(part: char, index: usize, hub: Arc<FilterSyncHub>) -> Self {
        let name = |label: &str| format!("{part} {label}");
        let percent = FloatRange::Linear {
            min: 0.0,
            max: 100.0,
        };
        // 低い Cutoff の方が、ノブの同じ角度で細かく動く。
        let cutoff_range = FloatRange::Skewed {
            min: 20.0,
            max: 20_000.0,
            factor: FloatRange::skew_factor(-2.0),
        };
        let hub_type = Arc::clone(&hub);
        let kind = EnumParam::new(name("Type"), FilterType::Lowpass)
            .with_callback(Arc::new(move |value| hub_type.on_type(index, value)));
        let amount = follow_float(
            &hub,
            index,
            FloatKnob::Amount,
            FloatParam::new(name("Amount"), 0.0, percent).with_unit(" %"),
        );
        let drive = follow_float(
            &hub,
            index,
            FloatKnob::Drive,
            FloatParam::new(name("Drive"), 0.0, percent).with_unit(" %"),
        );
        let cutoff = follow_float(
            &hub,
            index,
            FloatKnob::Cutoff,
            FloatParam::new(name("Cutoff"), 20_000.0, cutoff_range).with_unit(" Hz"),
        );
        let resonance = follow_float(
            &hub,
            index,
            FloatKnob::Resonance,
            FloatParam::new(name("Resonance"), 0.0, percent).with_unit(" %"),
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
            kind,
            amount,
            drive,
            cutoff,
            resonance,
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

impl Default for FilterParams {
    fn default() -> Self {
        Self::new('A', 0, Arc::new(FilterSyncHub::new()))
    }
}

pub fn bind_filter_sync(filters: [&FilterParams; 4]) {
    filters[0].hub.bind(filters);
}

/// 揃えている列は A のノブを読む。外れている列は自分のノブを読む。
pub fn synced_filter<'a>(index: usize, filters: &'a [&'a FilterParams; 4]) -> &'a FilterParams {
    if filters[0].links(index) {
        filters[0]
    } else {
        filters[index]
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

    fn set_kind(param: &EnumParam<FilterType>, value: FilterType) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, value);
        }
    }

    fn peak_of_sine(freq: f32, params: &FilterParams, note: u8) -> f32 {
        let mut filter = Filter::new();
        let mut phase = 0.0;
        let step = freq / 48_000.0;
        let mut peak: f32 = 0.0;
        for i in 0..4_096 {
            let sample = (phase * std::f32::consts::TAU).sin();
            phase = (phase + step) % 1.0;
            let (left, right) = filter.next_sample(params, sample, sample, Some(note), 48_000.0);
            assert!(left.is_finite() && right.is_finite());
            if i > 3_000 {
                peak = peak.max(left.abs());
            }
        }
        peak
    }

    #[test]
    fn keytrack_doubles_cutoff_each_octave() {
        let full = tracked_cutoff(1_000.0, 72, 1.0);
        let half = tracked_cutoff(1_000.0, 72, 0.5);
        let still = tracked_cutoff(1_000.0, 72, 0.0);
        let down = tracked_cutoff(1_000.0, 48, 1.0);
        assert!((full - 2_000.0).abs() < 0.01);
        assert!((half - 1_414.213_5).abs() < 0.1);
        assert!((still - 1_000.0).abs() < 0.01);
        assert!((down - 500.0).abs() < 0.01);
        assert!((tracked_cutoff(1_000.0, 60, 1.0) - 1_000.0).abs() < 0.01);
    }

    #[test]
    fn default_lowpass_keeps_a_constant_level() {
        let params = FilterParams::default();
        let mut filter = Filter::new();
        let mut last_left = 0.0;
        let mut last_right = 0.0;
        for _ in 0..1_000 {
            let (left, right) = filter.next_sample(&params, 0.5, -0.25, Some(60), 48_000.0);
            assert!(left.is_finite() && right.is_finite());
            last_left = left;
            last_right = right;
        }
        assert!(
            (last_left - 0.5).abs() < 0.02,
            "left settled at {last_left}"
        );
        assert!(
            (last_right + 0.25).abs() < 0.02,
            "right settled at {last_right}"
        );
    }

    #[test]
    fn lowpass_passes_a_low_sine_and_cuts_a_high_one() {
        let params = FilterParams::default();
        set_float(&params.cutoff, 1_000.0);
        let low = peak_of_sine(200.0, &params, 60);
        let high = peak_of_sine(8_000.0, &params, 60);
        assert!(low > 0.5, "low peak {low}");
        assert!(high < 0.15, "high peak {high}");
    }

    #[test]
    fn highpass_cuts_a_low_sine() {
        let params = FilterParams::default();
        set_float(&params.cutoff, 2_000.0);
        set_kind(&params.kind, FilterType::Highpass);
        let low = peak_of_sine(80.0, &params, 60);
        let high = peak_of_sine(8_000.0, &params, 60);
        assert!(low < 0.15, "low peak {low}");
        assert!(high > 0.4, "high peak {high}");
    }

    #[test]
    fn bandpass_prefers_the_cutoff() {
        let params = FilterParams::default();
        set_float(&params.cutoff, 1_000.0);
        set_kind(&params.kind, FilterType::Bandpass);
        let at = peak_of_sine(1_000.0, &params, 60);
        let below = peak_of_sine(100.0, &params, 60);
        assert!(at > below * 3.0, "at {at}, below {below}");
    }

    #[test]
    fn notch_dips_at_the_cutoff() {
        let params = FilterParams::default();
        set_float(&params.cutoff, 1_000.0);
        set_kind(&params.kind, FilterType::Notch);
        let at = peak_of_sine(1_000.0, &params, 60);
        let away = peak_of_sine(200.0, &params, 60);
        assert!(at < away * 0.5, "at {at}, away {away}");
    }

    #[test]
    fn resonance_raises_the_peak_and_stays_finite() {
        let plain = FilterParams::default();
        set_float(&plain.cutoff, 1_000.0);
        let peaked = FilterParams::default();
        set_float(&peaked.cutoff, 1_000.0);
        set_float(&peaked.resonance, 100.0);
        let quiet = peak_of_sine(1_000.0, &plain, 60);
        let loud = peak_of_sine(1_000.0, &peaked, 60);
        assert!(loud > quiet * 2.0, "quiet {quiet}, loud {loud}");
        assert!(loud < 20.0, "loud {loud}");
    }

    #[test]
    fn drive_changes_a_quiet_sine_and_stays_within_bounds() {
        let clean = FilterParams::default();
        let driven = FilterParams::default();
        set_float(&driven.drive, 100.0);
        let mut plain = Filter::new();
        let mut hot = Filter::new();
        let mut differed = false;
        for i in 0..2_000 {
            let sample = 0.4 * (i as f32 * 440.0 / 48_000.0 * std::f32::consts::TAU).sin();
            let (left, _) = plain.next_sample(&clean, sample, sample, Some(60), 48_000.0);
            let (driven_left, _) = hot.next_sample(&driven, sample, sample, Some(60), 48_000.0);
            assert!(driven_left.is_finite());
            assert!(driven_left.abs() <= 1.01);
            if i > 1_000 && (left - driven_left).abs() > 0.05 {
                differed = true;
            }
        }
        assert!(differed);
    }

    #[test]
    fn keytrack_opens_the_filter_on_a_higher_note() {
        let params = FilterParams::default();
        set_float(&params.cutoff, 1_000.0);
        set_float(&params.amount, 100.0);
        let low_note = peak_of_sine(1_500.0, &params, 60);
        let high_note = peak_of_sine(1_500.0, &params, 72);
        assert!(
            high_note > low_note * 1.5,
            "low {low_note}, high {high_note}"
        );
    }

    #[test]
    fn sync_copies_the_chosen_column() {
        let hub = Arc::new(FilterSyncHub::new());
        let filters = [
            FilterParams::new('A', 0, Arc::clone(&hub)),
            FilterParams::new('B', 1, Arc::clone(&hub)),
            FilterParams::new('C', 2, Arc::clone(&hub)),
            FilterParams::new('D', 3, Arc::clone(&hub)),
        ];
        bind_filter_sync([&filters[0], &filters[1], &filters[2], &filters[3]]);
        set_float(&filters[0].cutoff, 1_234.0);
        set_bool(&filters[0].sync, true);
        set_bool(&filters[0].link_b, true);
        assert!((filters[1].cutoff.value() - 1_234.0).abs() < 0.1);
        set_float(&filters[1].drive, 40.0);
        assert!((filters[0].drive.value() - 40.0).abs() < 0.1);
        set_kind(&filters[0].kind, FilterType::Highpass);
        set_bool(&filters[0].link_c, true);
        assert_eq!(filters[2].kind.value(), FilterType::Highpass);
        assert!((filters[2].cutoff.value() - 1_234.0).abs() < 0.1);
        set_bool(&filters[0].sync, false);
        set_float(&filters[0].cutoff, 2_000.0);
        assert!((filters[1].cutoff.value() - 1_234.0).abs() < 0.1);
    }
}

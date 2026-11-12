//! WRAC のオーディオスレッドから使う音の入口。
//!
//! ノブは `MoteParams` の原子的な値を画面スレッドが書き、発音中は `Voices` が読む。
//! パラメータ名の検索は画面スレッドだけが行う。

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use nice_plug::prelude::*;

use crate::params::MoteParams;
use crate::voice::Voices;

/// 画面のキー入力を、オーディオスレッドがロックせずに読むための枠。
const NOTE_CAPACITY: usize = 64;

/// 画面と音が共有するノブ。`Voices` 自体はオーディオスレッドだけが持つ。
pub struct SharedEngine {
    params: Arc<MoteParams>,
    sample_rate: AtomicU32,
    notes: NoteRing,
}

impl SharedEngine {
    pub fn new() -> Arc<Self> {
        let params = Arc::new(MoteParams::default());
        params.bind_amp_sync();
        params.bind_filter_sync();
        params.bind_filter_env_sync();
        params.bind_mod_env_sync();
        params.bind_lfo_sync();
        let shared = Arc::new(Self {
            params,
            sample_rate: AtomicU32::new(48_000.0_f32.to_bits()),
            notes: NoteRing::new(),
        });
        shared.prime_smoothers(true);
        shared
    }

    pub fn sample_rate(&self) -> f32 {
        f32::from_bits(self.sample_rate.load(Ordering::Relaxed))
    }

    pub fn set_sample_rate(&self, sample_rate: f32) {
        self.sample_rate
            .store(sample_rate.to_bits(), Ordering::Relaxed);
        self.prime_smoothers(true);
    }

    /// `id` は `o1_volume` のような、nice-plug が公開している名前。値は 0 から 1。
    pub fn set_normalized(&self, id: &str, normalized: f32) -> bool {
        let normalized = normalized.clamp(0.0, 1.0);
        let map = self.params.param_map();
        let Some((_, ptr, _)) = map.iter().find(|(name, _, _)| name == id) else {
            return false;
        };
        let sample_rate = self.sample_rate();
        unsafe {
            ptr._internal_set_normalized_value(normalized);
            ptr._internal_update_smoother(sample_rate, false);
        }
        true
    }

    /// 画面スレッドがノートオンを積む。枠が満杯なら捨てる。
    pub fn push_note_on(&self, key: u8, velocity: f32) -> bool {
        self.notes.push(pack_note(true, key, velocity))
    }

    /// 画面スレッドがノートオフを積む。枠が満杯なら捨てる。
    pub fn push_note_off(&self, key: u8) -> bool {
        self.notes.push(pack_note(false, key, 0.0))
    }

    fn pop_note(&self) -> Option<(bool, u8, f32)> {
        self.notes.pop().map(unpack_note)
    }

    fn prime_smoothers(&self, reset: bool) {
        let sample_rate = self.sample_rate();
        for (_, ptr, _) in self.params.param_map() {
            unsafe {
                ptr._internal_update_smoother(sample_rate, reset);
            }
        }
    }
}

/// 発音中の状態。オーディオスレッドだけが触る。
pub struct Engine {
    shared: Arc<SharedEngine>,
    voices: Voices,
}

impl Engine {
    pub fn activate(shared: Arc<SharedEngine>, sample_rate: f32) -> Self {
        shared.set_sample_rate(sample_rate);
        let mut voices = Voices::new();
        voices.set_sample_rate(sample_rate);
        Self { shared, voices }
    }

    /// 画面から積まれたノートを、このブロックの頭で発音へ渡す。
    pub fn drain_gui_notes(&mut self) {
        while let Some((on, key, velocity)) = self.shared.pop_note() {
            if on {
                self.note_on(key, velocity);
            } else {
                self.note_off(Some(key));
            }
        }
    }

    pub fn note_on(&mut self, key: u8, velocity: f32) {
        let oscs = self.shared.params.oscillators();
        let lfos = self.shared.params.lfos();
        self.voices
            .note_on(key, velocity, &oscs, &lfos, &self.shared.params.master);
    }

    pub fn note_off(&mut self, key: Option<u8>) {
        let oscs = self.shared.params.oscillators();
        let lfos = self.shared.params.lfos();
        self.voices
            .note_off(key, &self.shared.params.master, &oscs, &lfos);
    }

    pub fn set_bend(&mut self, value: f32) {
        self.voices.set_bend(value);
    }

    pub fn set_transport(&mut self, tempo: Option<f64>, numerator: Option<i32>) {
        self.voices.set_transport(tempo, numerator);
    }

    pub fn next_sample(&mut self) -> (f32, f32) {
        let oscs = self.shared.params.oscillators();
        let filters = self.shared.params.filters();
        let filter_envs = self.shared.params.filter_envelopes();
        let mod_envs = self.shared.params.mod_envelopes();
        let lfos = self.shared.params.lfos();
        let amps = self.shared.params.amp_envelopes();
        let chain = crate::voice::ChainParams {
            oscs: &oscs,
            filters: &filters,
            filter_envs: &filter_envs,
            mod_envs: &mod_envs,
            lfos: &lfos,
            amps: &amps,
        };
        self.voices
            .next_sample(&chain, &self.shared.params.master)
    }

    pub fn is_sounding(&self) -> bool {
        self.voices.is_sounding()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_on_makes_a_tone() {
        let shared = SharedEngine::new();
        let mut engine = Engine::activate(shared, 48_000.0);
        engine.note_on(60, 0.8);
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            let (left, right) = engine.next_sample();
            peak = peak.max(left.abs()).max(right.abs());
        }
        assert!(peak > 0.01, "peak was {peak}");
    }

    #[test]
    fn known_parameter_id_is_accepted() {
        let shared = SharedEngine::new();
        assert!(shared.set_normalized("o1_volume", 0.2));
        assert!(shared.set_normalized("f1_type", 1.0));
        assert!(!shared.set_normalized("f1_kind", 0.0));
        assert!(!shared.set_normalized("not_a_param", 0.5));
    }

    #[test]
    fn queued_note_on_makes_a_tone() {
        let shared = SharedEngine::new();
        assert!(shared.push_note_on(60, 0.8));
        let mut engine = Engine::activate(shared, 48_000.0);
        engine.drain_gui_notes();
        let mut peak = 0.0_f32;
        for _ in 0..4_800 {
            let (left, right) = engine.next_sample();
            peak = peak.max(left.abs()).max(right.abs());
        }
        assert!(peak > 0.01, "peak was {peak}");
    }
}

/// 画面スレッドが書き、オーディオスレッドが読む。起動時に枠だけ確保する。
struct NoteRing {
    slots: Box<[AtomicU64; NOTE_CAPACITY]>,
    write: AtomicUsize,
    read: AtomicUsize,
}

impl NoteRing {
    fn new() -> Self {
        Self {
            slots: Box::new(std::array::from_fn(|_| AtomicU64::new(0))),
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
        }
    }

    fn push(&self, packed: u64) -> bool {
        let write = self.write.load(Ordering::Relaxed);
        let read = self.read.load(Ordering::Acquire);
        if write.wrapping_sub(read) >= NOTE_CAPACITY {
            return false;
        }
        self.slots[write % NOTE_CAPACITY].store(packed, Ordering::Release);
        self.write.store(write.wrapping_add(1), Ordering::Release);
        true
    }

    fn pop(&self) -> Option<u64> {
        let read = self.read.load(Ordering::Relaxed);
        let write = self.write.load(Ordering::Acquire);
        if read == write {
            return None;
        }
        let packed = self.slots[read % NOTE_CAPACITY].load(Ordering::Acquire);
        self.read.store(read.wrapping_add(1), Ordering::Release);
        Some(packed)
    }
}

fn pack_note(on: bool, key: u8, velocity: f32) -> u64 {
    let velocity = (velocity.clamp(0.0, 1.0) * 65535.0).round() as u16;
    u64::from(key) | (u64::from(u8::from(on)) << 8) | (u64::from(velocity) << 16)
}

fn unpack_note(packed: u64) -> (bool, u8, f32) {
    let key = (packed & 0xFF) as u8;
    let on = ((packed >> 8) & 1) == 1;
    let velocity = ((packed >> 16) & 0xFFFF) as f32 / 65535.0;
    (on, key, velocity)
}

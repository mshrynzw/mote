//! MIDI ノートを最大 16 まで持つ。
//!
//! Polyphony はそのうち同時に押せる数。Mono は 1 ノートだけ。
//! オンのオシレーターは、Unison を混ぜてから自分のフィルターと Amp Env を通る。
//! 鳴っていないノートと、オフのオシレーターは計算しない。

use crate::envelope::{synced_amp, AmpEnvelope, AmpParams};
use crate::filter::{synced_filter, Filter, FilterParams};
use crate::filter_env::{synced_filter_env, FilterEnv, FilterEnvParams};
use crate::master::{bend_semitones, note_limit, MasterParams, PlayMode};
use crate::oscillator::{OscParams, Oscillator};
use crate::wavetable::WaveBank;
use nice_plug::prelude::*;

use crate::master::MAX_NOTES;

/// 1 つの MIDI ノート。オシレーター 4 基分の状態を、起動時に持っておく。
pub struct NoteVoice {
    oscillators: [Oscillator; 4],
    filters: [Filter; 4],
    filter_envs: [FilterEnv; 4],
    amps: [AmpEnvelope; 4],
    sample_rate: f32,
    /// このスロットのキー。Release が終わると `None`。
    key: Option<u8>,
    /// キーを押しているあいだ true。離すと Release に入る。
    held: bool,
    /// 新しいノートほど大きい。古いノートを選ぶときに使う。
    age: u64,
    /// いま聞こえている MIDI ノート。ポルタメント中は小数になる。
    glide_note: f32,
    glide_target: f32,
    glide_left: u32,
    glide_step: f32,
}

impl NoteVoice {
    pub fn new() -> Self {
        Self {
            oscillators: std::array::from_fn(|_| Oscillator::new()),
            filters: std::array::from_fn(|_| Filter::new()),
            filter_envs: std::array::from_fn(|_| FilterEnv::new()),
            amps: std::array::from_fn(|_| AmpEnvelope::new()),
            sample_rate: 48_000.0,
            key: None,
            held: false,
            age: 0,
            glide_note: 60.0,
            glide_target: 60.0,
            glide_left: 0,
            glide_step: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn reset(&mut self) {
        let sample_rate = self.sample_rate;
        for oscillator in &mut self.oscillators {
            oscillator.reset();
        }
        for filter in &mut self.filters {
            filter.reset();
        }
        for filter_env in &mut self.filter_envs {
            filter_env.reset();
        }
        for amp in &mut self.amps {
            amp.reset();
        }
        self.sample_rate = sample_rate;
        self.key = None;
        self.held = false;
        self.age = 0;
        self.glide_note = 60.0;
        self.glide_target = 60.0;
        self.glide_left = 0;
        self.glide_step = 0.0;
    }

    pub fn is_sounding(&self) -> bool {
        self.held || self.amps.iter().any(|amp| amp.is_busy())
    }

    /// エンベロープの高さの合計。小さい Release を再利用するときに使う。
    pub fn amp_level(&self) -> f32 {
        self.amps.iter().map(|amp| amp.level()).sum()
    }

    /// `from` が `Some` のときだけ、その音程から Portamento の時間で移る。
    fn assign(
        &mut self,
        key: u8,
        age: u64,
        start_envelopes: bool,
        from: Option<f32>,
        portamento_seconds: f32,
        params: &[&OscParams; 4],
    ) {
        self.key = Some(key);
        self.held = true;
        self.age = age;
        if let Some(pitch) = from {
            self.glide_note = pitch;
        }
        self.begin_glide(f32::from(key), portamento_seconds, from.is_some());
        for (index, osc_params) in params.iter().enumerate() {
            if osc_params.on.value() {
                self.oscillators[index].note_on(osc_params);
                if start_envelopes {
                    self.filter_envs[index].start();
                    self.amps[index].start();
                }
            } else {
                self.filter_envs[index].silence();
                self.amps[index].reset();
            }
        }
    }

    fn begin_glide(&mut self, target: f32, seconds: f32, fingered: bool) {
        self.glide_target = target;
        let steps = if fingered && seconds > 0.0 {
            (seconds * self.sample_rate).round().max(1.0) as u32
        } else {
            0
        };
        if steps == 0 {
            self.glide_note = target;
            self.glide_left = 0;
            self.glide_step = 0.0;
            return;
        }
        self.glide_step = (target - self.glide_note) / steps as f32;
        self.glide_left = steps;
    }

    /// `None` は、このスロットを問わず止める。
    pub fn note_off(&mut self, key: Option<u8>) {
        let stop = match (self.key, key) {
            (Some(_), None) => self.held,
            (Some(active), Some(released)) => self.held && active == released,
            _ => false,
        };
        if stop {
            self.release_key();
        }
    }

    fn release_key(&mut self) {
        if !self.held {
            return;
        }
        self.held = false;
        for filter_env in &mut self.filter_envs {
            filter_env.release();
        }
        for amp in &mut self.amps {
            amp.release();
        }
    }

    /// 左右 1 サンプル。`pitch_semitones` は Transpose とベンドの合計。
    pub fn next_sample(
        &mut self,
        oscs: &[&OscParams; 4],
        filters: &[&FilterParams; 4],
        filter_envs: &[&FilterEnvParams; 4],
        amps: &[&AmpParams; 4],
        pitch_semitones: f32,
        bank: &WaveBank,
    ) -> (f32, f32) {
        if !self.is_sounding() {
            return (0.0, 0.0);
        }
        let sounding = self.glide_note + pitch_semitones;
        let mut left = 0.0;
        let mut right = 0.0;
        let mut active = 0u32;
        for (index, osc_params) in oscs.iter().enumerate() {
            if !osc_params.on.value() {
                self.filters[index].silence();
                self.filter_envs[index].silence();
                self.amps[index].reset();
                continue;
            }
            active += 1;
            let (osc_left, osc_right) = self.oscillators[index].next_sample(
                osc_params,
                bank,
                Some(sounding),
                self.sample_rate,
            );
            let env_params = synced_filter_env(index, filter_envs);
            let env_octaves = self.filter_envs[index].next_octaves(env_params, self.sample_rate);
            let (filtered_left, filtered_right) = self.filters[index].next_sample(
                synced_filter(index, filters),
                osc_left,
                osc_right,
                Some(sounding),
                self.sample_rate,
                env_octaves,
            );
            let env = self.amps[index].next(synced_amp(index, amps).settings(), self.sample_rate);
            left += filtered_left * env;
            right += filtered_right * env;
        }
        self.advance_glide();
        if !self.held && self.amps.iter().all(|amp| !amp.is_busy()) {
            self.key = None;
        }
        if active == 0 {
            return (0.0, 0.0);
        }
        let scale = 1.0 / active as f32;
        (left * scale, right * scale)
    }

    fn advance_glide(&mut self) {
        if self.glide_left == 0 {
            self.glide_note = self.glide_target;
            return;
        }
        self.glide_note += self.glide_step;
        self.glide_left -= 1;
        if self.glide_left == 0 {
            self.glide_note = self.glide_target;
        }
    }
}

impl Default for NoteVoice {
    fn default() -> Self {
        Self::new()
    }
}

/// 押しているキーを、押した順に覚える。Mono で前のキーへ戻るために使う。
struct KeyStack {
    keys: [u8; 128],
    len: usize,
}

impl KeyStack {
    fn new() -> Self {
        Self {
            keys: [0; 128],
            len: 0,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn top(&self) -> Option<u8> {
        if self.len == 0 {
            None
        } else {
            Some(self.keys[self.len - 1])
        }
    }

    /// 同じキーが既にあれば一度外して、いちばん新しい位置へ置く。
    fn push(&mut self, key: u8) {
        self.remove(key);
        if self.len < self.keys.len() {
            self.keys[self.len] = key;
            self.len += 1;
        }
    }

    fn remove(&mut self, key: u8) -> bool {
        let Some(found) = self.keys[..self.len].iter().position(|item| *item == key) else {
            return false;
        };
        for index in found..self.len - 1 {
            self.keys[index] = self.keys[index + 1];
        }
        self.len -= 1;
        true
    }
}

/// 16 ノート分。波形表は 1 つを共有する。
pub struct Voices {
    slots: [NoteVoice; MAX_NOTES],
    stack: KeyStack,
    bank: WaveBank,
    sample_rate: f32,
    next_age: u64,
    bend: f32,
    last_mode: PlayMode,
    volume: Smoother<f32>,
    last_volume: f32,
    transpose: Smoother<f32>,
    last_transpose: f32,
}

impl Voices {
    pub fn new() -> Self {
        let volume = Smoother::new(SmoothingStyle::Linear(10.0));
        volume.reset(0.0);
        let transpose = Smoother::new(SmoothingStyle::Linear(10.0));
        transpose.reset(0.0);
        Self {
            slots: std::array::from_fn(|_| NoteVoice::new()),
            stack: KeyStack::new(),
            bank: WaveBank::new(48_000.0),
            sample_rate: 48_000.0,
            next_age: 1,
            bend: 0.5,
            last_mode: PlayMode::Poly,
            volume,
            last_volume: 0.0,
            transpose,
            last_transpose: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.bank = WaveBank::new(sample_rate);
        for slot in &mut self.slots {
            slot.set_sample_rate(sample_rate);
        }
    }

    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            slot.reset();
        }
        self.stack.clear();
        self.next_age = 1;
        self.bend = 0.5;
        self.last_mode = PlayMode::Poly;
        self.volume.reset(0.0);
        self.last_volume = 0.0;
        self.transpose.reset(0.0);
        self.last_transpose = 0.0;
    }

    pub fn set_bend(&mut self, value: f32) {
        self.bend = value.clamp(0.0, 1.0);
    }

    pub fn is_sounding(&self) -> bool {
        self.slots.iter().any(|slot| slot.is_sounding())
    }

    pub fn note_on(
        &mut self,
        key: u8,
        velocity: f32,
        oscs: &[&OscParams; 4],
        master: &MasterParams,
    ) {
        if velocity <= 0.0 {
            self.note_off(Some(key), master, oscs);
            return;
        }
        let mode = master.mode.value();
        let legato = master.legato.value();
        let portamento = master.portamento.value().max(0.0);
        let limit = note_limit(mode, master.polyphony.value());
        let fingered = self.slots.iter().any(|slot| slot.held) || !self.stack.is_empty();
        let previous = self.newest_pitch();
        let age = self.next_age;
        self.next_age = self.next_age.wrapping_add(1);
        match mode {
            PlayMode::Poly => {
                let index = self.slot_for(key, limit);
                let from = if fingered { previous } else { None };
                self.slots[index].assign(key, age, true, from, portamento, oscs);
            }
            PlayMode::Mono => {
                self.stack.push(key);
                let index = self.mono_slot();
                let start_envelopes = !fingered || !legato;
                let from = if fingered {
                    previous.or(Some(self.slots[index].glide_note))
                } else {
                    None
                };
                self.slots[index].assign(key, age, start_envelopes, from, portamento, oscs);
                self.release_except(index);
            }
        }
    }

    pub fn note_off(&mut self, key: Option<u8>, master: &MasterParams, oscs: &[&OscParams; 4]) {
        let Some(key) = key else {
            self.stack.clear();
            for slot in &mut self.slots {
                slot.note_off(None);
            }
            return;
        };
        if master.mode.value() == PlayMode::Mono {
            let was_top = self.stack.top() == Some(key);
            self.stack.remove(key);
            if let Some(previous) = self.stack.top() {
                if was_top {
                    let index = self.mono_slot();
                    let start_envelopes = !master.legato.value();
                    let from = Some(self.slots[index].glide_note);
                    let age = self.next_age;
                    self.next_age = self.next_age.wrapping_add(1);
                    self.slots[index].assign(
                        previous,
                        age,
                        start_envelopes,
                        from,
                        master.portamento.value().max(0.0),
                        oscs,
                    );
                }
            } else if let Some(index) = self.held_index(key) {
                self.slots[index].release_key();
            }
            return;
        }
        if let Some(index) = self.held_index(key) {
            self.slots[index].release_key();
        }
    }

    /// 左右 1 サンプル。Master Volume は、鳴っているノートを足したあとで掛ける。
    pub fn next_sample(
        &mut self,
        oscs: &[&OscParams; 4],
        filters: &[&FilterParams; 4],
        filter_envs: &[&FilterEnvParams; 4],
        amps: &[&AmpParams; 4],
        master: &MasterParams,
    ) -> (f32, f32) {
        self.apply_mode(master);
        self.trim_polyphony(note_limit(master.mode.value(), master.polyphony.value()));
        let transpose_target = master.transpose.value() as f32;
        if (transpose_target - self.last_transpose).abs() > 0.01 {
            self.transpose
                .set_target(self.sample_rate, transpose_target);
            self.last_transpose = transpose_target;
        }
        let volume_target = master.volume.value();
        if (volume_target - self.last_volume).abs() > 0.01 {
            self.volume.set_target(self.sample_rate, volume_target);
            self.last_volume = volume_target;
        }
        let pitch = self.transpose.next() + bend_semitones(self.bend, master.bend.value() as f32);
        let mut left = 0.0;
        let mut right = 0.0;
        for slot in &mut self.slots {
            let (slot_left, slot_right) =
                slot.next_sample(oscs, filters, filter_envs, amps, pitch, &self.bank);
            left += slot_left;
            right += slot_right;
        }
        let gain = nice_plug::util::db_to_gain(self.volume.next());
        let left = left * gain;
        let right = right * gain;
        (
            if left.is_finite() { left } else { 0.0 },
            if right.is_finite() { right } else { 0.0 },
        )
    }

    fn apply_mode(&mut self, master: &MasterParams) {
        let mode = master.mode.value();
        if mode == self.last_mode {
            return;
        }
        self.last_mode = mode;
        if mode == PlayMode::Mono {
            self.trim_polyphony(1);
            self.stack.clear();
            if let Some(index) = self.newest_held() {
                if let Some(key) = self.slots[index].key {
                    self.stack.push(key);
                }
            }
        } else {
            self.stack.clear();
        }
    }

    fn trim_polyphony(&mut self, limit: usize) {
        while self.slots.iter().filter(|slot| slot.held).count() > limit {
            let Some(index) = self.oldest_held() else {
                break;
            };
            self.slots[index].release_key();
        }
    }

    fn slot_for(&mut self, key: u8, limit: usize) -> usize {
        if let Some(index) = self.held_index(key) {
            return index;
        }
        let held = self.slots.iter().filter(|slot| slot.held).count();
        if held >= limit {
            return self.oldest_held().unwrap_or(0);
        }
        if let Some(index) = self.slots.iter().position(|slot| !slot.is_sounding()) {
            return index;
        }
        self.quietest_releasing().unwrap_or(0)
    }

    fn mono_slot(&self) -> usize {
        self.slots
            .iter()
            .position(|slot| slot.held)
            .or_else(|| self.slots.iter().position(|slot| slot.is_sounding()))
            .unwrap_or(0)
    }

    fn held_index(&self, key: u8) -> Option<usize> {
        self.slots
            .iter()
            .position(|slot| slot.held && slot.key == Some(key))
    }

    fn oldest_held(&self) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.held)
            .min_by_key(|(_, slot)| slot.age)
            .map(|(index, _)| index)
    }

    fn newest_held(&self) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.held)
            .max_by_key(|(_, slot)| slot.age)
            .map(|(index, _)| index)
    }

    fn newest_pitch(&self) -> Option<f32> {
        self.slots
            .iter()
            .filter(|slot| slot.held || slot.is_sounding())
            .max_by_key(|slot| slot.age)
            .map(|slot| slot.glide_note)
    }

    fn quietest_releasing(&self) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| !slot.held && slot.is_sounding())
            .min_by(|(_, left), (_, right)| {
                left.amp_level()
                    .partial_cmp(&right.amp_level())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index)
    }

    fn release_except(&mut self, keep: usize) {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if index != keep {
                slot.release_key();
            }
        }
    }
}

impl Default for Voices {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::MoteParams;
    use nice_plug::params::InternalParamMut;

    fn params() -> MoteParams {
        MoteParams::default()
    }

    fn render(voices: &mut Voices, knobs: &MoteParams) -> (f32, f32) {
        voices.next_sample(
            &knobs.oscillators(),
            &knobs.filters(),
            &knobs.filter_envelopes(),
            &knobs.amp_envelopes(),
            &knobs.master,
        )
    }

    fn set_mode(param: &nice_plug::prelude::EnumParam<PlayMode>, mode: PlayMode) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, mode);
        }
    }

    fn set_int(param: &nice_plug::prelude::IntParam, value: i32) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, value);
        }
    }

    fn set_float(param: &nice_plug::prelude::FloatParam, value: f32) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, value);
        }
    }

    fn set_bool(param: &nice_plug::prelude::BoolParam, value: bool) {
        unsafe {
            let _ = InternalParamMut::_internal_set_plain_value(param, value);
        }
    }

    #[test]
    fn note_on_then_off_returns_to_silence() {
        let mut voices = Voices::new();
        voices.set_sample_rate(48_000.0);
        let knobs = params();
        let oscs = knobs.oscillators();
        voices.note_on(69, 1.0, &oscs, &knobs.master);

        let mut heard = false;
        for _ in 0..32 {
            let (left, right) = render(&mut voices, &knobs);
            if left.abs() > 0.0001 || right.abs() > 0.0001 {
                heard = true;
            }
        }
        assert!(heard);

        voices.note_off(Some(69), &knobs.master, &oscs);
        assert!(voices.is_sounding());
        for _ in 0..4_000 {
            render(&mut voices, &knobs);
        }
        let (left, right) = render(&mut voices, &knobs);
        assert!(!voices.is_sounding());
        assert_eq!(left, 0.0);
        assert_eq!(right, 0.0);
    }

    #[test]
    fn fifth_note_replaces_the_oldest_when_polyphony_is_four() {
        let mut voices = Voices::new();
        let knobs = params();
        let oscs = knobs.oscillators();
        for key in 60..64 {
            voices.note_on(key, 1.0, &oscs, &knobs.master);
        }
        voices.note_on(64, 1.0, &oscs, &knobs.master);
        let held: Vec<u8> = voices
            .slots
            .iter()
            .filter(|slot| slot.held)
            .filter_map(|slot| slot.key)
            .collect();
        assert_eq!(held.len(), 4);
        assert!(!held.contains(&60));
        assert!(held.contains(&64));
    }

    #[test]
    fn lowering_polyphony_releases_the_oldest_note() {
        let mut voices = Voices::new();
        let knobs = params();
        let oscs = knobs.oscillators();
        for key in 60..64 {
            voices.note_on(key, 1.0, &oscs, &knobs.master);
        }
        set_int(&knobs.master.polyphony, 2);
        render(&mut voices, &knobs);
        let held: Vec<u8> = voices
            .slots
            .iter()
            .filter(|slot| slot.held)
            .filter_map(|slot| slot.key)
            .collect();
        assert_eq!(held, vec![62, 63]);
    }

    #[test]
    fn mono_returns_to_the_previous_key() {
        let mut voices = Voices::new();
        let knobs = params();
        set_mode(&knobs.master.mode, PlayMode::Mono);
        let oscs = knobs.oscillators();
        voices.note_on(60, 1.0, &oscs, &knobs.master);
        voices.note_on(64, 1.0, &oscs, &knobs.master);
        voices.note_off(Some(64), &knobs.master, &oscs);
        let held: Vec<u8> = voices
            .slots
            .iter()
            .filter(|slot| slot.held)
            .filter_map(|slot| slot.key)
            .collect();
        assert_eq!(held, vec![60]);
    }

    #[test]
    fn legato_keeps_the_envelope_and_retrigger_restarts_it() {
        let mut legato = Voices::new();
        legato.set_sample_rate(48_000.0);
        let knobs = params();
        set_mode(&knobs.master.mode, PlayMode::Mono);
        set_bool(&knobs.master.legato, true);
        set_float(&knobs.amp1.attack, 1.0);
        let oscs = knobs.oscillators();
        legato.note_on(60, 1.0, &oscs, &knobs.master);
        for _ in 0..4_800 {
            render(&mut legato, &knobs);
        }
        let before = legato.slots[0].amp_level();
        legato.note_on(67, 1.0, &oscs, &knobs.master);
        let after = legato.slots[0].amp_level();
        assert!(before > 0.05, "before {before}");
        assert!(
            (after - before).abs() < 0.02,
            "before {before}, after {after}"
        );

        let mut retrigger = Voices::new();
        retrigger.set_sample_rate(48_000.0);
        set_mode(&knobs.master.mode, PlayMode::Mono);
        set_bool(&knobs.master.legato, false);
        retrigger.note_on(60, 1.0, &oscs, &knobs.master);
        for _ in 0..4_800 {
            render(&mut retrigger, &knobs);
        }
        retrigger.note_on(67, 1.0, &oscs, &knobs.master);
        assert!(retrigger.slots[0].amp_level() < 0.02);
    }

    #[test]
    fn portamento_glides_only_while_a_key_is_held() {
        let mut voices = Voices::new();
        voices.set_sample_rate(48_000.0);
        let knobs = params();
        set_float(&knobs.master.portamento, 1.0);
        let oscs = knobs.oscillators();
        voices.note_on(60, 1.0, &oscs, &knobs.master);
        voices.note_on(72, 1.0, &oscs, &knobs.master);
        let gliding = voices
            .slots
            .iter()
            .find(|slot| slot.key == Some(72))
            .unwrap();
        assert!((gliding.glide_note - 60.0).abs() < 0.01);
        for _ in 0..24_000 {
            render(&mut voices, &knobs);
        }
        let midway = voices
            .slots
            .iter()
            .find(|slot| slot.key == Some(72))
            .unwrap();
        assert!(
            (midway.glide_note - 66.0).abs() < 1.0,
            "glide {}",
            midway.glide_note
        );

        let mut fresh = Voices::new();
        fresh.set_sample_rate(48_000.0);
        fresh.note_on(60, 1.0, &oscs, &knobs.master);
        fresh.note_off(Some(60), &knobs.master, &oscs);
        fresh.note_on(72, 1.0, &oscs, &knobs.master);
        let jumped = fresh
            .slots
            .iter()
            .find(|slot| slot.held && slot.key == Some(72))
            .unwrap();
        assert!((jumped.glide_note - 72.0).abs() < 0.01);
    }

    #[test]
    fn master_volume_down_makes_the_note_quieter() {
        fn energy(volume: f32) -> f32 {
            let mut voices = Voices::new();
            voices.set_sample_rate(48_000.0);
            let knobs = params();
            set_float(&knobs.master.volume, volume);
            voices.note_on(69, 1.0, &knobs.oscillators(), &knobs.master);
            let mut sum = 0.0;
            for i in 0..2_000 {
                let (left, right) = render(&mut voices, &knobs);
                if i > 1_000 {
                    sum += left * left + right * right;
                }
            }
            sum
        }

        let unity = energy(0.0);
        let quiet = energy(-60.0);
        assert!(unity > quiet * 100.0, "unity {unity}, quiet {quiet}");
    }

    #[test]
    fn low_cutoff_quiets_a_high_note() {
        fn energy(cutoff: f32) -> f32 {
            let mut voices = Voices::new();
            voices.set_sample_rate(48_000.0);
            let knobs = params();
            unsafe {
                let _ = InternalParamMut::_internal_set_plain_value(&knobs.filter1.cutoff, cutoff);
            }
            voices.note_on(90, 1.0, &knobs.oscillators(), &knobs.master);
            let mut sum = 0.0;
            for i in 0..4_000 {
                let (left, right) = render(&mut voices, &knobs);
                if i > 2_000 {
                    sum += left * left + right * right;
                }
            }
            sum
        }

        let open = energy(20_000.0);
        let closed = energy(200.0);
        assert!(open > closed * 4.0, "open {open}, closed {closed}");
    }

    #[test]
    fn filter_env_amount_opens_a_low_cutoff() {
        fn energy(amount: f32) -> f32 {
            let mut voices = Voices::new();
            voices.set_sample_rate(48_000.0);
            let knobs = params();
            unsafe {
                let _ = InternalParamMut::_internal_set_plain_value(&knobs.filter1.cutoff, 200.0);
                let _ =
                    InternalParamMut::_internal_set_plain_value(&knobs.filter_env1.amount, amount);
            }
            voices.note_on(69, 1.0, &knobs.oscillators(), &knobs.master);
            let mut sum = 0.0;
            for i in 0..4_000 {
                let (left, right) = render(&mut voices, &knobs);
                if i > 2_000 {
                    sum += left * left + right * right;
                }
            }
            sum
        }

        let shut = energy(0.0);
        let open = energy(100.0);
        assert!(open > shut * 2.0, "open {open}, shut {shut}");
    }

    #[test]
    fn only_oscillator_1_is_on_by_default() {
        let knobs = params();
        assert!(knobs.osc1.on.value());
        assert!(!knobs.osc2.on.value());
        assert!(!knobs.osc3.on.value());
        assert!(!knobs.osc4.on.value());
    }
}

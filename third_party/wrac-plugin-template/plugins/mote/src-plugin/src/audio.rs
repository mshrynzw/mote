//! DSP running on the audio thread.
//!
//! This sample simply multiplies the input by a gain and writes it back.
//! [`ActiveProcessor::process`] is a realtime function called repeatedly for each small buffer,
//! so the rule is to **avoid allocations and locks**. Shared state is read lock-free from
//! [`SharedState`].

use std::any::Any;
use std::sync::Arc;

use wrac_clap_adapter::interface::{
    ActiveProcessor, AudioPairedChannels, AudioPortChannels, AudioProcessBuffer, InactiveProcessor,
    InputEvent, ParamFlushContext, PluginResult, ProcessContext, ProcessStatus,
};

use crate::plugin::{
    LatencyStore, MAX_LATENCY_FRAMES, PARAM_BYPASS_ID, PARAM_GAIN_ID, WracGainParamOutputQueue,
    apply_param_input_events, parameter_host_input_to_plain,
};
use crate::state::SharedState;

/// The DSP instance created at `activate()` and owned by the host's audio thread.
/// It lives until `deactivate()`, during which `process()` is called repeatedly.
///
/// Fields should contain only things **the audio thread can read without waiting**.
/// `shared` uses atomics and is safe to read during `process()`.
/// `audio_channel_count` is a snapshot copied from the plugin's audio layout store at
/// activate time. Because the adapter rejects layout changes while active, the running
/// active processor's contract cannot change mid-flight. Even when a product's DSP must vary
/// with layout, it is safer to convert the needed settings at activate time and pass them
/// in rather than storing an `Arc<RwLock<Layout>>`.
pub(crate) struct WracGainAudioProcessor {
    shared: Arc<SharedState>,
    param_output_queue: Arc<WracGainParamOutputQueue>,
    // Gain itself does not use channel count, but this field demonstrates the pattern
    // "snapshot layout at activate time and store it as a field."
    // In debug builds, the actual buffer is verified to match this snapshot.
    audio_channel_count: u32,
    // Follows the selected delay every block, ahead of the latency reported for this
    // processor until the host reactivates the plugin (see `plugin::latency`).
    latency: Arc<LatencyStore>,
    delay: DelayLine,
    engine: mote::Engine,
}

impl WracGainAudioProcessor {
    pub(crate) fn new(
        shared: Arc<SharedState>,
        param_output_queue: Arc<WracGainParamOutputQueue>,
        audio_channel_count: u32,
        latency: Arc<LatencyStore>,
        engine: mote::Engine,
    ) -> Self {
        Self {
            shared,
            param_output_queue,
            audio_channel_count,
            latency,
            // Sized for the longest delay at activation, so a new selection never makes
            // process() allocate.
            delay: DelayLine::new(audio_channel_count as usize, MAX_LATENCY_FRAMES as usize),
            engine,
        }
    }
}

impl ActiveProcessor for WracGainAudioProcessor {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }

    /// Processes one block. `context` contains the audio I/O, the parameter event list
    /// `events.input` for this block, and the sample count `frames_count`.
    ///
    /// The buffer is split at each parameter event's timestamp, and the gain current at
    /// that moment is applied to each segment, achieving sample-accurate automation
    /// (gain is constant between events).
    fn process(&mut self, context: ProcessContext<'_>) -> PluginResult<ProcessStatus> {
        #[cfg(debug_assertions)]
        {
            // Abort immediately on any allocation. Wrapping every process() call in
            // debug builds ensures violations are not swallowed by the DAW or adapter.
            assert_no_alloc::assert_no_alloc(|| self.process_no_alloc(context))
        }

        #[cfg(not(debug_assertions))]
        {
            self.process_no_alloc(context)
        }
    }

    fn flush_params(&mut self, mut context: ParamFlushContext<'_>) -> PluginResult<()> {
        apply_param_input_events(&self.shared, &context.events.input);
        self.param_output_queue.drain(&mut context.events.output);
        Ok(())
    }
}

impl WracGainAudioProcessor {
    fn process_no_alloc(&mut self, mut context: ProcessContext<'_>) -> PluginResult<ProcessStatus> {
        #[cfg(debug_assertions)]
        assert_audio_layout_matches_processor_snapshot(
            &mut context.audio,
            self.audio_channel_count,
        );

        self.param_output_queue.drain(&mut context.events.output);
        self.engine.drain_gui_notes();
        if let Some(transport) = context.transport {
            let tempo = transport.flags.has_tempo().then_some(transport.tempo);
            let numerator = transport
                .flags
                .has_time_signature()
                .then_some(i32::from(transport.tsig_num));
            self.engine.set_transport(tempo, numerator);
        }

        let frames_count = context.frames_count as usize;
        let mut segment_start = 0;
        for event in context.events.input.iter() {
            let event_time = (event.time() as usize).min(frames_count);
            if event_time > segment_start {
                render_engine(&mut self.engine, &mut context.audio, segment_start, event_time);
                segment_start = event_time;
            }
            handle_note(&mut self.engine, event);
        }
        if segment_start < frames_count {
            render_engine(&mut self.engine, &mut context.audio, segment_start, frames_count);
        }

        Ok(if self.engine.is_sounding() {
            ProcessStatus::Continue
        } else {
            ProcessStatus::ContinueIfNotQuiet
        })
    }
}

pub(crate) struct WracGainInactiveProcessor {
    shared: Arc<SharedState>,
    param_output_queue: Arc<WracGainParamOutputQueue>,
}

impl WracGainInactiveProcessor {
    pub(crate) fn new(
        shared: Arc<SharedState>,
        param_output_queue: Arc<WracGainParamOutputQueue>,
    ) -> Self {
        Self {
            shared,
            param_output_queue,
        }
    }
}

impl InactiveProcessor for WracGainInactiveProcessor {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }

    fn flush_params(&mut self, mut context: ParamFlushContext<'_>) -> PluginResult<()> {
        apply_param_input_events(&self.shared, &context.events.input);
        self.param_output_queue.drain(&mut context.events.output);
        Ok(())
    }
}

fn apply_realtime_param_event(
    shared: &SharedState,
    parameter_id: u32,
    host_value: f64,
    gain: &mut f32,
    bypass: &mut bool,
) {
    // Keep host-domain decoding shared with the non-RT parameter API, but keep DSP meaning
    // explicit here. Gain and bypass affect the current block differently than arbitrary
    // parameters, so hiding this behind a fully generic map would obscure realtime behavior.
    let Ok(plain_value) = parameter_host_input_to_plain(parameter_id, host_value) else {
        return;
    };
    let Some(applied) = shared.set_parameter_value(parameter_id, plain_value) else {
        return;
    };
    match parameter_id {
        PARAM_GAIN_ID => *gain = applied,
        PARAM_BYPASS_ID => *bypass = applied >= 0.5,
        _ => {}
    }
}

#[cfg(debug_assertions)]
fn assert_audio_layout_matches_processor_snapshot(
    audio: &mut AudioProcessBuffer<'_>,
    expected_channel_count: u32,
) {
    // Debug-only verification that the actual buffer matches the activate-time snapshot.
    // The store's lock is not read here. Protecting memory safety from invalid buffers
    // is the adapter's responsibility; this is a demonstration of snapshot usage, not a
    // replacement. Product DSPs that don't use channel count may remove this assertion entirely.
    debug_assert_eq!(
        audio.port_pair_count(),
        1,
        "WRAC Gain expects exactly one main audio port pair"
    );

    for port_index in 0..audio.port_pair_count() {
        let Some(port_pair) = audio.port_pair(port_index) else {
            continue;
        };
        debug_assert_eq!(
            port_pair.channel_pair_count(),
            expected_channel_count as usize,
            "audio buffer channel count must match the layout captured at activate()"
        );
    }
}

fn handle_note(engine: &mut mote::Engine, event: InputEvent) {
    match event {
        InputEvent::NoteOn(note) if (0..=127).contains(&note.key) => {
            engine.note_on(note.key as u8, note.velocity.clamp(0.0, 1.0) as f32);
        }
        InputEvent::NoteOff(note) | InputEvent::NoteChoke(note) => {
            if (0..=127).contains(&note.key) {
                engine.note_off(Some(note.key as u8));
            } else {
                engine.note_off(None);
            }
        }
        InputEvent::Midi(midi) => {
            let status = midi.data[0] & 0xF0;
            let key = midi.data[1];
            let velocity = midi.data[2];
            if status == 0x90 && velocity > 0 {
                engine.note_on(key, velocity as f32 / 127.0);
            } else if status == 0x80 || status == 0x90 {
                engine.note_off(Some(key));
            } else if status == 0xE0 {
                let bend = (u16::from(midi.data[2]) << 7 | u16::from(midi.data[1])) as f32 / 16383.0;
                engine.set_bend(bend);
            }
        }
        _ => {}
    }
}

fn render_engine(
    engine: &mut mote::Engine,
    audio: &mut AudioProcessBuffer<'_>,
    start: usize,
    end: usize,
) {
    for mut port in audio.into_iter() {
        let Ok(channels) = port.channels() else {
            continue;
        };
        match channels {
            AudioPortChannels::F32(paired) => write_pair(engine, paired, start, end),
            AudioPortChannels::F64(paired) => write_pair(engine, paired, start, end),
        }
        break;
    }
}

fn write_pair<T: Copy + Default + FromEngine>(
    engine: &mut mote::Engine,
    paired: AudioPairedChannels<'_, T>,
    start: usize,
    end: usize,
) {
    let mut channels = paired.into_iter();
    let left = channels.next();
    let right = channels.next();
    match (left, right) {
        (Some(mut left), Some(mut right)) => {
            if let (Some(left), Some(right)) = (left.output_mut(), right.output_mut()) {
                let last = end.min(left.len()).min(right.len());
                for index in start..last {
                    let (l, r) = engine.next_sample();
                    left[index] = T::from_sample(l);
                    right[index] = T::from_sample(r);
                }
            }
        }
        (Some(mut left), None) => {
            if let Some(left) = left.output_mut() {
                let last = end.min(left.len());
                for index in start..last {
                    let (sample, _) = engine.next_sample();
                    left[index] = T::from_sample(sample);
                }
            }
        }
        _ => {
            for _ in start..end {
                let _ = engine.next_sample();
            }
        }
    }
}

trait FromEngine: Copy {
    fn from_sample(value: f32) -> Self;
}

impl FromEngine for f32 {
    fn from_sample(value: f32) -> Self {
        value
    }
}

impl FromEngine for f64 {
    fn from_sample(value: f32) -> Self {
        value as f64
    }
}

/// Delays and applies gain to the `[start, end)` range of each port.
/// The buffer passed by the host can be either `f32` or `f64`, so both are handled.
///
/// The delay is applied bypassed or not: the reported latency must describe the audio
/// in every state, or the host's compensation would be wrong whenever bypass toggles.
/// It runs in the same per-sample pass as the gain because the output of a separate
/// pass would be overwritten when the host gives distinct input and output buffers.
fn process_audio_range(
    audio: &mut AudioProcessBuffer<'_>,
    delay: &mut DelayLine,
    start: usize,
    end: usize,
    gain: f32,
) -> PluginResult<()> {
    let len = end.saturating_sub(start);
    for (port_index, mut port_pair) in audio.into_iter().enumerate() {
        // WRAC Gain has a single main port pair (see the layout assertion above); the
        // delay buffers belong to it.
        let delay = if port_index == 0 {
            Some(&mut *delay)
        } else {
            None
        };
        match port_pair.channels()? {
            AudioPortChannels::F32(channels) => {
                process_channels_range(channels, delay, start, len, gain)
            }
            AudioPortChannels::F64(channels) => {
                process_channels_range(channels, delay, start, len, gain as f64)
            }
        }
    }
    Ok(())
}

/// Sample types the host may hand over. The delay buffers store `f64` for both.
trait Sample: Copy + Default + std::ops::Mul<Output = Self> {
    fn to_f64(self) -> f64;
    fn from_f64(value: f64) -> Self;
}

impl Sample for f32 {
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn from_f64(value: f64) -> Self {
        value as f32
    }
}

impl Sample for f64 {
    fn to_f64(self) -> f64 {
        self
    }
    fn from_f64(value: f64) -> Self {
        value
    }
}

/// Delays and applies gain to each sample in every channel of one port (paired in/out).
///
/// `map_samples_range` operates in-place and supports "in-place processing"
/// where the input and output point to the same buffer.
fn process_channels_range<T: Sample>(
    channels: AudioPairedChannels<'_, T>,
    mut delay: Option<&mut DelayLine>,
    start: usize,
    len: usize,
    gain: T,
) {
    for (channel_index, mut channel) in channels.into_iter().enumerate() {
        let mut channel_delay = delay
            .as_deref_mut()
            .and_then(|delay| delay.channel(channel_index, start));
        channel.map_samples_range(start, len, |sample| {
            let sample = match channel_delay.as_mut() {
                Some(channel_delay) => T::from_f64(channel_delay.push(sample.to_f64())),
                None => sample,
            };
            sample * gain
        });
    }
}

/// Delays every channel by a selectable number of samples.
///
/// One ring buffer per channel, sized for the longest delay. Each sample is written at
/// the write position and the one written `delay` samples earlier is read back, so a
/// new delay takes effect from the next block without reallocating. All channels advance
/// together, so the write position at the start of the block is common to them.
struct DelayLine {
    buffers: Vec<Vec<f64>>,
    block_write: usize,
    delay: usize,
}

/// One channel's view of the delay line, starting at a frame within the block.
struct ChannelDelay<'a> {
    buffer: &'a mut [f64],
    write: usize,
    delay: usize,
}

impl ChannelDelay<'_> {
    /// Stores `sample` and returns the one stored `delay` samples earlier.
    fn push(&mut self, sample: f64) -> f64 {
        let len = self.buffer.len();
        self.buffer[self.write] = sample;
        let delayed = self.buffer[(self.write + len - self.delay) % len];
        self.write = (self.write + 1) % len;
        delayed
    }
}

impl DelayLine {
    fn new(channel_count: usize, max_delay_frames: usize) -> Self {
        Self {
            // One slot more than the longest delay: the sample read back at the maximum
            // delay must not be the one just written.
            buffers: vec![vec![0.0; max_delay_frames + 1]; channel_count],
            block_write: 0,
            delay: 0,
        }
    }

    /// Changing the delay jumps the read position, so the audio is discontinuous at the
    /// switch. That is accepted: the switch follows a deliberate settings change.
    fn set_delay(&mut self, frames: usize) {
        let max = self.buffers.first().map_or(0, |buffer| buffer.len() - 1);
        self.delay = frames.min(max);
    }

    /// Returns `None` when there is no delay, so the caller passes samples through.
    fn channel(&mut self, channel_index: usize, frame_in_block: usize) -> Option<ChannelDelay<'_>> {
        if self.delay == 0 {
            return None;
        }
        let buffer = self.buffers.get_mut(channel_index)?;
        let write = (self.block_write + frame_in_block) % buffer.len();
        Some(ChannelDelay {
            buffer: buffer.as_mut_slice(),
            write,
            delay: self.delay,
        })
    }

    fn advance(&mut self, frames: usize) {
        if let Some(len) = self.buffers.first().map(Vec::len) {
            self.block_write = (self.block_write + frames) % len;
        }
    }
}

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::{CHANNELS, SAMPLE_RATE};

const MIN_LAG_MS: i64 = 200;
const LAG_MARGIN_MS: i64 = 150;
const MAX_LAG_MS: i64 = 1_000;
const MAX_BUFFERED_FRAMES: usize = SAMPLE_RATE as usize * 2;
const LOOKAHEAD_FRAMES: usize = SAMPLE_RATE as usize * 3 / 1_000;
const RELEASE_FRAMES: f32 = SAMPLE_RATE as f32 * 0.08;
const CEILING: f32 = 0.891_250_9;

#[derive(Debug, Clone, Default)]
pub struct MixBus(Arc<(Mutex<BusState>, Condvar)>);

#[derive(Debug, Default)]
struct BusState {
    start: i64,
    samples: VecDeque<f32>,
    closed: bool,
}

impl BusState {
    fn end(&self) -> i64 {
        self.start + (self.samples.len() / CHANNELS) as i64
    }

    fn drop_before(&mut self, frame: i64) {
        let excess = ((frame - self.start).max(0) as usize).min(self.samples.len() / CHANNELS);
        self.samples.drain(..excess * CHANNELS);
        self.start = self.start.max(frame);
    }
}

#[derive(Debug, Clone, Default)]
pub enum Mixdown {
    #[default]
    None,
    Feed {
        bus: MixBus,
        duplicates: Vec<String>,
    },
    Mix {
        buses: Vec<MixBus>,
        lag_ms: i64,
    },
}

pub fn mix_lag_ms(feed_offsets_ms: impl IntoIterator<Item = i32>) -> i64 {
    let late = feed_offsets_ms
        .into_iter()
        .map(|offset| -i64::from(offset))
        .max()
        .unwrap_or(0)
        .max(0);
    (late + LAG_MARGIN_MS).clamp(MIN_LAG_MS, MAX_LAG_MS)
}

impl MixBus {
    pub fn push(&self, start: i64, samples: &[f32]) {
        let (state, ready) = &*self.0;
        let Ok(mut state) = state.lock() else { return };
        if state.samples.is_empty() {
            state.start = start;
        }
        let gap = (start - state.end()).max(0) as usize;
        state
            .samples
            .extend(std::iter::repeat_n(0.0, gap * CHANNELS));
        let overlap = ((state.end() - start).max(0) as usize).min(samples.len() / CHANNELS);
        state.samples.extend(&samples[overlap * CHANNELS..]);
        let buffered = state.samples.len() / CHANNELS;
        if buffered > MAX_BUFFERED_FRAMES {
            let oldest = state.start + (buffered - MAX_BUFFERED_FRAMES) as i64;
            state.drop_before(oldest);
        }
        ready.notify_all();
    }

    pub fn close(&self) {
        let (state, ready) = &*self.0;
        if let Ok(mut state) = state.lock() {
            state.closed = true;
        }
        ready.notify_all();
    }

    fn wait_until(&self, frame: i64, timeout: Duration) {
        let (state, ready) = &*self.0;
        let deadline = Instant::now() + timeout;
        let Ok(mut guard) = state.lock() else { return };
        while !guard.closed && guard.end() < frame {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return;
            }
            match ready.wait_timeout(guard, left) {
                Ok((next, _)) => guard = next,
                Err(_) => return,
            }
        }
    }

    fn mix_into(&self, start: i64, out: &mut [f32], force: usize) -> usize {
        let (state, _) = &*self.0;
        let frames = out.len() / CHANNELS;
        let Ok(mut state) = state.lock() else {
            return frames;
        };
        state.drop_before(start);
        let silent_before = ((state.start - start).max(0) as usize).min(frames);
        let available = state.samples.len() / CHANNELS;
        let mixed = (frames - silent_before).min(available);
        for (sample, mic) in out[silent_before * CHANNELS..(silent_before + mixed) * CHANNELS]
            .iter_mut()
            .zip(state.samples.drain(..mixed * CHANNELS))
        {
            *sample += mic;
        }
        let ready = (silent_before + mixed).max(force.min(frames));
        state.start = state.start.max(start + ready as i64);
        ready
    }
}

#[derive(Debug)]
struct Limiter {
    gain: f32,
    delayed: VecDeque<f32>,
    required: VecDeque<f32>,
}

impl Default for Limiter {
    fn default() -> Self {
        Self {
            gain: 1.0,
            delayed: VecDeque::new(),
            required: VecDeque::new(),
        }
    }
}

impl Limiter {
    fn push(&mut self, samples: &[f32], out: &mut Vec<f32>) {
        for frame in samples.chunks_exact(CHANNELS) {
            let peak = frame
                .iter()
                .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
            self.required
                .push_back(if peak > CEILING { CEILING / peak } else { 1.0 });
            self.delayed.extend(frame);
            if self.required.len() > LOOKAHEAD_FRAMES {
                self.emit(out);
            }
        }
    }

    fn finish(&mut self, out: &mut Vec<f32>) {
        while !self.required.is_empty() {
            self.emit(out);
        }
    }

    fn emit(&mut self, out: &mut Vec<f32>) {
        let released = (self.gain + (1.0 - self.gain) / RELEASE_FRAMES).min(1.0);
        self.gain = self
            .required
            .iter()
            .enumerate()
            .map(|(ahead, required)| {
                required + (1.0 - required) * ahead as f32 / LOOKAHEAD_FRAMES as f32
            })
            .fold(released, f32::min);
        self.required.pop_front();
        let gain = self.gain;
        out.extend(
            self.delayed
                .drain(..CHANNELS)
                .map(|sample| (sample * gain).clamp(-1.0, 1.0)),
        );
    }
}

#[derive(Debug)]
pub struct MixIn {
    buses: Vec<MixBus>,
    lag_frames: usize,
    pending: Vec<f32>,
    start: i64,
    limiter: Limiter,
}

impl MixIn {
    pub fn new(buses: Vec<MixBus>, lag_ms: i64) -> Self {
        Self {
            buses,
            lag_frames: lag_ms.max(0) as usize * SAMPLE_RATE as usize / 1_000,
            pending: Vec::new(),
            start: 0,
            limiter: Limiter::default(),
        }
    }

    pub fn take_ready(&mut self, filled: &[f32], written: i64, out: &mut Vec<f32>) {
        self.append(filled, written);
        let frames = self.pending.len() / CHANNELS;
        let ready = self.mix(frames.saturating_sub(self.lag_frames));
        self.release(ready, out);
    }

    pub fn take_all(&mut self, filled: &[f32], written: i64, out: &mut Vec<f32>, wait: Duration) {
        self.append(filled, written);
        let deadline = Instant::now() + wait;
        for bus in &self.buses {
            bus.wait_until(written, deadline.saturating_duration_since(Instant::now()));
        }
        let frames = self.pending.len() / CHANNELS;
        let ready = self.mix(frames);
        self.release(ready, out);
        self.limiter.finish(out);
    }

    fn mix(&mut self, force: usize) -> usize {
        let frames = self.pending.len() / CHANNELS;
        self.buses
            .iter()
            .map(|bus| bus.mix_into(self.start, &mut self.pending, force))
            .min()
            .unwrap_or(frames)
    }

    fn append(&mut self, filled: &[f32], written: i64) {
        self.pending.extend_from_slice(filled);
        self.start = written - (self.pending.len() / CHANNELS) as i64;
    }

    fn release(&mut self, frames: usize, out: &mut Vec<f32>) {
        let ready: Vec<f32> = self.pending.drain(..frames * CHANNELS).collect();
        self.limiter.push(&ready, out);
        self.start += frames as i64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAG_MS: i64 = 200;
    const LAG_FRAMES: usize = SAMPLE_RATE as usize / 5;

    fn block(value: f32, frames: usize) -> Vec<f32> {
        vec![value; frames * CHANNELS]
    }

    fn finish(mix: &mut MixIn, written: i64, out: &mut Vec<f32>) {
        mix.take_all(&[], written, out, Duration::ZERO);
    }

    fn sine(frequency: f32, amplitude: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|frame| {
                let phase =
                    2.0 * std::f32::consts::PI * frequency * frame as f32 / SAMPLE_RATE as f32;
                [amplitude * phase.sin(); CHANNELS]
            })
            .collect()
    }

    fn peak(samples: &[f32]) -> f32 {
        samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()))
    }

    #[test]
    fn microphone_samples_are_added_at_the_same_frames() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(vec![bus.clone()], LAG_MS);
        let mut out = Vec::new();
        bus.push(0, &block(0.25, 480));
        bus.close();
        mix.take_ready(&block(0.5, 480), 480, &mut out);
        assert_eq!(out, block(0.75, 480 - LOOKAHEAD_FRAMES));
        finish(&mut mix, 480, &mut out);
        assert_eq!(out, block(0.75, 480));
    }

    #[test]
    fn the_mix_waits_for_a_late_microphone() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(vec![bus.clone()], LAG_MS);
        let mut out = Vec::new();
        mix.take_ready(&block(0.5, 480), 480, &mut out);
        assert!(out.is_empty());
        bus.push(0, &block(0.25, 480));
        bus.close();
        finish(&mut mix, 480, &mut out);
        assert_eq!(out, block(0.75, 480));
    }

    #[test]
    fn a_stalled_microphone_never_holds_the_mix_more_than_the_lag() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(vec![bus.clone()], LAG_MS);
        let mut out = Vec::new();
        let frames = SAMPLE_RATE as usize;
        mix.take_ready(&block(0.5, frames), frames as i64, &mut out);
        assert_eq!(out, block(0.5, frames - LAG_FRAMES - LOOKAHEAD_FRAMES));
        bus.push(0, &block(0.25, frames));
        bus.close();
        finish(&mut mix, frames as i64, &mut out);
        assert_eq!(out.len(), frames * CHANNELS);
        assert!(
            out[(frames - LAG_FRAMES) * CHANNELS..]
                .iter()
                .all(|sample| *sample == 0.75)
        );
    }

    #[test]
    fn a_microphone_450_ms_late_is_still_mixed() {
        let lag_ms = mix_lag_ms([-450, 0]);
        assert_eq!(lag_ms, 600);
        let bus = MixBus::default();
        let mut mix = MixIn::new(vec![bus.clone()], lag_ms);
        let mut out = Vec::new();
        let packet = SAMPLE_RATE as usize / 100;
        let late = 45;
        for step in 0..200 {
            mix.take_ready(&block(0.25, packet), ((step + 1) * packet) as i64, &mut out);
            if step >= late {
                bus.push(((step - late) * packet) as i64, &block(0.5, packet));
            }
        }
        for step in 200 - late..200 {
            bus.push((step * packet) as i64, &block(0.5, packet));
        }
        bus.close();
        finish(&mut mix, (200 * packet) as i64, &mut out);
        assert_eq!(out.len(), 200 * packet * CHANNELS);
        assert!(out.iter().all(|sample| *sample == 0.75));
    }

    #[test]
    fn a_wait_shorter_than_the_offset_cuts_the_microphone() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(vec![bus.clone()], LAG_MS);
        let mut out = Vec::new();
        let packet = SAMPLE_RATE as usize / 100;
        for step in 0..100 {
            mix.take_ready(&block(0.25, packet), ((step + 1) * packet) as i64, &mut out);
            if step >= 45 {
                bus.push(((step - 45) * packet) as i64, &block(0.5, packet));
            }
        }
        assert!(out.contains(&0.25));
    }

    #[test]
    fn the_lag_follows_the_most_negative_offset_within_bounds() {
        assert_eq!(mix_lag_ms([]), 200);
        assert_eq!(mix_lag_ms([0, 120]), 200);
        assert_eq!(mix_lag_ms([-100]), 250);
        assert_eq!(mix_lag_ms([-1_000]), 1_000);
    }

    #[test]
    fn the_end_flushes_everything_once_the_microphone_closes() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(vec![bus.clone()], LAG_MS);
        let mut out = Vec::new();
        bus.push(0, &block(0.25, 100));
        bus.close();
        mix.take_all(&block(0.5, 480), 480, &mut out, Duration::from_secs(5));
        assert_eq!(out.len(), 480 * CHANNELS);
        assert_eq!(out[..100 * CHANNELS], block(0.75, 100));
        assert_eq!(out[100 * CHANNELS..], block(0.5, 380));
    }

    #[test]
    fn several_tracks_add_up_and_the_slowest_one_sets_the_pace() {
        let game = MixBus::default();
        let microphone = MixBus::default();
        let mut mix = MixIn::new(vec![game.clone(), microphone.clone()], LAG_MS);
        let mut out = Vec::new();
        game.push(0, &block(0.25, 480));
        microphone.push(0, &block(0.125, 200));
        mix.take_ready(&block(0.0, 480), 480, &mut out);
        assert_eq!(out, block(0.375, 200 - LOOKAHEAD_FRAMES));
        microphone.push(200, &block(0.125, 280));
        game.close();
        microphone.close();
        finish(&mut mix, 480, &mut out);
        assert_eq!(out, block(0.375, 480));
    }

    #[test]
    fn two_sines_at_minus_3_dbfs_are_limited_below_minus_1_dbfs_without_clipping() {
        let frames = SAMPLE_RATE as usize;
        let amplitude = 10f32.powf(-3.0 / 20.0);
        let first = MixBus::default();
        let second = MixBus::default();
        first.push(0, &sine(440.0, amplitude, frames));
        second.push(0, &sine(441.0, amplitude, frames));
        first.close();
        second.close();
        let mut mix = MixIn::new(vec![first, second], LAG_MS);
        let mut out = Vec::new();
        mix.take_all(&block(0.0, frames), frames as i64, &mut out, Duration::ZERO);
        assert_eq!(out.len(), frames * CHANNELS);
        let level = peak(&out);
        assert!(level <= CEILING + 1e-6, "crête {level}");
        assert!(level > 0.8, "crête {level}");
        assert!(out.iter().all(|sample| sample.abs() < 1.0));
    }

    #[test]
    fn quiet_audio_passes_through_the_limiter_unchanged() {
        let frames = 4_800;
        let quiet = sine(440.0, 0.5, frames);
        let bus = MixBus::default();
        bus.push(0, &quiet);
        bus.close();
        let mut mix = MixIn::new(vec![bus], LAG_MS);
        let mut out = Vec::new();
        mix.take_all(&block(0.0, frames), frames as i64, &mut out, Duration::ZERO);
        assert_eq!(out, quiet);
    }
}

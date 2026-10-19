use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use super::{CHANNELS, SAMPLE_RATE};

const MAX_LAG_FRAMES: usize = SAMPLE_RATE as usize / 5;
const MAX_BUFFERED_FRAMES: usize = SAMPLE_RATE as usize * 2;

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
    Feed(MixBus),
    Mix(MixBus),
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
            *sample = (*sample + mic).clamp(-1.0, 1.0);
        }
        let ready = (silent_before + mixed).max(force.min(frames));
        state.start = state.start.max(start + ready as i64);
        ready
    }
}

#[derive(Debug, Default)]
pub struct MixIn {
    bus: MixBus,
    pending: Vec<f32>,
    start: i64,
}

impl MixIn {
    pub fn new(bus: MixBus) -> Self {
        Self {
            bus,
            pending: Vec::new(),
            start: 0,
        }
    }

    pub fn take_ready(&mut self, filled: &[f32], written: i64, out: &mut Vec<f32>) {
        self.append(filled, written);
        let frames = self.pending.len() / CHANNELS;
        let ready = self.bus.mix_into(
            self.start,
            &mut self.pending,
            frames.saturating_sub(MAX_LAG_FRAMES),
        );
        self.release(ready, out);
    }

    pub fn take_all(&mut self, filled: &[f32], written: i64, out: &mut Vec<f32>, wait: Duration) {
        self.append(filled, written);
        self.bus.wait_until(written, wait);
        let frames = self.pending.len() / CHANNELS;
        let ready = self.bus.mix_into(self.start, &mut self.pending, frames);
        self.release(ready, out);
    }

    fn append(&mut self, filled: &[f32], written: i64) {
        self.pending.extend_from_slice(filled);
        self.start = written - (self.pending.len() / CHANNELS) as i64;
    }

    fn release(&mut self, frames: usize, out: &mut Vec<f32>) {
        out.extend(self.pending.drain(..frames * CHANNELS));
        self.start += frames as i64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(value: f32, frames: usize) -> Vec<f32> {
        vec![value; frames * CHANNELS]
    }

    #[test]
    fn microphone_samples_are_added_at_the_same_frames() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(bus.clone());
        let mut out = Vec::new();
        bus.push(0, &block(0.25, 480));
        mix.take_ready(&block(0.5, 480), 480, &mut out);
        assert_eq!(out, block(0.75, 480));
    }

    #[test]
    fn the_mix_waits_for_a_late_microphone() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(bus.clone());
        let mut out = Vec::new();
        mix.take_ready(&block(0.5, 480), 480, &mut out);
        assert!(out.is_empty());
        bus.push(0, &block(0.25, 480));
        mix.take_ready(&[], 480, &mut out);
        assert_eq!(out, block(0.75, 480));
    }

    #[test]
    fn a_stalled_microphone_never_holds_the_mix_more_than_the_lag() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(bus.clone());
        let mut out = Vec::new();
        let frames = SAMPLE_RATE as usize;
        mix.take_ready(&block(0.5, frames), frames as i64, &mut out);
        assert_eq!(out, block(0.5, frames - MAX_LAG_FRAMES));
        bus.push(0, &block(0.25, frames));
        mix.take_ready(&[], frames as i64, &mut out);
        assert_eq!(out.len(), frames * CHANNELS);
        assert!(
            out[(frames - MAX_LAG_FRAMES) * CHANNELS..]
                .iter()
                .all(|sample| *sample == 0.75)
        );
    }

    #[test]
    fn the_end_flushes_everything_once_the_microphone_closes() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(bus.clone());
        let mut out = Vec::new();
        bus.push(0, &block(0.25, 100));
        bus.close();
        mix.take_all(&block(0.5, 480), 480, &mut out, Duration::from_secs(5));
        assert_eq!(out.len(), 480 * CHANNELS);
        assert_eq!(out[..100 * CHANNELS], block(0.75, 100));
        assert_eq!(out[100 * CHANNELS..], block(0.5, 380));
    }

    #[test]
    fn the_sum_is_clamped() {
        let bus = MixBus::default();
        let mut mix = MixIn::new(bus.clone());
        let mut out = Vec::new();
        bus.push(0, &block(0.75, 10));
        mix.take_ready(&block(0.75, 10), 10, &mut out);
        assert_eq!(out, block(1.0, 10));
    }
}

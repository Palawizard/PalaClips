use crate::clock::{HNS_PER_SECOND, Hns};

use super::filler::{FillStats, GapFiller};
use super::{CHANNELS, SAMPLE_RATE};

#[derive(Debug)]
struct Input {
    filler: GapFiller,
    pending: Vec<f32>,
}

#[derive(Debug)]
pub struct StreamSum {
    origin: Hns,
    written: i64,
    inputs: Vec<Input>,
    retired: FillStats,
}

impl StreamSum {
    pub fn new(origin: Hns) -> Self {
        Self {
            origin,
            written: 0,
            inputs: Vec::new(),
            retired: FillStats::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    pub fn reset(&mut self, inputs: usize, written: i64) {
        for input in self.inputs.drain(..) {
            self.retired += input.filler.stats();
        }
        self.written = written;
        self.inputs = (0..inputs)
            .map(|_| Input {
                filler: GapFiller::starting_at(self.origin, self.written),
                pending: Vec::new(),
            })
            .collect();
    }

    pub fn push(&mut self, input: usize, qpc: Hns, samples: &[f32]) {
        if let Some(input) = self.inputs.get_mut(input) {
            input.filler.push(qpc, samples, &mut input.pending);
        }
    }

    pub fn fill_until(&mut self, qpc: Hns) {
        for input in &mut self.inputs {
            input.filler.fill_until(qpc, &mut input.pending);
        }
    }

    pub fn take(&mut self, out: &mut Vec<f32>) -> Option<Hns> {
        let frames = self
            .inputs
            .iter()
            .map(|input| input.pending.len() / CHANNELS)
            .min()
            .filter(|frames| *frames > 0)?;
        let start = self.origin + Hns(self.written * HNS_PER_SECOND / i64::from(SAMPLE_RATE));
        let samples = frames * CHANNELS;
        let first = out.len();
        out.resize(first + samples, 0.0);
        for input in &mut self.inputs {
            for (sum, sample) in out[first..].iter_mut().zip(input.pending.drain(..samples)) {
                *sum += sample;
            }
        }
        self.written += frames as i64;
        Some(start)
    }

    pub fn stats(&self) -> FillStats {
        let mut stats = self.retired;
        for input in &self.inputs {
            stats += input.filler.stats();
        }
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Hns = Hns(5_000_000_000);
    const PACKET: usize = 480;

    fn at_frame(frame: i64) -> Hns {
        ORIGIN + Hns(frame * HNS_PER_SECOND / i64::from(SAMPLE_RATE))
    }

    fn packet(value: f32) -> Vec<f32> {
        vec![value; PACKET * CHANNELS]
    }

    #[test]
    fn a_single_input_passes_through_like_a_gap_filler() {
        let mut sum = StreamSum::new(ORIGIN);
        sum.reset(1, 0);
        let mut filler = GapFiller::new(ORIGIN);
        let (mut summed, mut direct) = (Vec::new(), Vec::new());
        for index in [0, 1, 2, 5, 6] {
            let qpc = at_frame(index * PACKET as i64);
            sum.push(0, qpc, &packet(0.25));
            filler.push(qpc, &packet(0.25), &mut direct);
            sum.take(&mut summed);
        }
        assert_eq!(summed, direct);
        assert_eq!(sum.stats(), filler.stats());
    }

    #[test]
    fn two_inputs_are_added_sample_by_sample() {
        let mut sum = StreamSum::new(ORIGIN);
        sum.reset(2, 0);
        let mut out = Vec::new();
        sum.push(0, at_frame(0), &packet(0.25));
        assert_eq!(sum.take(&mut out), None);
        sum.push(1, at_frame(0), &packet(0.5));
        assert_eq!(sum.take(&mut out), Some(ORIGIN));
        assert_eq!(out, vec![0.75; PACKET * CHANNELS]);
    }

    #[test]
    fn a_silent_input_waits_for_the_silence_horizon_then_adds_nothing() {
        let mut sum = StreamSum::new(ORIGIN);
        sum.reset(2, 0);
        let mut out = Vec::new();
        sum.push(0, at_frame(0), &packet(0.5));
        sum.fill_until(at_frame(PACKET as i64));
        sum.take(&mut out);
        assert_eq!(out, packet(0.5));
    }

    #[test]
    fn inputs_added_later_start_at_the_track_position() {
        let mut sum = StreamSum::new(ORIGIN);
        sum.reset(1, 0);
        let mut out = Vec::new();
        sum.push(0, at_frame(0), &packet(0.5));
        sum.take(&mut out);
        sum.reset(2, PACKET as i64);
        let next = at_frame(PACKET as i64);
        sum.push(0, next, &packet(0.25));
        sum.push(1, next, &packet(0.25));
        assert_eq!(sum.take(&mut out), Some(next));
        assert_eq!(out.len(), 2 * PACKET * CHANNELS);
        assert_eq!(out[PACKET * CHANNELS..], packet(0.5)[..]);
        assert_eq!(sum.stats().silence_frames, 0);
    }

    #[test]
    fn the_block_time_maps_back_to_the_same_frame() {
        let mut sum = StreamSum::new(ORIGIN);
        sum.reset(1, 0);
        let mut out = Vec::new();
        for index in 0..10_000 {
            sum.push(0, at_frame(index * 7), &[0.0; 7 * CHANNELS]);
            let start = sum.take(&mut out).expect("bloc");
            assert_eq!((start - ORIGIN).to_samples(SAMPLE_RATE), index * 7);
        }
    }
}

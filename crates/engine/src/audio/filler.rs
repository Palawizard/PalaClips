use crate::clock::Hns;

use super::{CHANNELS, SAMPLE_RATE};

const GAP_TOLERANCE_FRAMES: i64 = SAMPLE_RATE as i64 / 200;
const DRIFT_THRESHOLD_FRAMES: f64 = SAMPLE_RATE as f64 / 1_000.0;
const DRIFT_SMOOTHING: f64 = 0.02;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FillStats {
    pub silence_frames: u64,
    pub dropped_frames: u64,
    pub drift_corrections: u64,
}

#[derive(Debug)]
pub struct GapFiller {
    origin: Hns,
    written: i64,
    end: Option<i64>,
    smoothed_error: f64,
    stats: FillStats,
}

impl GapFiller {
    pub fn new(origin: Hns) -> Self {
        Self {
            origin,
            written: 0,
            end: None,
            smoothed_error: 0.0,
            stats: FillStats::default(),
        }
    }

    pub fn written_frames(&self) -> i64 {
        self.written
    }

    pub fn stats(&self) -> FillStats {
        self.stats
    }

    pub fn set_end(&mut self, qpc: Hns) {
        self.end = Some(self.frame_at(qpc).max(0));
    }

    pub fn reached_end(&self) -> bool {
        self.end.is_some_and(|end| self.written >= end)
    }

    pub fn push(&mut self, qpc: Hns, samples: &[f32], out: &mut Vec<f32>) {
        let frames = (samples.len() / CHANNELS) as i64;
        let error = self.frame_at(qpc) - self.written;
        let mut skipped = 0;
        if error > GAP_TOLERANCE_FRAMES {
            self.silence(error, out);
            self.smoothed_error = 0.0;
        } else if error < -GAP_TOLERANCE_FRAMES {
            skipped = (-error).min(frames);
            self.stats.dropped_frames += skipped as u64;
            self.smoothed_error = 0.0;
        } else if frames > 0 {
            self.smoothed_error += (error as f64 - self.smoothed_error) * DRIFT_SMOOTHING;
            if self.smoothed_error > DRIFT_THRESHOLD_FRAMES {
                out.extend_from_slice(&samples[..CHANNELS]);
                self.written += 1;
                self.smoothed_error -= 1.0;
                self.stats.drift_corrections += 1;
            } else if self.smoothed_error < -DRIFT_THRESHOLD_FRAMES {
                skipped = 1;
                self.smoothed_error += 1.0;
                self.stats.drift_corrections += 1;
            }
        }
        out.extend_from_slice(&samples[skipped as usize * CHANNELS..frames as usize * CHANNELS]);
        self.written += frames - skipped;
        self.clamp_to_end(out);
    }

    pub fn fill_until(&mut self, qpc: Hns, out: &mut Vec<f32>) {
        let target = self
            .end
            .map_or(self.frame_at(qpc), |end| end.min(self.frame_at(qpc)));
        let missing = target - self.written;
        if missing > 0 {
            self.silence(missing, out);
        }
        self.clamp_to_end(out);
    }

    fn clamp_to_end(&mut self, out: &mut Vec<f32>) {
        if let Some(end) = self.end
            && self.written > end
        {
            let excess = (self.written - end) as usize * CHANNELS;
            out.truncate(out.len().saturating_sub(excess));
            self.written = end;
        }
    }

    fn frame_at(&self, qpc: Hns) -> i64 {
        (qpc - self.origin).to_samples(SAMPLE_RATE)
    }

    fn silence(&mut self, frames: i64, out: &mut Vec<f32>) {
        out.resize(out.len() + frames as usize * CHANNELS, 0.0);
        self.written += frames;
        self.stats.silence_frames += frames as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::HNS_PER_SECOND;

    const ORIGIN: Hns = Hns(5_000_000_000);
    const PACKET: usize = 480;

    fn packet(value: f32) -> Vec<f32> {
        vec![value; PACKET * CHANNELS]
    }

    fn at_frame(frame: i64) -> Hns {
        ORIGIN + Hns(frame * HNS_PER_SECOND / SAMPLE_RATE as i64)
    }

    #[test]
    fn jittered_packets_pass_through_unchanged() {
        let mut filler = GapFiller::new(ORIGIN);
        let mut out = Vec::new();
        for index in 0..1_000 {
            let jitter = Hns::from_millis([0, 2, -2, 1, -1][index % 5]);
            let qpc = at_frame((index * PACKET) as i64) + jitter;
            filler.push(qpc, &packet(0.5), &mut out);
        }
        assert_eq!(out.len(), 1_000 * PACKET * CHANNELS);
        assert_eq!(filler.stats(), FillStats::default());
    }

    #[test]
    fn a_gap_is_filled_with_silence() {
        let mut filler = GapFiller::new(ORIGIN);
        let mut out = Vec::new();
        filler.push(at_frame(0), &packet(0.5), &mut out);
        filler.push(at_frame(PACKET as i64 + 4_800), &packet(0.5), &mut out);
        assert_eq!(filler.stats().silence_frames, 4_800);
        assert_eq!(filler.written_frames(), 2 * PACKET as i64 + 4_800);
        let silence = &out[PACKET * CHANNELS..(PACKET + 4_800) * CHANNELS];
        assert!(silence.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn an_overlap_is_trimmed() {
        let mut filler = GapFiller::new(ORIGIN);
        let mut out = Vec::new();
        filler.push(at_frame(0), &packet(0.5), &mut out);
        filler.push(at_frame(PACKET as i64 - 400), &packet(0.25), &mut out);
        assert_eq!(filler.stats().dropped_frames, 400);
        assert_eq!(filler.written_frames(), 2 * PACKET as i64 - 400);
    }

    #[test]
    fn samples_before_origin_are_dropped_and_late_start_is_padded() {
        let mut filler = GapFiller::new(ORIGIN);
        let mut out = Vec::new();
        filler.push(at_frame(-(PACKET as i64)), &packet(0.5), &mut out);
        assert!(out.is_empty());
        filler.push(at_frame(2_400), &packet(0.5), &mut out);
        assert_eq!(filler.stats().silence_frames, 2_400);
        assert_eq!(out.len(), (2_400 + PACKET) * CHANNELS);
    }

    #[test]
    fn fill_until_pads_up_to_the_requested_time() {
        let mut filler = GapFiller::new(ORIGIN);
        let mut out = Vec::new();
        filler.push(at_frame(0), &packet(0.5), &mut out);
        filler.fill_until(ORIGIN + Hns(HNS_PER_SECOND), &mut out);
        assert_eq!(filler.written_frames(), SAMPLE_RATE as i64);
        filler.fill_until(ORIGIN, &mut out);
        assert_eq!(filler.written_frames(), SAMPLE_RATE as i64);
    }

    #[test]
    fn output_stops_exactly_at_the_end() {
        let mut filler = GapFiller::new(ORIGIN);
        let mut out = Vec::new();
        filler.push(at_frame(0), &packet(0.5), &mut out);
        filler.set_end(at_frame(700));
        assert!(!filler.reached_end());
        filler.push(at_frame(PACKET as i64), &packet(0.5), &mut out);
        assert!(filler.reached_end());
        assert_eq!(filler.written_frames(), 700);
        assert_eq!(out.len(), 700 * CHANNELS);
        filler.fill_until(at_frame(2_000), &mut out);
        assert_eq!(out.len(), 700 * CHANNELS);
        assert_eq!(filler.stats().silence_frames, 0);
    }

    #[test]
    fn device_clock_drift_is_absorbed_without_gaps_over_an_hour() {
        for ppm in [-200.0, 200.0] {
            let mut filler = GapFiller::new(ORIGIN);
            let mut out = Vec::new();
            let packets = 360_000;
            for index in 0..packets {
                let device_frame = (index * PACKET) as f64;
                let qpc = ORIGIN
                    + Hns((device_frame * (1.0 + ppm / 1e6) * HNS_PER_SECOND as f64
                        / SAMPLE_RATE as f64) as i64);
                filler.push(qpc, &packet(0.5), &mut out);
                out.clear();
            }
            let end = ORIGIN
                + Hns(
                    ((packets * PACKET) as f64 * (1.0 + ppm / 1e6) * HNS_PER_SECOND as f64
                        / SAMPLE_RATE as f64) as i64,
                );
            let error = (end - ORIGIN).to_samples(SAMPLE_RATE) - filler.written_frames();
            assert!(
                error.abs() <= 2 * DRIFT_THRESHOLD_FRAMES as i64,
                "{ppm} ppm : {error}"
            );
            assert_eq!(filler.stats().silence_frames, 0, "{ppm} ppm");
            assert_eq!(filler.stats().dropped_frames, 0, "{ppm} ppm");
            assert!(filler.stats().drift_corrections > 0, "{ppm} ppm");
        }
    }
}

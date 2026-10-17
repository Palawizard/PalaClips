use crate::config::SYNC_OFFSET_RANGE_MS;

pub const FLASH_LEVEL: f32 = 128.0;
pub const MIN_PAIRS: usize = 5;
pub const MAX_SPREAD_MS: f64 = 20.0;
const BEEP_FLOOR: f32 = 0.01;
const BEEP_SHARE_OF_PEAK: f32 = 0.3;
const QUIET_BEFORE_BEEP_S: f64 = 0.1;
const PAIR_WINDOW_S: std::ops::Range<f64> = -0.2..0.8;

pub fn flash_times(brightness: &[(f64, f32)]) -> Vec<f64> {
    brightness
        .windows(2)
        .filter(|pair| pair[1].1 > FLASH_LEVEL && pair[0].1 <= FLASH_LEVEL)
        .map(|pair| pair[1].0)
        .collect()
}

pub fn beep_times(samples: &[f32], start_s: f64, rate: u32) -> Vec<f64> {
    let peak = samples
        .iter()
        .fold(0.0f32, |peak, value| peak.max(value.abs()));
    let threshold = BEEP_FLOOR.max(BEEP_SHARE_OF_PEAK * peak);
    let quiet_needed = (QUIET_BEFORE_BEEP_S * f64::from(rate)) as usize;
    let mut quiet = quiet_needed;
    let mut beeps = Vec::new();
    for (index, value) in samples.iter().enumerate() {
        if value.abs() > threshold {
            if quiet >= quiet_needed {
                beeps.push(start_s + index as f64 / f64::from(rate));
            }
            quiet = 0;
        } else {
            quiet += 1;
        }
    }
    beeps
}

pub fn pair_offsets_ms(flashes: &[f64], beeps: &[f64]) -> Vec<f64> {
    flashes
        .iter()
        .filter_map(|flash| {
            beeps
                .iter()
                .find(|beep| PAIR_WINDOW_S.contains(&(*beep - flash)))
                .map(|beep| (beep - flash) * 1_000.0)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct Measurement {
    pub pairs: usize,
    pub late_ms: f64,
    pub spread_ms: f64,
}

pub fn measure(offsets_ms: &[f64]) -> Option<Measurement> {
    if offsets_ms.len() < MIN_PAIRS {
        return None;
    }
    let late_ms = median(offsets_ms);
    let deviations: Vec<f64> = offsets_ms
        .iter()
        .map(|offset| (offset - late_ms).abs())
        .collect();
    let spread_ms = median(&deviations);
    (spread_ms <= MAX_SPREAD_MS).then_some(Measurement {
        pairs: offsets_ms.len(),
        late_ms,
        spread_ms,
    })
}

pub fn proposed_offset_ms(current_ms: i32, late_ms: f64) -> i32 {
    (f64::from(current_ms) - late_ms).round().clamp(
        f64::from(*SYNC_OFFSET_RANGE_MS.start()),
        f64::from(*SYNC_OFFSET_RANGE_MS.end()),
    ) as i32
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beep_track(rate: u32, seconds: usize, starts_s: &[f64]) -> Vec<f32> {
        let mut samples = vec![0.0f32; rate as usize * seconds];
        for start in starts_s {
            let first = (start * f64::from(rate)) as usize;
            for (offset, sample) in samples[first..first + rate as usize / 20]
                .iter_mut()
                .enumerate()
            {
                *sample = 0.5 * (offset as f32 * 0.13).sin().signum();
            }
        }
        samples
    }

    #[test]
    fn flashes_are_rising_edges_above_mid_grey() {
        let frames = [
            (0.0, 10.0),
            (0.5, 200.0),
            (0.6, 210.0),
            (0.7, 20.0),
            (1.5, 129.0),
        ];
        assert_eq!(flash_times(&frames), [0.5, 1.5]);
    }

    #[test]
    fn beeps_need_silence_before_them() {
        let samples = beep_track(48_000, 4, &[1.0, 2.04, 3.0]);
        let beeps = beep_times(&samples, 10.0, 48_000);
        assert_eq!(beeps.len(), 3);
        assert!((beeps[0] - 11.0).abs() < 0.001);
        assert!((beeps[2] - 13.0).abs() < 0.001);
    }

    #[test]
    fn silence_has_no_beeps() {
        assert!(beep_times(&[0.0; 48_000], 0.0, 48_000).is_empty());
    }

    #[test]
    fn each_flash_pairs_with_the_next_beep() {
        let offsets = pair_offsets_ms(&[1.0, 2.0, 3.0], &[1.12, 2.121, 9.0]);
        assert_eq!(offsets.len(), 2);
        assert!((offsets[0] - 120.0).abs() < 0.01);
    }

    #[test]
    fn measurement_uses_the_median_and_rejects_noise() {
        let steady = measure(&[118.0, 120.0, 121.0, 119.0, 400.0, 120.0]).expect("steady");
        assert_eq!(steady.late_ms, 120.0);
        assert!(measure(&[120.0, 121.0]).is_none());
        assert!(measure(&[0.0, 100.0, 200.0, 300.0, 400.0]).is_none());
    }

    #[test]
    fn late_sound_is_pulled_earlier_within_bounds() {
        assert_eq!(proposed_offset_ms(0, 120.4), -120);
        assert_eq!(proposed_offset_ms(-100, 20.0), -120);
        assert_eq!(proposed_offset_ms(0, -35.0), 35);
        assert_eq!(proposed_offset_ms(-900, 300.0), -1_000);
    }
}

use std::ops::{Add, Sub};

use serde::{Deserialize, Serialize};

pub const HNS_PER_SECOND: i64 = 10_000_000;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct Hns(pub i64);

impl Hns {
    pub const ZERO: Hns = Hns(0);

    pub fn from_secs_f64(secs: f64) -> Self {
        Hns((secs * HNS_PER_SECOND as f64).round() as i64)
    }

    pub const fn from_millis(ms: i64) -> Self {
        Hns(ms * 10_000)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / HNS_PER_SECOND as f64
    }

    pub fn to_timebase(self, num: i64, den: i64) -> i64 {
        debug_assert!(num > 0 && den > 0);
        let value = self.0 as i128 * den as i128;
        let divisor = HNS_PER_SECOND as i128 * num as i128;
        let rounded = if value >= 0 {
            (value + divisor / 2) / divisor
        } else {
            (value - divisor / 2) / divisor
        };
        rounded as i64
    }

    pub fn to_samples(self, sample_rate: u32) -> i64 {
        self.to_timebase(1, sample_rate as i64)
    }
}

impl Add for Hns {
    type Output = Hns;
    fn add(self, rhs: Hns) -> Hns {
        Hns(self.0 + rhs.0)
    }
}

impl Sub for Hns {
    type Output = Hns;
    fn sub(self, rhs: Hns) -> Hns {
        Hns(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_round_trip() {
        let t = Hns::from_secs_f64(1.5);
        assert_eq!(t, Hns(15_000_000));
        assert!((t.as_secs_f64() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn converts_to_48k_samples() {
        assert_eq!(Hns(HNS_PER_SECOND).to_samples(48_000), 48_000);
        assert_eq!(Hns::from_millis(10).to_samples(48_000), 480);
    }

    #[test]
    fn converts_to_60fps_timebase() {
        assert_eq!(Hns(HNS_PER_SECOND).to_timebase(1, 60), 60);
        assert_eq!(Hns(166_667).to_timebase(1, 60), 1);
    }

    #[test]
    fn does_not_overflow_over_a_day() {
        let day = Hns(HNS_PER_SECOND * 86_400);
        assert_eq!(day.to_timebase(1, 90_000), 90_000 * 86_400);
    }

    #[test]
    fn rounds_negative_values_away_from_zero() {
        assert_eq!(Hns(-15_000_000).to_timebase(1, 1), -2);
        assert_eq!(Hns(-14_999_999).to_timebase(1, 1), -1);
    }
}

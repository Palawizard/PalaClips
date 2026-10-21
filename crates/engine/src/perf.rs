use std::collections::VecDeque;
use std::time::Instant;

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

use crate::clock::Hns;

const REPEATED_WINDOW: usize = 10;
const LATE_WINDOW: usize = 60;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CaptureCounters {
    pub frames: u64,
    pub repeated: u64,
    pub late: u64,
    pub max_lateness: Hns,
}

#[derive(Debug)]
pub struct ProcessUsage {
    cores: f64,
    last: Option<(Instant, u64)>,
}

impl Default for ProcessUsage {
    fn default() -> Self {
        Self {
            cores: std::thread::available_parallelism().map_or(1, |cores| cores.get()) as f64,
            last: None,
        }
    }
}

impl ProcessUsage {
    pub fn cpu_percent(&mut self) -> Option<f64> {
        let now = Instant::now();
        let busy = process_cpu_time()?;
        let previous = self.last.replace((now, busy));
        let (then, busy_then) = previous?;
        let wall = now.duration_since(then).as_secs_f64();
        (wall > 0.0).then(|| {
            busy.saturating_sub(busy_then) as f64 / 10_000_000.0 / wall / self.cores * 100.0
        })
    }
}

fn process_cpu_time() -> Option<u64> {
    let mut times = [FILETIME::default(); 4];
    let [creation, exit, kernel, user] = &mut times;
    // SAFETY: the pseudo-handle of the current process is always valid and every pointer targets a live FILETIME.
    unsafe { GetProcessTimes(GetCurrentProcess(), creation, exit, kernel, user) }.ok()?;
    let ticks =
        |time: &FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    Some(ticks(kernel) + ticks(user))
}

#[derive(Debug, Default)]
pub struct FrameHistory {
    previous: Option<CaptureCounters>,
    seconds: VecDeque<CaptureCounters>,
    late_total: u64,
    cpu_max: f64,
}

impl FrameHistory {
    pub fn push(&mut self, counters: CaptureCounters, cpu_percent: Option<f64>) {
        let previous = self.previous.replace(counters).unwrap_or_default();
        let second = CaptureCounters {
            frames: counters.frames.saturating_sub(previous.frames),
            repeated: counters.repeated.saturating_sub(previous.repeated),
            late: counters.late.saturating_sub(previous.late),
            max_lateness: counters.max_lateness,
        };
        self.late_total = counters.late;
        self.seconds.push_back(second);
        while self.seconds.len() > LATE_WINDOW {
            self.seconds.pop_front();
        }
        self.cpu_max = self.cpu_max.max(cpu_percent.unwrap_or(0.0));
    }

    pub fn cpu_max(&self) -> f64 {
        self.cpu_max
    }

    pub fn repeated_percent(&self) -> f64 {
        let recent = self.seconds.iter().rev().take(REPEATED_WINDOW);
        let (frames, repeated) = recent.fold((0, 0), |(frames, repeated), second| {
            (frames + second.frames, repeated + second.repeated)
        });
        if frames == 0 {
            0.0
        } else {
            repeated as f64 * 100.0 / frames as f64
        }
    }

    pub fn late_last_minute(&self) -> u64 {
        self.seconds.iter().map(|second| second.late).sum()
    }

    pub fn late_total(&self) -> u64 {
        self.late_total
    }

    pub fn lag_max_ms(&self) -> f64 {
        self.previous
            .map_or(0.0, |counters| counters.max_lateness.0 as f64 / 10_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counters(frames: u64, repeated: u64, late: u64) -> CaptureCounters {
        CaptureCounters {
            frames,
            repeated,
            late,
            max_lateness: Hns::from_millis(4),
        }
    }

    #[test]
    fn repeated_frames_are_a_share_of_the_last_ten_seconds() {
        let mut history = FrameHistory::default();
        for second in 1..=30 {
            let repeated = if second <= 20 {
                second * 60
            } else {
                20 * 60 + (second - 20) * 6
            };
            history.push(counters(second * 60, repeated, 0), Some(1.0));
        }
        assert!((history.repeated_percent() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn late_frames_count_since_arming_and_over_the_last_minute() {
        let mut history = FrameHistory::default();
        history.push(counters(60, 0, 5), None);
        for second in 2..=70 {
            history.push(counters(second * 60, 0, 5), None);
        }
        assert_eq!(history.late_total(), 5);
        assert_eq!(history.late_last_minute(), 0);
        history.push(counters(71 * 60, 0, 8), None);
        assert_eq!(history.late_total(), 8);
        assert_eq!(history.late_last_minute(), 3);
    }

    #[test]
    fn the_cpu_maximum_is_kept_since_arming() {
        let mut history = FrameHistory::default();
        for cpu in [0.4, 2.5, 0.8] {
            history.push(counters(60, 0, 0), Some(cpu));
        }
        assert_eq!(history.cpu_max(), 2.5);
        assert_eq!(history.lag_max_ms(), 4.0);
    }

    #[test]
    fn machine_cpu_is_measured_between_two_samples() {
        let mut usage = ProcessUsage::default();
        assert!(usage.cpu_percent().is_none());
        let start = Instant::now();
        while start.elapsed().as_millis() < 120 {
            std::hint::black_box((0..1_000).sum::<u64>());
        }
        let cpu = usage.cpu_percent().expect("second sample");
        assert!(cpu > 0.0 && cpu <= 100.0, "{cpu}");
    }
}

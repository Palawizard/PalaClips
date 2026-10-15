use std::ops::Range;

use crate::clock::{HNS_PER_SECOND, Hns};

#[derive(Debug, Clone)]
pub struct CfrClock {
    origin: Hns,
    fps: u32,
    next: i64,
}

impl CfrClock {
    pub fn new(origin: Hns, fps: u32) -> Self {
        Self {
            origin,
            fps,
            next: 0,
        }
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn origin(&self) -> Hns {
        self.origin
    }

    pub fn tick_time(&self, index: i64) -> Hns {
        let offset = index as i128 * HNS_PER_SECOND as i128 / self.fps as i128;
        Hns(self.origin.0 + offset as i64)
    }

    pub fn next_deadline(&self) -> Hns {
        self.tick_time(self.next)
    }

    pub fn take_due(&mut self, now: Hns) -> Range<i64> {
        let start = self.next;
        if now >= self.origin {
            let elapsed = (now - self.origin).0 as i128;
            let last_due = (((elapsed + 1) * self.fps as i128 - 1) / HNS_PER_SECOND as i128) as i64;
            self.next = self.next.max(last_due + 1);
        }
        start..self.next
    }

    pub fn emitted(&self) -> i64 {
        self.next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Hns = Hns(1_000_000_000);

    #[test]
    fn first_tick_is_due_at_origin() {
        let mut clock = CfrClock::new(ORIGIN, 60);
        assert_eq!(clock.take_due(ORIGIN), 0..1);
        assert_eq!(clock.take_due(ORIGIN), 1..1);
    }

    #[test]
    fn nothing_is_due_before_origin_or_before_next_tick() {
        let mut clock = CfrClock::new(ORIGIN, 60);
        assert!(clock.take_due(Hns(ORIGIN.0 - 1)).is_empty());
        clock.take_due(ORIGIN);
        assert!(clock.take_due(Hns(clock.next_deadline().0 - 1)).is_empty());
    }

    #[test]
    fn a_tick_is_due_exactly_at_its_deadline() {
        for fps in [30, 60, 144, 165, 240] {
            let mut clock = CfrClock::new(ORIGIN, fps);
            for index in 0..1_000 {
                let deadline = clock.next_deadline();
                assert!(clock.take_due(Hns(deadline.0 - 1)).is_empty());
                assert_eq!(clock.take_due(deadline), index..index + 1);
            }
        }
    }

    #[test]
    fn late_wakeup_emits_every_missed_tick() {
        let mut clock = CfrClock::new(ORIGIN, 60);
        clock.take_due(ORIGIN);
        let late = clock.tick_time(4);
        assert_eq!(clock.take_due(late), 1..5);
    }

    #[test]
    fn ticks_follow_exact_rational_times_without_drift() {
        let clock = CfrClock::new(ORIGIN, 60);
        assert_eq!(clock.tick_time(60), ORIGIN + Hns(HNS_PER_SECOND));
        assert_eq!(
            clock.tick_time(60 * 3_600),
            ORIGIN + Hns(HNS_PER_SECOND * 3_600)
        );
        let clock = CfrClock::new(ORIGIN, 144);
        assert_eq!(clock.tick_time(144 * 60), ORIGIN + Hns(HNS_PER_SECOND * 60));
    }

    #[test]
    fn ten_seconds_at_60_fps_emit_600_frames() {
        let mut clock = CfrClock::new(ORIGIN, 60);
        let mut emitted = 0;
        let mut now = ORIGIN;
        while now < ORIGIN + Hns(10 * HNS_PER_SECOND) {
            emitted += clock.take_due(now).count();
            now = now + Hns(12_345);
        }
        assert_eq!(emitted, 600);
    }
}

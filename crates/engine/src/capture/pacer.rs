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

pub fn bounded_due(due: Range<i64>, keep: usize) -> (Range<i64>, u64) {
    let len = (due.end - due.start).max(0) as u64;
    match len.checked_sub(keep as u64) {
        Some(dropped) if dropped > 0 => (due.end - keep as i64..due.end, dropped),
        _ => (due, 0),
    }
}

pub fn emit_due<E>(
    due: Range<i64>,
    mut emit: impl FnMut(i64, bool) -> Result<(), E>,
) -> Result<(), E> {
    let newest = due.end - 1;
    for index in due {
        emit(index, index == newest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGIN: Hns = Hns(1_000_000_000);

    #[test]
    fn a_short_backlog_is_kept_whole() {
        assert_eq!(bounded_due(10..13, 30), (10..13, 0));
        assert_eq!(bounded_due(10..40, 30), (10..40, 0));
        assert_eq!(bounded_due(5..5, 30), (5..5, 0));
    }

    #[test]
    fn a_long_backlog_keeps_only_the_newest_ticks() {
        assert_eq!(bounded_due(0..100, 30), (70..100, 70));
        assert_eq!(bounded_due(0..100, usize::MAX), (0..100, 0));
    }

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

    fn run_encoder(fresh_cost: Hns, repeat_cost: Hns) -> (Vec<i64>, Vec<i64>, Hns) {
        let period = Hns(HNS_PER_SECOND / 60);
        let mut clock = CfrClock::new(ORIGIN, 60);
        let mut now = ORIGIN;
        let mut emitted = Vec::new();
        let mut fresh = Vec::new();
        let mut max_lateness = Hns::ZERO;
        while now < ORIGIN + Hns(10 * HNS_PER_SECOND) {
            let due = clock.take_due(now);
            if due.is_empty() {
                now = clock.next_deadline();
                continue;
            }
            max_lateness = max_lateness.max(now - clock.tick_time(due.start));
            let captured_at = now;
            let result: Result<(), ()> = emit_due(due, |index, newest| {
                emitted.push(index);
                if newest {
                    assert!(captured_at - clock.tick_time(index) < period);
                    fresh.push(index);
                    now = now + fresh_cost;
                } else {
                    now = now + repeat_cost;
                }
                Ok(())
            });
            assert!(result.is_ok());
        }
        (emitted, fresh, max_lateness)
    }

    #[test]
    fn a_slow_encoder_repeats_frames_instead_of_falling_behind() {
        let period = Hns(HNS_PER_SECOND / 60);
        let (emitted, fresh, max_lateness) = run_encoder(Hns(period.0 * 3), Hns(period.0 / 20));
        assert_eq!(emitted, (0..emitted.len() as i64).collect::<Vec<_>>());
        assert!(emitted.len() >= 599);
        assert!(max_lateness < Hns(period.0 * 4), "{max_lateness:?}");
        assert!(fresh.len() < emitted.len() / 2);
    }

    #[test]
    fn an_encoder_that_keeps_up_gets_a_fresh_image_on_every_tick() {
        let period = Hns(HNS_PER_SECOND / 60);
        let (emitted, fresh, max_lateness) = run_encoder(Hns(period.0 / 4), Hns(period.0 / 20));
        assert_eq!(emitted.len(), 600);
        assert_eq!(fresh, emitted);
        assert_eq!(max_lateness, Hns::ZERO);
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

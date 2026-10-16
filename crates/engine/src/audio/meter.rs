use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, Default)]
pub struct PeakMeter(Arc<AtomicU32>);

impl PeakMeter {
    pub fn record(&self, peak: f32) {
        self.0.fetch_max(peak.abs().to_bits(), Ordering::Relaxed);
    }

    pub fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0, Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_loudest_peak_until_read() {
        let meter = PeakMeter::default();
        meter.record(0.25);
        meter.record(-0.75);
        meter.record(0.5);
        assert_eq!(meter.clone().take(), 0.75);
        assert_eq!(meter.take(), 0.0);
    }
}

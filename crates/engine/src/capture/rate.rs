use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};

use crate::clock::Hns;
use crate::events::{EngineEvent, Localized, Severity};

const TOLERANCE: f64 = 0.98;
const SUSTAINED: Hns = Hns::from_millis(5_000);

pub fn composition_rate() -> Option<f64> {
    let mut timing = DWM_TIMING_INFO {
        cbSize: size_of::<DWM_TIMING_INFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `timing` is a valid DWM_TIMING_INFO with cbSize set, as the API requires.
    unsafe { DwmGetCompositionTimingInfo(HWND::default(), &mut timing) }.ok()?;
    let rate = timing.rateRefresh;
    (rate.uiDenominator > 0 && rate.uiNumerator > 0)
        .then(|| f64::from(rate.uiNumerator) / f64::from(rate.uiDenominator))
}

#[derive(Debug)]
pub struct RateWatch {
    fps: u32,
    below_since: Option<Hns>,
    alerted: Option<u32>,
}

impl RateWatch {
    pub fn new(fps: u32) -> Self {
        Self {
            fps,
            below_since: None,
            alerted: None,
        }
    }

    pub fn observe(&mut self, now: Hns, rate: f64) -> Option<EngineEvent> {
        if rate >= f64::from(self.fps) * TOLERANCE {
            self.below_since = None;
            return None;
        }
        let since = *self.below_since.get_or_insert(now);
        let hz = rate.round() as u32;
        if now - since < SUSTAINED || self.alerted == Some(hz) {
            return None;
        }
        self.alerted = Some(hz);
        let fps = self.fps;
        tracing::warn!(
            composition_hz = rate,
            fps,
            "Windows compose l'écran moins vite que le fps demandé"
        );
        Some(EngineEvent::Alert {
            severity: Severity::Warning,
            text: Localized::new("capture_source_rate_limited")
                .with("rate", hz)
                .with("fps", fps),
            message: format!(
                "Windows ne compose ton écran qu'à {hz} Hz alors que tu enregistres à {fps} i/s : la vidéo répétera des images. Désactive G-Sync pour cet écran, ou passe tes autres écrans à au moins {fps} Hz."
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(value: i64) -> Hns {
        Hns::from_millis(value * 1_000)
    }

    fn alerts(watch: &mut RateWatch, samples: &[(i64, f64)]) -> usize {
        samples
            .iter()
            .filter(|(at, rate)| watch.observe(seconds(*at), *rate).is_some())
            .count()
    }

    #[test]
    fn a_composition_rate_below_the_fps_for_five_seconds_alerts_once() {
        let mut watch = RateWatch::new(240);
        let samples: Vec<(i64, f64)> = (0..=20).map(|at| (at, 164.9)).collect();
        assert_eq!(alerts(&mut watch, &samples[..5]), 0);
        assert_eq!(alerts(&mut watch, &samples[5..]), 1);
    }

    #[test]
    fn a_rate_at_the_fps_or_a_short_dip_never_alerts() {
        let mut watch = RateWatch::new(240);
        let mut samples: Vec<(i64, f64)> = (0..30).map(|at| (at, 239.8)).collect();
        samples.extend((30..34).map(|at| (at, 120.0)));
        samples.extend((34..60).map(|at| (at, 240.0)));
        assert_eq!(alerts(&mut watch, &samples), 0);
    }

    #[test]
    fn the_alert_comes_back_only_when_the_rate_changes() {
        let mut watch = RateWatch::new(240);
        let mut samples: Vec<(i64, f64)> = (0..10).map(|at| (at, 165.0)).collect();
        samples.extend((10..12).map(|at| (at, 240.0)));
        samples.extend((12..20).map(|at| (at, 164.8)));
        samples.extend((20..30).map(|at| (at, 60.0)));
        assert_eq!(alerts(&mut watch, &samples), 2);
    }

    #[test]
    fn the_alert_gives_the_real_rate_and_the_requested_fps() {
        let mut watch = RateWatch::new(240);
        let alert = (0..=5)
            .filter_map(|at| watch.observe(seconds(at), 164.9))
            .next();
        let Some(EngineEvent::Alert { text, .. }) = alert else {
            panic!("alerte attendue");
        };
        assert_eq!(text.code, "capture_source_rate_limited");
        assert_eq!(
            text.params.get("rate").map(ToString::to_string).as_deref(),
            Some("165")
        );
    }

    #[test]
    fn the_composition_rate_is_readable() {
        assert!(composition_rate().is_some_and(|rate| rate > 20.0));
    }
}

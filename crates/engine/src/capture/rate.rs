use std::collections::VecDeque;

use windows::Wdk::Graphics::Direct3D::D3DKMTCheckExclusiveOwnership;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWM_TIMING_INFO, DwmGetCompositionTimingInfo};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};

use crate::clock::Hns;

use crate::events::{
    ALERT_CPU_ENCODER_OVERLOADED as CPU_ENCODER_OVERLOADED,
    ALERT_EXCLUSIVE_FULLSCREEN as EXCLUSIVE_FULLSCREEN, ALERT_GPU_SATURATED as GPU_SATURATED,
    ALERT_RATE_LIMITED as RATE_LIMITED, EngineEvent, Localized, Severity,
};

const TOLERANCE: f64 = 0.98;
const SUSTAINED: Hns = Hns::from_millis(5_000);
const LATE_SHARE: f64 = 0.1;
const FREEZE_WINDOW: usize = 5;
const MINECRAFT_JAVA: [&str; 2] = ["javaw.exe", "java.exe"];

pub fn exclusive_fullscreen_advice(game: Option<&str>) -> (Localized, &'static str) {
    if game.is_some_and(|exe| {
        MINECRAFT_JAVA
            .iter()
            .any(|java| exe.eq_ignore_ascii_case(java))
    }) {
        (
            Localized::new("exclusive_fullscreen_advice_minecraft"),
            "Minecraft Java : quitte le plein écran (F11) et agrandis la fenêtre, ou installe un mod de plein écran sans bordure comme Borderless Fullscreen.",
        )
    } else {
        (
            Localized::new("exclusive_fullscreen_advice"),
            "Passe le jeu en plein écran fenêtré (ou sans bordure) dans ses options d'affichage.",
        )
    }
}

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
struct Sustained {
    raise_after: Hns,
    last: Option<Hns>,
    bad_for: Hns,
    good_for: Hns,
    active: bool,
}

impl Default for Sustained {
    fn default() -> Self {
        Self {
            raise_after: SUSTAINED,
            last: None,
            bad_for: Hns::ZERO,
            good_for: Hns::ZERO,
            active: false,
        }
    }
}

impl Sustained {
    fn observe(&mut self, now: Hns, bad: Option<bool>) -> Option<bool> {
        let elapsed = self.last.map_or(Hns::ZERO, |last| now - last);
        self.last = Some(now);
        match bad? {
            true => {
                self.good_for = Hns::ZERO;
                self.bad_for = self.bad_for + elapsed;
                (!self.active && self.bad_for >= self.raise_after).then(|| {
                    self.active = true;
                    true
                })
            }
            false => {
                self.bad_for = Hns::ZERO;
                self.good_for = self.good_for + elapsed;
                (self.active && self.good_for >= SUSTAINED).then(|| {
                    self.active = false;
                    false
                })
            }
        }
    }
}

#[derive(Debug)]
pub struct RateWatch {
    fps: u32,
    sustained: Sustained,
    alerted: Option<u32>,
}

impl RateWatch {
    pub fn new(fps: u32) -> Self {
        Self {
            fps,
            sustained: Sustained::default(),
            alerted: None,
        }
    }

    pub fn observe(&mut self, now: Hns, rate: f64, new_frames: u64) -> Option<EngineEvent> {
        let below = rate < f64::from(self.fps) * TOLERANCE;
        let hz = rate.round() as u32;
        match self
            .sustained
            .observe(now, (new_frames > 0).then_some(below))
        {
            Some(false) => {
                self.alerted = None;
                tracing::info!(composition_hz = rate, "cadence de composition revenue");
                return Some(EngineEvent::AlertCleared { code: RATE_LIMITED });
            }
            Some(true) => {}
            None if self.sustained.active && below && self.alerted != Some(hz) => {}
            None => return None,
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
            text: Localized::new(RATE_LIMITED)
                .with("rate", hz)
                .with("fps", fps),
            message: format!(
                "Windows ne compose ton écran qu'à {hz} Hz alors que tu enregistres à {fps} i/s : la vidéo répétera des images. Désactive G-Sync pour cet écran, ou passe tes autres écrans à au moins {fps} Hz."
            ),
        })
    }
}

#[derive(Debug, Default)]
pub struct SaturationWatch {
    sustained: Sustained,
    cpu_encoder: bool,
}

impl SaturationWatch {
    pub fn cpu_encoder() -> Self {
        Self {
            cpu_encoder: true,
            ..Self::default()
        }
    }

    pub fn observe(&mut self, now: Hns, frames: u64, late: u64) -> Option<EngineEvent> {
        let starved = (frames > 0).then_some(late as f64 >= frames as f64 * LATE_SHARE);
        let code = if self.cpu_encoder {
            CPU_ENCODER_OVERLOADED
        } else {
            GPU_SATURATED
        };
        match self.sustained.observe(now, starved)? {
            true if self.cpu_encoder => {
                tracing::warn!(frames, late, "l'encodage par le processeur ne suit pas");
                Some(EngineEvent::Alert {
                    severity: Severity::Warning,
                    text: Localized::new(code),
                    message: "Ton processeur n'arrive pas à encoder la vidéo à temps : des images sont répétées ou sautées. Baisse la résolution ou les images par seconde dans Réglages > Replay & qualité (720p30 est le plus léger).".into(),
                })
            }
            true => {
                tracing::warn!(frames, late, "capture en retard : GPU saturé");
                Some(EngineEvent::Alert {
                    severity: Severity::Warning,
                    text: Localized::new(code),
                    message: "Ton GPU est à fond : PalaClips n'obtient plus son tour et la vidéo saute. Limite les fps du jeu pour laisser un peu de marge, ou choisis « Priorité à la capture » dans Réglages > Replay & qualité.".into(),
                })
            }
            false => {
                tracing::info!("capture de nouveau à l'heure");
                Some(EngineEvent::AlertCleared { code })
            }
        }
    }
}

pub fn exclusive_fullscreen() -> bool {
    // SAFETY: D3DKMTCheckExclusiveOwnership takes no arguments and only reads kernel state.
    let owned = unsafe { D3DKMTCheckExclusiveOwnership() };
    // SAFETY: SHQueryUserNotificationState has no preconditions.
    owned
        || unsafe { SHQueryUserNotificationState() }
            .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

#[derive(Debug)]
pub struct FreezeWatch {
    recent: VecDeque<u64>,
    sustained: Sustained,
}

impl Default for FreezeWatch {
    fn default() -> Self {
        Self {
            recent: VecDeque::with_capacity(FREEZE_WINDOW),
            sustained: Sustained {
                raise_after: Hns::ZERO,
                ..Sustained::default()
            },
        }
    }
}

impl FreezeWatch {
    pub fn observe(
        &mut self,
        now: Hns,
        new_frames: u64,
        exclusive: impl FnOnce() -> bool,
    ) -> Option<EngineEvent> {
        if self.recent.len() == FREEZE_WINDOW {
            self.recent.pop_front();
        }
        self.recent.push_back(new_frames);
        let starved = self.recent.len() == FREEZE_WINDOW
            && self.recent.iter().sum::<u64>() < FREEZE_WINDOW as u64;
        let frozen = if starved {
            exclusive().then_some(true)
        } else {
            Some(false)
        };
        match self.sustained.observe(now, frozen)? {
            true => {
                let game = crate::sources::foreground_window().map(|window| window.exe_name);
                tracing::warn!(?game, "capture figée : plein écran exclusif");
                let (advice, advice_message) = exclusive_fullscreen_advice(game.as_deref());
                Some(EngineEvent::Alert {
                    severity: Severity::Warning,
                    text: Localized::new(EXCLUSIVE_FULLSCREEN).with("advice", advice),
                    message: format!(
                        "Le jeu est en plein écran exclusif : Windows ne transmet plus son image et la vidéo reste figée. {advice_message}"
                    ),
                })
            }
            false => {
                tracing::info!("capture de nouveau alimentée");
                Some(EngineEvent::AlertCleared {
                    code: EXCLUSIVE_FULLSCREEN,
                })
            }
        }
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
            .filter(|(at, rate)| {
                matches!(
                    watch.observe(seconds(*at), *rate, 165),
                    Some(EngineEvent::Alert { .. })
                )
            })
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
    fn a_still_screen_never_alerts_even_when_windows_slows_its_refresh() {
        let mut watch = RateWatch::new(240);
        let still = (0..60).filter(|at| watch.observe(seconds(*at), 60.0, 0).is_some());
        assert_eq!(still.count(), 0);
    }

    #[test]
    fn only_seconds_with_new_frames_count_toward_the_alert() {
        let mut watch = RateWatch::new(240);
        let mut alerted = 0;
        for at in 0..40 {
            let new_frames = if at % 2 == 0 { 60 } else { 0 };
            alerted += usize::from(watch.observe(seconds(at), 60.0, new_frames).is_some());
            if at < 10 {
                assert_eq!(alerted, 0, "{at} s");
            }
        }
        assert_eq!(alerted, 1);
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
    fn the_alert_is_withdrawn_once_the_rate_is_back_for_five_seconds() {
        let mut watch = RateWatch::new(240);
        let events: Vec<EngineEvent> = (0..30)
            .filter_map(|at| {
                let rate = if at < 10 { 165.0 } else { 240.0 };
                watch.observe(seconds(at), rate, 165)
            })
            .collect();
        assert!(matches!(events[0], EngineEvent::Alert { .. }));
        assert!(matches!(
            events[1],
            EngineEvent::AlertCleared { code: RATE_LIMITED }
        ));
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn the_alert_gives_the_real_rate_and_the_requested_fps() {
        let mut watch = RateWatch::new(240);
        let alert = (0..=5)
            .filter_map(|at| watch.observe(seconds(at), 164.9, 165))
            .next();
        let Some(EngineEvent::Alert { text, .. }) = alert else {
            panic!("alerte attendue");
        };
        assert_eq!(text.code, RATE_LIMITED);
        assert_eq!(
            text.params.get("rate").map(ToString::to_string).as_deref(),
            Some("165")
        );
    }

    #[test]
    fn late_frames_for_five_seconds_raise_the_saturation_alert_then_withdraw_it() {
        let mut watch = SaturationWatch::default();
        let mut events = Vec::new();
        for at in 0..40 {
            let late = match at {
                0..10 => 0,
                10..25 => 30,
                _ => 1,
            };
            if let Some(event) = watch.observe(seconds(at), 60, late) {
                events.push((at, event));
            }
        }
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], (14, EngineEvent::Alert { .. })));
        assert!(matches!(
            events[1],
            (
                29,
                EngineEvent::AlertCleared {
                    code: GPU_SATURATED
                }
            )
        ));
    }

    #[test]
    fn a_cpu_encoder_that_falls_behind_raises_its_own_alert() {
        let mut watch = SaturationWatch::cpu_encoder();
        let codes: Vec<_> = (0..30)
            .filter_map(|at| watch.observe(seconds(at), 30, if at < 15 { 20 } else { 0 }))
            .map(|event| match event {
                EngineEvent::Alert { text, .. } => text.code,
                EngineEvent::AlertCleared { code } => code,
                _ => "",
            })
            .collect();
        assert_eq!(codes, [CPU_ENCODER_OVERLOADED, CPU_ENCODER_OVERLOADED]);
    }

    #[test]
    fn a_few_late_frames_or_a_short_stall_never_raise_the_saturation_alert() {
        let mut watch = SaturationWatch::default();
        let raised = (0..60).filter(|at| {
            let late = if (20..23).contains(at) { 60 } else { 3 };
            watch.observe(seconds(*at), 60, late).is_some()
        });
        assert_eq!(raised.count(), 0);
    }

    #[test]
    fn a_still_screen_without_exclusive_fullscreen_never_alerts() {
        let mut watch = FreezeWatch::default();
        let raised = (0..60).filter(|at| watch.observe(seconds(*at), 0, || false).is_some());
        assert_eq!(raised.count(), 0);
    }

    #[test]
    fn a_frozen_capture_under_exclusive_fullscreen_alerts_then_clears_when_frames_return() {
        let mut watch = FreezeWatch::default();
        let mut events = Vec::new();
        for at in 0..40 {
            let new_frames = if (10..25).contains(&at) { 0 } else { 60 };
            if let Some(event) = watch.observe(seconds(at), new_frames, || true) {
                events.push((at, event));
            }
        }
        assert_eq!(events.len(), 2);
        assert!(
            matches!(events[0], (14, EngineEvent::Alert { ref text, .. }) if text.code == EXCLUSIVE_FULLSCREEN)
        );
        assert!(matches!(
            events[1],
            (
                29,
                EngineEvent::AlertCleared {
                    code: EXCLUSIVE_FULLSCREEN
                }
            )
        ));
    }

    #[test]
    fn minecraft_java_gets_its_own_fullscreen_advice() {
        assert_eq!(
            exclusive_fullscreen_advice(Some("javaw.exe")).0.code,
            "exclusive_fullscreen_advice_minecraft"
        );
        assert_eq!(
            exclusive_fullscreen_advice(Some("JAVA.EXE")).0.code,
            "exclusive_fullscreen_advice_minecraft"
        );
        assert_eq!(
            exclusive_fullscreen_advice(Some("FortniteClient-Win64-Shipping.exe"))
                .0
                .code,
            "exclusive_fullscreen_advice"
        );
        assert_eq!(
            exclusive_fullscreen_advice(None).0.code,
            "exclusive_fullscreen_advice"
        );
    }

    #[test]
    fn a_nearly_frozen_capture_under_exclusive_fullscreen_alerts() {
        let mut watch = FreezeWatch::default();
        let raised: Vec<i64> = (0..30)
            .filter(|at| {
                let new_frames = if at % 3 == 0 { 1 } else { 0 };
                matches!(
                    watch.observe(seconds(*at), new_frames, || true),
                    Some(EngineEvent::Alert { .. })
                )
            })
            .collect();
        assert_eq!(raised, vec![4]);
    }

    #[test]
    fn one_frame_per_second_or_more_is_not_frozen() {
        let mut watch = FreezeWatch::default();
        let raised = (0..30).filter(|at| watch.observe(seconds(*at), 1, || true).is_some());
        assert_eq!(raised.count(), 0);
    }

    #[test]
    fn exclusive_ownership_is_not_checked_while_frames_arrive() {
        let mut watch = FreezeWatch::default();
        for at in 0..10 {
            watch.observe(seconds(at), 60, || {
                panic!("vérifié alors que des images arrivent")
            });
        }
    }

    #[test]
    fn the_composition_rate_is_readable() {
        assert!(composition_rate().is_some_and(|rate| rate > 20.0));
    }
}

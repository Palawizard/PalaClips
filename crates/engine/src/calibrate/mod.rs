#[cfg(windows)]
mod decode;
#[cfg(windows)]
mod emitter;
mod measure;

use crate::config::{
    AudioSettings, CaptureTarget, EngineConfig, OutputResolution, ReplayStorage, VideoCodec,
    VideoQuality, VideoSettings,
};
use crate::events::TrackKind;

#[cfg(windows)]
pub use decode::read_signals;
#[cfg(windows)]
pub use emitter::emit_flashes;
pub use measure::{
    Measurement, beep_times, flash_times, measure, pair_offsets_ms, proposed_offset_ms,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AudioSignal {
    pub start_s: f64,
    pub rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipSignals {
    pub brightness: Vec<(f64, f32)>,
    pub tracks: Vec<AudioSignal>,
}

pub const EMIT_SECONDS: u32 = 10;
const CALIBRATION_HEIGHT: u32 = 360;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct TrackMeasurement {
    pub kind: TrackKind,
    pub title: String,
    pub measurement: Option<Measurement>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AppProposal {
    pub exe_name: String,
    pub offset_ms: i32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Calibration {
    pub tracks: Vec<TrackMeasurement>,
    pub mix_ms: Option<i32>,
    pub microphone_ms: Option<i32>,
    pub game_ms: Option<i32>,
    pub apps: Vec<AppProposal>,
}

pub fn calibration_config(config: &EngineConfig, own_exe: &str) -> EngineConfig {
    let mut calibration = config.clone();
    calibration.capture = CaptureTarget::PrimaryMonitor;
    calibration.video = VideoSettings {
        fps: 60,
        resolution: OutputResolution::Height {
            height: CALIBRATION_HEIGHT,
        },
        codec: VideoCodec::H264,
        quality: VideoQuality::Light,
        capture_cursor: false,
        draw_border: false,
        ..config.video.clone()
    };
    calibration.audio.app_tracks.clear();
    calibration.audio.auto_game_track = true;
    calibration.audio.game_exe = Some(own_exe.to_owned());
    calibration.replay.storage = ReplayStorage::Ram;
    calibration.replay.duration_secs = 30;
    calibration
}

pub fn proposals(audio: &AudioSettings, tracks: Vec<TrackMeasurement>) -> Calibration {
    let late = |kind: TrackKind| {
        tracks
            .iter()
            .find(|track| track.kind == kind)
            .and_then(|track| track.measurement)
            .map(|measurement| measurement.late_ms)
    };
    let game_ms =
        late(TrackKind::Game).map(|late_ms| proposed_offset_ms(audio.game_sync_offset_ms, late_ms));
    Calibration {
        mix_ms: late(TrackKind::Mix)
            .map(|late_ms| proposed_offset_ms(audio.full_mix_sync_offset_ms, late_ms)),
        microphone_ms: late(TrackKind::Microphone)
            .map(|late_ms| proposed_offset_ms(audio.microphone_sync_offset_ms, late_ms)),
        game_ms,
        apps: game_ms
            .map(|offset_ms| {
                audio
                    .app_tracks
                    .iter()
                    .map(|track| AppProposal {
                        exe_name: track.exe_name.clone(),
                        offset_ms,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        tracks,
    }
}

pub fn measure_tracks(signals: &ClipSignals) -> Vec<Option<Measurement>> {
    let flashes = flash_times(&signals.brightness);
    signals
        .tracks
        .iter()
        .map(|track| {
            measure(&pair_offsets_ms(
                &flashes,
                &beep_times(&track.samples, track.start_s, track.rate),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flashing(seconds: usize, fps: usize) -> Vec<(f64, f32)> {
        (0..seconds * fps)
            .map(|frame| {
                let at = frame as f64 / fps as f64;
                let lit = frame % fps < fps / 20;
                (at, if lit { 250.0 } else { 5.0 })
            })
            .collect()
    }

    fn beeping(seconds: usize, late_s: f64) -> AudioSignal {
        let rate = 48_000;
        let mut samples = vec![0.0f32; rate * seconds];
        for second in 1..seconds {
            let first = ((second as f64 + late_s) * rate as f64) as usize;
            for sample in samples.iter_mut().skip(first).take(rate / 20) {
                *sample = 0.4;
            }
        }
        AudioSignal {
            start_s: 0.0,
            rate: rate as u32,
            samples,
        }
    }

    fn measured(kind: TrackKind, late_ms: f64) -> TrackMeasurement {
        TrackMeasurement {
            kind,
            title: String::new(),
            measurement: Some(Measurement {
                pairs: 9,
                late_ms,
                spread_ms: 2.0,
            }),
        }
    }

    #[test]
    fn calibration_captures_the_primary_screen_cheaply_and_follows_palaclips() {
        let mut config = EngineConfig {
            capture: CaptureTarget::Window {
                exe_name: "game.exe".into(),
                title_hint: None,
            },
            ..EngineConfig::default()
        };
        config.audio.app_tracks.push(crate::config::AppTrack {
            exe_name: "discord.exe".into(),
            label: "Discord".into(),
            enabled: true,
            sync_offset_ms: 0,
        });
        config.audio.full_mix_sync_offset_ms = -80;
        let calibration = calibration_config(&config, "palaclips-app.exe");
        assert_eq!(calibration.capture, CaptureTarget::PrimaryMonitor);
        assert_eq!(calibration.video.codec, VideoCodec::H264);
        assert!(calibration.audio.app_tracks.is_empty());
        assert_eq!(
            calibration.audio.game_exe.as_deref(),
            Some("palaclips-app.exe")
        );
        assert_eq!(calibration.audio.full_mix_sync_offset_ms, -80);
        calibration.validate().expect("valid calibration config");
    }

    #[test]
    fn apps_share_the_process_loopback_measurement_of_the_game_track() {
        let mut audio = AudioSettings {
            full_mix_sync_offset_ms: -50,
            game_sync_offset_ms: 10,
            ..AudioSettings::default()
        };
        audio.app_tracks.push(crate::config::AppTrack {
            exe_name: "discord.exe".into(),
            label: "Discord".into(),
            enabled: true,
            sync_offset_ms: -300,
        });
        let calibration = proposals(
            &audio,
            vec![
                measured(TrackKind::Mix, 70.0),
                TrackMeasurement {
                    kind: TrackKind::Microphone,
                    title: String::new(),
                    measurement: None,
                },
                measured(TrackKind::Game, 40.0),
            ],
        );
        assert_eq!(calibration.mix_ms, Some(-120));
        assert_eq!(calibration.microphone_ms, None);
        assert_eq!(calibration.game_ms, Some(-30));
        assert_eq!(calibration.apps[0].offset_ms, -30);
    }

    #[test]
    fn each_track_gets_its_own_lateness() {
        let signals = ClipSignals {
            brightness: flashing(10, 60),
            tracks: vec![beeping(10, 0.12), beeping(10, 0.0), AudioSignal::default()],
        };
        let measured = measure_tracks(&signals);
        let late = |index: usize| measured[index].map(|m| m.late_ms.round());
        assert_eq!(late(0), Some(120.0));
        assert_eq!(late(1), Some(0.0));
        assert_eq!(measured[2], None);
    }
}

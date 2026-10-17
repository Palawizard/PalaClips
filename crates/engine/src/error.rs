use thiserror::Error;

use crate::config::VideoCodec;
use crate::events::Localized;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissingTarget {
    PrimaryMonitor,
    Monitor(String),
    Window(String),
}

impl std::fmt::Display for MissingTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrimaryMonitor => write!(f, "aucun écran principal"),
            Self::Monitor(name) => write!(f, "l'écran {name} n'est pas branché"),
            Self::Window(exe) => write!(f, "aucune fenêtre de {exe} n'est ouverte"),
        }
    }
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("réglage invalide : {0}")]
    InvalidConfig(String),

    #[error(
        "aucun encodeur vidéo matériel n'a fonctionné : PalaClips encode uniquement avec la carte          graphique (NVIDIA NVENC, AMD AMF ou Intel Quick Sync). Mets à jour le pilote de ta carte          graphique puis relance PalaClips"
    )]
    NoHardwareEncoder,

    #[error("ta carte graphique ne sait pas encoder en {} : choisis un autre codec", .0.label())]
    CodecUnavailable(VideoCodec),

    #[error("la capture audio par application nécessite Windows 10 build 20348 ou plus récent")]
    UnsupportedWindowsBuild,

    #[error("cible de capture introuvable : {0}")]
    TargetNotFound(MissingTarget),

    #[error("la capture a échoué : {0}")]
    Capture(String),

    #[error(
        "sortie vidéo refusée : {} est limité à {limit} px de côté et la vidéo ferait {width}×{height} : choisis une résolution de sortie plus basse ou un autre codec",
        .codec.label()
    )]
    OutputTooLarge {
        codec: VideoCodec,
        limit: u32,
        width: u32,
        height: u32,
    },

    #[error(
        "sortie vidéo refusée : la carte graphique refuse d'encoder {width}×{height} à {fps} i/s en {} : choisis une résolution de sortie plus basse ou un autre codec",
        .codec.label()
    )]
    OutputUnsupported {
        codec: VideoCodec,
        width: u32,
        height: u32,
        fps: u32,
    },

    #[error("l'encodeur vidéo a échoué : {0}")]
    Encoder(String),

    #[error("écriture du fichier vidéo impossible : {0}")]
    Mux(String),

    #[error("la capture audio a échoué : {0}")]
    Audio(String),

    #[error("la capture audio a échoué : la piste « {0} » s'est arrêtée")]
    TrackStopped(String),

    #[error("la source de capture n'est plus disponible : la fenêtre capturée a été fermée")]
    CapturedWindowClosed,

    #[error("la source de capture n'est plus disponible : la capture vidéo s'est arrêtée")]
    VideoCaptureStopped,

    #[error("erreur disque : {0}")]
    Io(#[from] std::io::Error),

    #[error("un calibrage est déjà en cours")]
    CalibrationBusy,

    #[error("un enregistrement est en cours")]
    RecordingRunning,

    #[error("le disque des clips est plein")]
    DiskFull,

    #[error("le calibrage s'est arrêté")]
    CalibrationStopped,

    #[error("le moteur PalaClips est arrêté")]
    Stopped,

    #[error("aucun replay à sauvegarder : le replay n'est pas armé")]
    ReplayNotArmed,

    #[error("aucun replay à sauvegarder : aucun flux")]
    NoStream,

    #[error("moins de {} s en mémoire depuis le dernier clip", min_clip_secs())]
    ClipTooShort,

    #[error("fonctionnalité pas encore implémentée : {0}")]
    NotImplemented(&'static str),
}

fn min_clip_secs() -> i64 {
    crate::replay::MIN_CLIP.0 / crate::clock::HNS_PER_SECOND
}

impl EngineError {
    pub fn localized(&self) -> Localized {
        match self {
            Self::InvalidConfig(detail) => Localized::new("invalid_config").with("detail", detail),
            Self::NoHardwareEncoder => Localized::new("no_hardware_encoder"),
            Self::CodecUnavailable(codec) => {
                Localized::new("codec_unavailable").with("codec", codec.label())
            }
            Self::UnsupportedWindowsBuild => Localized::new("unsupported_windows_build"),
            Self::TargetNotFound(MissingTarget::PrimaryMonitor) => {
                Localized::new("primary_monitor_missing")
            }
            Self::TargetNotFound(MissingTarget::Monitor(name)) => {
                Localized::new("monitor_missing").with("name", name)
            }
            Self::TargetNotFound(MissingTarget::Window(exe)) => {
                Localized::new("window_missing").with("exe", exe)
            }
            Self::Capture(detail) => Localized::new("capture_failed").with("detail", detail),
            Self::OutputTooLarge {
                codec,
                limit,
                width,
                height,
            } => Localized::new("output_too_large")
                .with("codec", codec.label())
                .with("limit", limit)
                .with("width", width)
                .with("height", height),
            Self::OutputUnsupported {
                codec,
                width,
                height,
                fps,
            } => Localized::new("output_unsupported")
                .with("codec", codec.label())
                .with("width", width)
                .with("height", height)
                .with("fps", fps),
            Self::Encoder(detail) => Localized::new("encoder_failed").with("detail", detail),
            Self::Mux(detail) => Localized::new("mux_failed").with("detail", detail),
            Self::Audio(detail) => Localized::new("audio_failed").with("detail", detail),
            Self::TrackStopped(track) => Localized::new("track_stopped").with("track", track),
            Self::CapturedWindowClosed => Localized::new("captured_window_closed"),
            Self::VideoCaptureStopped => Localized::new("video_capture_stopped"),
            Self::Io(error) => Localized::new("disk_error").with("detail", error.to_string()),
            Self::CalibrationBusy => Localized::new("calibration_busy"),
            Self::RecordingRunning => Localized::new("recording_running"),
            Self::DiskFull => Localized::new("disk_full"),
            Self::CalibrationStopped => Localized::new("calibration_stopped"),
            Self::Stopped => Localized::new("engine_stopped"),
            Self::ReplayNotArmed => Localized::new("replay_not_armed"),
            Self::NoStream => Localized::new("no_stream"),
            Self::ClipTooShort => Localized::new("clip_too_short").with("seconds", min_clip_secs()),
            Self::NotImplemented(detail) => {
                Localized::new("not_implemented").with("detail", detail)
            }
        }
    }
}

impl From<EngineError> for Localized {
    fn from(error: EngineError) -> Self {
        error.localized()
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refused_output_keeps_its_numbers_as_parameters() {
        let error = EngineError::OutputTooLarge {
            codec: VideoCodec::H264,
            limit: 4096,
            width: 5120,
            height: 1440,
        };
        let text = error.localized();
        assert_eq!(text.code, "output_too_large");
        assert_eq!(text.params["codec"], "H.264");
        assert_eq!(text.params["width"], 5120);
        assert!(error.to_string().contains("5120×1440"));
    }

    #[test]
    fn a_too_short_clip_names_the_minimum_duration() {
        let text = EngineError::ClipTooShort.localized();
        assert_eq!(text.code, "clip_too_short");
        assert_eq!(text.params["seconds"], 3);
    }
}

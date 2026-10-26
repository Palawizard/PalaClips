mod filler;
mod meter;
mod mixdown;
mod sum;
mod track;

pub use filler::{FillStats, GapFiller};
pub use meter::PeakMeter;
pub use mixdown::{MixBus, Mixdown, mix_lag_ms};
pub use track::{AudioStream, AudioTrack, AudioTrackStats};

use crate::events::TrackKind;
use crate::sources::CurrentGame;

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum AudioSource {
    SystemMix {
        device_id: Option<String>,
    },
    DesktopExceptPalaClips,
    Microphone {
        device_id: Option<String>,
        gain_db: f32,
    },
    Application {
        exe_name: String,
    },
    Game(CurrentGame),
    TracksMix,
}

impl AudioSource {
    pub fn label(&self) -> String {
        match self {
            Self::SystemMix { .. } | Self::DesktopExceptPalaClips | Self::TracksMix => {
                "Mix complet".into()
            }
            Self::Microphone { .. } => "Micro".into(),
            Self::Application { exe_name } => exe_name
                .strip_suffix(".exe")
                .or_else(|| exe_name.strip_suffix(".EXE"))
                .unwrap_or(exe_name)
                .into(),
            Self::Game(_) => "Jeu".into(),
        }
    }

    pub fn kind(&self) -> TrackKind {
        match self {
            Self::SystemMix { .. } | Self::DesktopExceptPalaClips | Self::TracksMix => {
                TrackKind::Mix
            }
            Self::Microphone { .. } => TrackKind::Microphone,
            Self::Application { .. } => TrackKind::Application,
            Self::Game(_) => TrackKind::Game,
        }
    }

    pub fn is_one_of(&self, exe_names: &[String]) -> bool {
        let exe_name = match self {
            Self::Application { exe_name } => Some(exe_name.as_str()),
            Self::Game(game) => game.exe_name(),
            _ => None,
        };
        exe_name.is_some_and(|exe_name| {
            exe_names
                .iter()
                .any(|other| other.eq_ignore_ascii_case(exe_name))
        })
    }

    pub fn is_application(&self) -> bool {
        matches!(self, Self::Application { .. } | Self::Game(_))
    }
}

mod filler;
mod track;

pub use filler::{FillStats, GapFiller};
pub use track::{AudioStream, AudioTrack, AudioTrackStats};

use crate::sources::CurrentGame;

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum AudioSource {
    SystemMix {
        device_id: Option<String>,
    },
    Microphone {
        device_id: Option<String>,
        gain_db: f32,
    },
    Application {
        exe_name: String,
    },
    Game(CurrentGame),
}

impl AudioSource {
    pub fn label(&self) -> String {
        match self {
            Self::SystemMix { .. } => "Mix complet".into(),
            Self::Microphone { .. } => "Micro".into(),
            Self::Application { exe_name } => exe_name
                .strip_suffix(".exe")
                .or_else(|| exe_name.strip_suffix(".EXE"))
                .unwrap_or(exe_name)
                .into(),
            Self::Game(_) => "Jeu".into(),
        }
    }

    pub fn is_application(&self) -> bool {
        matches!(self, Self::Application { .. } | Self::Game(_))
    }
}

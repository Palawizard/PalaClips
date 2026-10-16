mod filler;
mod track;

pub use filler::{FillStats, GapFiller};
pub use track::{AudioStream, AudioTrack, AudioTrackStats};

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
}

impl AudioSource {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SystemMix { .. } => "Mix complet",
            Self::Microphone { .. } => "Micro",
        }
    }
}

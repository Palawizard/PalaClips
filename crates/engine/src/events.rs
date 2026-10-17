use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Localized {
    pub code: &'static str,
    pub params: Map<String, Value>,
}

impl Localized {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            params: Map::new(),
        }
    }

    pub fn with(mut self, name: &str, value: impl Serialize) -> Self {
        self.params.insert(
            name.to_owned(),
            serde_json::to_value(value).unwrap_or(Value::Null),
        );
        self
    }
}

impl std::fmt::Display for Localized {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.params.is_empty() {
            write!(f, "{}", self.code)
        } else {
            write!(f, "{} {}", self.code, Value::Object(self.params.clone()))
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    #[default]
    Idle,
    ReplayArmed,
    Recording,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Mix,
    Microphone,
    Application,
    Game,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TrackDescriptor {
    pub title: String,
    pub kind: TrackKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackState {
    Waiting,
    Capturing,
    Silent,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    StateChanged {
        state: EngineState,
    },
    CaptureStarted {
        source: Localized,
        width: u32,
        height: u32,
        fps: u32,
        tracks: Vec<TrackDescriptor>,
    },
    TracksRenamed {
        tracks: Vec<TrackDescriptor>,
    },
    TrackChanged {
        track: usize,
        title: String,
        state: TrackState,
        source: Option<String>,
    },
    GameNamed {
        exe_name: String,
        name: String,
    },
    ClipSaved {
        path: String,
        duration_secs: f64,
        save_latency_ms: u64,
        thumbnail: Option<String>,
    },
    AudioLevels {
        levels: Vec<f32>,
    },
    ReplayStats {
        buffered_secs: f64,
        bytes: u64,
    },
    Alert {
        severity: Severity,
        #[serde(flatten)]
        text: Localized,
        message: String,
    },
}

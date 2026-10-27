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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CapturePerformance {
    pub encoder: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub repeated_percent: f64,
    pub late_total: u64,
    pub late_last_minute: u64,
    pub lag_max_ms: f64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveOrigin {
    Replay,
    PostRoll,
    Recording,
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
        origin: SaveOrigin,
    },
    AudioLevels {
        levels: Vec<f32>,
    },
    ReplayStats {
        buffered_secs: f64,
        bytes: u64,
    },
    PerformanceStats {
        cpu_percent: Option<f64>,
        cpu_max_percent: Option<f64>,
        engine_bytes: Option<u64>,
        buffer_bytes: u64,
        buffered_secs: f64,
        capture: Option<CapturePerformance>,
    },
    RecordingStats {
        elapsed_secs: f64,
        bytes: u64,
    },
    PostRoll {
        ends_in_secs: Option<f64>,
    },
    Alert {
        severity: Severity,
        #[serde(flatten)]
        text: Localized,
        message: String,
    },
    AlertCleared {
        code: &'static str,
    },
}

pub const ALERT_RATE_LIMITED: &str = "capture_source_rate_limited";
pub const ALERT_GPU_SATURATED: &str = "capture_gpu_saturated";
pub const ALERT_CPU_ENCODER_OVERLOADED: &str = "cpu_encoder_overloaded";
pub const ALERT_EXCLUSIVE_FULLSCREEN: &str = "capture_exclusive_fullscreen";
pub const CAPTURE_ALERTS: &[&str] = &[
    ALERT_RATE_LIMITED,
    ALERT_GPU_SATURATED,
    ALERT_CPU_ENCODER_OVERLOADED,
    ALERT_EXCLUSIVE_FULLSCREEN,
];

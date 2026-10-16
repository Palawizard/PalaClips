use serde::Serialize;

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

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    StateChanged {
        state: EngineState,
    },
    ClipSaved {
        path: String,
        duration_secs: f64,
        save_latency_ms: u64,
    },
    AudioLevels {
        levels: Vec<(String, f32)>,
    },
    ReplayStats {
        buffered_secs: f64,
        bytes: u64,
    },
    Alert {
        severity: Severity,
        code: String,
        message: String,
    },
}

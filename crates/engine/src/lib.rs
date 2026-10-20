#[cfg(windows)]
pub mod audio;
pub mod calibrate;
#[cfg(windows)]
pub mod capture;
pub mod clock;
pub mod config;
#[cfg(windows)]
pub mod diagnostic;
#[cfg(windows)]
pub mod encode;
#[cfg(windows)]
pub mod engine;
pub mod error;
pub mod events;
#[cfg(windows)]
pub mod games;
#[cfg(windows)]
pub mod library;
pub mod migrate;
#[cfg(windows)]
pub mod mux;
#[cfg(windows)]
pub mod paths;
#[cfg(windows)]
pub mod perf;
#[cfg(windows)]
pub mod record;
pub mod replay;
#[cfg(windows)]
pub mod sources;

pub use config::EngineConfig;
pub use error::{EngineError, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

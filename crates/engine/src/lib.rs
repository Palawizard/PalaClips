#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod capture;
pub mod clock;
pub mod config;
#[cfg(windows)]
pub mod encode;
#[cfg(windows)]
pub mod engine;
pub mod error;
pub mod events;
#[cfg(windows)]
pub mod mux;
#[cfg(windows)]
pub mod paths;
pub mod replay;
#[cfg(windows)]
pub mod sources;

pub use config::EngineConfig;
pub use error::{EngineError, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod capture;
pub mod clock;
pub mod config;
#[cfg(windows)]
pub mod encode;
pub mod error;
pub mod events;
#[cfg(windows)]
pub mod mux;
pub mod replay;

pub use config::EngineConfig;
pub use error::{EngineError, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod clock;
pub mod config;
pub mod error;
pub mod events;
pub mod replay;

pub use config::EngineConfig;
pub use error::{EngineError, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

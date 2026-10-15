mod pacer;
mod targets;

pub use pacer::CfrClock;
pub use targets::{ResolvedTarget, TargetInfo, list_targets, resolve_target};

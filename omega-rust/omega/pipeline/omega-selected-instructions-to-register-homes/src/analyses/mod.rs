//! Optimizer module role: stage group. Current selected-program facts and their independent validation.

mod legality;
mod live_ranges;
mod liveness;
mod reanalysis;

pub use legality::*;
pub use live_ranges::*;
pub use liveness::*;
pub use reanalysis::*;

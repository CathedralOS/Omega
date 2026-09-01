//! Optimizer module role: stage group. Read-only facts used by allocation and exact lowering rules.

pub(crate) mod allocation_legality;
pub(crate) mod allocator_availability;
pub(crate) mod fixed_precolored_intervals;
pub(crate) mod live_ranges;
pub(crate) mod liveness;
pub(crate) mod recovery_classification;
mod selected_input;

pub use allocation_legality::*;
pub use allocator_availability::*;
pub use fixed_precolored_intervals::*;
pub use live_ranges::*;
pub use liveness::*;
pub use recovery_classification::*;
pub use selected_input::ValidatedSelectedAnalysis;

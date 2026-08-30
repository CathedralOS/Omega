//! Optimizer module role: stage group. Register-environment, liveness, legality, copy, and home custody stages.

pub(crate) mod allocation_legality;
pub(crate) mod fixed_view_copies;
pub(crate) mod live_ranges;
pub(crate) mod liveness;
pub(crate) mod register_environment;
pub(crate) mod register_homes;
pub(crate) mod selected_reanalysis;

pub use allocation_legality::*;
pub use fixed_view_copies::*;
pub use live_ranges::*;
pub use liveness::*;
pub use omega_regalloc::ORDERED_ALLOCATION_RECOVERY_RULES;
pub use register_environment::*;
pub use register_homes::*;
pub use selected_reanalysis::*;

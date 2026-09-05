//! Optimizer module role: stage group. Current selected-program facts and their independent validation.

mod legality;
mod live_ranges;
mod liveness;
mod machine_effects;
mod reanalysis;

pub use legality::*;
pub use live_ranges::*;
pub use liveness::*;
pub use machine_effects::*;
pub use reanalysis::*;

pub(crate) mod allocation_legality;
pub(crate) mod allocator_availability;
pub(crate) mod fixed_precolored_intervals;
mod fixed_precolored_segment_homes;
pub(crate) mod fixed_precolored_split_requirements;
pub(crate) mod recovery_classification;
mod selected_input;
pub(crate) mod spill_choice;
pub use allocation_legality::*;
pub use allocator_availability::*;
pub use fixed_precolored_intervals::*;
pub use fixed_precolored_segment_homes::*;
pub use fixed_precolored_split_requirements::*;
pub use recovery_classification::*;
pub use selected_input::{OwnedSelectedProgram, SelectedProgramRef, ValidatedSelectedAnalysis};
pub use spill_choice::*;

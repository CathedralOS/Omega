//! Exact selected-lowering and post-allocation machine custody stages.

pub(crate) mod active_resident_rematerialization;
pub(crate) mod literal_fold_homes;
pub(crate) mod literal_folds;
pub(crate) mod machine_effects;
pub(crate) mod post_allocation_machine_effects;
pub(crate) mod post_allocation_optimizations;
pub(crate) mod selected_lowering_catalog;

pub use active_resident_rematerialization::*;
pub use literal_fold_homes::*;
pub use literal_folds::*;
pub use machine_effects::*;
pub use post_allocation_machine_effects::*;
pub use post_allocation_optimizations::*;
pub use selected_lowering_catalog::ORDERED_SELECTED_LOWERING_RULES;

//! Optimizer module role: stage group. Ordered custody boundaries from target selection through native artifacts.

pub(crate) mod artifacts;
pub(crate) mod encoding;
pub(crate) mod layout;
pub(crate) mod machine;
pub(crate) mod realization;
pub(crate) mod selection;

pub use artifacts::*;
pub use encoding::*;
pub use layout::*;
pub use machine::*;
pub use omega_allocation_legality_to_fixed_view_copies::*;
pub use omega_allocation_legality_to_register_homes::*;
pub use omega_callee_saved_requirements_to_save_storage::*;
pub use omega_fixed_view_copies_to_reanalyzed_legality::*;
pub use omega_live_ranges_to_allocation_legality::*;
pub use omega_liveness_to_live_ranges::*;
pub use omega_regalloc::ORDERED_ALLOCATION_RECOVERY_RULES;
pub use omega_register_homes_to_callee_saved_requirements::*;
pub use omega_selected_instructions_to_liveness::*;
pub use omega_spill_access_constraints_to_frame_requirements::*;
pub use omega_target_to_register_environment::FrameAbiPreservationConvention;
pub use realization::*;
pub use selection::*;

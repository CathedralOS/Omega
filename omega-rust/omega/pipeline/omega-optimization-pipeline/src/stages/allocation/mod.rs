//! Optimizer module role: stage group. Liveness, legality, copy, and home custody stages.

pub(crate) mod frame_requirements;

pub use frame_requirements::*;
pub use omega_callee_saved_requirements_to_save_storage::*;
pub use omega_regalloc::ORDERED_ALLOCATION_RECOVERY_RULES;
pub use omega_register_homes_to_callee_saved_requirements::*;
pub use omega_target_to_register_environment::FrameAbiPreservationConvention;

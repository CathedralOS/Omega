//! Optimizer module role: stage group. Liveness, legality, copy, and home custody stages.

pub(crate) mod abi_preservation;
mod callee_save_storage;
pub(crate) mod callee_saved_requirements;
pub(crate) mod frame_requirements;
pub(crate) mod register_homes;

pub use callee_save_storage::*;
pub use callee_saved_requirements::*;
pub use frame_requirements::*;
pub use omega_regalloc::ORDERED_ALLOCATION_RECOVERY_RULES;
pub use register_homes::*;

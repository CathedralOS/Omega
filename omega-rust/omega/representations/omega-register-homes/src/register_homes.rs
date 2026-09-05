//! Current physical-home assignments and their exact prerequisite identities.
//!
//! `storage` owns per-function assignments; `evidence` names the input analyses.
//! `identity` and `codec` preserve the canonical version-6 artifact contract.
//! This representation contains no prior pipeline stages or validated authority.

pub mod codec;
pub mod evidence;
pub mod identity;
pub mod storage;

pub use codec::RegisterHomeDecodeError;
pub use evidence::*;
pub use identity::{RegisterHomeIdentity, register_home_identity};
pub use storage::*;

use omega_register_model::TargetRegisterEnvironmentIdentity;

/// Bounded, deterministic physical homes for one transition-free legality
/// plan. The artifact grants no spill, frame, instruction-emission, or
/// publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterHomePlan {
    pub legality: AllocationLegalityIdentity,
    pub ranges: LiveRangeIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub functions: Vec<FunctionRegisterHomes>,
    pub structural_unit_functions: Vec<FunctionRegisterHomes>,
}

#[cfg(test)]
mod tests;

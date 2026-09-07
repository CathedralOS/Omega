//! Current selected program, allocation constraints, and physical-home assignments.
//!
//! `constraints` owns allocator availability and fixed-register requirements.
//! `storage` owns per-function assignments; `recovery` records spill choices
//! and recovery eligibility. These are raw records, not validated authority.
//! `preservation` records allocation-visible ABI save requirements, not frames
//! or executable save/restore decisions.
//! `identity` and `codec` preserve the canonical version-6 artifact contract.
//! This representation contains no prior pipeline stages or validated authority.

pub mod codec;
pub mod constraints;
pub mod identity;
pub mod preservation;
pub mod recovery;
pub mod storage;
pub mod view;

pub use codec::RegisterHomeDecodeError;
pub use constraints::*;
pub use identity::{
    AbstractSpillAccessConstraintPlanIdentity, RegisterHomeIdentity, register_home_identity,
};
pub use preservation::*;
pub use recovery::*;
pub use storage::*;
pub use view::AllocatedProgramRef;

/// One current allocated program, independent of the route that produced it.
/// Immutable artifacts may be shared with replay evidence without copying
/// their contents or making execution traverse that evidence. Raw data alone
/// grants no allocation, emission, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocatedProgram {
    pub selected: std::sync::Arc<selected_instructions::SelectedInstructionPlan>,
    pub homes: std::sync::Arc<RegisterHomePlan>,
}

#[cfg(test)]
mod tests;

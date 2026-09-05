//! Current selected program and its physical-home assignments.
//!
//! `storage` owns per-function assignments; `evidence` names the input analyses.
//! `preservation` records allocation-visible ABI save requirements, not frames
//! or executable save/restore decisions.
//! `identity` and `codec` preserve the canonical version-6 artifact contract.
//! This representation contains no prior pipeline stages or validated authority.

pub mod codec;
pub mod evidence;
pub mod identity;
pub mod preservation;
pub mod storage;
pub mod view;

pub use codec::RegisterHomeDecodeError;
pub use evidence::*;
pub use identity::{
    AbstractSpillAccessConstraintPlanIdentity, RegisterHomeIdentity, register_home_identity,
};
pub use preservation::*;
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

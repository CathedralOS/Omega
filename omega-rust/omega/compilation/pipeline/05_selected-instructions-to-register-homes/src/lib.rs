#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected instructions to register homes.
//!
//! Start at [`stage_register_allocation`] in `register_allocation.rs`. It
//! consumes the preceding selected X-to-X stage's current program and proof,
//! sequences home assignment and admitted pressure recovery, and publishes one
//! [`RetainedAllocation`]. That preceding stage owns selected-lowering rewrites
//! and reusable selected-program analyses; allocation does not rerun it.
//!
//! Module map, in route order:
//! - `register_allocation` owns the entry and its exact route.
//! - `assignment` holds the stages that route sequences: `transformed` homes
//!   after a completed rewrite run, the optional `recovery` rules,
//!   `runtime_spill` pressure recovery — including residual pressure a
//!   completed run hands over — `baseline` homes over `home_assignment`, and
//!   the `post_allocation_manifest` every route publishes.
//! - `rewrites` stages active-resident rematerialization for a recovery rule.
//! - `output` owns the current allocation facts every route publishes.
//! - `preservation` derives allocation-visible callee-saved requirements.
//!
//! Every name the route owns is re-exported below from the module that owns
//! it, so the root reads as a map. Upstream vocabulary (`register_model`,
//! `register_homes`, `selected_instructions`, and the preceding stage
//! `selected_instructions_to_selected_instructions`) is imported from its
//! owner where it is used. The root re-exports upstream names in two places
//! only: the `register_homes` records a stage publishes beside its operations
//! (post-allocation manifest, logical spill operations, stack-slot coloring),
//! and the seven selected-program analysis names listed last, which the
//! post-allocation stages still import through this crate.

/// One owner row per receipt accessor: the field's own name read back as its
/// own type. A receipt whose rows are not one of the shared identity blocks
/// lists them here instead of restating the body at the site; `ref` selects
/// the `&self` receiver and `borrow` the `&self` receiver with a `&` field
/// read, for receipts whose callers hold a reference.
macro_rules! receipt_accessor_rows {
    (ref $name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(&self) -> $ty { self.$field } )*
    };
    (borrow $name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(&self) -> $ty { &self.$field } )*
    };
    ($name:ident { $( $(#[$meta:meta])* $field:ident : $ty:ty ),* $(,)? }) => {
        $( $(#[$meta])* pub const fn $field(self) -> $ty { self.$field } )*
    };
}

pub(crate) use optimization_core::digest_encoder;
mod assignment;
mod output;
mod preservation;
mod register_allocation;
mod rewrites;

// The entry and its route.
pub use register_allocation::{RegisterAllocationError, stage_register_allocation};

// Stages sequenced by `stage_register_allocation`, in route order.

pub(crate) use assignment::baseline::{
    OptimizedPostCopyRegisterHomeCustodyError, OptimizedRegisterHomeCustodyError,
    StagedOptimizedPostCopyRegisterHomeCustodyReceipt, StagedOptimizedRegisterHomeCustodyReceipt,
    StagedOptimizedRegisterHomes, StagedOptimizedRegisterHomesAfterFixedViewCopies,
};
pub use assignment::home_assignment::{
    FunctionRegisterHomes, RegisterHomeDecodeError, RegisterHomeIdentity, RegisterHomePlan,
    ValidatedRegisterHomes, VirtualRegisterHome, register_home_identity,
};
pub(crate) use assignment::home_assignment::{
    RegisterHomeError, RegisterHomeValidationReceipt, assign_register_homes,
};
pub use assignment::post_allocation_manifest::{
    PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData, ValidatedPostAllocationOptimizationManifest,
};

pub(crate) use assignment::runtime_spill::RuntimeSpillAllocationError;

pub(crate) use assignment::transformed::{
    OptimizedPostPreAllocationHomeCustodyError, OptimizedPostSelectedLoweringHomeCustodyError,
    StagedOptimizedPostLiteralFoldHomeCustodyReceipt,
    StagedOptimizedPostPreAllocationHomeCustodyReceipt,
    StagedOptimizedPostSelectedLoweringHomeCustodyReceipt,
    StagedOptimizedRegisterHomesAfterLiteralFolds, StagedOptimizedRegisterHomesAfterPreAllocation,
    StagedOptimizedRegisterHomesAfterSelectedLowering,
};

// Rematerialization staged by the active-resident recovery rule.

pub(crate) use rewrites::{
    OptimizedActiveResidentRematerializationError, StagedOptimizedActiveResidentRematerialization,
    StagedOptimizedActiveResidentRematerializationCustodyReceipt,
    StagedOptimizedActiveResidentRematerializationPressure,
    stage_optimized_active_resident_rematerialization_pressure,
};

// Current allocation facts.
pub(crate) use output::AllocationAdmissionError;
pub use output::{AllocationEvidence, AllocationOutput, RetainedAllocation};

// Allocation-visible callee-saved requirements.
pub use preservation::{
    AllocatedCalleeSavedRequirementError, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedUnitRequirement, CalleeSavedModificationWitness,
    FunctionAllocatedCalleeSavedRequirements, ValidatedAllocatedCalleeSavedRequirements,
    encode_callee_saved_modification_witness_identity, stage_allocated_callee_saved_requirements,
    validate_allocated_callee_saved_requirements,
};

// Validated spill boundaries not yet sequenced by `stage_register_allocation`.

// Upstream selected-program analyses. `AllocationOutput` carries the ranges
// and legality; the post-allocation stages name these through this crate.
pub use selected_instructions_to_selected_instructions::{
    MachineEffectStageError, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedPreAllocationMachineEffects, ValidatedSelectedAnalysis, analyze_machine_effects,
};

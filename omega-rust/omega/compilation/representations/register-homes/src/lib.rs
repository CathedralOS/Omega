#![forbid(unsafe_code)]

//! Physical register homes as data, independent of the allocating algorithm.
//!
//! Start at [`register_homes`]. Decode and identity checks establish canonical
//! representation only; allocation validity belongs to the independent verifier.

pub(crate) use optimization_core::digest_encoder;
pub(crate) mod codec_bytes;
pub(crate) mod cursor;
pub mod register_homes;
pub use register_homes::{
    AbstractSpillAccessConstraintPlanIdentity, AllocatedCalleeSavedRequirementIdentity,
    AllocatedCalleeSavedRequirementPlan, AllocatedCalleeSavedRequirementPolicy,
    AllocatedCalleeSavedUnitRequirement, AllocatedProgram, AllocatedProgramRef,
    AllocationLegalityIdentity, AllocationLegalityPlan, AllocatorAvailabilityIdentity,
    AllocatorAvailabilityPlan, AllocatorAvailabilityPolicy, CalleeSavedModificationWitness,
    EntryFixedViewTransition, FixedPrecoloredHomeDomainId, FixedPrecoloredInterval,
    FixedPrecoloredIntervalPlan, FixedPrecoloredIntervalPlanIdentity,
    FixedPrecoloredIntervalPolicy, FixedPrecoloredRegisterSplitRequirements,
    FixedPrecoloredSegmentHomePlan, FixedPrecoloredSegmentHomePlanIdentity,
    FixedPrecoloredSegmentHomePolicy, FixedPrecoloredSourceFragmentRequirements,
    FixedPrecoloredSourceSegment, FixedPrecoloredSourceSegmentHome, FixedPrecoloredSourceSegmentId,
    FixedPrecoloredSourceSegmentOpening, FixedPrecoloredSplitRequirementPlan,
    FixedPrecoloredSplitRequirementPlanIdentity, FixedPrecoloredSplitRequirementPolicy,
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan, FixedViewCopyPolicy,
    FixedViewCopySourceEvidence, FunctionAllocatedCalleeSavedRequirements,
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSegmentHomes, FunctionFixedPrecoloredSplitRequirements,
    FunctionRecoveryClassification, FunctionRegisterHomes, FunctionSpillChoices,
    NoAdmittedRecoveryReason, PostAllocationManifestStage, PostAllocationOptimizationManifest,
    PostAllocationOptimizationManifestDecodeError, PostAllocationOptimizationManifestError,
    PostAllocationSelectedTransformation, PostAllocationSpillStatus, PostAllocationStatistics,
    PostAllocationUnavailableData, PressureContender, PressureRecoveryClassification,
    PressureResident, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, RegisterClassAvailability, RegisterHomeDecodeError, RegisterHomeIdentity,
    RegisterHomePlan, SpillChoice, SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy,
    VirtualEarlyClobberPointLegality, VirtualLegalitySpan, VirtualRegisterAllocationLegality,
    VirtualRegisterHome, allocation_legality, allocation_legality_function_digest,
    allocation_legality_identity_from_digests, allocator_availability,
    allocator_availability_identity, classification, codec, constraints,
    encode_callee_saved_modification_witness_identity, fixed_precolored_interval_plan_identity,
    fixed_precolored_intervals, fixed_precolored_segment_home_plan_identity,
    fixed_precolored_segment_homes, fixed_precolored_split_requirement_plan_identity,
    fixed_precolored_split_requirements, fixed_view_copy, fixed_view_copy_identity, identity,
    post_allocation_manifest, preservation, recovery, recovery_classification_identity,
    register_home_identity, spill_choice, spill_choice_identity, storage, view,
};
pub(crate) use register_homes::{FixedViewCopyDecodeError, RecoveryClassificationDecodeError};

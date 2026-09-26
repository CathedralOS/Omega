#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by validated instruction selection.
//!
//! Start at [`stage_optimized_instruction_selection`] in `optimized.rs`: it
//! owns the target-register environment, runs [`legalize_target_operations`]
//! and then [`select_instructions`], and joins their custody in
//! [`validate_optimized_selection_custody`], which replays both through
//! [`validate_legalized_operations`] and [`validate_selected_instructions`].
//! `legalization` is the raw-target to legal-operation join, `selection` the
//! legal-operation to selected-CFG join, and `structural_inputs` the
//! input-only reconstruction of structural storage, parameter shape and
//! unobserved-owned eligibility both consume.

pub mod isa_aarch64;
pub mod isa_x86_64;
mod legalization;
pub mod legalized_operations;
mod optimized;
pub mod register_environment;
pub mod register_model;
pub mod selected_instructions;
mod selection;
mod structural_inputs;

pub use selected_instructions::{
    AddressFoldIdentity, ArchitecturalUnitAction, ArchitecturalUnitActionKind,
    ArchitecturalUnitLiveRange, BlockLiveness, BlockMachineEffects, BlockPointDomain,
    ConstantBooleanIdentity, ConstantBranchIdentity, CopyAffinity, CopyRemovalIdentity,
    DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse, EdgeRegisterTransfer,
    EntryDefinition, FixedViewCopyIdentity, FrameStorageSlotId, FunctionLiveRanges,
    FunctionLiveness, FunctionMachineEffects, InstructionLiveness, InstructionMachineEffects,
    LiteralFoldIdentity, LiveRangeEdgeConnector, LiveRangeFragment, LiveRangeIdentity,
    LiveRangePlan, LiveRangePoint, LivenessIdentity, LivenessPlan, LivenessPosition,
    LocalStorageSlotId, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectCatalog, MachineEffectCatalogIdentity,
    MachineEffectCatalogValidationError, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineIdentityBytes, MachineLatencyKnowledge, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, OperandPosition,
    OutgoingArgumentSlotId, OutgoingArgumentSlotRole, PackedByteWidth,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, PressureRematerializationIdentity, RedundantExtensionIdentity,
    SaturatingCarrier, SaturatingOperation, SelectedAddressBase, SelectedBlock, SelectedBlockId,
    SelectedBlockOrigin, SelectedBoundarySettlement, SelectedBoundarySettlementPayload,
    SelectedCallContract, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedFunction, SelectedFunctions,
    SelectedInstruction, SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedNormalizedForeignCall, SelectedOperand, SelectedOutgoingArgumentSlot,
    SelectedSelectionConstraints, SelectedStructuralBinding, SelectedStructuralCaseEdge,
    SelectedStructuralTransport, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueBinding, SelectedValueTransport, SuccessorLiveness, TrappingForm,
    TrappingOperation, ValidatedMachineEffectCatalog, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange, VirtualOccurrence,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin, alternative_family_tag, calls,
    constraints, control_flow, effects, encode_machine_alternative_identity,
    encode_machine_alternative_key_identity, encode_machine_encoded_effects_identity, identity,
    instructions, live_range_identity, live_ranges, liveness, liveness_identity,
    machine_effect_catalog_identity, plan_identity, pre_allocation_machine_effect_identity,
    provenance, saturating_family_tag, structural_case, trapping_family_tag,
    validate_machine_effect_catalog, values,
};

// The stage entry and its custody join.
pub use optimized::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    selection_constraints, stage_optimized_instruction_selection,
    validate_optimized_selection_custody,
};

// The two joins the entry sequences, each with its independent replay.
pub use legalization::{
    LegalizationError, LegalizationSource, LegalizationValidationReceipt,
    ValidatedLegalizedOperations, legalize_target_operations, validate_legalized_operations,
};
pub use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
    select_instructions, selected_instruction_plan_identity, validate_selected_instructions,
};

#[cfg(test)]
mod tests;

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.
#[cfg(test)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

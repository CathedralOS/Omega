#![forbid(unsafe_code)]

//! Optimizer module role: crate map. The selected-instruction representation.
//!
//! Start at [`selected_instructions::SelectedInstructionPlan`]. Its subordinate
//! areas own control flow, values, instructions, calls, and target effects.

pub mod selected_instructions;
pub use selected_instructions::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    BlockLiveness, BlockMachineEffects, BlockPointDomain, CopyAffinity, DistinctUseDefTie,
    EarlyClobberConstraint, EarlyClobberUse, EdgeRegisterTransfer, EntryDefinition,
    FixedViewCopyIdentity, FrameStorageSlotId, FunctionLiveRanges, FunctionLiveness,
    FunctionMachineEffects, InstructionLiveness, InstructionMachineEffects, LiteralFoldIdentity,
    LiveRangeEdgeConnector, LiveRangeFragment, LiveRangeIdentity, LiveRangePlan, LiveRangePoint,
    LivenessIdentity, LivenessPlan, LivenessPosition, LocalStorageSlotId, MachineAlternative,
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogIdentity, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    OperandPosition, OutgoingArgumentSlotId, OutgoingArgumentSlotRole, PackedByteWidth,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, PressureRematerializationIdentity, SaturatingCarrier,
    SaturatingOperation, SelectedBlock, SelectedBlockId, SelectedBlockOrigin,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedCallContract,
    SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedConstraintKeys,
    SelectedFixedInputConstraint, SelectedFunction, SelectedFunctions, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionKind, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, SelectedInstructionProvenance, SelectedLocalStorageSlot,
    SelectedMemoryAccess, SelectedMemoryAccessOrigin, SelectedMemoryAccessRole,
    SelectedNormalizedForeignCall, SelectedOperand, SelectedOutgoingArgumentSlot,
    SelectedSelectionConstraints, SelectedStructuralBinding, SelectedStructuralCaseEdge,
    SelectedStructuralTransport, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueBinding, SelectedValueTransport, SuccessorLiveness, ValidatedMachineEffectCatalog,
    VirtualFixedConstraint, VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange,
    VirtualOccurrence, VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin, calls,
    constraints, control_flow, effects, identity, instructions, live_range_identity, live_ranges,
    liveness, liveness_identity, machine_effect_catalog_identity, plan_identity,
    pre_allocation_machine_effect_identity, provenance, saturating_family_tag,
    selected_instruction_plan_identity, structural_case, validate_machine_effect_catalog, values,
};

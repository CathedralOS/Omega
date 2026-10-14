#![forbid(unsafe_code)]

//! Optimizer module role: crate map. The selected-instruction representation.
//!
//! Start at [`selected_instructions::SelectedInstructionPlan`]. Its subordinate
//! areas own control flow, values, instructions, calls, and target effects.

mod codec_bytes;

pub(crate) use optimization_core::digest_encoder;
pub mod selected_instructions;
pub use selected_instructions::{
    AddressFoldIdentity, ArchitecturalUnitAction, ArchitecturalUnitActionKind,
    ArchitecturalUnitLiveRange, BlockLiveness, BlockMachineEffects, BlockPointDomain,
    ConstantBooleanIdentity, ConstantBranchIdentity, CopiedCallOperandIdentity, CopyAffinity,
    CopyRemovalIdentity, DeadCompareIdentity, DistinctUseDefTie, EarlyClobberConstraint,
    EarlyClobberUse, EdgeRegisterTransfer, EntryDefinition, EquivalentCompareIdentity,
    FixedViewCopyIdentity, FrameStorageSlotId, FunctionLiveRanges, FunctionLiveness,
    FunctionMachineEffects, InstructionLiveness, InstructionMachineEffects, IsaCatalogValidation,
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
    PreAllocationMachineEffectPlan, PressureRematerializationIdentity, RedundantCompareIdentity,
    RedundantExtensionIdentity, SaturatingCarrier, SaturatingOperation, SelectedAddressBase,
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedBoundarySettlement,
    SelectedBoundarySettlementPayload, SelectedCallContract, SelectedCallTarget,
    SelectedCaseDispatchSource, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedFunction, SelectedFunctions,
    SelectedImageAddressReference, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, SelectedLocalStorageSlot, SelectedMemoryAccess,
    SelectedMemoryAccessOrigin, SelectedMemoryAccessRole, SelectedNormalizedForeignCall,
    SelectedOperand, SelectedOutgoingArgumentSlot, SelectedScalarReferenceSource,
    SelectedSelectionConstraints, SelectedStructuralBinding, SelectedStructuralCaseEdge,
    SelectedStructuralTransport, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueBinding, SelectedValueTransport, StoredLoadForwardingIdentity, SuccessorLiveness,
    TrappingForm, TrappingOperation, ValidatedMachineEffectCatalog, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange, VirtualOccurrence,
    VirtualRegister, VirtualRegisterId, VirtualRegisterOrigin, alternative_family_tag, calls,
    constraints, control_flow, effects, encode_machine_alternative_identity,
    encode_machine_alternative_key_identity, encode_machine_encoded_effects_identity,
    float_bit_transfer_properties, identity, instruction_position, instructions,
    is_float_bit_transfer, live_range_function_digest, live_range_identity_from_digests,
    live_ranges, liveness, liveness_function_digest, liveness_identity_from_digests,
    machine_effect_catalog_identity, plan_identity, pre_allocation_machine_effect_identity,
    provenance, saturating_family_tag, selected_instruction_plan_identity, structural_case,
    terminator_instruction, terminator_instruction_mut, terminator_return_edge,
    terminator_successors, terminator_successors_mut, trapping_family_tag,
    validate_isa_machine_effect_catalog, validate_machine_effect_catalog, values,
};
pub use selected_instructions::{
    AtomicForm, AtomicOperation, AtomicWidth, FloatResultPolicyForm, FloatResultPolicyKind,
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
    IeeeFloatFormat, IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    PlacedScalarForm, ieee_float_arithmetic_is_trapping,
};
pub use terminal_psi::ByteSequencePredicate;

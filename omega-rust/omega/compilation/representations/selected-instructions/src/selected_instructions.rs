//! Selected instructions before physical register assignment.
//!
//! This is the current program, not a chain of transformation-stage objects.
//! `control_flow` owns functions, blocks and successor edges; `values` owns
//! virtual registers and operand uses; `instructions` owns executable forms.
//! `calls` retains the distinct structural ABI shapes. `constraints` and
//! `effects` describe target restrictions; `provenance` retains semantic,
//! proof and fuel links. Physical homes belong to the allocated representation.
//! `liveness` and `live_ranges` retain facts about this selected program and
//! their canonical identities, without granting analysis admission authority.
//!
//! Moving or eliminating an instruction must preserve or explicitly transport
//! those links. A register copy is not permission to duplicate a linear value.
//! Function bodies use copy-on-write storage: a selected rewrite retains the
//! roster and shares unchanged functions instead of duplicating their blocks,
//! instructions and register tables. Wire identities still describe contents.

mod atomic;
mod placed_scalar;
pub use placed_scalar::PlacedScalarForm;
pub mod calls;
mod float_result_policy;
mod ieee_float_arithmetic;
pub use atomic::{AtomicForm, AtomicOperation, AtomicWidth};
pub use float_result_policy::{FloatResultPolicyForm, FloatResultPolicyKind};
pub use ieee_float_arithmetic::{
    IeeeFloatArithmeticForm, IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation,
    IeeeFloatFormat, IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain,
    float_bit_transfer_properties, ieee_float_arithmetic_is_trapping, is_float_bit_transfer,
};
pub mod constraints;
pub mod control_flow;
mod functions;
pub use functions::SelectedFunctions;
pub mod effects;
pub mod identity;
pub mod instructions;
mod packed_byte_width;
pub use packed_byte_width::PackedByteWidth;
pub mod live_ranges;
pub mod liveness;
pub mod plan_identity;
pub mod provenance;
pub mod structural_case;
pub mod values;

pub use calls::normalized_foreign::SelectedNormalizedForeignCall;
pub use calls::ordinary::{
    FrameStorageSlotId, LocalStorageSlotId, OutgoingArgumentSlotId, OutgoingArgumentSlotRole,
    SelectedBoundarySettlement, SelectedBoundarySettlementPayload, SelectedCallContract,
    SelectedLocalStorageSlot, SelectedMemoryAccess, SelectedMemoryAccessOrigin,
    SelectedMemoryAccessRole, SelectedOutgoingArgumentSlot,
};
pub use constraints::{
    SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedSelectionConstraints,
};
pub use control_flow::{
    SelectedAddressBase, SelectedBlock, SelectedBlockOrigin, SelectedFunction,
    SelectedScalarReferenceSource, SelectedStructuralBinding, SelectedStructuralTransport,
    SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding,
    SelectedValueTransport, terminator_instruction, terminator_instruction_mut,
    terminator_return_edge, terminator_successors, terminator_successors_mut,
};
pub use effects::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, IsaCatalogValidation,
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineIdentityBytes, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, PreAllocationMachineEffectDecodeError,
    PreAllocationMachineEffectIdentity, PreAllocationMachineEffectPlan,
    ValidatedMachineEffectCatalog, alternative_family_tag, canonical_machine_effect_catalog,
    encode_machine_alternative_identity, encode_machine_alternative_key_identity,
    encode_machine_encoded_effects_identity, machine_effect_catalog_identity,
    pre_allocation_machine_effect_identity, saturating_family_tag, trapping_family_tag,
    validate_isa_machine_effect_catalog, validate_machine_effect_catalog,
};
pub use identity::{
    AddressFoldIdentity, ConstantBooleanIdentity, ConstantBranchIdentity,
    CopiedCallOperandIdentity, CopyRemovalIdentity, DeadCompareIdentity, EquivalentCompareIdentity,
    FixedViewCopyIdentity, LiteralFoldIdentity, PressureRematerializationIdentity,
    RedundantCompareIdentity, RedundantExtensionIdentity, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionPlanIdentity, StoredLoadForwardingIdentity, VirtualRegisterId,
};
pub use instructions::{
    SelectedCallTarget, SelectedImageAddressReference, SelectedInstruction,
    SelectedInstructionKind, instruction_position,
};
pub use legalized_operations::{
    SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
};
pub use live_ranges::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    BlockPointDomain, CopyAffinity, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse,
    EdgeRegisterTransfer, FunctionLiveRanges, LiveRangeEdgeConnector, LiveRangeFragment,
    LiveRangeIdentity, LiveRangePlan, LiveRangePoint, VirtualFixedConstraint,
    VirtualFixedConstraintSite, VirtualInterference, VirtualLiveRange, VirtualOccurrence,
    live_range_function_digest, live_range_identity_from_digests,
};
pub use liveness::{
    BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessIdentity,
    LivenessPlan, LivenessPosition, OperandPosition, SuccessorLiveness, liveness_function_digest,
    liveness_identity_from_digests,
};
pub use plan_identity::selected_instruction_plan_identity;
pub use provenance::SelectedInstructionProvenance;
pub use structural_case::{
    SelectedCaseDispatchSource, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedStructuralCaseEdge,
};
pub use values::{SelectedOperand, VirtualRegister, VirtualRegisterOrigin};

use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;
impl SelectedInstructionPlan {
    /// The selected function for `machine`, when the plan carries it.
    pub fn function(&self, machine: MachineId) -> Option<&SelectedFunction> {
        self.functions.function(machine)
    }

    /// Every function's blocks, in function order.
    pub fn blocks(&self) -> impl Iterator<Item = &SelectedBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstructionPlan {
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: SelectedFunctions,
}

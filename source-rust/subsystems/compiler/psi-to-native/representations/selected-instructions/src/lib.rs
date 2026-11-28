#![forbid(unsafe_code)]

/*
  Pre-allocation target-selected instruction representation (`SelectedInstructionPlan`),
  target machine-effect catalogs (`MachineEffectCatalog`), and pre-allocation
  analysis facts (`LivenessPlan`, `LiveRangePlan`, `PreAllocationMachineEffectPlan`).

  We represent the program after target instruction selection and before
  physical register home assignment. `control_flow`, `functions`, and `blocks`
  own the copy-on-write function and basic-block rosters alongside their
  successor edges and edge-transport bindings; `values` owns `VirtualRegister`
  declarations and `SelectedOperand` register-constraint uses; `instructions`
  and `atomic` own the executable `SelectedInstructionKind` forms; `calls`
  retains ordinary, dynamic, and normalized foreign ABI call contracts and
  frame storage slots; `constraints` and `effects` describe target register
  restrictions, machine-effect catalogs, and per-instruction effect plans; and
  `liveness` and `live_ranges` hold pre-allocation dataflow and interference
  facts with their canonical SHA-256 identities.
*/

/*
  Target instruction selection fixes the concrete machine instruction kinds,
  register classes, fixed ABI register views, tied use-def operands, and
  implicit architectural unit clobbers that a target requires, while keeping
  values in virtual registers so pre-allocation rewrites and register-pressure
  recovery (spilling, live-range splitting, rematerialization, and copy
  coalescing) can transform the program before physical home assignment. Because
  register-pressure recovery evaluates and validates one candidate rewrite per
  step, `SelectedFunctions` and `SelectedBlocks` store functions and blocks in
  copy-on-write `Arc` entries that retain their mechanical bitset lookups
  across clones, detaching and invalidating only the function and blocks a
  rewrite mutates. No stage hashes a selected plan: a stage that must name the
  plan it read shares its `Arc<SelectedInstructionPlan>` and compares it.
*/

/*
  Produced by `05_target-operations-to-selected-instructions` from
  `LegalizedOperationPlan` and transformed in place by
  `06_selected-instructions-to-selected-instructions` (using independent
  validators in `selected-instruction-analyses`). Consumed by
  `07_selected-instructions-to-register-homes` to assign physical register and
  spill-slot homes (`RegisterHomePlan`). `StagedOptimizedCustodyCore`
  bundles the nine upstream pipeline-custody identities that selection,
  selected optimization, and register allocation carry across stage receipts.
*/

pub(crate) mod codec_bytes {
    pub(crate) use ::wire::codec_bytes::push_encoded;
}

pub(crate) use ::wire::digest_encoder;

mod atomic;
mod blocks;
pub mod calls;
pub mod compaction;
pub mod constraints;
pub mod control_flow;
pub mod effects;
mod functions;
pub mod identity;
pub mod instructions;
pub mod live_ranges;
pub mod liveness;
mod program_contracts;
pub mod provenance;
pub mod structural_case;
pub mod test_support;
pub mod values;

#[doc(hidden)]
pub use crate as selected_instructions;

pub use atomic::{AtomicForm, AtomicOperation, AtomicWidth};
pub use blocks::{BlockSlotEvent, BlockSlotSummary, SelectedBlocks};
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
    SelectedFunctionContracts, SelectedScalarReferenceSource, SelectedStructuralBinding,
    SelectedStructuralBlockParameters, SelectedStructuralTransport, SelectedSuccessor,
    SelectedSuccessorRole, SelectedTerminator, SelectedValueBinding, SelectedValueTransport,
    terminator_instruction, terminator_instruction_mut, terminator_return_edge,
    terminator_successors, terminator_successors_mut,
};
pub use effects::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, IsaCatalogValidation,
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineIdentityBytes, MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind,
    MachineSizeKnowledge, MachineTrapBehavior, PreAllocationMachineEffectPlan,
    PreparedMachineEffectCatalog, alternative_family_tag, canonical_machine_effect_catalog,
    encode_machine_alternative_identity, encode_machine_alternative_key_identity,
    encode_machine_encoded_effects_identity, machine_effect_catalog_identity,
    saturating_family_tag, trapping_family_tag, validate_isa_machine_effect_catalog,
    validate_machine_effect_catalog,
};
pub use functions::SelectedFunctions;
pub use identity::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
pub use instructions::{
    AccessorScalarForm, FloatResultPolicyForm, FloatResultPolicyKind, IeeeFloatArithmeticForm,
    IeeeFloatArithmeticOperation, IeeeFloatBinaryOperation, IeeeFloatFormat,
    IeeeFloatIntegerCarrier, IeeeFloatRoundingDirection, IeeeFloatToIntegerDomain, PackedByteWidth,
    PackedShuffleFastPattern, SelectedCallTarget, SelectedImageAddressReference,
    SelectedInstruction, SelectedInstructionKind, float_bit_transfer_properties,
    ieee_float_arithmetic_is_trapping, instruction_position, is_float_bit_transfer,
};
pub use live_ranges::{
    ArchitecturalUnitAction, ArchitecturalUnitActionKind, ArchitecturalUnitLiveRange,
    BlockPointDomain, CopyAffinity, DistinctUseDefTie, EarlyClobberConstraint, EarlyClobberUse,
    EdgeRegisterTransfer, FunctionLiveRanges, LiveRangeEdgeConnector, LiveRangeFragment,
    LiveRangePlan, LiveRangePoint, VirtualFixedConstraint, VirtualFixedConstraintSite,
    VirtualInterference, VirtualInterferences, VirtualLiveRange, VirtualOccurrence,
};
pub use liveness::{
    BlockLiveness, EntryDefinition, FunctionLiveness, InstructionLiveness, LivenessPlan,
    LivenessPosition, OperandPosition, SuccessorLiveness,
};
pub use program_contracts::SelectedProgramContracts;
pub use provenance::SelectedInstructionProvenance;
pub use structural_case::{
    SelectedCaseDispatchSource, SelectedCasePayloadBinding, SelectedCasePayloadTransport,
    SelectedStructuralCaseEdge,
};
pub use target_vocabulary::{
    SaturatingCarrier, SaturatingOperation, TrappingForm, TrappingOperation,
};
pub use terminal_psi::{
    ByteSequencePredicate, ControlRegister, GlobalDescriptorTableOperation, InterruptControlKind,
    MemoryFenceKind, RegisterOperation, RegisterTargetApplicability, SegmentRegister,
    SegmentReload, SystemRegister,
};
pub use values::{SelectedOperand, VirtualRegister, VirtualRegisterOrigin};

use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstructionPlan {
    pub contracts: std::sync::Arc<SelectedProgramContracts>,
    pub psi: TerminalPsiIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: SelectedFunctions,
}

impl SelectedInstructionPlan {
    pub fn function(&self, machine: MachineId) -> Option<&SelectedFunction> {
        self.functions.function(machine)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &SelectedBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgram {
    pub plan: std::sync::Arc<SelectedInstructionPlan>,
    pub optimization_unit: optimization_core::OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
}

impl SelectedProgram {
    pub fn plan(&self) -> &SelectedInstructionPlan {
        &self.plan
    }

    pub fn shared_plan(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.plan)
    }

    pub const fn optimization_unit(&self) -> optimization_core::OptimizationUnitIdentity {
        self.optimization_unit
    }

    pub const fn work_schedule(&self) -> WorkScheduleIdentity {
        self.work_schedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrentSelectedProgram<'plan> {
    pub plan: &'plan SelectedInstructionPlan,
    pub optimization_unit: optimization_core::OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedOptimizedCustodyCore {
    pub psi: terminal_psi::TerminalPsiIdentity, // Terminal Psi identity of the staged upstream result.
    pub target: target::NativeTarget,           // Native target of the staged upstream result.
    pub entry: semantic_vocabulary::MachineId,  // Entry machine the staged result lowers.
    pub selection: optimization_core::OptimizationSelectionIdentity, // Optimization selections the staged result was lowered under.
    pub budget_per_pass: optimization_core::OptimizationWorkBudget, // Per-pass work budget the staged result was lowered under.
    pub optimization_unit: optimization_core::OptimizationUnitIdentity, // Optimization-unit identity of the staged result.
    pub work_schedule: semantic_vocabulary::WorkScheduleIdentity, // Work-schedule identity of the staged result.
    pub register_environment: register_model::TargetRegisterEnvironmentIdentity, // Validated target-register-environment identity of the staged result.
}

#[macro_export]
macro_rules! staging_custody_core_accessors {
    () => {
        pub const fn psi(self) -> TerminalPsiIdentity {
            self.core.psi
        }

        pub const fn target(self) -> target::NativeTarget {
            self.core.target
        }

        pub const fn entry(self) -> MachineId {
            self.core.entry
        }

        pub const fn selection(self) -> OptimizationSelectionIdentity {
            self.core.selection
        }

        pub const fn budget_per_pass(self) -> OptimizationWorkBudget {
            self.core.budget_per_pass
        }

        pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
            self.core.optimization_unit
        }

        pub const fn work_schedule(self) -> WorkScheduleIdentity {
            self.core.work_schedule
        }

        pub const fn register_environment(
            self,
        ) -> register_model::TargetRegisterEnvironmentIdentity {
            self.core.register_environment
        }

        pub const fn core(self) -> $crate::StagedOptimizedCustodyCore {
            self.core
        }
    };
}

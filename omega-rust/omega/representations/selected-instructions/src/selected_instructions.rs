//! Selected instructions before physical register assignment.
//!
//! This is the current program, not a chain of transformation-stage objects.
//! `control_flow` owns functions, blocks and successor edges; `values` owns
//! virtual registers and operand uses; `instructions` owns executable forms.
//! `calls` retains the distinct structural ABI shapes. `constraints` and
//! `effects` describe target restrictions; `provenance` retains semantic,
//! proof and fuel links. Physical homes belong to the allocated representation.
//!
//! Moving or eliminating an instruction must preserve or explicitly transport
//! those links. A register copy is not permission to duplicate a linear value.

pub mod calls;
pub mod constraints;
pub mod control_flow;
pub mod effects;
pub mod identity;
pub mod instructions;
pub mod provenance;
pub mod values;

pub use calls::projected_return::{
    SelectedProjectedStructuralCallReturn, SelectedProjectedStructuralCallReturnRecipe,
    SelectedStructuralCallConstraint, SelectedStructuralCopyConstraint,
    SelectedStructuralCopyOperand, SelectedStructuralFixedOperand,
    SelectedStructuralFragmentConstraint, SelectedStructuralFragmentSite,
    SelectedStructuralReturnConstraint, SelectedStructuralTransfer,
};
pub use calls::structural_unit::{
    SelectedBoundarySettlement, SelectedMicrosoftX64OwnedIndirectPairLayout,
    SelectedStructuralUnitAbi, SelectedStructuralUnitAbiRecipe, SelectedStructuralUnitCallArgument,
    SelectedStructuralUnitCallInstruction, SelectedStructuralUnitCallSource,
    SelectedStructuralUnitFunction, SelectedStructuralUnitIndirectBinding,
    SelectedStructuralUnitParameter, SelectedStructuralUnitReturn,
};
pub use constraints::{
    SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedSelectionConstraints,
};
pub use control_flow::{SelectedBlock, SelectedFunction, SelectedSuccessor, SelectedTerminator};
pub use effects::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, MachineAlternative,
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogIdentity, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, StructuralUnitCallBarrier, StructuralUnitCallEffect,
    StructuralUnitCallEffectDeclaration, StructuralUnitCallFrameEffect,
    StructuralUnitCallMachineEffects, StructuralUnitCallMemoryEffect,
    StructuralUnitFunctionMachineEffects, ValidatedMachineEffectCatalog,
    machine_effect_catalog_identity, pre_allocation_machine_effect_identity,
    validate_machine_effect_catalog,
};
pub use identity::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
pub use instructions::{SelectedInstruction, SelectedInstructionKind};
pub use provenance::{SelectedInstructionProvenance, SelectionCustodyReceipt};
pub use values::{SelectedOperand, VirtualRegister, VirtualRegisterOrigin};

use semantic_vocabulary::{FuelScheduleIdentity, MachineId};
use target::NativeTarget;
use terminal_psi::TerminalPsiIdentity;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstructionPlan {
    pub psi: TerminalPsiIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub target: NativeTarget,
    pub entry: MachineId,
    pub functions: Vec<SelectedFunction>,
    /// Structural-ABI Unit functions are deliberately kept out of the scalar
    /// VReg roster. Their selected call bundle has no allocator-managed value
    /// and cannot acquire a fabricated scalar operand merely to enter the
    /// ordinary instruction vocabulary.
    pub structural_unit_functions: Vec<SelectedStructuralUnitFunction>,
    /// Atomic result-bearing structural selections retain their own semantic
    /// and ABI roster. They intentionally create no scalar virtual register.
    pub projected_structural_call_returns: Vec<SelectedProjectedStructuralCallReturn>,
}

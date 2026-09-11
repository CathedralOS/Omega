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

pub mod calls;
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
pub mod provenance;
pub mod structural_case;
pub mod values;

pub use calls::ordinary::*;
pub use constraints::{
    SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedSelectionConstraints,
};
pub use control_flow::{
    SelectedBlock, SelectedBlockOrigin, SelectedFunction, SelectedStructuralBinding,
    SelectedStructuralTransport, SelectedSuccessor, SelectedSuccessorRole, SelectedTerminator,
    SelectedValueBinding, SelectedValueTransport,
};
pub use effects::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, MachineAlternative,
    MachineAlternativeApplicability, MachineAlternativeFamily, MachineAlternativeKey,
    MachineBarrier, MachineCallEffect, MachineCleanupEffect, MachineEffectCatalog,
    MachineEffectCatalogIdentity, MachineEffectCatalogValidationError, MachineEffectDeclaration,
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedMemoryEffect,
    MachineEncodedStackEffect, MachineEncodedTrapBehavior, MachineLatencyKnowledge,
    MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, ValidatedMachineEffectCatalog, machine_effect_catalog_identity,
    pre_allocation_machine_effect_identity, validate_machine_effect_catalog,
};
pub use identity::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
pub use instructions::{SelectedInstruction, SelectedInstructionKind};
pub use live_ranges::*;
pub use liveness::*;
pub use provenance::SelectedInstructionProvenance;
pub use structural_case::{
    SelectedCasePayloadBinding, SelectedCasePayloadTransport, SelectedStructuralCaseEdge,
};
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
    pub functions: SelectedFunctions,
}

#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Pre-allocation instruction and machine-effect carriers.
//!
//! Selected plans retain virtual-register CFG, structural-Unit ABI, target
//! constraints, provenance, and independently admitted machine effects.

mod constraints;
mod identity;
mod instruction;
mod machine_effects;
mod plan;
mod structural_unit;

pub use constraints::{
    SelectedConstraintKeys, SelectedFixedInputConstraint, SelectedSelectionConstraints,
};
pub use identity::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
pub use instruction::{
    SelectedBlock, SelectedFunction, SelectedInstruction, SelectedInstructionKind,
    SelectedInstructionProvenance, SelectedOperand, SelectedSuccessor, SelectedTerminator,
    VirtualRegister, VirtualRegisterOrigin,
};
pub use machine_effects::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, StructuralUnitCallBarrier, StructuralUnitCallEffect,
    StructuralUnitCallEffectDeclaration, StructuralUnitCallFrameEffect,
    StructuralUnitCallMemoryEffect, ValidatedMachineEffectCatalog, machine_effect_catalog_identity,
    validate_machine_effect_catalog,
};
pub use plan::SelectedInstructionPlan;
pub use structural_unit::{
    SelectedBoundarySettlement, SelectedMicrosoftX64OwnedIndirectPairLayout,
    SelectedStructuralUnitAbi, SelectedStructuralUnitAbiRecipe, SelectedStructuralUnitCallArgument,
    SelectedStructuralUnitCallInstruction, SelectedStructuralUnitCallSource,
    SelectedStructuralUnitFunction, SelectedStructuralUnitIndirectBinding,
    SelectedStructuralUnitParameter, SelectedStructuralUnitReturn,
};

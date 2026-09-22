//! Optimizer module role: stage group. Target effect vocabulary and program effects.
//!
//! `catalog` describes the target's legal mechanisms. `program` records the
//! effects of one selected program with exact source and environment identities.

pub mod catalog;
pub mod program;

pub use catalog::{
    MachineAlternative, MachineAlternativeApplicability, MachineAlternativeFamily,
    MachineAlternativeKey, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineEffectCatalog, MachineEffectCatalogIdentity, MachineEffectCatalogValidationError,
    MachineEffectDeclaration, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedMemoryEffect, MachineEncodedStackEffect, MachineEncodedTrapBehavior,
    MachineLatencyKnowledge, MachineMemoryEffect, MachineSemanticKind, MachineSizeKnowledge,
    MachineTrapBehavior, ValidatedMachineEffectCatalog, machine_effect_catalog_identity,
    saturating_family_tag, validate_machine_effect_catalog,
};
pub use program::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, pre_allocation_machine_effect_identity,
};

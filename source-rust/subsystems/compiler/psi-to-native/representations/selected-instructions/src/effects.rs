/*
  Target machine-effect catalog vocabulary (`catalog`) and per-program
  pre-allocation machine effects (`program`).
*/

pub mod catalog;
pub mod program;

pub use catalog::{
    IsaCatalogValidation, MachineAlternative, MachineAlternativeApplicability,
    MachineAlternativeFamily, MachineAlternativeKey, MachineBarrier, MachineCallEffect,
    MachineCleanupEffect, MachineEffectCatalog, MachineEffectCatalogIdentity,
    MachineEffectCatalogValidationError, MachineEffectDeclaration, MachineEncodedControlEffect,
    MachineEncodedEffects, MachineEncodedMemoryEffect, MachineEncodedStackEffect,
    MachineEncodedTrapBehavior, MachineIdentityBytes, MachineLatencyKnowledge, MachineMemoryEffect,
    MachineSemanticKind, MachineSizeKnowledge, MachineTrapBehavior, PreparedMachineEffectCatalog,
    alternative_family_tag, canonical_machine_effect_catalog, encode_machine_alternative_identity,
    encode_machine_alternative_key_identity, encode_machine_encoded_effects_identity,
    machine_effect_catalog_identity, saturating_family_tag, trapping_family_tag,
    validate_isa_machine_effect_catalog, validate_machine_effect_catalog,
};
pub use program::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, pre_allocation_machine_effect_identity,
};

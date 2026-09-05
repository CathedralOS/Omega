//! Optimizer module role: stage group. Target effect vocabulary and program effects.
//!
//! `catalog` describes the target's legal mechanisms. `program` records the
//! effects of one selected program with exact source and environment identities.

pub mod catalog;
pub mod program;

pub use catalog::*;
pub use program::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, StructuralUnitCallMachineEffects,
    StructuralUnitFunctionMachineEffects, pre_allocation_machine_effect_identity,
};

#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Analyze the current selected program's effects.
//!
//! Input is a validated selected representation and an explicit target environment.
//! Output binds that program, environment and effect catalog, independently of
//! which optimization passes produced it. Upstream transformation replay belongs
//! to the producer of the selected input, not to this analysis.

mod analysis;
mod catalog;
mod error;
mod facts;

pub use error::MachineEffectStageError;
pub use facts::{
    BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects, MachineEffectError,
    PreAllocationMachineEffectDecodeError, PreAllocationMachineEffectIdentity,
    PreAllocationMachineEffectPlan, PreAllocationMachineEffectReceipt,
    ValidatedPreAllocationMachineEffects, pre_allocation_machine_effect_identity,
    validate_pre_allocation_machine_effects,
};
pub(crate) use facts::{analyze_pre_allocation_machine_effects, machine_semantic_kind};

use crate::ValidatedSelectedAnalysis;
use register_environment::ValidatedTargetRegisterEnvironment;

/// Construct and independently validate the target's machine-effect catalog
/// against `constraints`.
///
/// Optimizations whose descriptors admit a machine-effect surface bind this
/// catalog's identity and resolve producer, consumer, and rewritten
/// declarations in it rather than re-deriving target-specific declarations.
pub(crate) fn validated_machine_effect_catalog(
    target: target::NativeTarget,
    constraints: &register_model::ValidatedRegisterConstraintCatalog,
) -> Result<selected_instructions::ValidatedMachineEffectCatalog, MachineEffectStageError> {
    catalog::validated_catalog(target, constraints)
}

/// Construct and independently replay effects for the current selected program.
pub fn analyze_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    environment: &ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectStageError> {
    let effects = analysis::analyze(selected, environment)?;
    validate_machine_effects(selected, environment, &effects)?;
    Ok(effects)
}

/// Rejoin a retained effect result to the exact selected program and target.
pub fn validate_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    environment: &ValidatedTargetRegisterEnvironment,
    effects: &ValidatedPreAllocationMachineEffects,
) -> Result<(), MachineEffectStageError> {
    analysis::revalidate(selected, environment, effects)?;
    Ok(())
}

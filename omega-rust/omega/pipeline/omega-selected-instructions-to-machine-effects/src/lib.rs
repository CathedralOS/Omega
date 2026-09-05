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

pub use error::MachineEffectStageError;

use omega_machine_optimizer::ValidatedPreAllocationMachineEffects;
use omega_selected_instructions_to_register_homes::ValidatedSelectedAnalysis;
use omega_target_to_register_environment::ValidatedTargetRegisterEnvironment;

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

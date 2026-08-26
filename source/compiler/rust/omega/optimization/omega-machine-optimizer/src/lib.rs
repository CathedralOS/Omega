#![forbid(unsafe_code)]

//! Independently reconstructed machine-effect facts for the clean selected CFG.
//!
//! This crate owns no rewrite schedule, physical-home assignment, encoder, or
//! publication authority. Its sealed inputs are already validated selected
//! plans, and its output is a sidecar over those immutable instructions.

mod effect_codec;
mod effect_compute;
mod effect_identity;
mod effect_model;
mod effect_validate;

pub use effect_codec::TerminalPreAllocationMachineEffectDecodeError;
pub use effect_identity::terminal_pre_allocation_machine_effect_identity;
pub use effect_model::*;
pub use effect_validate::validate_terminal_pre_allocation_machine_effects;

use omega_regalloc::ValidatedTerminalSelectedAnalysis;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_terminal_selected_instructions::ValidatedTerminalMachineEffectCatalog;

/// Compute and independently reconstruct the complete pre-allocation effect
/// sidecar. This grants no transformation, home, emission, or publication
/// authority.
#[allow(clippy::too_many_arguments)]
pub fn analyze_terminal_pre_allocation_machine_effects<S: ValidatedTerminalSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedTerminalMachineEffectCatalog,
) -> Result<ValidatedTerminalPreAllocationMachineEffects, TerminalMachineEffectError> {
    let plan = effect_compute::compute_terminal_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
    )?;
    validate_terminal_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
        plan,
    )
}

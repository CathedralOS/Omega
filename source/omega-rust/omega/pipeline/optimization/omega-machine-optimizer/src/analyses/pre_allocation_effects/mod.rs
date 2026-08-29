//! Complete machine-effect analysis for one validated selected CFG.

pub(crate) mod codec;
mod compute;
pub(crate) mod identity;
mod model;
mod validate;

pub use codec::PreAllocationMachineEffectDecodeError;
pub use identity::pre_allocation_machine_effect_identity;
pub use model::*;
pub use validate::validate_pre_allocation_machine_effects;

use omega_regalloc::ValidatedSelectedAnalysis;
use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile,
};
use omega_selected_instructions::ValidatedMachineEffectCatalog;

/// Compute and independently reconstruct the complete pre-allocation effect
/// sidecar. This grants no transformation, home, emission, or publication
/// authority.
#[allow(clippy::too_many_arguments)]
pub fn analyze_pre_allocation_machine_effects<S: ValidatedSelectedAnalysis>(
    selected: &S,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    catalog: &ValidatedMachineEffectCatalog,
) -> Result<ValidatedPreAllocationMachineEffects, MachineEffectError> {
    let plan = compute::compute_terminal_pre_allocation_machine_effects(
        selected,
        register_environment,
        physical,
        constraints,
        reservations,
        selected_keys,
        catalog,
    )?;
    validate_pre_allocation_machine_effects(
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

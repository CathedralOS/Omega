//! Whole-plan identity and structural-function custody.

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    RegisterHomeError, RegisterHomePlan, ValidatedAllocationLegality, ValidatedLiveRanges,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
    plan: &RegisterHomePlan,
) -> Result<(), RegisterHomeError> {
    if plan.legality != legality.receipt().identity()
        || plan.ranges != ranges.receipt().identity()
        || plan.register_environment != register_environment
        || plan.allocator_availability != legality.receipt().allocator_availability()
        || legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || constraints.physical_identity() != physical.identity()
        || reservations.physical_identity() != physical.identity()
        || reservations.target() != ranges.plan().target
        || target_register_environment_identity(
            ranges.plan().target,
            physical,
            constraints,
            reservations,
            selected_keys,
        ) != register_environment
        || plan.functions.len() != legality.plan().functions.len()
        || plan.functions.len() != ranges.plan().functions.len()
        || plan.structural_unit_functions.len() != legality.plan().structural_unit_functions.len()
        || plan.structural_unit_functions.len() != ranges.plan().structural_unit_functions.len()
    {
        return Err(RegisterHomeError::RootMismatch);
    }
    for (function_index, ((actual, legality), ranges)) in plan
        .structural_unit_functions
        .iter()
        .zip(&legality.plan().structural_unit_functions)
        .zip(&ranges.plan().structural_unit_functions)
        .enumerate()
    {
        if actual.machine != legality.machine
            || actual.machine != ranges.machine
            || !actual.assignments.is_empty()
            || !legality.virtual_registers.is_empty()
            || !ranges.virtual_registers.is_empty()
        {
            return Err(RegisterHomeError::FunctionMismatch {
                function: function_index,
            });
        }
    }
    Ok(())
}

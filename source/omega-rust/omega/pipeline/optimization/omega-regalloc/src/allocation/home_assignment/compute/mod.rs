//! Deterministic transition-free physical-home assignment.

mod conflicts;
mod domain;
mod placement;

use omega_register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog,
    ValidatedRegisterReservationProfile, target_register_environment_identity,
};

use crate::{
    FunctionRegisterHomes, RegisterHomeError, RegisterHomePlan, ValidatedAllocationLegality,
    ValidatedLiveRanges,
};

pub(crate) use placement::compute_function;

pub(crate) fn compute_terminal_register_homes(
    legality: &ValidatedAllocationLegality,
    ranges: &ValidatedLiveRanges,
    register_environment: TargetRegisterEnvironmentIdentity,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
    reservations: &ValidatedRegisterReservationProfile,
    selected_keys: TargetRegisterEnvironmentConstraintKeys,
) -> Result<RegisterHomePlan, RegisterHomeError> {
    if legality.receipt().ranges() != ranges.receipt().identity()
        || legality.receipt().register_environment() != register_environment
        || ranges.plan().target.architecture != physical.model().architecture
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
        || legality.plan().functions.len() != ranges.plan().functions.len()
        || legality.plan().structural_unit_functions.len()
            != ranges.plan().structural_unit_functions.len()
    {
        return Err(RegisterHomeError::RootMismatch);
    }
    let functions = legality
        .plan()
        .functions
        .iter()
        .zip(&ranges.plan().functions)
        .enumerate()
        .map(|(index, (legality, ranges))| {
            if legality.machine != ranges.machine {
                return Err(RegisterHomeError::FunctionMismatch { function: index });
            }
            compute_function(index, legality, ranges, physical)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let structural_unit_functions = legality
        .plan()
        .structural_unit_functions
        .iter()
        .zip(&ranges.plan().structural_unit_functions)
        .enumerate()
        .map(|(index, (legality, ranges))| {
            if legality.machine != ranges.machine
                || !legality.virtual_registers.is_empty()
                || !ranges.virtual_registers.is_empty()
            {
                return Err(RegisterHomeError::FunctionMismatch { function: index });
            }
            Ok(FunctionRegisterHomes {
                machine: ranges.machine,
                assignments: Vec::new(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RegisterHomePlan {
        legality: legality.receipt().identity(),
        ranges: ranges.receipt().identity(),
        register_environment,
        allocator_availability: legality.receipt().allocator_availability(),
        functions,
        structural_unit_functions,
    })
}

//! Optimizer module role: executable entrance. Canonical selected-plan construction by result family.
//!
//! This entrance owns the complete function-roster join. Scalar, plain Unit,
//! and structural Unit mechanics descend into their named family entrances.

mod scalar;
mod structural_unit;
mod unit;

use crate::selection::constraints::require_key_rows;
use crate::selection::shared::*;

pub(super) fn build_plan(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedInstructionPlan, SelectedInstructionError> {
    let target = legalized.plan();
    if !target.projected_structural_call_returns.is_empty() {
        return Err(SelectedInstructionError::ProjectedStructuralCallReturnNotYetSelectable);
    }
    require_key_rows(constraints.keys, catalog)?;
    let mut functions = target
        .functions
        .iter()
        .enumerate()
        .map(|(index, source)| scalar::build(index, source, constraints, physical, catalog))
        .collect::<Result<Vec<_>, _>>()?;
    functions.extend(
        target
            .unit_functions
            .iter()
            .map(|source| unit::build(source, constraints.keys, catalog))
            .collect::<Result<Vec<_>, _>>()?,
    );
    functions.sort_by_key(|function| function.machine);
    let mut structural_unit_functions = target
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(index, source)| {
            structural_unit::build(index, source, target, constraints.keys, catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;
    structural_unit_functions.sort_by_key(|function| function.machine);
    Ok(SelectedInstructionPlan {
        psi: target.psi,
        fuel_schedule: target.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions,
        structural_unit_functions,
    })
}

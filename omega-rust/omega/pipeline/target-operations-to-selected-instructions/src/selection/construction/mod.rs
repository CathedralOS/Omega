//! Optimizer module role: executable entrance. Canonical selected-plan construction by result family.
//!
//! This entrance owns the complete function-roster join. Scalar, plain Unit,
//! and structural Unit mechanics descend into their named family entrances.

mod projected_structural_call_return;
mod scalar_graph;

use crate::selection::constraints::require_key_rows;
use crate::selection::shared::*;

pub(super) fn build_plan(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedInstructionPlan, SelectedInstructionError> {
    let target = legalized.plan();
    require_key_rows(&constraints.keys, catalog)?;
    let mut functions = target
        .scalar_functions
        .iter()
        .enumerate()
        .map(|(index, source)| {
            scalar_graph::build(index, source, target.target, constraints, physical, catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;
    functions.sort_by_key(|function| function.machine);
    let projected_structural_call_returns = target
        .projected_structural_call_returns
        .iter()
        .map(|source| {
            projected_structural_call_return::select(
                source,
                legalized.receipt().identity(),
                constraints,
                physical,
                catalog,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SelectedInstructionPlan {
        psi: target.psi,
        fuel_schedule: target.fuel_schedule,
        target: target.target,
        entry: target.entry,
        functions,
        projected_structural_call_returns,
    })
}

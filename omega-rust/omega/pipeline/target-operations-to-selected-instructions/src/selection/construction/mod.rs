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
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<SelectedInstructionPlan, SelectedInstructionError> {
    let target = legalized.plan();
    if target.target != environment.target() {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let physical = environment.physical();
    let catalog = environment.constraints();
    require_key_rows(&constraints.keys, catalog)?;
    let mut functions = target
        .scalar_functions
        .iter()
        .enumerate()
        .map(|(index, source)| {
            scalar_graph::build_with_environment(index, source, constraints, environment)
        })
        .collect::<Result<Vec<_>, _>>()?;
    functions.sort_by_key(|function| function.machine);
    for (index, function) in functions.iter_mut().enumerate() {
        super::edge_transfers::prepare(index, function, constraints, catalog)?;
    }
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

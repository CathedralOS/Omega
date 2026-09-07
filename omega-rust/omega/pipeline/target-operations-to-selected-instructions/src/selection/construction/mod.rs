//! Optimizer module role: executable entrance. Canonical selected-plan construction by result family.
//!
//! This entrance owns the complete function-roster join. Scalar, plain Unit,
//! and structural Unit mechanics descend into their named family entrances.

mod integer_sequence;
mod projected_structural_call_return;
mod scalar;
mod scalar_graph;
mod scalar_leaf;
mod shared_return;
mod structural_unit;

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
        .functions
        .iter()
        .enumerate()
        .map(|(index, source)| match source {
            legalized_operations::LegalizedFunction::SharedReturnConditional(source) => {
                shared_return::build(index, source, constraints, physical, catalog)
            }
            legalized_operations::LegalizedFunction::Conditional(source) => {
                scalar::build(index, source, constraints, physical, catalog)
            }
            legalized_operations::LegalizedFunction::Leaf(source) => {
                scalar_leaf::build(index, source, target.target, constraints, physical, catalog)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    functions.extend(
        target
            .scalar_functions
            .iter()
            .enumerate()
            .map(|(index, source)| {
                scalar_graph::build(
                    index + target.functions.len(),
                    source,
                    target.target,
                    constraints,
                    physical,
                    catalog,
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    functions.sort_by_key(|function| function.machine);
    let mut structural_unit_functions = target
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(index, source)| {
            structural_unit::build(index, source, target, &constraints.keys, catalog)
        })
        .collect::<Result<Vec<_>, _>>()?;
    structural_unit_functions.sort_by_key(|function| function.machine);
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
        structural_unit_functions,
        projected_structural_call_returns,
    })
}

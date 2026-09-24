//! Optimizer module role: executable entrance. Source-shape projection entrance: roster custody before function lowering.

mod function;

use abstract_operations::{AbstractOperationPlan, AbstractParameter};
use optimization_unit::{PsiOptimizationUnit, ValueDefinition, ValueDefinitionSite};

use crate::OptimizedAbstractProjectionError;

pub(super) fn project_plan(
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<AbstractOperationPlan, OptimizedAbstractProjectionError> {
    validate_function_roster(source, unit)?;
    let functions = unit
        .functions
        .iter()
        .map(function::project)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AbstractOperationPlan {
        psi: unit.psi,
        entry: unit.entry,
        structural_types: unit.structural_types.clone(),
        boundary_machines: unit.boundary_machines.clone(),
        provider_candidates: unit.provider_candidates.clone(),
        functions,
    })
}

fn validate_function_roster(
    source: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), OptimizedAbstractProjectionError> {
    if source.functions.len() != unit.functions.len() + unit.pruned_machines.len() {
        return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
    }
    let mut active = unit.functions.iter();
    let mut next_active = active.next();
    for (ordinal, source_function) in source.functions.iter().enumerate() {
        if next_active.is_some_and(|function| function.machine == source_function.machine) {
            next_active = active.next();
            continue;
        }
        let ordinal = u32::try_from(ordinal)
            .map_err(|_| OptimizedAbstractProjectionError::FunctionRosterMismatch)?;
        if !unit.pruned_machines.iter().any(|custody| {
            custody.source_ordinal == ordinal && custody.machine == source_function.machine
        }) {
            return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
        }
    }
    if next_active.is_some() {
        return Err(OptimizedAbstractProjectionError::FunctionRosterMismatch);
    }
    Ok(())
}

pub(super) fn project_parameter(
    definition: &ValueDefinition,
    expected_site: ValueDefinitionSite,
    error: OptimizedAbstractProjectionError,
) -> Result<AbstractParameter, OptimizedAbstractProjectionError> {
    if definition.site != expected_site {
        return Err(error);
    }
    Ok(AbstractParameter {
        value: definition.value,
        scalar_type: definition.scalar_type,
    })
}

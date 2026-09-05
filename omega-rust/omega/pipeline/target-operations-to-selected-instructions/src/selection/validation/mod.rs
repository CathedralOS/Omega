//! Optimizer module role: executable entrance. Independent selected-plan reconstruction and admission.

mod blocks;
mod functions;
mod integrity;
mod projected_structural_call_return;
mod roots;
mod scalar_call_unit;
mod structural_unit;
mod virtual_registers;

use super::identity::receipt;
use super::shared::*;
use functions::{validate_function, validate_structural_unit_function, validate_unit_function};

pub fn validate_selected_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    plan: SelectedInstructionPlan,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let target = legalized.plan();
    roots::validate_initial_roots(target, constraints, physical, catalog, &plan)?;
    for (function_index, selected) in plan.functions.iter().enumerate() {
        let scalar = target
            .functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        let unit = target
            .unit_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        let scalar_call_unit = target
            .scalar_call_unit_functions
            .iter()
            .filter(|source| source.machine == selected.machine)
            .collect::<Vec<_>>();
        match (
            scalar.as_slice(),
            unit.as_slice(),
            scalar_call_unit.as_slice(),
        ) {
            ([source], [], []) => validate_function(
                function_index,
                source,
                selected,
                constraints,
                physical,
                catalog,
            )?,
            ([], [source], []) => {
                validate_unit_function(function_index, source, selected, constraints.keys, catalog)?
            }
            ([], [], [source]) => {
                scalar_call_unit::validate(function_index, source, selected, constraints, catalog)?
            }
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    roots::validate_structural_roster(target, &plan)?;
    for (function_index, selected) in plan.structural_unit_functions.iter().enumerate() {
        let Some(source) = target
            .structural_unit_functions
            .iter()
            .find(|source| source.machine == selected.machine)
        else {
            return Err(SelectedInstructionError::SourceCustodyMismatch);
        };
        validate_structural_unit_function(
            function_index + plan.functions.len(),
            source,
            selected,
            target,
            constraints.keys,
            catalog,
        )?;
    }
    for (source, selected) in target
        .projected_structural_call_returns
        .iter()
        .zip(&plan.projected_structural_call_returns)
    {
        projected_structural_call_return::validate(
            source,
            legalized.receipt().identity(),
            selected,
            constraints,
            physical,
            catalog,
        )?;
    }
    let receipt = receipt(&plan, legalized);
    Ok(ValidatedSelectedInstructions {
        plan: plan.into(),
        receipt,
    })
}

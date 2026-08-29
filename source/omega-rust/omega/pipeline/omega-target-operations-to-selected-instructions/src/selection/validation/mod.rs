//! Independent selected-plan reconstruction and admission.

mod blocks;
mod functions;
mod integrity;
mod virtual_registers;

use super::constraints::require_key_rows;
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
    if target.psi != plan.psi
        || target.target != plan.target
        || target.entry != plan.entry
        || target.fuel_schedule != plan.fuel_schedule
        || physical.model().architecture != target.target.architecture
        || catalog.architecture() != target.target.architecture
    {
        return Err(SelectedInstructionError::TargetRegisterArchitectureMismatch);
    }
    if target.functions.len() + target.unit_functions.len() != plan.functions.len()
        || target.structural_unit_functions.len() != plan.structural_unit_functions.len()
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let mut expected_machines = target
        .functions
        .iter()
        .map(|function| function.machine)
        .chain(
            target
                .unit_functions
                .iter()
                .map(|function| function.machine),
        )
        .collect::<Vec<_>>();
    expected_machines.sort_unstable();
    if plan
        .functions
        .iter()
        .map(|function| function.machine)
        .ne(expected_machines)
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let expected_fixed_inputs = target
        .functions
        .iter()
        .map(|source| {
            1 + usize::from(matches!(
                source.when_true.value,
                SourceLeafValue::EntryParameter { .. }
            ))
        })
        .sum::<usize>();
    if constraints.fixed_inputs.len() != expected_fixed_inputs {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    require_key_rows(constraints.keys, catalog)?;
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
        match (scalar.as_slice(), unit.as_slice()) {
            ([source], []) => validate_function(
                function_index,
                source,
                selected,
                constraints,
                physical,
                catalog,
            )?,
            ([], [source]) => {
                validate_unit_function(function_index, source, selected, constraints.keys, catalog)?
            }
            _ => return Err(SelectedInstructionError::SourceCustodyMismatch),
        }
    }
    let mut expected_structural_machines = target
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .collect::<Vec<_>>();
    expected_structural_machines.sort_unstable();
    if plan
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .ne(expected_structural_machines)
    {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
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
    let receipt = receipt(&plan, legalized);
    Ok(ValidatedSelectedInstructions { plan, receipt })
}

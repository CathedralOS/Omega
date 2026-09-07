//! Optimizer module role: executable entrance. Independent selected-plan reconstruction and admission.

mod blocks;
mod def_use;
mod functions;
pub(super) mod integer_sequence;
mod integrity;
mod ordinary_roster;
mod projected_structural_call_return;
mod roots;
pub(super) mod scalar_graph;
mod structural_unit;
mod virtual_registers;

use super::identity::receipt;
use super::shared::*;
use functions::validate_structural_unit_function;

pub fn validate_selected_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    plan: SelectedInstructionPlan,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let target = legalized.plan();
    roots::validate_initial_roots(target, constraints, physical, catalog, &plan)?;
    ordinary_roster::validate(target, &plan.functions, constraints, physical, catalog)?;
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
            &constraints.keys,
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

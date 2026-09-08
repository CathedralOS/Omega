//! Optimizer module role: executable entrance. Independent selected-plan reconstruction and admission.

mod def_use;
mod integrity;
mod ordinary_roster;
mod projected_structural_call_return;
mod roots;
pub(super) mod scalar_graph;
#[cfg(test)]
mod structural_case_tests;

use super::identity::receipt;
use super::shared::*;

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

//! Optimizer module role: executable entrance. Independent selected-plan reconstruction and admission.

mod def_use;
mod integrity;
mod ordinary_roster;
mod roots;
pub(super) mod scalar_graph;
#[cfg(test)]
mod structural_case_tests;
pub(in crate::selection) mod value_transport;

use super::identity::receipt;
use super::shared::*;

pub fn validate_selected_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
    plan: SelectedInstructionPlan,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let environment = register_environment::validate_target_register_environment(
        legalized.plan().target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?;
    validate_with_environment(legalized, constraints, &environment, plan)
}

pub(super) fn validate_with_environment(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    plan: SelectedInstructionPlan,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let target = legalized.plan();
    if target.target != environment.target() {
        return Err(SelectedInstructionError::SourceCustodyMismatch);
    }
    let physical = environment.physical();
    let catalog = environment.constraints();
    roots::validate_initial_roots(target, constraints, physical, catalog, &plan)?;
    ordinary_roster::validate(target, &plan.functions, constraints, environment)?;
    let receipt = receipt(&plan, legalized);
    Ok(ValidatedSelectedInstructions {
        plan: plan.into(),
        receipt,
    })
}

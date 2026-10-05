//! Optimizer module role: executable entrance. Independent selected-plan reconstruction and admission.

use crate::legalization::ValidatedLegalizedOperations;
use crate::selection::model::{SelectedInstructionError, ValidatedSelectedInstructions};
use selected_instructions::{SelectedInstructionPlan, SelectedSelectionConstraints};
mod def_use;
mod integrity;
mod ordinary_roster;
mod roots;
pub(super) mod scalar_graph;
pub(in crate::selection) mod value_transport;

use super::identity::receipt;

pub(super) fn validate_with_environment(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
    plan: SelectedInstructionPlan,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let target = legalized.plan();
    if target.target != environment.target() {
        return Err(SelectedInstructionError::custody());
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

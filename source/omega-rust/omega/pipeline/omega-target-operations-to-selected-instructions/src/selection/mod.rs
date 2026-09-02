//! Optimizer module role: executable entrance. Instruction selection: propose one bounded selected CFG, then validate it independently.

mod constraints;
mod construction;
mod identity;
mod model;
mod shared;
mod validation;

pub use identity::{
    selected_instruction_plan_identity, selected_instruction_plan_identity_v11_legacy,
    selected_instruction_plan_identity_v13_legacy,
};
pub use model::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
};
pub use validation::validate_selected_instructions;

use omega_register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use omega_selected_instructions::SelectedSelectionConstraints;

use crate::legalization::ValidatedLegalizedOperations;
use construction::build_plan;

pub fn select_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let plan = build_plan(legalized, constraints, physical, catalog)?;
    validate_selected_instructions(legalized, constraints, physical, catalog, plan)
}

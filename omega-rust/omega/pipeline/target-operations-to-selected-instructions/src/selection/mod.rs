//! Optimizer module role: executable entrance. Instruction selection: propose one bounded selected CFG, then validate it independently.
//!
//! Validate the immutable target/register/catalog join once at this entrance.
//! Construction and independent replay borrow it across functions; rebuilding
//! the same catalog for every function or replay mutation adds no evidence.
//! The public raw validation entrance still validates its own supplied join.

mod aggregate_result_input;
mod block_order;
mod byte_view_homes;
mod constraints;
mod construction;
mod edge_transfers;
mod established_view_input;
mod identity;
mod literal_storage_input;
mod model;
pub(crate) mod primitive_local_input;
mod read_result_input;
mod scalar_array_input;
mod scalar_call_abi;
mod shared;
mod validation;
pub(crate) mod value_transport;

pub use identity::selected_instruction_plan_identity;
pub use model::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
};
pub use validation::validate_selected_instructions;

use register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use selected_instructions::SelectedSelectionConstraints;

use crate::legalization::ValidatedLegalizedOperations;
use construction::build_plan;

pub fn select_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let environment = register_environment::validate_target_register_environment(
        legalized.plan().target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| SelectedInstructionError::SourceCustodyMismatch)?;
    let plan = build_plan(legalized, constraints, &environment)?;
    validation::validate_with_environment(legalized, constraints, &environment, plan)
}

//! Optimizer module role: executable entrance. Instruction selection: propose one bounded selected CFG, then validate it independently.
//!
//! The stage entry validates the target/register/catalog join once per
//! realization and lends it here. Construction and independent replay borrow
//! that one environment across functions; rebuilding it for every function or
//! replay mutation adds no evidence.

mod address_join_input;
mod aggregate_result_input;
mod arrival_parameter_input;
mod atomic_access;
mod block_order;
mod byte_concatenation_input;
mod byte_owner_input;
mod byte_view_homes;
mod constraints;
mod construction;
mod edge_transfers;
mod element_view_homes;
mod established_view_input;
mod field_bytes;
mod identity;
mod integer_conversion;
mod literal_compare_input;
mod model;
mod owned_block_home_input;
mod parameter_use;
pub(crate) mod primitive_local_input;
mod record_input;
mod reference_input;
mod returned_view_input;
mod scalar_array_input;
mod scalar_call_abi;
mod structural_array_input;
mod structural_local_input;
mod validation;
pub(crate) mod value_transport;
mod zeroed_structural_input;

pub use model::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
};
pub use selected_instructions::selected_instruction_plan_identity;

use selected_instructions::SelectedSelectionConstraints;

use crate::legalization::ValidatedLegalizedOperations;
use construction::build_plan;

pub fn select_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let plan = build_plan(legalized, constraints, environment)?;
    validation::validate_with_environment(legalized, constraints, environment, plan)
}

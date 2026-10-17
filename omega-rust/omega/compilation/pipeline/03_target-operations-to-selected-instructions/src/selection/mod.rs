//! Optimizer module role: executable entrance. Instruction selection: construct one bounded selected CFG and seal it.
//!
//! The stage entry validates the target/register/catalog join once per
//! realization and lends it here. Construction borrows that one environment
//! across functions.

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
mod storage_origin_input;
mod scalar_array_input;
pub(crate) use scalar_call_abi::scalar_shape;
mod scalar_call_abi;
mod structural_array_input;
mod structural_local_input;
pub(crate) mod value_transport;
mod zeroed_structural_input;

pub(crate) use model::SelectedInstructionError;
pub use model::ValidatedSelectedInstructions;
pub use selected_instructions::selected_instruction_plan_identity;

use selected_instructions::SelectedSelectionConstraints;

use crate::legalization::ValidatedLegalizedOperations;
use construction::build_plan;

pub(crate) fn select_instructions(
    legalized: &ValidatedLegalizedOperations,
    constraints: &SelectedSelectionConstraints,
    environment: &register_environment::ValidatedTargetRegisterEnvironment,
) -> Result<ValidatedSelectedInstructions, SelectedInstructionError> {
    let plan = build_plan(legalized, constraints, environment)?;
    let receipt = identity::receipt(&plan, legalized);
    Ok(ValidatedSelectedInstructions {
        plan: plan.into(),
        receipt,
    })
}

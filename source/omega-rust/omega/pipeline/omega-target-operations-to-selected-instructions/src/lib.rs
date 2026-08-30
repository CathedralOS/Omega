#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by validated instruction selection.
//!
//! Enter `legalization/mod.rs` for the raw-target to legal-operation join and
//! `selection/mod.rs` for the legal-operation to selected-CFG join.

mod legalization;
mod selection;

pub use legalization::{
    LegalizationError, LegalizationValidationReceipt, ValidatedLegalizedOperations,
    legalization_validator_identity, legalize_target_operations, validate_legalized_operations,
};
pub use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
    select_instructions, selected_instruction_plan_identity, validate_selected_instructions,
};

#[cfg(test)]
mod tests;

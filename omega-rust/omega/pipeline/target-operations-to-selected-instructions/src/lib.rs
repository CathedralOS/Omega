#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by validated instruction selection.
//!
//! Enter `legalization/mod.rs` for the raw-target to legal-operation join and
//! `selection/mod.rs` for the legal-operation to selected-CFG join.

mod legalization;
mod optimized;
mod selection;

pub use legalization::{
    LegalizationError, LegalizationValidationReceipt,
    ProjectedStructuralCallReturnLegalizationError,
    ProjectedStructuralCallReturnLegalizationReceipt, ValidatedLegalizedOperations,
    legalization_validator_identity, legalization_validator_identity_v17_legacy,
    legalization_validator_identity_v18_legacy, legalization_validator_identity_v19_legacy,
    legalization_validator_identity_v20_legacy, legalization_validator_identity_v21_legacy,
    legalize_target_operations, validate_legalized_operations,
};
pub use optimized::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    selection_constraints, stage_optimized_instruction_selection,
    validate_optimized_selection_custody,
};
pub use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
    select_instructions, selected_instruction_plan_identity,
    selected_instruction_plan_identity_v11_legacy, selected_instruction_plan_identity_v13_legacy,
    selected_instruction_plan_identity_v14_legacy, selected_instruction_plan_identity_v15_legacy,
    selected_instruction_plan_identity_v16_legacy, validate_selected_instructions,
};

#[cfg(test)]
mod tests;

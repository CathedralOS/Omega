#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by validated instruction selection.
//!
//! Start at `optimized.rs`: the stage entrance owns the target-register
//! environment, runs legalization and selection, replays both independently
//! and joins their custody. `legalization` is the raw-target to
//! legal-operation join, `selection` the legal-operation to selected-CFG join,
//! and `structural_inputs` the input-only reconstruction of structural
//! storage, parameter shape and unobserved-owned eligibility both consume.

mod legalization;
mod optimized;
mod selection;
mod structural_inputs;

pub use legalization::{
    LegalizationError, LegalizationSource, LegalizationValidationReceipt,
    ValidatedLegalizedOperations, legalization_validator_identity,
    legalization_validator_identity_v17_legacy, legalization_validator_identity_v18_legacy,
    legalization_validator_identity_v19_legacy, legalization_validator_identity_v20_legacy,
    legalization_validator_identity_v21_legacy, legalization_validator_identity_v22_legacy,
    legalize_target_operations, validate_legalized_operations,
};
#[cfg(feature = "test-support")]
pub use optimized::OptimizedSelectionCustodyFieldForTest;
pub use optimized::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    selection_constraints, stage_optimized_instruction_selection,
    validate_optimized_selection_custody,
};
pub use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
    select_instructions, selected_instruction_plan_identity, validate_selected_instructions,
};

#[cfg(test)]
mod tests;

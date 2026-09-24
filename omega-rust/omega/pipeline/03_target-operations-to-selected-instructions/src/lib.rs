#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by validated instruction selection.
//!
//! Start at [`stage_optimized_instruction_selection`] in `optimized.rs`: it
//! owns the target-register environment, runs [`legalize_target_operations`]
//! and then [`select_instructions`], and joins their custody in
//! [`validate_optimized_selection_custody`], which replays both through
//! [`validate_legalized_operations`] and [`validate_selected_instructions`].
//! `legalization` is the raw-target to legal-operation join, `selection` the
//! legal-operation to selected-CFG join, and `structural_inputs` the
//! input-only reconstruction of structural storage, parameter shape and
//! unobserved-owned eligibility both consume.

mod legalization;
mod optimized;
mod selection;
mod structural_inputs;

// The stage entry and its custody join.
#[cfg(feature = "test-support")]
pub use optimized::OptimizedSelectionCustodyFieldForTest;
pub use optimized::{
    OptimizedSelectionCustodyError, OptimizedSelectionPipelineError,
    StagedOptimizedSelectedInstructions, StagedOptimizedSelectionCustodyReceipt,
    selection_constraints, stage_optimized_instruction_selection,
    validate_optimized_selection_custody,
};

// The two joins the entry sequences, each with its independent replay.
pub use legalization::{
    LegalizationError, LegalizationSource, LegalizationValidationReceipt,
    ValidatedLegalizedOperations, legalize_target_operations, validate_legalized_operations,
};
pub use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
    select_instructions, selected_instruction_plan_identity, validate_selected_instructions,
};

#[cfg(test)]
mod tests;

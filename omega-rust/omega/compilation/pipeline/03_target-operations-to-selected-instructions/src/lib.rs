#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Mandatory target legalization followed by instruction selection.
//!
//! Start at [`stage_optimized_instruction_selection`] in `optimized.rs`: it
//! owns the target-register environment, runs [`legalize_target_operations`]
//! and then [`select_instructions`], each of which seals its own plan with a
//! receipt, and records their identities in one custody receipt.
//! `legalization` is the raw-target to legal-operation join, `selection` the
//! legal-operation to selected-CFG join, and `structural_inputs` the
//! input-only reconstruction of structural storage, parameter shape and
//! unobserved-owned eligibility both consume.

mod legalization;
mod optimized;
mod selection;
mod structural_inputs;

// The stage entry and its custody join.
pub use optimized::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt, selection_constraints,
    stage_optimized_instruction_selection,
};

// The two joins the entry sequences.
pub use legalization::{
    LegalizationError, LegalizationSource, LegalizationValidationReceipt,
    ValidatedLegalizedOperations, legalize_target_operations,
};
pub use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt, ValidatedSelectedInstructions,
    select_instructions, selected_instruction_plan_identity,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.

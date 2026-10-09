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
//! incoming ABI placement both consume. Owned arrivals retain those contracts
//! regardless of which operations share their function; optional elimination
//! of unused storage happens only after a correct general transport exists.
//!
//! A scalar observation is not a shared-loan formation. Its native consumers
//! reconstruct declared access, backing storage, codecs and ordered crossings;
//! they do not replace Psi's admitted compatibility with another loan rule.
//! Reference formation and transport retain their separate custody contracts,
//! consistent with pipeline.md's verification-placement boundary.

mod legalization;
mod optimized;
mod selection;
mod structural_inputs;

// The stage entry and its custody join.
pub use optimized::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt,
    stage_optimized_instruction_selection,
};
pub(crate) use optimized::{ selection_constraints};

// The two joins the entry sequences.
pub use legalization::{
};
pub(crate) use legalization::{
    LegalizationError, LegalizationSource, LegalizationValidationReceipt,
    ValidatedLegalizedOperations, legalize_target_operations};
pub use selection::{ ValidatedSelectedInstructions, selected_instruction_plan_identity,
};
pub(crate) use selection::{
    SelectedInstructionError, SelectedInstructionValidationReceipt,
    select_instructions};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.

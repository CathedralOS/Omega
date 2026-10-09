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
//! Calls and storage projection consume target rows with their retained homes
//! and placements; construction never replays reference custody. The explicit
//! [`verify_target_operation_plan`] audit retains the independent source/target
//! checks for corpus and receiving review, off the default compile path.

/// `Display` for the crate's `<context> failed: {self:?}` error convention.
macro_rules! failed_display {
    ($type:ty, $message:literal) => {
        impl std::fmt::Display for $type {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!($message, ": {:?}"), self)
            }
        }
    };
}

mod legalization;
mod optimized;
mod selection;
mod structural_inputs;

// The stage entry and its custody join.
pub use optimized::{
    OptimizedSelectionPipelineError, StagedOptimizedSelectedInstructions,
    StagedOptimizedSelectionCustodyReceipt, stage_optimized_instruction_selection,
};
// The two joins the entry sequences.
pub(crate) use legalization::{
    LegalizationError, ValidatedLegalizedOperations, legalize_target_operations,
};
pub(crate) use selection::{SelectedInstructionError, select_instructions};
pub use selection::{ValidatedSelectedInstructions, selected_instruction_plan_identity};

pub use legalization::{
    LegalizationError as TargetOperationAuditError, verify_target_operation_plan,
};

// The unit tests allocate heavily across nextest's parallel processes;
// mimalloc avoids the system allocator's zone locks and free lists.
// Test-only: the product keeps the system allocator.

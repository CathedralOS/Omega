#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Explicit resolved-layout optimization.
//!
//! Baseline layout construction precedes this phase; even empty selections
//! cross its checked entrance. Both identity and relaxation expose the same
//! current layout, while retained rewrite evidence is used only for replay.

mod phase;
mod x86_branch_relaxation;

pub use phase::*;
pub use x86_branch_relaxation::*;

#[cfg(test)]
use machine_code::ResolvedSelectedBlockLayout;
use machine_code::{
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate,
    ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
};
use selected_form_encoding_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, StagedOptimizedResolvedSelectedFormLayout,
    validate_optimized_resolved_selected_form_layout,
};

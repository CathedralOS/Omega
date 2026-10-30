#![forbid(unsafe_code)]

//! Resolved layout to optimized resolved layout.
//!
//! The phase operation is `execute_resolved_layout_optimization` (`phase`),
//! checked by `validate_resolved_layout_optimization`. Baseline layout
//! construction precedes this phase, and even an empty selection crosses its
//! checked entrance. The one current rewrite is x86 conditional-branch
//! relaxation (`x86_branch_relaxation`: `stage_optimized_x86_branch_relaxation`,
//! checked against its retained roots by
//! `validate_optimized_x86_branch_relaxation`). Identity and relaxation expose
//! the same current layout; retained rewrite evidence travels with the
//! artifact.

mod phase;
mod x86_branch_relaxation;

pub use phase::{
    ResolvedLayoutOptimization, ResolvedLayoutOptimizationError,
    execute_resolved_layout_optimization, validate_resolved_layout_optimization,
};
pub(crate) use x86_branch_relaxation::{
    FunctionRelativeLayoutCatalogError, stage_optimized_x86_branch_relaxation,
    validate_optimized_x86_branch_relaxation, x86_rel8_selected,
};
pub use x86_branch_relaxation::{
    OptimizedX86BranchRelaxationError, StagedOptimizedX86BranchRelaxation,
    X86BranchRelaxationAction, X86BranchRelaxationAttempt, X86BranchRelaxationAttemptOutcome,
    X86BranchRelaxationIdentity, X86BranchRelaxationPolicy, X86BranchRelaxationRevisionIdentity,
};

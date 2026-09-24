#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Selected-form encoding to resolved layout.
//!
//! Start at `resolved_selected_form_layout.rs`: required function-relative
//! layout independently checks its result before publication; the folder
//! beneath it holds computation, the model, ordinary rows and validation.
//! Optional layout rewrites belong to the following X-to-X phase. Current
//! layout data and content identity belong to machine-code.

mod resolved_selected_form_layout;

pub use resolved_selected_form_layout::{
    OptimizedResolvedSelectedFormLayoutError, ResolvedBranchEvidence,
    ResolvedConditionalBranchEvidence, ResolvedConditionalBranchPredicate, ResolvedJumpEvidence,
    ResolvedSelectedBlockLayout, ResolvedSelectedFormLayoutIdentity, ResolvedSelectedFormRow,
    ResolvedSelectedFunctionLayout, SelectedFunctionLayoutPolicy,
    StagedOptimizedResolvedSelectedFormLayout, admit_resolved_machine_layout,
    stage_optimized_resolved_selected_form_layout,
    validate_optimized_resolved_selected_form_layout,
};

//! Producer computation join for x86 branch relaxation.
//!
//! One production route scans, inspects, charges work and reflows; the
//! canonical artifact finalizer takes its trace. The stage does not re-derive
//! the relaxation it just produced (verification placement,
//! `omega-rust/pipeline.md`).

mod artifact;
mod branch_inspection;
mod production;
mod reflow;
#[cfg(test)]
mod tests;
mod work;

use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::ValidatedPhysicalRegisterModel;

use machine_code::ResolvedSelectedFunctionLayout;
use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::{
    StagedOptimizedX86BranchRelaxation, X86BranchRelaxationAction, X86BranchRelaxationAttempt,
    error::OptimizedX86BranchRelaxationError, identity::RevisionRoots,
};

struct RelaxationTrace {
    usage: OptimizationWorkUsage,
    attempts: Vec<X86BranchRelaxationAttempt>,
    actions: Vec<X86BranchRelaxationAction>,
    functions: Vec<ResolvedSelectedFunctionLayout>,
}

/// The roots every revision and artifact identity is bound to. The producer
/// takes them as data, so it needs no staged carrier and is exercisable on a
/// bare function roster.
fn revision_roots(source: &StagedOptimizedResolvedSelectedFormLayout) -> RevisionRoots {
    RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    }
}

pub(super) fn compute_relaxation(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
    budget: OptimizationWorkBudget,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let trace =
        production::compute_trace(revision_roots(source), source.functions(), physical, budget)?;
    artifact::finish_artifact(source, budget, trace)
}

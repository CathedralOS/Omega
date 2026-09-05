//! Canonical artifact assembly shared after separate production and replay traces.

use optimization_core::OptimizationWorkBudget;

use crate::StagedOptimizedResolvedSelectedFormLayout;

use super::super::{
    error::OptimizedX86BranchRelaxationError,
    identity::{RevisionRoots, artifact_identity, revision_identity},
    model::{StagedOptimizedX86BranchRelaxation, X86BranchRelaxationPolicy},
};
use super::RelaxationTrace;

pub(super) fn finish_artifact(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
    trace: RelaxationTrace,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = RevisionRoots {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
    };
    let output_revision = revision_identity(roots, &trace.functions);
    let layout = source.with_replayed_functions(trace.functions);
    let output = layout.identity();
    let policy = X86BranchRelaxationPolicy::X86RelaxConditionalBranchesToRel8V1;
    let identity = artifact_identity(
        roots,
        policy,
        budget,
        trace.usage,
        output,
        output_revision,
        &trace.attempts,
        &trace.actions,
        layout.functions(),
    );
    Ok(StagedOptimizedX86BranchRelaxation {
        source: source.identity(),
        selected: source.selected(),
        machine: source.machine(),
        pre_layout: source.pre_layout(),
        target: source.target(),
        policy,
        budget,
        usage: trace.usage,
        output,
        output_revision,
        identity,
        attempts: trace.attempts,
        actions: trace.actions,
        layout,
    })
}

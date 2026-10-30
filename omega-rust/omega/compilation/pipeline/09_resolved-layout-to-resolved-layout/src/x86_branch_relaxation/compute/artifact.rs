//! Canonical artifact assembly over the production trace.

use optimization_core::OptimizationWorkBudget;

use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::super::{
    StagedOptimizedX86BranchRelaxation, X86BranchRelaxationPolicy,
    error::OptimizedX86BranchRelaxationError,
    identity::{artifact_identity, revision_identity},
};
use super::RelaxationTrace;

pub(super) fn finish_artifact(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    budget: OptimizationWorkBudget,
    trace: RelaxationTrace,
) -> Result<StagedOptimizedX86BranchRelaxation, OptimizedX86BranchRelaxationError> {
    let roots = super::revision_roots(source);
    let output_revision = revision_identity(roots, &trace.functions);
    let mut program = source.program().clone();
    program.functions = trace.functions;
    program.identity = program.recomputed_identity();
    let layout = std::sync::Arc::new(program);
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

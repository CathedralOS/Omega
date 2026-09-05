use register_model::ValidatedPhysicalRegisterModel;
use target::{Architecture, NativeTarget};

use crate::StagedOptimizedResolvedSelectedFormLayout;

use super::{
    error::OptimizedX86BranchRelaxationError,
    model::{
        StagedOptimizedX86BranchRelaxation, X86BranchRelaxationAction, X86BranchRelaxationAttempt,
    },
};

pub(super) fn validate_roots(
    source: &StagedOptimizedResolvedSelectedFormLayout,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    ensure_x86_target(source.target(), physical)
}

pub(super) fn ensure_x86_target(
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    if target.architecture != Architecture::X86_64
        || physical.model().architecture != Architecture::X86_64
    {
        return Err(OptimizedX86BranchRelaxationError::UnsupportedTarget(target));
    }
    Ok(())
}

pub(super) fn compare_replayed_evidence(
    artifact: &StagedOptimizedX86BranchRelaxation,
    replayed: &StagedOptimizedX86BranchRelaxation,
) -> Result<(), OptimizedX86BranchRelaxationError> {
    compare_replayed_action_evidence(
        &artifact.attempts,
        &artifact.actions,
        &replayed.attempts,
        &replayed.actions,
    )?;
    if artifact.functions() != replayed.functions()
        || artifact.output != replayed.output
        || artifact.output_revision != replayed.output_revision
        || artifact.identity != replayed.identity
    {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

pub(super) fn compare_replayed_action_evidence(
    attempts: &[X86BranchRelaxationAttempt],
    actions: &[X86BranchRelaxationAction],
    replayed_attempts: &[X86BranchRelaxationAttempt],
    replayed_actions: &[X86BranchRelaxationAction],
) -> Result<(), OptimizedX86BranchRelaxationError> {
    if attempts != replayed_attempts || actions != replayed_actions {
        return Err(OptimizedX86BranchRelaxationError::ArtifactMismatch);
    }
    Ok(())
}

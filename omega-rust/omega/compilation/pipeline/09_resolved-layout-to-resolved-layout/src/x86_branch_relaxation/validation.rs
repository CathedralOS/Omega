use register_model::ValidatedPhysicalRegisterModel;
use target::{Architecture, NativeTarget};

use selected_form_encoding_to_resolved_layout::StagedOptimizedResolvedSelectedFormLayout;

use super::error::OptimizedX86BranchRelaxationError;

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

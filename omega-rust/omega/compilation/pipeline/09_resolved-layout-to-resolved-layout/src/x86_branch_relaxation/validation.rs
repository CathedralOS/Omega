use register_model::ValidatedPhysicalRegisterModel;
use target::{Architecture, NativeTarget};

use machine_code::ResolvedMachineLayout;

use super::error::OptimizedX86BranchRelaxationError;

pub(super) fn validate_roots(
    source: &ResolvedMachineLayout,
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

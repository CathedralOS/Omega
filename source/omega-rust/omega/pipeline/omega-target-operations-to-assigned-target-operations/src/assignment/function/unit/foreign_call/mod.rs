//! Optimizer module role: executable entrance. Normalized foreign-call admission and canonical replay.

mod normalized;

use crate::assignment::shared::*;

pub(super) fn assign(
    binding: &omega_target_operations::NormalizedForeignCallBinding,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    preceding_operations: &[TargetUnitOperation],
    assigned_homes: &BTreeMap<ValueId, AssignedUnitScalarHome>,
) -> Result<Vec<AssignedNormalizedForeignScalarArgument>, AssignmentError> {
    if binding.locator.target().native_target() != target
        || !matches!(
            (target.object_format, binding.locator.locator()),
            (
                omega_target::ObjectFormat::Elf,
                omega_target::ForeignLocatorCandidate::ElfVersioned { .. }
            ) | (
                omega_target::ObjectFormat::MachO,
                omega_target::ForeignLocatorCandidate::MachODylibSymbol { .. }
            )
        )
    {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    normalized::assign_normalized_foreign_scalar_arguments_for_plan(
        &binding.boundary_entry_plan,
        target,
        scalar_arguments,
        preceding_operations,
        assigned_homes,
    )
}

#[cfg(test)]
mod tests;

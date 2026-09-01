//! Optimizer module role: executable entrance. Normalized foreign-call admission and canonical replay.

mod normalized;

use crate::assignment::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    psi_operation: OperationId,
    binding: &omega_target_operations::NormalizedForeignCallBinding,
    target: NativeTarget,
    scalar_arguments: &[omega_target_operations::NormalizedForeignScalarArgument],
    result_home: Option<omega_target_operations::TargetUnitScalarHomeRequirement>,
    preceding_operations: &[TargetUnitOperation],
    native_callback: Option<&omega_target_operations::TargetNativeCallbackArgument>,
    assigned_homes: &mut BTreeMap<ValueId, AssignedUnitScalarHome>,
    next_home: &mut u32,
) -> Result<
    (
        Vec<AssignedNormalizedForeignScalarArgument>,
        Option<AssignedUnitScalarHome>,
    ),
    AssignmentError,
> {
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
    let arguments = normalized::assign_normalized_foreign_scalar_call_for_plan(
        &binding.boundary_entry_plan,
        target,
        scalar_arguments,
        result_home.as_ref(),
        psi_operation,
        preceding_operations,
        assigned_homes,
        native_callback,
    )?;
    let result_home = result_home
        .map(|result| {
            super::scalar_call::allocate_unit_scalar_home(
                result,
                assigned_homes,
                next_home,
                AssignmentError::ExpressionStackFrameNotEncodable,
            )
        })
        .transpose()?;
    Ok((arguments, result_home))
}

#[cfg(test)]
mod tests;

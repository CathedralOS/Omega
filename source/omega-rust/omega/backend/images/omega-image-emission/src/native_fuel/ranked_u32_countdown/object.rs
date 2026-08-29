//! Object-boundary replay for charge-interleaved ranked semantic branches.

use omega_machine_code::NativeFuelRankedU32CountdownRebaseRecord;
use omega_target::NativeTarget;
use psi_core::MachineId;

use super::coordinates;
use crate::NativeFuelValidationError;

pub(super) fn admit_rebased_branches(
    target: NativeTarget,
    machine: MachineId,
    expected: &mut [u8],
    supplied: &[u8],
    record: NativeFuelRankedU32CountdownRebaseRecord,
) -> Result<(), NativeFuelValidationError> {
    let invalid = || NativeFuelValidationError::InvalidRankedCountdownRebasing(machine);
    if !coordinates::validate_final_branches(target, supplied, record) {
        return Err(invalid());
    }
    for (offset, count) in coordinates::branch_spans(target, record).ok_or_else(invalid)? {
        accept(expected, supplied, offset, count).ok_or_else(invalid)?;
    }
    Ok(())
}

fn accept(expected: &mut [u8], supplied: &[u8], offset: usize, count: usize) -> Option<()> {
    expected
        .get_mut(offset..offset.checked_add(count)?)?
        .copy_from_slice(supplied.get(offset..offset.checked_add(count)?)?);
    Some(())
}

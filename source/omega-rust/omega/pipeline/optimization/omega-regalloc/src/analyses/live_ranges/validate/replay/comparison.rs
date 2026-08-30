//! Exact comparisons between retained and independently reconstructed rows.

use crate::{EarlyClobberConstraint, FunctionLiveRanges, LiveRangeError};

pub(super) fn require_structural_function(
    function: usize,
    actual: &FunctionLiveRanges,
    expected: &FunctionLiveRanges,
) -> Result<(), LiveRangeError> {
    if actual != expected {
        return Err(LiveRangeError::FunctionMismatch { function });
    }
    Ok(())
}

pub(super) fn require_function(
    function: usize,
    actual: &FunctionLiveRanges,
    expected: &FunctionLiveRanges,
) -> Result<(), LiveRangeError> {
    if actual.machine != expected.machine {
        return Err(LiveRangeError::FunctionMismatch { function });
    }
    if actual.block_domains != expected.block_domains {
        let block = expected
            .block_domains
            .iter()
            .zip(&actual.block_domains)
            .find(|(expected, actual)| expected != actual)
            .map_or(0, |(expected, _)| expected.block.0);
        return Err(LiveRangeError::BlockDomainMismatch { function, block });
    }
    if actual.virtual_registers != expected.virtual_registers {
        let register = expected
            .virtual_registers
            .iter()
            .zip(&actual.virtual_registers)
            .find(|(expected, actual)| expected != actual)
            .map_or(0, |(expected, _)| expected.virtual_register.0);
        return Err(LiveRangeError::VirtualRegisterMismatch { function, register });
    }
    if actual.tied_pairs != expected.tied_pairs {
        return Err(LiveRangeError::TiedPairMismatch { function });
    }
    require_early_clobber_rows(function, &actual.early_clobbers, &expected.early_clobbers)?;
    if actual.architectural_units != expected.architectural_units {
        let unit = expected
            .architectural_units
            .iter()
            .zip(&actual.architectural_units)
            .find(|(expected, actual)| expected != actual)
            .map_or(0, |(expected, _)| expected.unit.0);
        return Err(LiveRangeError::ArchitecturalUnitMismatch { function, unit });
    }
    if actual.interference != expected.interference {
        return Err(LiveRangeError::InterferenceMismatch { function });
    }
    Ok(())
}

pub(super) fn require_early_clobber_rows(
    function: usize,
    actual: &[EarlyClobberConstraint],
    expected: &[EarlyClobberConstraint],
) -> Result<(), LiveRangeError> {
    if actual != expected {
        return Err(LiveRangeError::EarlyClobberMismatch { function });
    }
    Ok(())
}

//! Positional function traversal and unsupported-domain indexes.

use super::super::early_clobber_uses;
use std::collections::BTreeSet;

use selected_instructions::VirtualRegisterId;

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{
    FunctionAllocationLegality, FunctionFixedPrecoloredIntervals,
    FunctionFixedPrecoloredSplitRequirements,
};
use selected_instructions::FunctionLiveRanges;

use super::partition;
use crate::work_accounting::{Work, count};

pub(super) fn derive(
    function: usize,
    ranges: &FunctionLiveRanges,
    legality: &FunctionAllocationLegality,
    fixed: &FunctionFixedPrecoloredIntervals,
    work: &mut Work,
) -> Result<FunctionFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    let tied = tied_registers(ranges);
    let early_uses = early_clobber_uses(ranges)?;
    work.rule_evaluations(1)?;
    work.validation_steps(1)?;
    work.validation_steps(count(ranges.tied_pairs.len())?)?;
    work.validation_steps(count(ranges.early_clobbers.len())?)?;
    work.validation_steps(count(early_uses)?)?;
    work.iterations(1)?;
    if ranges.machine != legality.machine
        || ranges.machine != fixed.machine
        || ranges.virtual_registers.len() != legality.virtual_registers.len()
    {
        return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
    }

    let mut fixed_offset = 0;
    let mut registers = Vec::with_capacity(ranges.virtual_registers.len());
    for (range, legal) in ranges
        .virtual_registers
        .iter()
        .zip(&legality.virtual_registers)
    {
        let start = fixed_offset;
        while fixed
            .intervals
            .get(fixed_offset)
            .is_some_and(|row| row.virtual_register == range.virtual_register)
        {
            fixed_offset += 1;
        }
        if fixed
            .intervals
            .get(fixed_offset)
            .is_some_and(|row| row.virtual_register < range.virtual_register)
        {
            return Err(FixedPrecoloredSplitRequirementError::RegisterMismatch {
                function,
                register: range.virtual_register.0,
            });
        }
        registers.push(partition::register(
            function,
            range,
            legal,
            &fixed.intervals[start..fixed_offset],
            tied.contains(&range.virtual_register),
            work,
        )?);
    }
    if fixed_offset != fixed.intervals.len() {
        return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
    }
    Ok(FunctionFixedPrecoloredSplitRequirements {
        machine: ranges.machine,
        registers,
    })
}

fn tied_registers(ranges: &FunctionLiveRanges) -> BTreeSet<VirtualRegisterId> {
    ranges
        .tied_pairs
        .iter()
        .flat_map(|row| [row.use_virtual_register, row.def_virtual_register])
        .collect()
}

// Early-clobber participants are admitted: the analysis never intersects
// `early_clobber_points` candidates into source domains, so a register
// partitions identically whether or not it carries an early-clobber hazard.
// The hazard itself is re-derived on the transformed plan and enforced by
// physical write-unit/use-unit conflicts at post-copy assignment. The use
// count still prices the constraint rows the function carries.

//! Positional function traversal and unsupported-domain indexes.

use std::collections::BTreeSet;

use omega_selected_instructions::VirtualRegisterId;

use crate::{
    FixedPrecoloredSplitRequirementError, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements, FunctionLiveRanges,
};

use super::{partition, work::Work};

pub(super) fn derive(
    function: usize,
    ranges: &FunctionLiveRanges,
    legality: &FunctionAllocationLegality,
    fixed: &FunctionFixedPrecoloredIntervals,
    work: &mut Work,
) -> Result<FunctionFixedPrecoloredSplitRequirements, FixedPrecoloredSplitRequirementError> {
    let tied = tied_registers(ranges);
    let (early, early_uses) = early_clobber_registers(ranges)?;
    work.function(
        ranges.tied_pairs.len(),
        ranges.early_clobbers.len(),
        early_uses,
    )?;
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
            early.contains(&range.virtual_register),
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

fn early_clobber_registers(
    ranges: &FunctionLiveRanges,
) -> Result<(BTreeSet<VirtualRegisterId>, usize), FixedPrecoloredSplitRequirementError> {
    let mut registers = BTreeSet::new();
    let mut use_count = 0usize;
    for row in &ranges.early_clobbers {
        registers.insert(row.def_virtual_register);
        registers.extend(row.uses.iter().map(|used| used.virtual_register));
        use_count = use_count
            .checked_add(row.uses.len())
            .ok_or(FixedPrecoloredSplitRequirementError::WorkOverflow)?;
    }
    Ok((registers, use_count))
}

//! Keyed function reconstruction for independent replay.

use std::collections::{BTreeMap, BTreeSet};

use selected_instructions::VirtualRegisterId;

use crate::{
    FixedPrecoloredInterval, FixedPrecoloredSplitRequirementError, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals, FunctionFixedPrecoloredSplitRequirements, FunctionLiveRanges,
    VirtualRegisterAllocationLegality,
};

use super::{partition, work::Work};

pub(super) fn reconstruct(
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
    if ranges.machine != legality.machine || ranges.machine != fixed.machine {
        return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
    }

    let mut legal = legality_by_register(function, legality)?;
    let mut fixed = fixed_by_register(fixed);
    let mut registers = Vec::with_capacity(ranges.virtual_registers.len());
    for range in &ranges.virtual_registers {
        let register = range.virtual_register.0;
        let legality = legal
            .remove(&range.virtual_register)
            .ok_or(FixedPrecoloredSplitRequirementError::RegisterMismatch { function, register })?;
        let fixed = fixed.remove(&range.virtual_register).unwrap_or_default();
        registers.push(partition::register(
            function,
            range,
            legality,
            &fixed,
            tied.contains(&range.virtual_register),
            early.contains(&range.virtual_register),
            work,
        )?);
    }
    if !legal.is_empty() || !fixed.is_empty() {
        return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
    }
    Ok(FunctionFixedPrecoloredSplitRequirements {
        machine: ranges.machine,
        registers,
    })
}

fn legality_by_register(
    function: usize,
    legality: &FunctionAllocationLegality,
) -> Result<
    BTreeMap<VirtualRegisterId, &VirtualRegisterAllocationLegality>,
    FixedPrecoloredSplitRequirementError,
> {
    let mut keyed = BTreeMap::new();
    for row in &legality.virtual_registers {
        if keyed.insert(row.virtual_register, row).is_some() {
            return Err(FixedPrecoloredSplitRequirementError::RegisterMismatch {
                function,
                register: row.virtual_register.0,
            });
        }
    }
    Ok(keyed)
}

fn fixed_by_register(
    fixed: &FunctionFixedPrecoloredIntervals,
) -> BTreeMap<VirtualRegisterId, Vec<&FixedPrecoloredInterval>> {
    let mut keyed = BTreeMap::<_, Vec<_>>::new();
    for row in &fixed.intervals {
        keyed.entry(row.virtual_register).or_default().push(row);
    }
    keyed
}

fn tied_registers(ranges: &FunctionLiveRanges) -> BTreeSet<VirtualRegisterId> {
    let mut result = BTreeSet::new();
    for row in &ranges.tied_pairs {
        result.insert(row.use_virtual_register);
        result.insert(row.def_virtual_register);
    }
    result
}

fn early_clobber_registers(
    ranges: &FunctionLiveRanges,
) -> Result<(BTreeSet<VirtualRegisterId>, usize), FixedPrecoloredSplitRequirementError> {
    let mut result = BTreeSet::new();
    let mut use_count = 0usize;
    for row in &ranges.early_clobbers {
        result.insert(row.def_virtual_register);
        for used in &row.uses {
            result.insert(used.virtual_register);
            use_count = use_count
                .checked_add(1)
                .ok_or(FixedPrecoloredSplitRequirementError::WorkOverflow)?;
        }
    }
    Ok((result, use_count))
}

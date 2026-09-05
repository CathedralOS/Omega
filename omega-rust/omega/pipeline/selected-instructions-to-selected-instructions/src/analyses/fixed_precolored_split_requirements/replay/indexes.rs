//! Function indexes owned by independent replay.

use std::collections::BTreeMap;

use crate::{
    FixedPrecoloredSplitRequirementError, FunctionAllocationLegality,
    FunctionFixedPrecoloredIntervals,
};

pub(super) fn legality(
    functions: &[FunctionAllocationLegality],
) -> Result<
    BTreeMap<semantic_vocabulary::MachineId, &FunctionAllocationLegality>,
    FixedPrecoloredSplitRequirementError,
> {
    let mut keyed = BTreeMap::new();
    for (function, row) in functions.iter().enumerate() {
        if keyed.insert(row.machine, row).is_some() {
            return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
        }
    }
    Ok(keyed)
}

pub(super) fn fixed(
    functions: &[FunctionFixedPrecoloredIntervals],
) -> Result<
    BTreeMap<semantic_vocabulary::MachineId, &FunctionFixedPrecoloredIntervals>,
    FixedPrecoloredSplitRequirementError,
> {
    let mut keyed = BTreeMap::new();
    for (function, row) in functions.iter().enumerate() {
        if keyed.insert(row.machine, row).is_some() {
            return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
        }
    }
    Ok(keyed)
}

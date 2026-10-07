//! Function indexes owned by independent replay.

use std::collections::BTreeMap;

use crate::FixedPrecoloredSplitRequirementError;
use register_homes::{FunctionAllocationLegality, FunctionFixedPrecoloredIntervals};

pub(super) fn legality(
    functions: &[FunctionAllocationLegality],
) -> Result<
    BTreeMap<semantic_vocabulary::MachineId, &FunctionAllocationLegality>,
    FixedPrecoloredSplitRequirementError,
> {
    keyed_by_machine(functions, |row| row.machine)
}

pub(super) fn fixed(
    functions: &[FunctionFixedPrecoloredIntervals],
) -> Result<
    BTreeMap<semantic_vocabulary::MachineId, &FunctionFixedPrecoloredIntervals>,
    FixedPrecoloredSplitRequirementError,
> {
    keyed_by_machine(functions, |row| row.machine)
}

/// The function rows keyed by machine; a duplicate machine is a
/// `FunctionMismatch` naming that row's position.
fn keyed_by_machine<'row, Row>(
    functions: &'row [Row],
    machine: impl Fn(&Row) -> semantic_vocabulary::MachineId,
) -> Result<BTreeMap<semantic_vocabulary::MachineId, &'row Row>, FixedPrecoloredSplitRequirementError>
{
    let mut keyed = BTreeMap::new();
    for (function, row) in functions.iter().enumerate() {
        if keyed.insert(machine(row), row).is_some() {
            return Err(FixedPrecoloredSplitRequirementError::FunctionMismatch { function });
        }
    }
    Ok(keyed)
}

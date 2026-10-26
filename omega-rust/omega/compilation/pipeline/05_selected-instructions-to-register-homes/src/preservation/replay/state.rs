use std::collections::BTreeMap;

use semantic_vocabulary::MachineId;

/// The replay walk collects its counts and findings in the same traversal
/// owner the compute route uses: the routes differ in how each home is found —
/// this one keys the replayed roster and reads it by register — never in what a
/// function, block, instruction, operand or write costs.
pub(in crate::preservation) use super::super::compute::state::{
    DirectTraversal as ReplayTraversal, add,
};

#[allow(clippy::needless_lifetimes)] // The explicit replay lifetime makes independent custody visible to architecture review.
pub(super) fn keyed_homes<'home>(
    functions: &'home [crate::FunctionRegisterHomes],
) -> Result<
    BTreeMap<MachineId, &'home crate::FunctionRegisterHomes>,
    super::super::AllocatedCalleeSavedRequirementError,
> {
    let mut keyed = BTreeMap::new();
    for function in functions {
        if keyed.insert(function.machine, function).is_some() {
            return Err(super::super::AllocatedCalleeSavedRequirementError::FunctionRosterMismatch);
        }
    }
    Ok(keyed)
}

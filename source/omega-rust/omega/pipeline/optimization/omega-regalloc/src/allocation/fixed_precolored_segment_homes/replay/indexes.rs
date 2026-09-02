use std::collections::BTreeMap;

use psi_core::MachineId;

use crate::{FixedPrecoloredSegmentHomeError, FunctionFixedPrecoloredSplitRequirements};

pub(super) fn requirements(
    functions: &[FunctionFixedPrecoloredSplitRequirements],
) -> Result<
    BTreeMap<MachineId, &FunctionFixedPrecoloredSplitRequirements>,
    FixedPrecoloredSegmentHomeError,
> {
    let mut indexed = BTreeMap::new();
    for function in functions {
        if indexed.insert(function.machine, function).is_some() {
            return Err(FixedPrecoloredSegmentHomeError::RootMismatch);
        }
    }
    Ok(indexed)
}

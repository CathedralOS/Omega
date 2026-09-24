//! Source-state reachability is independent of descriptor bindings. The live
//! mask marks every state the entry state's successor closure reaches; states
//! outside it are authored but unexecuted, and pruning them keeps their
//! parameter and claim rows from appearing as definitions without sources.

use super::super::LoweringError;
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};

pub(crate) fn live(
    plan: &CheckedComposedUnitControlMachinePlan,
) -> Result<Vec<bool>, LoweringError> {
    CheckedComposedUnitControlStatePlan::live_mask(&plan.states)
        .ok_or(LoweringError::Unsupported("Unit graph target disappeared"))
}

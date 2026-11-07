//! The returning tail of a scalar completion. Psi 05 plans the tail's
//! exits; this reads the statement prefix they begin after and refuses an
//! exit roster the completion has no route for, such as an ordered arm that
//! transfers or crashes instead of returning.
use super::{CheckedTrees, LoweringError};
use crate::shared::checked_facts::flow_facts;
use checked_trees::CheckedScalarBranchDestination;

/// The ordered guarded exits of a returning scalar tail, as Psi 05 planned
/// them. Every arm and the fallback return a value.
pub(crate) fn returning_exits<'checked>(
    checked: &'checked CheckedTrees,
    arms: arena::HandleSpan<checked_trees::CheckedScalarGuardedExit>,
    fallback: Option<&CheckedScalarBranchDestination>,
) -> Result<&'checked [checked_trees::CheckedScalarGuardedExit], LoweringError> {
    flow_facts(checked)
        .guarded_exits
        .returning_exits(arms, fallback)
        .map_err(LoweringError::Unsupported)
}

//! The returning tail of a scalar completion. Stage 04 plans the tail's
//! exits; this reads the statement prefix they begin after and refuses an
//! exit roster the completion has no route for, such as an ordered arm that
//! transfers or crashes instead of returning.
use super::{CheckedTrees, LoweringError, unsupported};
use checked_trees::CheckedScalarBranchDestination;

/// The ordered guarded exits of a returning scalar tail, as stage 04 planned
/// them. Every arm and the fallback return a value; an ordered roster that
/// transfers or crashes has no route through the scalar completion.
pub(crate) fn returning_exits<'checked>(
    checked: &'checked CheckedTrees,
    arms: arena::HandleSpan<checked_trees::CheckedScalarGuardedExit>,
    fallback: Option<&CheckedScalarBranchDestination>,
) -> Result<&'checked [checked_trees::CheckedScalarGuardedExit], LoweringError> {
    let arms = checked
        .facts
        .flow
        .terminal_scalar_graphs
        .guarded_exits
        .span(arms)
        .ok_or(LoweringError::Unsupported(
            "ordered scalar guard roster is stale",
        ))?;
    if arms.is_empty() {
        return unsupported("ordered scalar guard roster is empty");
    }
    if arms
        .iter()
        .map(|arm| &arm.destination)
        .chain(fallback)
        .any(|destination| {
            !matches!(
                destination,
                CheckedScalarBranchDestination::Return {
                    is_continuation: false,
                    ..
                }
            )
        })
    {
        return unsupported("ordered scalar exit transfers or crashes instead of returning");
    }
    Ok(arms)
}

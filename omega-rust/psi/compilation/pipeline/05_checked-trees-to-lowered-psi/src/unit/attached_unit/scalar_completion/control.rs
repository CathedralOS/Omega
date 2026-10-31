//! The returning tail of a scalar completion. Stage 04 plans the tail's
//! exits; this reads the statement prefix they begin after and refuses an
//! exit roster the completion has no route for, such as an ordered arm that
//! transfers or crashes instead of returning.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::checked_facts::flow_facts;
use checked_trees::CheckedScalarBranchDestination;

/// The ordered guarded exits of a returning scalar tail, as stage 04 planned
/// them. Every arm and the fallback return a value.
pub(crate) fn returning_exits<'checked>(
    checked: &'checked CheckedTrees,
    arms: arena::HandleSpan<checked_trees::CheckedScalarGuardedExit>,
    fallback: Option<&CheckedScalarBranchDestination>,
) -> Result<&'checked [checked_trees::CheckedScalarGuardedExit], LoweringError> {
    let arms =
        flow_facts(checked)
            .guarded_exits
            .exits
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

/// Exact normal destinations of this executable scalar control plan. Authored
/// cleanup rows can also describe dead suffixes; they do not make those exits
/// executable or justify allocating a completion block for them.
pub(crate) fn returning_destinations(
    checked: &CheckedTrees,
    control: &checked_trees::CheckedUnitScalarControlPlan,
) -> Result<Vec<CheckedScalarBranchDestination>, LoweringError> {
    use checked_trees::CheckedScalarStateTerminator;
    let destinations = match &control.terminator {
        CheckedScalarStateTerminator::Return { statement_ordinal } => {
            vec![CheckedScalarBranchDestination::Return {
                statement_ordinal: *statement_ordinal,
                is_continuation: false,
            }]
        }
        CheckedScalarStateTerminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            vec![when_true.clone(), when_false.clone()]
        }
        CheckedScalarStateTerminator::Guarded { arms, fallback } => {
            returning_exits(checked, *arms, fallback.as_ref())?
                .iter()
                .map(|arm| arm.destination.clone())
                .chain(fallback.iter().cloned())
                .collect()
        }
        _ => return unsupported("scalar completion has no normal return destination"),
    };
    let mut selected = Vec::new();
    for destination in destinations {
        if !matches!(destination, CheckedScalarBranchDestination::Return { .. }) {
            return unsupported("scalar completion has a nonreturning destination");
        }
        if !selected.contains(&destination) {
            selected.push(destination);
        }
    }
    Ok(selected)
}

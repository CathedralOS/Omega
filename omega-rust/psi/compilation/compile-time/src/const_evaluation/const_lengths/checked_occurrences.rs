//! Final source checking evidence required before replaying a folded length.
//! Provider-plan authority stays with Omega; these joins establish only the
//! retained source occurrence, requirement and primitive arithmetic policy.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

use crate::{SelectedBuildTimeBinaryOperator, SelectedBuildTimeProviderBody};

pub(super) fn validate(
    checked: &CheckedTrees,
    operators: &[SelectedBuildTimeBinaryOperator],
    provider_bodies: &[SelectedBuildTimeProviderBody],
) -> Result<(), Vec<Diagnostic>> {
    for selected in operators {
        let facts = &checked.facts.operators;
        let mut matching =
            facts
                .resolved_expression_selections()
                .filter(|(expression, origin, _, _)| {
                    *expression == selected.expression && *origin == selected.origin
                });
        let (Some((_, _, requirement, policy)), None) = (matching.next(), matching.next()) else {
            return Err(vec![Diagnostic::error(
                "folded operator has no unique final checked occurrence",
            )]);
        };
        if requirement != selected.requirement || policy != selected.policy {
            return Err(vec![Diagnostic::error(
                "folded operator differs from final checked requirement or arithmetic policy",
            )]);
        }
    }
    for body in provider_bodies {
        // A token machine normalizes from a spelled use to a requirement call.
        // Exactly one occurrence must survive across the three fact arenas.
        let mut requirements = checked
            .facts
            .operators
            .resolved_expression_selections()
            .filter(|(expression, origin, _, _)| {
                *expression == body.expression && *origin == body.origin
            })
            .map(|(_, _, requirement, _)| requirement);
        let (Some(requirement), None) = (requirements.next(), requirements.next()) else {
            return Err(vec![Diagnostic::error(
                "folded provider body has no unique final checked occurrence",
            )]);
        };
        if requirement != body.requirement {
            return Err(vec![Diagnostic::error(
                "folded provider body differs from final checked requirement",
            )]);
        }
    }
    Ok(())
}

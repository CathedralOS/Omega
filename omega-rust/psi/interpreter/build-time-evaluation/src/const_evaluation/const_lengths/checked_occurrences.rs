//! Final source checking evidence required before replaying a folded length.
//! Provider-plan authority stays with Omega; these joins establish only the
//! retained source occurrence, requirement and primitive arithmetic policy.

use checked_trees::{CheckedOperatorOccurrence, CheckedTrees};
use diagnostics::Diagnostic;

use crate::{SelectedBuildTimeBinaryOperator, SelectedBuildTimeProviderBody};

pub(super) fn validate(
    checked: &CheckedTrees,
    operators: &[SelectedBuildTimeBinaryOperator],
    provider_bodies: &[SelectedBuildTimeProviderBody],
) -> Result<(), Vec<Diagnostic>> {
    for selected in operators {
        let mut matching = checked.facts.operators.resolved_uses().filter(|fact| {
            fact.expression == selected.expression
                && fact.origin == selected.origin
                && fact.occurrence == CheckedOperatorOccurrence::Expression
        });
        let (Some(fact), None) = (matching.next(), matching.next()) else {
            return Err(vec![Diagnostic::error(
                "folded operator has no unique final checked occurrence",
            )]);
        };
        if fact.selected_operator_symbol != selected.requirement
            || fact.policy_adapter != selected.policy
        {
            return Err(vec![Diagnostic::error(
                "folded operator differs from final checked requirement or arithmetic policy",
            )]);
        }
    }
    for body in provider_bodies {
        // A token machine normalizes from a spelled use to a requirement call.
        // Exactly one occurrence must survive across the three fact arenas.
        let spelled = checked.facts.operators.resolved_uses().filter(|fact| {
            fact.expression == body.expression
                && fact.origin == body.origin
                && fact.occurrence == CheckedOperatorOccurrence::Expression
        });
        let named = checked
            .facts
            .operators
            .named_uses()
            .filter(|fact| fact.expression == body.expression && fact.origin == body.origin);
        let requirements = checked
            .facts
            .operators
            .named_requirement_uses()
            .filter(|fact| fact.expression == body.expression && fact.origin == body.origin);
        let mut requirements = spelled
            .map(|fact| fact.selected_operator_symbol)
            .chain(named.map(|fact| fact.selected_operator_symbol))
            .chain(requirements.map(|fact| fact.requirement_symbol));
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

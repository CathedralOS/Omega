use checked_trees::{CheckFacts, CheckedOperatorResolutionIssue};
use diagnostics::Diagnostic;

use crate::labels::symbol_name;

mod requires;

pub(crate) fn check_operator_resolution(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = facts
        .operators
        .resolution_issues()
        .filter_map(|issue| {
            if issue.is_ambiguous() {
                Some(ambiguous_operator_diagnostic(program, issue))
            } else if issue.is_inadmissible() {
                Some(inadmissible_operator_diagnostic(program, issue))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    diagnostics.extend(requires::selected_binary_requires_diagnostics(
        program, facts,
    ));

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The binding-site rule (chapter 8): a domain-owned meaning participates only
/// when an operand declaration, mint, or signature `requires` selects its
/// semantic role. This use has domain candidates, none selected, and no
/// builtin/root fallback.
fn inadmissible_operator_diagnostic(
    program: &typed_trees::TypedTrees,
    issue: CheckedOperatorResolutionIssue<'_>,
) -> Diagnostic {
    let operand = program
        .expression_table
        .display_name(issue.operator_use.expression);
    let required_facts = issue
        .candidates
        .iter()
        .filter(|candidate| candidate.is_domain_owned())
        .map(|candidate| {
            format!(
                "a binding declared, minted, or signature-qualified in {}",
                symbol_name(program, candidate.domain_symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(" or ");
    Diagnostic::error(format!(
        "operator spelling `{}` in `{operand}` has no admissible meaning: \
         the domain operator meaning needs {required_facts}, \
         and no builtin meaning exists for the operand type",
        issue.spelling().symbol(),
    ))
}

fn ambiguous_operator_diagnostic(
    program: &typed_trees::TypedTrees,
    issue: CheckedOperatorResolutionIssue<'_>,
) -> Diagnostic {
    let candidates = issue
        .candidates
        .iter()
        .map(|candidate| {
            let operator = symbol_name(program, candidate.operator_symbol);
            if candidate.is_trait_backed() {
                format!(
                    "`{operator}` selected by proof-static conformance `{}`",
                    symbol_name(program, candidate.conformance_symbol)
                )
            } else if candidate.domain_symbol.is_valid() {
                format!(
                    "`{operator}` owned by domain `{}`",
                    symbol_name(program, candidate.domain_symbol)
                )
            } else {
                format!("`{operator}` (builtin/root)")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    Diagnostic::error(format!(
        "ambiguous operator spelling `{}` has {} viable candidates: {candidates}. The static \
         operand-domain tuple does not uniquely select one -- narrow the binding's semantic \
         qualification or choose a clearer operation.",
        issue.spelling().symbol(),
        issue.candidate_count(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::{Arena, HandleSpan};
    use checked_trees::{
        CheckedOperatorCandidateFact, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
        CheckedValueOrigin,
    };
    use language_core::operator_spelling::OperatorSpelling;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    #[test]
    fn rejects_ambiguous_operator_resolution_with_candidate_details() {
        let mut candidates = Arena::default();
        let candidate_span = candidates.insert_many([
            CheckedOperatorCandidateFact::root(SymbolHandle::from_arena_index(10)).with_signature(
                Default::default(),
                Default::default(),
                HandleSpan::from_parts(arena::Handle::from_arena_index(20), 1),
                0,
                2,
                false,
            ),
            CheckedOperatorCandidateFact::domain(
                SymbolHandle::from_arena_index(11),
                SymbolHandle::from_arena_index(12),
            )
            .with_signature(
                Default::default(),
                Default::default(),
                HandleSpan::from_parts(arena::Handle::from_arena_index(30), 3),
                1,
                2,
                false,
            ),
        ]);
        let mut uses = Arena::default();
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(1),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            policy_adapter: Default::default(),
            provider_plan_report_fingerprint: 0,
            provider_plan_commitment: Default::default(),
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: candidate_span,
            candidate_count: 2,
            status: CheckedOperatorResolutionStatus::Ambiguous,
        });
        let facts = CheckFacts {
            operators: checked_trees::CheckedOperatorFacts::with_roots(
                uses,
                Default::default(),
                candidates,
            ),
            ..Default::default()
        };

        let program = typed_trees::TypedTrees::default();
        let diagnostics = check_operator_resolution(&program, &facts)
            .expect_err("ambiguous operator should be rejected");
        let message = &diagnostics[0].message;

        assert!(message.contains("ambiguous operator spelling `[]`"));
        assert!(message.contains("2 viable candidates"));
        // Candidate details render by symbol display name (empty in this
        // synthetic fixture); the stable fragments are the candidate KINDS.
        // (The old "root operator 10 params 2 contracts 1" numeric form
        // rotted silently -- this crate's unit tests are outside the root
        // gate; caught during the usize-retirement sweep.)
        assert!(message.contains("(builtin/root)"));
        assert!(message.contains("owned by domain"));
    }

    #[test]
    fn does_not_reject_missing_operator_resolution_before_core_contracts_are_wired() {
        let mut uses = Arena::default();
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(1),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            policy_adapter: Default::default(),
            provider_plan_report_fingerprint: 0,
            provider_plan_commitment: Default::default(),
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: HandleSpan::empty(),
            candidate_count: 0,
            status: CheckedOperatorResolutionStatus::Missing,
        });
        let facts = CheckFacts {
            operators: checked_trees::CheckedOperatorFacts::with_roots(
                uses,
                Default::default(),
                Arena::default(),
            ),
            ..Default::default()
        };

        let program = typed_trees::TypedTrees::default();
        check_operator_resolution(&program, &facts)
            .expect("missing operators stay reportable until core contracts are wired");
    }
}

use omega_checked_trees::{CheckFacts, CheckedOperatorResolutionIssue};
use omega_core::diagnostics::Diagnostic;

pub(crate) fn check_operator_resolution(facts: &CheckFacts) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = facts
        .operators
        .resolution_issues()
        .filter(|issue| issue.is_ambiguous())
        .map(ambiguous_operator_diagnostic)
        .collect::<Vec<_>>();

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn ambiguous_operator_diagnostic(issue: CheckedOperatorResolutionIssue<'_>) -> Diagnostic {
    Diagnostic::error(format!(
        "ambiguous operator spelling `{}` has {} viable candidates: {}",
        issue.spelling().symbol(),
        issue.candidate_count(),
        issue
            .candidates
            .iter()
            .map(|candidate| {
                let owner = if candidate.domain_symbol.is_valid() {
                    format!("domain {}", candidate.domain_symbol.arena_index())
                } else {
                    "root".to_owned()
                };
                format!(
                    "{} operator {} params {} contracts {}",
                    owner,
                    candidate.operator_symbol.arena_index(),
                    candidate.parameter_count,
                    candidate.contract_count
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_checked_trees::{
        CheckedOperatorCandidateFact, CheckedOperatorResolutionStatus, CheckedOperatorUseFact,
        CheckedValueOrigin,
    };
    use omega_core::arena::{Arena, HandleSpan};
    use omega_core::operator_spelling::OperatorSpelling;
    use omega_core::symbols::SymbolHandle;
    use omega_typed_trees::expression::ExpressionHandle;

    #[test]
    fn rejects_ambiguous_operator_resolution_with_candidate_details() {
        let mut candidates = Arena::default();
        let candidate_span = candidates.insert_many([
            CheckedOperatorCandidateFact::root(SymbolHandle::from_arena_index(10)).with_signature(
                Default::default(),
                Default::default(),
                0,
                2,
                1,
                false,
            ),
            CheckedOperatorCandidateFact::domain(
                SymbolHandle::from_arena_index(11),
                SymbolHandle::from_arena_index(12),
            )
            .with_signature(Default::default(), Default::default(), 1, 2, 3, false),
        ]);
        let mut uses = Arena::default();
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(1),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: candidate_span,
            candidate_count: 2,
            status: CheckedOperatorResolutionStatus::Ambiguous,
        });
        let facts = CheckFacts {
            operators: omega_checked_trees::CheckedOperatorFacts::with_roots(uses, candidates),
            ..Default::default()
        };

        let diagnostics =
            check_operator_resolution(&facts).expect_err("ambiguous operator should be rejected");
        let message = &diagnostics[0].message;

        assert!(message.contains("ambiguous operator spelling `[]`"));
        assert!(message.contains("2 viable candidates"));
        assert!(message.contains("root operator 10 params 2 contracts 1"));
        assert!(message.contains("domain 12 operator 11 params 2 contracts 3"));
    }

    #[test]
    fn does_not_reject_missing_operator_resolution_before_core_contracts_are_wired() {
        let mut uses = Arena::default();
        uses.append(CheckedOperatorUseFact {
            expression: ExpressionHandle::from_arena_index(1),
            origin: CheckedValueOrigin::default(),
            spelling: OperatorSpelling::Index,
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: HandleSpan::empty(),
            candidate_count: 0,
            status: CheckedOperatorResolutionStatus::Missing,
        });
        let facts = CheckFacts {
            operators: omega_checked_trees::CheckedOperatorFacts::with_roots(
                uses,
                Arena::default(),
            ),
            ..Default::default()
        };

        check_operator_resolution(&facts)
            .expect("missing operators stay reportable until core contracts are wired");
    }
}

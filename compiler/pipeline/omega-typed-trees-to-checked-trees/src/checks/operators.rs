use omega_checked_trees::{CheckFacts, CheckedOperatorResolutionIssue};
use omega_core::diagnostics::Diagnostic;
use omega_core::operator_spelling::OperatorSpelling;

use crate::labels::symbol_name;

pub(crate) fn check_operator_resolution(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = facts
        .operators
        .resolution_issues()
        .filter_map(|issue| {
            if issue.is_ambiguous() {
                Some(ambiguous_operator_diagnostic(issue))
            } else if issue.is_inadmissible() {
                Some(inadmissible_operator_diagnostic(program, issue))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    diagnostics.extend(undischargeable_selected_contract_diagnostics(
        program, facts,
    ));

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The positive proof-context rule (chapter 8): a domain-owned operator
/// meaning participates only when the operand's domain membership is PROVEN in
/// the current context. This use has domain candidates, none proven, and no
/// builtin or root meaning to fall back to — name exactly which fact would
/// admit each candidate.
fn inadmissible_operator_diagnostic(
    program: &omega_typed_trees::TypedTrees,
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
                "a proven fact placing the left operand in {}",
                symbol_name(program, candidate.domain_symbol)
            )
        })
        .collect::<Vec<_>>()
        .join(" or ");
    Diagnostic::error(format!(
        "operator spelling `{}` in `{operand}` has no admissible meaning: \
         the domain operator meaning needs {required_facts} in the current context, \
         and no builtin meaning exists for the operand type",
        issue.spelling().symbol(),
    ))
}

/// Honesty guard: contract discharge for spelled BINARY operator uses is not
/// wired yet (slice `[]`/`[..]` contracts discharge through the ranges seam).
/// Selecting a `requires`-carrying meaning and then never checking the
/// contract would silently weaken the program, so reject loudly instead.
fn undischargeable_selected_contract_diagnostics<'facts>(
    program: &'facts omega_typed_trees::TypedTrees,
    facts: &'facts CheckFacts,
) -> impl Iterator<Item = Diagnostic> + 'facts {
    facts
        .operators
        .resolved_uses()
        .filter(|operator_use| {
            !matches!(
                operator_use.spelling,
                OperatorSpelling::Index | OperatorSpelling::Range
            )
        })
        .filter_map(|operator_use| {
            let selected = facts.operators.selected_candidate(operator_use)?;
            let carries_requires = program
                .signature_contracts
                .span_or_empty(selected.contracts)
                .iter()
                .any(|contract| {
                    contract.kind == omega_typed_trees::signature::SignatureContractKind::Requires
                });
            carries_requires.then(|| {
                Diagnostic::error(format!(
                    "operator spelling `{}` selected meaning `{}` carries `requires` \
                     contracts, but contract discharge at spelled binary use sites \
                     is not implemented yet",
                    operator_use.spelling.symbol(),
                    symbol_name(program, selected.operator_symbol),
                ))
            })
        })
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
                HandleSpan::from_parts(omega_core::arena::Handle::from_arena_index(20), 1),
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
                HandleSpan::from_parts(omega_core::arena::Handle::from_arena_index(30), 3),
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
            selected_operator_symbol: SymbolHandle::invalid(),
            candidates: candidate_span,
            candidate_count: 2,
            status: CheckedOperatorResolutionStatus::Ambiguous,
        });
        let facts = CheckFacts {
            operators: omega_checked_trees::CheckedOperatorFacts::with_roots(uses, candidates),
            ..Default::default()
        };

        let program = omega_typed_trees::TypedTrees::default();
        let diagnostics = check_operator_resolution(&program, &facts)
            .expect_err("ambiguous operator should be rejected");
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

        let program = omega_typed_trees::TypedTrees::default();
        check_operator_resolution(&program, &facts)
            .expect("missing operators stay reportable until core contracts are wired");
    }
}

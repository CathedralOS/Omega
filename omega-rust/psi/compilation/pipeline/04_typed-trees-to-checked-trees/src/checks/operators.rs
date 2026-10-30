use checked_trees::{CheckFacts, CheckedOperatorResolutionIssue};
use diagnostics::Diagnostic;

use crate::labels::symbol_name;

mod destinations;
mod float_semantic;
pub(crate) use float_semantic::{
    context_proves_float_semantic_term, finite_float_semantic_term, float_boolean_term_value,
    instantiated_float_semantic_term, selected_float_boolean_requirement,
};
mod requires;

pub(crate) use requires::{
    InvocationContexts, closed_operators_are_builtin, named_operator_route_is_false,
    operator_route_is_false,
};

pub(crate) fn check_operator_resolution(
    program: &dyn ::validation::CheckingInput,
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
    diagnostics.extend(destinations::check(program, facts));
    for (_, operator_use) in facts.operators.uses.iter() {
        let selected = facts.operators.selected_candidate(operator_use);
        if operator_use.occurrence == checked_trees::CheckedOperatorOccurrence::Expression {
            continue;
        }
        let Some(selected) = selected else {
            diagnostics.push(Diagnostic::error(
                "floating Match equality requires one selected operator meaning",
            ));
            continue;
        };
        if operator_use.status != checked_trees::CheckedOperatorResolutionStatus::Resolved
            || operator_use.operands(program).is_none()
            || selected.parameter_count != 2
            || program.primitive_type_reference(selected.return_type)
                != Some(typed_trees::types::PrimitiveType::Bool)
        {
            diagnostics.push(Diagnostic::error(
                "selected Match equality must have exact operands and return Bool",
            ));
        }
    }

    crate::checks::finish(diagnostics)
}

/// The binding-site rule (chapter 8): a domain-owned meaning participates only
/// when an operand declaration, mint, or signature `requires` selects its
/// semantic role. This use has domain candidates, none selected, and no
/// builtin/root fallback.
fn inadmissible_operator_diagnostic(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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

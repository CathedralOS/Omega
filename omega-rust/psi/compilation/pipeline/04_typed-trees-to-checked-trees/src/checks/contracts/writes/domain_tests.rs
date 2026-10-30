//! Normalized domain tests retain their tested value, not authority to mint it.
//! Consume the same captured copy/view epochs as ordinary value obligations.
//! Alias atoms and nested memberships arrive as their own retained test rows;
//! no consumer reconstructs them from the shape of a Match or Boolean tree.

use super::ConstructorMemberships;
use checked_trees::{CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;
use typed_trees::expression::ExpressionHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    captures: &ConstructorMemberships<'_>,
    state: &FlowStateFact,
    statement: usize,
    expression: ExpressionHandle,
    contexts: &[facts::FactContextHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for test in program.expression_table.executable_domain_tests(expression) {
        let Some(domain) = program.domain_definition(test.domain) else {
            continue;
        };
        if domain.establishment_routes.is_empty() {
            continue;
        }
        let operands = captures.at(state.state_symbol, statement, test.subject);
        let proven = if operands.is_empty() {
            validation::canonical_place_from_expression_in_state(
                program,
                state.state_symbol,
                statement,
                test.subject,
            )
            .is_some_and(|place| {
                super::super::exits::exact_scalar_membership(
                    program,
                    facts,
                    contexts,
                    &place,
                    domain.symbol,
                    domain.semantic_id,
                )
            })
        } else {
            operands.iter().all(|operand| {
                retained(
                    program,
                    facts,
                    state,
                    *operand,
                    domain.symbol,
                    domain.semantic_id,
                )
            })
        };
        if !proven {
            diagnostics.push(Diagnostic::error(format!(
                "executable domain test `{}` requires retained authorized provenance; a predicate test cannot establish it",
                domain.name.as_str(),
            )).with_source_span(program.expression_table.source_span(expression)));
        }
    }
}

fn retained(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: crate::flow::CapturedOperand<'_>,
    domain: symbols::SymbolHandle,
    identity: language_semantics::SemanticDomainId,
) -> bool {
    let Some(values) = crate::flow::captured_operand_values(
        program,
        &facts.flow,
        &facts.semantic,
        state,
        capture,
        capture.type_reference,
        &[],
    ) else {
        return false;
    };
    !values.is_empty()
        && values.into_iter().all(|value| {
            [
                (
                    value.observation,
                    value.observation_type,
                    crate::flow::captured_observation_places(program, &facts.flow, &value),
                    capture.constraints,
                ),
                (
                    value.operand,
                    value.type_reference,
                    crate::flow::captured_value_places(program, &facts.flow, &value),
                    value.producer_invocation(capture.constraints),
                ),
            ]
            .into_iter()
            .any(|(operand, reference, places, invocation)| {
                let Some(places) = places.filter(|places| !places.is_empty()) else {
                    return false;
                };
                let contexts = crate::flow::operand_contexts(
                    program,
                    &facts.flow.contexts,
                    &facts.flow.control,
                    &facts.semantic,
                    operand.row(),
                    reference,
                    invocation,
                );
                places.iter().all(|place| {
                    super::super::exits::exact_scalar_membership(
                        program,
                        facts,
                        &contexts,
                        &place.projected,
                        domain,
                        identity,
                    )
                })
            })
        })
}

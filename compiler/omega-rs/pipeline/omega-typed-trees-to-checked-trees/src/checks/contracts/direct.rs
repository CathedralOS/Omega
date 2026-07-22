use omega_core::symbols::SymbolHandle;
use omega_facts::FactPlace;

use super::labels::{ContractTargetParameters, instantiate_call_contract_expression_label};
use super::places::{expression_is_boolean_place_like, expression_place_matches};
use crate::labels::{canonical_place_label, semantic_boolean_fact_label};

pub(super) fn direct_context_proves_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        let Some(candidate_label) = semantic_boolean_fact_label(program, semantic, fact) else {
            return false;
        };

        candidate_label == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if expression_place_matches(program, semantic, expression, candidate_place)))
    })
}

pub(super) fn direct_context_proves_instantiated_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &(impl ContractTargetParameters + ?Sized),
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label = instantiate_call_contract_expression_label(
        program,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    );

    semantic.context_view(context).facts().any(|fact| {
        let Some(candidate_label) = semantic_boolean_fact_label(program, semantic, fact) else {
            return false;
        };

        candidate_label == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if instantiate_call_contract_expression_label(
                        program,
                        caller_state_symbol,
                        statement_index,
                        call_site,
                        target_state,
                        expression,
                ) == canonical_place_label(
                    program,
                    semantic,
                    semantic.places.get(candidate_place),
                )))
    })
}

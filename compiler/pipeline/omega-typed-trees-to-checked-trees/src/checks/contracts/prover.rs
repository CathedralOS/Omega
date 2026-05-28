use omega_checked_trees::{FlowCallFact, FlowStateFact};
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, FactPlace};

use super::evaluator::call_site_proves_boolean_contract_expression;
use super::labels::{domain_proves_expression_label, instantiate_call_contract_expression_label};
use crate::labels::canonical_place_label;

pub(super) fn call_entry_contexts_prove_boolean_contract_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[omega_facts::FactContextHandle],
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let Some(call_site) = crate::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_state) = crate::find_state(program, call_flow.target_symbol) else {
        return false;
    };

    entry_contexts.iter().any(|entry_context| {
        let context = semantic.contexts.get(*entry_context);
        semantic_context_proves_instantiated_boolean_expression(
            program,
            semantic,
            context,
            state_flow.state_symbol,
            call_flow.statement_index,
            &call_site,
            target_state,
            expression,
        )
    }) || call_site_proves_boolean_contract_expression(
        program,
        state_flow,
        call_flow,
        &call_site,
        target_state,
        expression,
    )
}

pub(super) fn semantic_contexts_prove_contract_fact(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    entry_contexts: &[omega_facts::FactContextHandle],
    fact: &omega_facts::Fact,
) -> bool {
    match fact.payload {
        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic
                    .context_view(context)
                    .proves_place_domain_membership_in_program(program, place, domain_symbol)
            })
        }
        FactPayload::ContractBooleanExpression { expression, .. } => {
            matches!(
                program.expression_table.expression(expression),
                omega_checked_trees::expression::ExpressionNode::Boolean(true)
            ) || entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic_context_proves_boolean_expression(program, semantic, context, expression)
            })
        }
        _ => true,
    }
}

fn semantic_context_proves_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_boolean_expression(program, semantic, context, expression) {
        return true;
    }

    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            semantic_context_proves_boolean_expression(program, semantic, context, *inner)
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            omega_typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    && semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            omega_typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_boolean_expression(program, semantic, context, binary.left)
                    || semantic_context_proves_boolean_expression(
                        program,
                        semantic,
                        context,
                        binary.right,
                    )
            }
            _ => prove_boolean_expression_via_context_domain_membership(
                program, semantic, context, expression,
            ),
        },
        _ => prove_boolean_expression_via_context_domain_membership(
            program, semantic, context, expression,
        ),
    }
}

fn semantic_context_proves_instantiated_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if direct_context_proves_instantiated_boolean_expression(
        program,
        semantic,
        context,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    ) {
        return true;
    }

    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            semantic_context_proves_instantiated_boolean_expression(
                program,
                semantic,
                context,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                *inner,
            )
        }
        omega_typed_trees::expression::ExpressionNode::Binary(binary) => match binary.operator {
            omega_typed_trees::expression::BinaryOperator::And => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.left,
                ) && semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            omega_typed_trees::expression::BinaryOperator::Or => {
                semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.left,
                ) || semantic_context_proves_instantiated_boolean_expression(
                    program,
                    semantic,
                    context,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    binary.right,
                )
            }
            _ => prove_instantiated_boolean_expression_via_context_domain_membership(
                program,
                semantic,
                context,
                caller_state_symbol,
                statement_index,
                call_site,
                target_state,
                expression,
            ),
        },
        _ => prove_instantiated_boolean_expression_via_context_domain_membership(
            program,
            semantic,
            context,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            expression,
        ),
    }
}

fn direct_context_proves_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let required_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        let candidate_expression = match fact.payload {
            FactPayload::BooleanExpression(candidate_expression)
            | FactPayload::ContractBooleanExpression {
                expression: candidate_expression,
                ..
            } => candidate_expression,
            _ => return false,
        };

        candidate_expression == expression
            || program.expression_table.display_name(candidate_expression) == required_label
            || (expression_is_boolean_place_like(program, expression)
                && matches!(fact.place, FactPlace::Place(candidate_place)
                    if expression_place_matches(program, semantic, expression, candidate_place)))
    })
}

fn direct_context_proves_instantiated_boolean_expression(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
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
        let candidate_expression = match fact.payload {
            FactPayload::BooleanExpression(candidate_expression)
            | FactPayload::ContractBooleanExpression {
                expression: candidate_expression,
                ..
            } => candidate_expression,
            _ => return false,
        };

        program.expression_table.display_name(candidate_expression) == required_label
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

pub(super) fn expression_is_boolean_place_like(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        omega_typed_trees::expression::ExpressionNode::Mutable(inner) => {
            expression_is_boolean_place_like(program, *inner)
        }
        omega_typed_trees::expression::ExpressionNode::Name(_)
        | omega_typed_trees::expression::ExpressionNode::Member(_)
        | omega_typed_trees::expression::ExpressionNode::Indexed(_) => true,
        _ => false,
    }
}

fn expression_place_matches(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    expression: omega_typed_trees::expression::ExpressionHandle,
    candidate_place: omega_facts::PlaceHandle,
) -> bool {
    let candidate_label =
        canonical_place_label(program, semantic, semantic.places.get(candidate_place));
    program.expression_table.display_name(expression) == candidate_label
}

fn prove_boolean_expression_via_context_domain_membership(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label = program.expression_table.display_name(expression);

    semantic.context_view(context).facts().any(|fact| {
        let (domain_symbol, value, place) = match fact.payload {
            FactPayload::DomainMembership {
                domain_symbol,
                value,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                value,
                ..
            } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, value, place)
            }
            _ => return false,
        };
        let canonical_base_label =
            canonical_place_label(program, semantic, semantic.places.get(place));
        let display_base_label = program.expression_table.display_name(value);
        domain_proves_expression_label(
            program,
            domain_symbol,
            &canonical_base_label,
            &candidate_label,
        ) || domain_proves_expression_label(
            program,
            domain_symbol,
            &display_base_label,
            &candidate_label,
        )
    })
}

fn prove_instantiated_boolean_expression_via_context_domain_membership(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    context: &omega_facts::FactContext,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: &crate::CallSite<'_>,
    target_state: &omega_typed_trees::state::State,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let candidate_label = instantiate_call_contract_expression_label(
        program,
        caller_state_symbol,
        statement_index,
        call_site,
        target_state,
        expression,
    );

    semantic.context_view(context).facts().any(|fact| {
        let (domain_symbol, value, place) = match fact.payload {
            FactPayload::DomainMembership {
                domain_symbol,
                value,
                ..
            }
            | FactPayload::ContractDomainMembership {
                domain_symbol,
                value,
                ..
            } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, value, place)
            }
            _ => return false,
        };
        let canonical_base_label =
            canonical_place_label(program, semantic, semantic.places.get(place));
        let display_base_label = program.expression_table.display_name(value);
        domain_proves_expression_label(
            program,
            domain_symbol,
            &canonical_base_label,
            &candidate_label,
        ) || domain_proves_expression_label(
            program,
            domain_symbol,
            &display_base_label,
            &candidate_label,
        )
    })
}

use psi_checked_trees::{FlowCallFact, FlowStateFact};
use psi_facts::{FactPayload, FactPlace};

mod assigned_values;
pub(super) use assigned_values::{prove_domain_at_place, scalar_value_at_place};
mod scalars;
pub(super) use scalars::{ScalarValue, evaluate as evaluate_scalar};
mod checked_scalars;
pub(super) use checked_scalars::evaluate as evaluate_checked_scalar;
mod booleans;
#[cfg(test)]
mod tests;

use self::booleans::{
    semantic_context_proves_boolean_expression,
    semantic_context_proves_instantiated_boolean_expression,
};
use super::evaluator::call_site_proves_boolean_contract_expression;

pub(super) fn semantic_contexts_prove_boolean_expression(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    entry_contexts: &[psi_facts::FactContextHandle],
    expression: psi_typed_trees::expression::ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        psi_typed_trees::expression::ExpressionNode::Boolean(true)
    ) || entry_contexts.iter().any(|entry_context| {
        semantic_context_proves_boolean_expression(
            program,
            semantic,
            semantic.contexts.get(*entry_context),
            expression,
        )
    })
}

pub(super) fn call_entry_contexts_prove_boolean_contract_expression(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[psi_facts::FactContextHandle],
    expression: psi_typed_trees::expression::ExpressionHandle,
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
    let Some(target_parameters) = crate::call_target_parameters(program, call_flow.target_symbol)
    else {
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
            target_parameters,
            expression,
        )
    }) || call_site_proves_boolean_contract_expression(
        program,
        state_flow,
        call_flow,
        &call_site,
        call_flow.target_symbol,
        target_parameters,
        expression,
    )
}

pub(super) fn semantic_contexts_prove_contract_fact(
    program: &psi_typed_trees::TypedTrees,
    semantic: &psi_facts::FactPlan,
    entry_contexts: &[psi_facts::FactContextHandle],
    fact: &psi_facts::Fact,
) -> bool {
    match fact.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic
                    .context_view(context)
                    .proves_place_domain_membership_in_program(program, place, domain_symbol)
                    || semantic.context_view(context).facts().any(|candidate| {
                        let candidate_domain = match candidate.payload {
                            FactPayload::DomainMembership { domain_symbol, .. }
                            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                                domain_symbol
                            }
                            _ => return false,
                        };
                        let FactPlace::Place(candidate_place) = candidate.place else {
                            return false;
                        };
                        crate::field_domain::domain_membership_implies(
                            program,
                            candidate_domain,
                            domain_symbol,
                        ) && semantic.places_match(program, candidate_place, place)
                    })
            }) || assigned_values::prove_domain(
                program,
                semantic,
                entry_contexts,
                place,
                domain_symbol,
            )
        }
        FactPayload::CarryPermission { permission, .. }
        | FactPayload::ContractCarryPermission { permission, .. } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic.context_view(context).facts().any(|candidate| {
                    let candidate_permission = match candidate.payload {
                        FactPayload::CarryPermission { permission, .. }
                        | FactPayload::ContractCarryPermission { permission, .. } => permission,
                        _ => return false,
                    };
                    let FactPlace::Place(candidate_place) = candidate.place else {
                        return false;
                    };
                    candidate_permission == permission
                        && semantic.places_match(program, candidate_place, place)
                })
            })
        }
        FactPayload::BooleanExpression(expression)
        | FactPayload::ContractBooleanExpression { expression, .. } => {
            matches!(
                program.expression_table.expression(expression),
                psi_checked_trees::expression::ExpressionNode::Boolean(true)
            ) || entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic_context_proves_boolean_expression(program, semantic, context, expression)
            })
        }
        FactPayload::PropositionApplication { .. }
        | FactPayload::ContractPropositionApplication { .. } => {
            let Some(required_label) = semantic.proposition_fact_label(program, fact) else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                let context = semantic.contexts.get(*entry_context);
                semantic
                    .context_view(context)
                    .proves_proposition_label(program, &required_label)
                    || required_label
                        .strip_prefix("boolean:")
                        .is_some_and(|required_boolean| {
                            semantic.context_view(context).facts().any(|candidate| {
                                semantic
                                    .boolean_fact_label(program, candidate)
                                    .is_some_and(|label| label == required_boolean)
                            })
                        })
            })
        }
        FactPayload::CarryOrigin { .. } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            entry_contexts.iter().any(|entry_context| {
                semantic
                    .context_view(semantic.contexts.get(*entry_context))
                    .facts()
                    .any(|candidate| {
                        matches!(candidate.payload, FactPayload::CarryOrigin { .. })
                            && matches!(candidate.place, FactPlace::Place(candidate_place)
                            if semantic.places_match(program, candidate_place, place))
                    })
            })
        }
        // These are evidence or deferred obligations, not propositions this
        // dispatcher can establish. An unfamiliar payload is never success.
        FactPayload::AssignedValue { .. }
        | FactPayload::BytePredicate { .. }
        | FactPayload::BooleanValue { .. }
        | FactPayload::TypeConstraint { .. }
        | FactPayload::ProofObligation { .. }
        | FactPayload::Contract { .. } => false,
    }
}

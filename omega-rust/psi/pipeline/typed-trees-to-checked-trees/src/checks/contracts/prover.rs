use checked_trees::{FlowCallFact, FlowStateFact};
use facts::{FactPayload, FactPlace};

mod assigned_values;
pub(super) use assigned_values::{
    prove_domain_at_place, scalar_value_at_place, segments_cover_subject,
};
mod scalars;
pub(super) use crate::values::evaluate_checked_scalar;
pub(super) use scalars::{
    ScalarValue, closed_boolean_value, evaluate as evaluate_scalar, evaluate_with_atoms,
    has_builtin_operators,
};
mod booleans;
pub(in crate::checks) mod call_guarantees;
mod field_actuals;
#[cfg(test)]
mod tests;

use self::booleans::{
    semantic_context_proves_boolean_expression,
    semantic_context_proves_instantiated_boolean_expression,
};
use super::evaluator::call_site_proves_boolean_contract_expression;

pub(super) fn semantic_contexts_prove_boolean_expression(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    entry_contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Boolean(true)
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
    program: &typed_trees::TypedTrees,
    facts: &checked_trees::CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    entry_contexts: &[facts::FactContextHandle],
    expression: typed_trees::expression::ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    let operators = &facts.operators;
    let semantic = &facts.semantic;
    let Some(call_site) = crate::semantic_calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    ) else {
        return false;
    };
    let Some(target_parameters) =
        crate::semantic_calls::call_target_parameters(program, call_flow.target_symbol)
    else {
        return false;
    };

    if call_guarantees::proves(
        program,
        facts,
        state_flow,
        call_flow,
        entry_contexts,
        expression,
        call_frames,
    ) {
        return true;
    }

    let mut field_handled = false;
    for entry_context in entry_contexts {
        if let Some(proven) = field_actuals::proves(
            program,
            semantic,
            semantic.contexts.get(*entry_context),
            state_flow.state_symbol,
            call_flow.statement_index,
            &call_site,
            target_parameters,
            expression,
        ) {
            field_handled = true;
            if proven {
                return true;
            }
        }
    }
    (!field_handled
        && entry_contexts.iter().any(|entry_context| {
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
        }))
        || call_site_proves_boolean_contract_expression(
            program,
            operators,
            state_flow,
            call_flow,
            &call_site,
            call_flow.target_symbol,
            target_parameters,
            expression,
            call_frames,
        )
}

pub(super) fn semantic_contexts_prove_contract_fact(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    entry_contexts: &[facts::FactContextHandle],
    fact: &facts::Fact,
) -> bool {
    match fact.payload {
        FactPayload::DomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        }
        | FactPayload::ContractDomainMembership {
            domain_symbol,
            semantic_domain,
            ..
        } => {
            let FactPlace::Place(place) = fact.place else {
                return false;
            };
            if indexed_membership(program, fact.payload) {
                // Indexed applications are invariant. In particular, neither
                // declaration implication nor a scalar value alone supplies
                // equality of their indices. Flow has already substituted
                // exact caller subjects and refreshed specialized identities.
                return semantic_domain.is_valid() && entry_contexts.iter().any(|entry_context| {
                    semantic.context_view(semantic.contexts.get(*entry_context)).facts().any(|candidate| {
                        matches!(candidate.payload,
                            FactPayload::DomainMembership { domain_symbol: candidate_domain, semantic_domain: candidate_instance, .. }
                            | FactPayload::ContractDomainMembership { domain_symbol: candidate_domain, semantic_domain: candidate_instance, .. }
                            if candidate_domain == domain_symbol && candidate_instance == semantic_domain)
                            && matches!(candidate.place, FactPlace::Place(candidate_place)
                                if semantic.places_match(program, candidate_place, place))
                    })
                });
            }
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
                        crate::facts::field_domain::domain_membership_implies(
                            program,
                            candidate_domain,
                            domain_symbol,
                        ) && (semantic.places_match(program, candidate_place, place)
                            || place_covers_subject(program, semantic, candidate_place, place))
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
                checked_trees::expression::ExpressionNode::Boolean(true)
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
        | FactPayload::AssignedIntegerBounds { .. }
        | FactPayload::AssignedScalarValue { .. }
        | FactPayload::StorageDependency { .. }
        | FactPayload::BytePredicate { .. }
        | FactPayload::BooleanValue { .. }
        | FactPayload::MatchPattern { .. }
        | FactPayload::TypeConstraint { .. }
        | FactPayload::ProofObligation { .. }
        | FactPayload::Contract { .. } => false,
    }
}

/// Whether a live fact at `candidate` covers the required `subject`
/// elementwise: same place root, and at every position either equal segments
/// or a containing `FixedRange` (the whole-extent `0..usize::MAX` row covers
/// a runtime `Index`; a finite range covers contained indices -- see
/// `assigned_values::segments_cover_subject`).
fn place_covers_subject(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    candidate: facts::PlaceHandle,
    subject: facts::PlaceHandle,
) -> bool {
    let candidate = semantic.places.get(candidate);
    let subject = semantic.places.get(subject);
    candidate.root == subject.root
        && assigned_values::segments_cover_subject(
            program,
            semantic.place_segments.span_or_empty(candidate.segments),
            semantic.place_segments.span_or_empty(subject.segments),
        )
}

pub(super) fn indexed_membership(program: &typed_trees::TypedTrees, payload: FactPayload) -> bool {
    let (FactPayload::DomainMembership { domain_symbol, .. }
    | FactPayload::ContractDomainMembership { domain_symbol, .. }) = payload
    else {
        return false;
    };
    program.domain_definitions().iter().any(|domain| {
        domain.symbol == domain_symbol
            && !typed_trees::domain::index_parameters(program, domain).is_empty()
    })
}

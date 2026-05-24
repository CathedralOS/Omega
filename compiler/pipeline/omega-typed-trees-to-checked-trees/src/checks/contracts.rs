use omega_checked_trees::{CheckFacts, FlowCallFact, FlowInvalidationSource, FlowStateFact};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, FactPlace, FactPlan};

use crate::labels::{
    call_target_label, canonical_place_label, canonical_place_label_from_parts, joined_place_label,
    machine_name, semantic_fact_requirement_label, symbol_name,
};

pub(crate) fn check_flow_call_contracts(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.states.iter() {
        for call_flow in facts.flow.calls.span_or_empty(state_flow.calls) {
            check_call_requires(program, facts, state_flow, call_flow, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_call_requires(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_contexts = facts
        .flow
        .semantic_context_refs
        .span_or_empty(call_flow.entry_semantic_contexts);
    for requires_context in facts
        .flow
        .semantic_context_refs
        .span_or_empty(call_flow.requires_contexts)
    {
        let context = facts.semantic.contexts.get(requires_context.context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = match fact.payload {
                FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                    let place = match fact.place {
                        FactPlace::Place(place) => place,
                        _ => {
                            diagnostics.push(Diagnostic::error(format!(
                                "cannot interpret requires contract for call {} from {}",
                                call_target_label(program, call_flow.target_symbol),
                                machine_name(program, state_flow.machine_symbol)
                            )));
                            continue;
                        }
                    };
                    entry_contexts.iter().any(|entry_context| {
                        let context = facts.semantic.contexts.get(entry_context.context);
                        context_proves_requirement_place_domain(
                            program,
                            &facts.semantic,
                            context,
                            place,
                            domain_symbol,
                        )
                    })
                }
                FactPayload::ContractBooleanExpression { expression, .. } => matches!(
                    program.expression_table.expression(expression),
                    omega_checked_trees::expression::ExpressionNode::Boolean(true)
                ),
                _ => true,
            };

            if !satisfied {
                let detail = match fact.payload {
                    FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                        let FactPlace::Place(place) = fact.place else {
                            unreachable!("contract domain membership already handled above")
                        };
                        explain_domain_requirement_failure(
                            program,
                            facts,
                            state_flow,
                            call_flow,
                            place,
                            domain_symbol,
                        )
                    }
                    _ => None,
                };
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove requires contract for call {} from {}: {}{}",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    semantic_fact_requirement_label(program, &facts.semantic, fact),
                    detail
                        .map(|message| format!(" ({message})"))
                        .unwrap_or_default()
                )));
            }
        }
    }
}

fn explain_domain_requirement_failure(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    required_place: omega_facts::PlaceHandle,
    required_domain: SymbolHandle,
) -> Option<String> {
    let mut detail = None;
    for invalidation in facts
        .flow
        .invalidations
        .span_or_empty(state_flow.invalidations)
        .iter()
    {
        if !invalidation_precedes_call(invalidation.source, call_flow) {
            continue;
        }

        let fact = facts.semantic.facts.get(invalidation.fact);
        let (fact_domain, fact_place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    continue;
                };
                (domain_symbol, place)
            }
            _ => continue,
        };

        if !facts.semantic.domain_implies(fact_domain, required_domain)
            || !places_match_requirement(program, &facts.semantic, fact_place, required_place)
        {
            continue;
        }

        let fact_place = facts.semantic.places.get(fact_place);
        let dependency_segments = facts
            .flow
            .invalidation_segments
            .span_or_empty(invalidation.dependency_segments);
        let invalidated =
            joined_place_label(program, &facts.semantic, fact_place, dependency_segments);
        let mutated = canonical_place_label_from_parts(
            program,
            invalidation.mutated_root,
            facts.flow
                .invalidation_segments
                .span_or_empty(invalidation.mutated_segments),
        );
        detail = Some(format!(
            "invalidated by prior mutation of {mutated}; {invalidated} is part of {}",
            symbol_name(program, required_domain)
        ));
    }

    detail
}

fn invalidation_precedes_call(
    source: FlowInvalidationSource,
    call_flow: &FlowCallFact,
) -> bool {
    match source {
        FlowInvalidationSource::Statement { statement_index } => {
            statement_index < call_flow.statement_index
        }
        FlowInvalidationSource::Call {
            statement_index,
            call_ordinal,
            ..
        } => {
            statement_index < call_flow.statement_index
                || (statement_index == call_flow.statement_index
                    && call_ordinal < call_flow.call_ordinal)
        }
    }
}

fn places_match_requirement(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    candidate: omega_facts::PlaceHandle,
    required: omega_facts::PlaceHandle,
) -> bool {
    semantic.places_equal(candidate, required)
        || canonical_place_label(program, semantic, semantic.places.get(candidate))
            == canonical_place_label(program, semantic, semantic.places.get(required))
}

pub(crate) fn context_proves_requirement_place_domain(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    context: &omega_facts::FactContext,
    required_place: omega_facts::PlaceHandle,
    required_domain: SymbolHandle,
) -> bool {
    let required_label =
        canonical_place_label(program, semantic, semantic.places.get(required_place));
    semantic.context_view(context).facts().any(|fact| {
        let (fact_domain, fact_place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, place)
            }
            _ => return false,
        };

        semantic.domain_implies(fact_domain, required_domain)
            && (semantic.places_equal(fact_place, required_place)
                || canonical_place_label(program, semantic, semantic.places.get(fact_place))
                    == required_label)
    })
}

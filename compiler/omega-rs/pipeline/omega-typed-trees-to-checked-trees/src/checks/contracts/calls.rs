use omega_checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, FactPlace, PlaceRoot};

use super::places::expression_is_boolean_place_like;
use super::prover::{
    call_entry_contexts_prove_boolean_contract_expression, semantic_contexts_prove_contract_fact,
};
use crate::labels::{
    call_target_label, canonical_place_label_from_parts, joined_place_label, machine_name,
    semantic_fact_requirement_label, symbol_name,
};

pub(super) fn check_call_requires(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut entry_contexts: Vec<_> = facts
        .flow
        .state_call_entry_semantic_contexts(
            state_flow,
            call_flow.statement_index,
            call_flow.call_ordinal,
            call_flow.target_symbol,
            call_flow.receiver_symbol,
        )
        .collect();
    // #66: immutable state-parameter domain facts (origin StateParameterDomain,
    // surfaced at ProgramPoint::State by append_state_parameter_domain_facts) are
    // always-holding STATE invariants -- the param's caller-enforced `requires`
    // plus immutability means no invalidation is possible. The per-statement flow
    // threading drops them on some branch-leaf and dispatch-specialized paths
    // (a forwarded `consume(text)` saw 0 entry contexts), so consult the State
    // context directly. Sound precisely because these facts cannot be invalidated;
    // mutable params are excluded by the producer and stay on the flow path.
    entry_contexts.extend(facts.semantic.context_handles_at_point(
        omega_facts::ProgramPoint::State {
            machine_symbol: state_flow.machine_symbol,
            state_symbol: state_flow.state_symbol,
        },
    ));
    for requires_context in facts
        .flow
        .semantic_constraint_contexts(call_flow.requires_constraints)
    {
        let context = facts.semantic.contexts.get(requires_context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = match fact.payload {
                FactPayload::ContractBooleanExpression { expression, .. } => {
                    if expression_is_boolean_place_like(program, expression) {
                        semantic_contexts_prove_contract_fact(
                            program,
                            &facts.semantic,
                            &entry_contexts,
                            fact,
                        )
                    } else {
                        call_entry_contexts_prove_boolean_contract_expression(
                            program,
                            &facts.semantic,
                            state_flow,
                            call_flow,
                            &entry_contexts,
                            expression,
                        )
                    }
                }
                _ => semantic_contexts_prove_contract_fact(
                    program,
                    &facts.semantic,
                    &entry_contexts,
                    fact,
                ),
            };
            // ch8 construction-grant: a string literal whose compile-time bytes
            // satisfy a domain's declared classifier predicate grants that
            // domain without a validating boundary call -- this is how a literal
            // flows into a `&[u8] in Utf8` (or any classifier-backed) target.
            let satisfied = satisfied
                || string_literal_grants_domain(program, &facts.semantic, fact.payload, fact.place);

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
                    FactPayload::ContractBooleanExpression { expression, .. } => {
                        explain_missing_boolean_fact(program, expression)
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

/// ch8 construction-grant: a string literal grants a domain `D` -- satisfying a
/// `requires <arg> in D` membership without a validating boundary call -- iff
/// `D`'s declared classifier is a recognized comptime byte-predicate over `self`
/// (`valid_utf8`/`no_nul`/`ascii_only`) AND that predicate holds for the
/// literal's compile-time bytes. Reuses the shared comptime byte-predicate
/// primitives (`super::grants`); the subject must be the literal itself (an
/// expression-rooted place with no field/index segments), not a derived place.
fn string_literal_grants_domain(
    program: &omega_typed_trees::TypedTrees,
    semantic: &omega_facts::FactPlan,
    payload: FactPayload,
    place: FactPlace,
) -> bool {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return false;
    };
    let FactPlace::Place(place_handle) = place else {
        return false;
    };
    let resolved = semantic.places.get(place_handle);
    if !resolved.segments.is_empty() {
        return false;
    }
    let PlaceRoot::Expression(expression) = resolved.root else {
        return false;
    };
    crate::field_domain::string_literal_expression_grants_domain(program, expression, domain_symbol)
}

/// Clear "needs fact X here" guidance for a proof-backed operator/contract that
/// is missing a required boolean fact (for example an index bound or a
/// domain-sensitive operator precondition). The caller has not established the
/// fact in the entry context, so name exactly what must hold before the call.
fn explain_missing_boolean_fact(
    program: &omega_typed_trees::TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    let fact = program.expression_table.display_name(expression);
    if fact.is_empty() {
        return None;
    }
    Some(format!(
        "needs fact `{fact}` here; establish it before the call via a prior \
         contract guarantee, domain membership, or guard"
    ))
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
        .state_call_prior_invalidations(state_flow, call_flow)
    {
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
            || !facts
                .semantic
                .places_match(program, fact_place, required_place)
        {
            continue;
        }

        let fact_place = facts.semantic.places.get(fact_place);
        let dependency_segments = facts
            .flow
            .invalidations
            .segments
            .span_or_empty(invalidation.dependency_segments);
        let invalidated =
            joined_place_label(program, &facts.semantic, fact_place, dependency_segments);
        let mutated = canonical_place_label_from_parts(
            program,
            invalidation.mutated_root,
            facts
                .flow
                .invalidations
                .segments
                .span_or_empty(invalidation.mutated_segments),
        );
        detail = Some(format!(
            "invalidated by prior mutation of {mutated}; {invalidated} is part of {}",
            symbol_name(program, required_domain)
        ));
    }

    detail
}

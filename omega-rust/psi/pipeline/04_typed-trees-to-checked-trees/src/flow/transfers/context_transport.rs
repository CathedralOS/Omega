//! Carrying each active context's facts about the written value's source
//! onto the written place: assigned cases and values, byte predicates and
//! qualifications, each with its source correspondence retained.

use super::{
    ExpressionHandle, Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FlowBuildContext,
    HandleSpan, ProgramPoint, SymbolHandle, retain_qualification_correspondence,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn transport(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    write: &super::write_target::StatementWrite,
    source_label: &str,
    stable_value_target: bool,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    context_handles: Vec<facts::FactContextHandle>,
    refs: &mut HandleSpan<facts::FactRef>,
) {
    let target_place = write.target_place;
    let source_place = write.source_place;
    for context_handle in context_handles {
        let context = semantic.contexts.get(context_handle);
        let facts_to_transfer: Vec<_> = semantic
            .refs
            .span_or_empty(context.facts)
            .iter()
            .filter_map(|reference| {
                let fact = *semantic.facts.get(reference.fact);
                match fact.payload {
                    FactPayload::AssignedCase { .. }
                    | FactPayload::AssignedValue { .. }
                    | FactPayload::AssignedScalarValue { .. }
                    | FactPayload::BytePredicate { .. } => {
                        if !stable_value_target {
                            return None;
                        }
                        let FactPlace::Place(fact_place) = fact.place else {
                            return None;
                        };
                        source_place
                            .filter(|source_place| {
                                semantic.places_match(program, fact_place, *source_place)
                            })
                            .map(|_| (fact.payload, fact.evidence, None))
                    }
                    FactPayload::DomainMembership {
                        domain,
                        domain_symbol,
                        semantic_domain,
                        ..
                    }
                    | FactPayload::ContractDomainMembership {
                        domain,
                        domain_symbol,
                        semantic_domain,
                        ..
                    } => {
                        let FactPlace::Place(fact_place) = fact.place else {
                            return None;
                        };
                        let fact_label = semantic.place_label(program, fact_place);
                        (source_place.is_some_and(|source_place| {
                            semantic.places_match(program, fact_place, source_place)
                        }) || (!crate::facts::field_domain::domain_requires_provenance(
                            program,
                            domain_symbol,
                        ) && fact_label == source_label))
                            .then_some((
                                FactPayload::DomainMembership {
                                    value: ExpressionHandle::invalid(),
                                    domain,
                                    domain_symbol,
                                    semantic_domain,
                                },
                                fact.evidence,
                                Some((reference.fact, fact_place)),
                            ))
                    }
                    FactPayload::CarryPermission { permission, .. }
                    | FactPayload::ContractCarryPermission { permission, .. } => {
                        let FactPlace::Place(fact_place) = fact.place else {
                            return None;
                        };
                        let fact_label = semantic.place_label(program, fact_place);
                        (source_place.is_some_and(|source_place| {
                            semantic.places_match(program, fact_place, source_place)
                        }) || fact_label == source_label)
                            .then_some((
                                FactPayload::CarryPermission {
                                    value: ExpressionHandle::invalid(),
                                    permission,
                                },
                                fact.evidence,
                                Some((reference.fact, fact_place)),
                            ))
                    }
                    FactPayload::CarryOrigin { .. } => {
                        let FactPlace::Place(fact_place) = fact.place else {
                            return None;
                        };
                        let fact_label = semantic.place_label(program, fact_place);
                        (source_place.is_some_and(|source_place| {
                            semantic.places_match(program, fact_place, source_place)
                        }) || fact_label == source_label)
                            .then_some((
                                FactPayload::CarryOrigin {
                                    value: ExpressionHandle::invalid(),
                                },
                                fact.evidence,
                                Some((reference.fact, fact_place)),
                            ))
                    }
                    FactPayload::BooleanExpression(expression) => {
                        (program.expression_table.display_name(expression) == source_label)
                            .then_some((
                                FactPayload::BooleanExpression(expression),
                                fact.evidence,
                                None,
                            ))
                    }
                    FactPayload::ContractBooleanExpression {
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => {
                        (program.expression_table.display_name(expression) == source_label)
                            .then_some((
                                FactPayload::BooleanExpression(expression),
                                fact.evidence,
                                None,
                            ))
                    }
                    _ => None,
                }
            })
            .collect();

        for (payload, evidence, source) in facts_to_transfer {
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(target_place),
                point: ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                },
                origin: FactOrigin::StatementTransfer,
                evidence,
                payload,
            });
            semantic.append_ref(refs, fact);
            if let Some((source_fact, source_fact_place)) = source
                && let Some(source_occurrence_place) = source_place
            {
                retain_qualification_correspondence(
                    program,
                    build,
                    semantic,
                    source_fact,
                    fact,
                    source_fact_place,
                    source_occurrence_place,
                    target_place,
                    ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    },
                    payload,
                    evidence,
                );
            }
        }
    }
}

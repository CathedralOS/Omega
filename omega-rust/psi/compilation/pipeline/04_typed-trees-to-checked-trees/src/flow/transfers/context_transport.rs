//! Carrying each active context's facts about the written value's source
//! onto the written place: assigned cases and values, byte predicates and
//! qualifications, each with its source correspondence retained.

use super::{
    ExpressionHandle, Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FlowBuildContext,
    HandleSpan, ProgramPoint, SymbolHandle, retain_qualification_correspondence,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn transport(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    write: &super::write_target::StatementWrite,
    completed_source: Option<typed_trees::types::TypeReferenceHandle>,
    stable_value_target: bool,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    context_handles: Vec<facts::FactContextHandle>,
    refs: &mut HandleSpan<facts::FactRef>,
) {
    if context_handles.is_empty() {
        return;
    }
    let target_place = write.target_place;
    let source_place = write.source_place;
    let source_value_place = source_place.and_then(|source| {
        crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(source),
        )
    });
    let mut value_carriers_agree: Option<bool> = None;
    for context_handle in context_handles {
        let context = semantic.contexts.get(context_handle);
        let facts_to_transfer: Vec<_> = semantic
            .refs
            .span_or_empty(context.facts)
            .iter()
            .filter_map(|reference| {
                let fact = *semantic.facts.get(reference.fact);
                // Transport belongs to this completed source occurrence. A
                // diagnostic label can coincide for different constructors,
                // calls or shadowed bindings and supplies no value identity.
                let matches_source = |fact_place| {
                    source_value_place.as_ref().is_some_and(|source| {
                        crate::flow::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(fact_place),
                        )
                        .is_some_and(|place| {
                            crate::flow::canonical_places_agree(program, &place, source)
                        })
                    })
                };
                match fact.payload {
                    FactPayload::CaseValue { .. }
                    | FactPayload::AssignedValue { .. }
                    | FactPayload::AssignedScalarValue { .. }
                    | FactPayload::AssignedIntegerBounds { .. }
                    | FactPayload::AssignedValueCapture { .. }
                    | FactPayload::BytePredicate { .. }
                    | FactPayload::FloatPredicate { .. } => {
                        if !stable_value_target {
                            return None;
                        }
                        let carriers_agree = *value_carriers_agree.get_or_insert_with(|| {
                            // Storage aliasing does not preserve a scalar value across a recast.
                            // Compare the source and installed referent carriers before transporting
                            // completed value observations; reference access itself is not a value.
                            source_place.is_some_and(|source| {
                                let source = crate::flow::canonical_place_from_semantic_place(
                                    program,
                                    semantic,
                                    semantic.places.get(source),
                                );
                                let target = crate::flow::canonical_place_from_semantic_place(
                                    program,
                                    semantic,
                                    semantic.places.get(target_place),
                                );
                                source.zip(target).is_some_and(|(source, target)| {
                                    super::completed_value_carriers_agree(
                                        program,
                                        state_symbol,
                                        statement_index,
                                        &source,
                                        &target,
                                        completed_source,
                                    )
                                })
                            })
                        });
                        if !carriers_agree {
                            return None;
                        }
                        let fact_place = fact.place.as_place()?;
                        matches_source(fact_place).then_some((fact.payload, fact.evidence, None))
                    }
                    facts::domain_membership_fields_pattern!(
                        domain,
                        domain_symbol,
                        semantic_domain
                    ) => {
                        let fact_place = fact.place.as_place()?;
                        matches_source(fact_place).then_some((
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
                    facts::carry_permission_pair_pattern!(permission, ..) => {
                        let fact_place = fact.place.as_place()?;
                        matches_source(fact_place).then_some((
                            FactPayload::CarryPermission {
                                value: ExpressionHandle::invalid(),
                                permission,
                            },
                            fact.evidence,
                            Some((reference.fact, fact_place)),
                        ))
                    }
                    FactPayload::CarryOrigin { .. } => {
                        let fact_place = fact.place.as_place()?;
                        matches_source(fact_place).then_some((
                            FactPayload::CarryOrigin {
                                value: ExpressionHandle::invalid(),
                            },
                            fact.evidence,
                            Some((reference.fact, fact_place)),
                        ))
                    }
                    FactPayload::BooleanExpression(expression) => {
                        (expression == write.source_expression).then_some((
                            FactPayload::BooleanExpression(expression),
                            fact.evidence,
                            None,
                        ))
                    }
                    FactPayload::ContractBooleanExpression {
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => (expression == write.source_expression)
                        .then_some((
                            FactPayload::BooleanExpression(expression),
                            fact.evidence,
                            None,
                        )),
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

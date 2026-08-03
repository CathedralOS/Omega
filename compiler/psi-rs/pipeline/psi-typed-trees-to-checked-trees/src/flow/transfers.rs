use super::*;
use psi_facts::PlaceHandle;

pub(super) fn propagate_statement_transfers(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    let (target_place, source_expression, source_place) = match statement {
        StatementNode::AssemblyFact(_) => return,
        StatementNode::LocalData(local_data) => (
            semantic.append_symbol_place(local_data.symbol),
            local_data.initial_value,
            contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                local_data.initial_value,
            ),
        ),
        StatementNode::Assignment(assignment) => {
            let Some(target_place) = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.target,
            ) else {
                return;
            };
            let source_place = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.value,
            );
            (target_place, assignment.value, source_place)
        }
        StatementNode::Call(_) | StatementNode::Expression(_) | StatementNode::Transition(_) => {
            return;
        }
    };
    let source_label = program.expression_table.display_name(source_expression);

    let mut refs = HandleSpan::empty();
    let context_handles: Vec<_> = ctx
        .contexts
        .semantic_context_refs
        .span_or_empty(*active_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();

    for context_handle in context_handles {
        let context = semantic.contexts.get(context_handle);
        let facts_to_transfer: Vec<_> = semantic
            .context_view(context)
            .facts()
            .filter_map(|fact| match fact.payload {
                FactPayload::DomainMembership {
                    domain,
                    domain_symbol,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain,
                    domain_symbol,
                    ..
                } => {
                    let FactPlace::Place(fact_place) = fact.place else {
                        return None;
                    };
                    let fact_label = crate::labels::canonical_place_label(
                        program,
                        semantic,
                        semantic.places.get(fact_place),
                    );
                    (source_place.is_some_and(|source_place| {
                        semantic.places_match(program, fact_place, source_place)
                    }) || fact_label == source_label)
                        .then_some((
                            FactPayload::DomainMembership {
                                value: ExpressionHandle::invalid(),
                                domain,
                                domain_symbol,
                            },
                            fact.evidence,
                        ))
                }
                FactPayload::CarryPermission { permission, .. }
                | FactPayload::ContractCarryPermission { permission, .. } => {
                    let FactPlace::Place(fact_place) = fact.place else {
                        return None;
                    };
                    let fact_label = crate::labels::canonical_place_label(
                        program,
                        semantic,
                        semantic.places.get(fact_place),
                    );
                    (source_place.is_some_and(|source_place| {
                        semantic.places_match(program, fact_place, source_place)
                    }) || fact_label == source_label)
                        .then_some((
                            FactPayload::CarryPermission {
                                value: ExpressionHandle::invalid(),
                                permission,
                            },
                            fact.evidence,
                        ))
                }
                FactPayload::CarryOrigin { .. } => {
                    let FactPlace::Place(fact_place) = fact.place else {
                        return None;
                    };
                    let fact_label = crate::labels::canonical_place_label(
                        program,
                        semantic,
                        semantic.places.get(fact_place),
                    );
                    (source_place.is_some_and(|source_place| {
                        semantic.places_match(program, fact_place, source_place)
                    }) || fact_label == source_label)
                        .then_some((
                            FactPayload::CarryOrigin {
                                value: ExpressionHandle::invalid(),
                            },
                            fact.evidence,
                        ))
                }
                FactPayload::BooleanExpression(expression) => {
                    (program.expression_table.display_name(expression) == source_label)
                        .then_some((FactPayload::BooleanExpression(expression), fact.evidence))
                }
                FactPayload::ContractBooleanExpression {
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => {
                    (program.expression_table.display_name(expression) == source_label)
                        .then_some((FactPayload::BooleanExpression(expression), fact.evidence))
                }
                _ => None,
            })
            .collect();

        for (payload, evidence) in facts_to_transfer {
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
            semantic.append_ref(&mut refs, fact);
        }
    }

    // #66 read-narrowing across a write: initializing or assigning any
    // domain-refined declared place ESTABLISHES that destination's domain. The
    // write checker separately proves the source satisfies the declaration;
    // recording the fact here makes a checked LET usable at later call and
    // operator boundaries just like a checked reassignment. An uninitialized
    // local grants nothing.
    let declared_target_domains = match statement {
        StatementNode::Assignment(assignment) => {
            match (
                crate::field_domain::machine_by_symbol(program, machine_symbol),
                crate::find_state_in_machine(program, machine_symbol, state_symbol),
            ) {
                (Some(machine), Some(state)) => {
                    crate::field_domain::assignment_target_domain_symbols(
                        program,
                        machine,
                        state,
                        assignment.target,
                    )
                }
                _ => Vec::new(),
            }
        }
        StatementNode::LocalData(local) if local.initial_value.is_valid() => {
            crate::field_domain::domain_constraint_symbols(program, local.type_reference)
        }
        _ => Vec::new(),
    };
    for domain_symbol in declared_target_domains {
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::from_origin(
                psi_language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                state_symbol,
            ),
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol,
            },
        });
        semantic.append_ref(&mut refs, fact);
    }

    if refs.is_empty() {
        return;
    }

    let context = semantic.append_context(
        ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        },
        refs,
    );
    let mut next_contexts =
        clone_flow_contexts(&mut ctx.contexts.semantic_context_refs, *active_contexts);
    ctx.contexts
        .semantic_context_refs
        .append_to_span(&mut next_contexts, FlowSemanticContextRef { context });
    *active_contexts = next_contexts;
    let mut next_constraints =
        clone_constraint_refs(&mut ctx.contexts.constraint_refs, *active_constraints);
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
        &mut next_constraints,
        FlowConstraintKind::SemanticContext { context },
    );
    *active_constraints = next_constraints;
}

fn contextual_expression_place(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    let _ = machine_symbol;
    crate::semantic_places::canonical_place_to_fact_place_in_state(
        program,
        semantic,
        state_symbol,
        statement_index,
        expression,
    )
}

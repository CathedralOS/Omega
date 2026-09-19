use crate::flow::CanonicalPlace;
use crate::flow::FlowBuildContext;
use crate::flow::append_constraint_ref;
use crate::flow::common;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowFacts, FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, ProgramPoint,
    QualificationEvidence,
};
use facts::{PlaceHandle, QualificationCorrespondence, QualificationPayloadIdentity};
use symbols::SymbolHandle;

mod byte_sequences;
mod constructed;
mod owned_qualifications;
mod projected;
mod scalar_values;

#[cfg(test)]
mod byte_sequence_tests;

pub(super) fn propagate_statement_transfers(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    assignment_source_contexts: HandleSpan<FlowSemanticContextRef>,
    active_contexts: &mut HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut HandleSpan<FlowConstraintRef>,
) {
    // An assignment through a reference local of finite candidate origins
    // (flow/reference_places) rewrites exactly one candidate with a value the
    // write checker proved in the destination's declared domains; every
    // other candidate keeps its previous contents. Each candidate therefore
    // holds a declared domain after the write exactly when it held it
    // before, and those rows are re-established below.
    let mut candidate_targets: Vec<CanonicalPlace> = Vec::new();
    let (target_place, source_expression, source_place) = match statement {
        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => return,
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
            // A write through a local `&mut` alias establishes its facts on
            // the exact storage it aliases -- the same place invalidation
            // retires -- so `alias.out = "XXX"` and `label_alias = "hello"`
            // close the window on the aliased field. A local binding
            // replacement rebinds the reference itself and an ambiguous
            // origin proves nothing exact, so both keep the alias place.
            let Some(target_place) = crate::flow::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            )
            .map(|canonical| {
                let mut owned_frames = None;
                let writes_through_alias =
                    crate::flow::shared_call_frames_or(ctx.call_frames, program, &mut owned_frames)
                        .zip(
                            program
                                .machines()
                                .iter()
                                .find(|machine| machine.symbol == machine_symbol),
                        )
                        .and_then(|(resolver, machine)| {
                            resolver.assignment_write_target(machine, statement)
                        })
                        // An unclassified target is a write through a
                        // reference local the resolver has no origin for;
                        // only a local binding replacement keeps the alias.
                        .is_none_or(|target| {
                            matches!(target, validation::AssignmentWriteTarget::Storage { .. })
                        });
                if !writes_through_alias {
                    return canonical;
                }
                match crate::flow::rebase_exact_local_place(
                    program,
                    state_symbol,
                    statement_index,
                    canonical.clone(),
                    ctx.call_frames,
                ) {
                    Some(exact) => exact,
                    None => {
                        if let PlaceRoot::Symbol(root) = canonical.root
                            && let Some(candidates) =
                                crate::flow::reference_result_candidates_before_statement(
                                    program,
                                    state_symbol,
                                    statement_index,
                                    root,
                                    ctx.call_frames,
                                )
                        {
                            candidate_targets = candidates
                                .into_iter()
                                .map(|mut candidate| {
                                    candidate.segments.extend_from_slice(&canonical.segments);
                                    candidate
                                })
                                .collect();
                        }
                        canonical
                    }
                }
            })
            .map(|canonical| {
                crate::semantic_places::append_place_with_segments(
                    semantic,
                    canonical.root,
                    &canonical.segments,
                )
            }) else {
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
    // A runtime index can change without writing the collection. Until value
    // facts retain index dependencies, only immutable selectors carry values.
    let stable_value_target = semantic
        .place_segments
        .span_or_empty(semantic.places.get(target_place).segments)
        .iter()
        .all(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::Case { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
            )
        });

    let mut refs = HandleSpan::empty();
    let context_handles: Vec<_> = ctx
        .contexts
        .semantic_context_refs
        .span_or_empty(*active_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();

    if let Some(source) = source_place {
        let source = semantic.places.get(source);
        let source = CanonicalPlace {
            root: source.root,
            segments: semantic
                .place_segments
                .span_or_empty(source.segments)
                .to_vec(),
        };
        if !crate::flow::place_cases_are_selected(
            program,
            semantic,
            &context_handles,
            machine_symbol,
            state_symbol,
            statement_index,
            &source,
        ) {
            return;
        }
    }

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
                        let fact_label = crate::labels::canonical_place_label(
                            program,
                            semantic,
                            semantic.places.get(fact_place),
                        );
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
                                Some((reference.fact, fact_place)),
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
            semantic.append_ref(&mut refs, fact);
            if let Some((source_fact, source_fact_place)) = source
                && let Some(source_occurrence_place) = source_place
            {
                retain_qualification_correspondence(
                    program,
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

    if let Some(source_place) = source_place {
        let destination_type = match statement {
            StatementNode::LocalData(local) => Some(local.type_reference),
            StatementNode::Assignment(assignment) => {
                crate::flow::expression_type_reference_in_state(
                    program,
                    state_symbol,
                    statement_index,
                    assignment.target,
                )
            }
            _ => None,
        };
        if let Some(destination_type) = destination_type {
            owned_qualifications::append_owned_qualification_transfer(
                program,
                semantic,
                ctx,
                *active_contexts,
                source_place,
                target_place,
                destination_type,
                ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                },
                &mut refs,
            );
        }
        projected::append_copied_field_predicates(
            program,
            semantic,
            ctx,
            *active_contexts,
            source_place,
            target_place,
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            &mut refs,
        );
    }

    // A builtin collection view lends the receiver's element storage to the
    // binding: `let view = rows.as_mut_slice()` makes `view[i]` the same
    // element as `rows[i]`, so live evidence below the receiver re-anchors
    // below the destination through the same stable-segment transport.
    let viewed_place = projected::collection_view_source_place(
        program,
        semantic,
        machine_symbol,
        state_symbol,
        statement_index,
        source_expression,
    );
    if let Some(viewed_place) = viewed_place {
        projected::append_copied_field_predicates(
            program,
            semantic,
            ctx,
            *active_contexts,
            viewed_place,
            target_place,
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            &mut refs,
        );
    }

    // A reference result lends its proven referent to the binding the same
    // way a view lends receiver storage: `let room = room_mut(level)` makes
    // `room` the storage the callee selected, and the local write origins
    // already track exactly which caller place that is. An expression-rooted
    // source has no canonical storage of its own, so evidence below the
    // referent re-anchors below the binding; a symbol-rooted source was
    // already transported above.
    if viewed_place.is_none()
        && source_place.is_some_and(|place| {
            matches!(
                semantic.places.get(place).root,
                facts::PlaceRoot::Expression(_)
            )
        })
        && let Some(referent_place) = projected::bound_reference_referent_place(
            program,
            semantic,
            ctx,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            target_place,
        )
    {
        projected::append_copied_field_predicates(
            program,
            semantic,
            ctx,
            *active_contexts,
            referent_place,
            target_place,
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            &mut refs,
        );
    }

    if stable_value_target {
        constructed::append_constructed_field_values(
            program,
            semantic,
            ctx,
            assignment_source_contexts,
            statement,
            source_expression,
            target_place,
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            &mut refs,
        );
        byte_sequences::append_concatenated_predicates(
            program,
            semantic,
            ctx,
            assignment_source_contexts,
            source_expression,
            target_place,
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            &mut refs,
        );
    }

    let scalar_value = scalar_values::capture_statement(
        program,
        borrow,
        semantic,
        ctx,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
        assignment_source_contexts,
    );
    let integer_bounds = if scalar_value.is_none() {
        scalar_values::capture_bounds(
            program,
            borrow,
            semantic,
            ctx,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            assignment_source_contexts,
        )
    } else {
        None
    };
    if let StatementNode::Assignment(assignment) = statement {
        byte_sequences::append_element_replacement_predicates(
            program,
            semantic,
            ctx,
            assignment_source_contexts,
            machine_symbol,
            state_symbol,
            statement_index,
            assignment.target,
            source_expression,
            scalar_value.as_ref(),
            integer_bounds.as_ref(),
            ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            &mut refs,
        );
    }

    if stable_value_target
        && let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(source_expression)
        && let Some(variant) = literal.case_symbol.filter(|variant| variant.is_valid())
        && program.symbols.get(variant).kind == symbols::SymbolKind::Variant
        && program.symbols.get(variant).parent == literal.type_symbol
    {
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedCase { variant },
        });
        semantic.append_ref(&mut refs, fact);
    }

    if stable_value_target
        && (matches!(
            program.expression_table.expression(source_expression),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::String(_)
        ) || (matches!(
            program.expression_table.expression(source_expression),
            ExpressionNode::Call(_)
        ) && semantic.places.get(target_place).segments.is_empty()))
    {
        // A call is retained only as the exact source occurrence of this
        // completed assignment. It is not evaluated here. Consumers need its
        // checked call/argument/guarantee joins before it supplies a value;
        // ordinary assignment invalidation still retires this provenance.
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedValue {
                value: source_expression,
            },
        });
        semantic.append_ref(&mut refs, fact);
    }

    if stable_value_target && let Some(bounds) = integer_bounds {
        let bounds = semantic.integer_ranges.append(bounds);
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedIntegerBounds { bounds },
        });
        semantic.append_ref(&mut refs, fact);
    }

    if stable_value_target && let Some(value) = scalar_value {
        let value = semantic.scalar_values.append(value);
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::AssignedScalarValue { value },
        });
        semantic.append_ref(&mut refs, fact);
    }

    // #66 read-narrowing across a write: initializing or assigning any
    // domain-refined declared place ESTABLISHES that destination's domain. The
    // write checker separately proves the source satisfies the declaration;
    // recording the fact here makes a checked LET usable at later call and
    // operator boundaries just like a checked reassignment. An uninitialized
    // local grants nothing.
    //
    // The fact carries the identity the typer interned on the declared
    // constraint, not only the definition symbol: an indexed application such
    // as `Extent in Granted & Resident<P, T>` is proved at a call `requires`
    // only against an exact instance identity, so a restated local that
    // recorded `NULL` here kept the domain for weakening and linearity but was
    // unprovable as a call premise.
    let declared_target_domains = match statement {
        StatementNode::Assignment(assignment) => {
            match (
                crate::facts::field_domain::machine_by_symbol(program, machine_symbol),
                crate::semantic_calls::find_state_in_machine(program, machine_symbol, state_symbol),
            ) {
                (Some(machine), Some(state)) => {
                    crate::facts::field_domain::assignment_target_domain_identities(
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
            crate::facts::field_domain::domain_constraint_identities(program, local.type_reference)
        }
        _ => Vec::new(),
    };
    // Routed membership follows its checked source; an annotation cannot
    // establish provenance. Initializer and write checks consume the source
    // facts before this statement, never these newly seeded predicate facts.
    let declared_target_domains: Vec<_> = declared_target_domains
        .into_iter()
        .filter(|(symbol, _)| {
            !crate::facts::field_domain::domain_requires_provenance(program, *symbol)
        })
        .collect();
    for (domain_symbol, semantic_domain) in &declared_target_domains {
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::from_origin(
                language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                state_symbol,
            ),
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: *domain_symbol,
                semantic_domain: *semantic_domain,
            },
        });
        semantic.append_ref(&mut refs, fact);
    }
    for candidate in candidate_targets {
        let candidate_place = crate::semantic_places::append_place_with_segments(
            semantic,
            candidate.root,
            &candidate.segments,
        );
        for (domain_symbol, semantic_domain) in &declared_target_domains {
            let was_live = ctx
                .contexts
                .semantic_context_refs
                .span_or_empty(assignment_source_contexts)
                .iter()
                .any(|reference| {
                    semantic
                        .context_view(semantic.contexts.get(reference.context))
                        .proves_place_domain_membership_in_program(
                            program,
                            candidate_place,
                            *domain_symbol,
                        )
                });
            if !was_live {
                continue;
            }
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(candidate_place),
                point: ProgramPoint::Statement {
                    machine_symbol,
                    state_symbol,
                    statement_index,
                },
                origin: FactOrigin::StatementTransfer,
                evidence: QualificationEvidence::from_origin(
                    language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                    state_symbol,
                ),
                payload: FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain: HandleSpan::empty(),
                    domain_symbol: *domain_symbol,
                    semantic_domain: *semantic_domain,
                },
            });
            semantic.append_ref(&mut refs, fact);
        }
    }

    if refs.is_empty() {
        return;
    }

    // One statement transports evidence for many independent storage
    // coordinates: a view binding re-anchors every element's declared fields
    // below the view. Invalidation drops a whole context once any of its
    // facts overlaps a write, so facts over distinct places take separate
    // contexts, as entry seeding already does; facts over one exact place
    // stay coupled.
    let point = ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    };
    let mut groups: Vec<(FactPlace, Vec<facts::FactRef>)> = Vec::new();
    for reference in semantic.refs.span_or_empty(refs) {
        let place = semantic.facts.get(reference.fact).place;
        if let Some((_, group)) =
            groups
                .iter_mut()
                .find(|(candidate, _)| match (*candidate, place) {
                    (FactPlace::Place(left), FactPlace::Place(right)) => {
                        semantic.places_equal(left, right)
                    }
                    _ => *candidate == place,
                })
        {
            group.push(*reference);
        } else {
            groups.push((place, vec![*reference]));
        }
    }
    let mut next_contexts =
        retained_flow_contexts(&ctx.contexts.semantic_context_refs, *active_contexts);
    let mut next_constraints =
        retained_constraint_refs(&ctx.contexts.constraint_refs, *active_constraints);
    for (_, group) in groups {
        let mut group_refs = HandleSpan::empty();
        for reference in group {
            semantic.refs.append_to_span(&mut group_refs, reference);
        }
        let context = semantic.append_context(point, group_refs);
        common::append_flow_reference(
            &mut ctx.contexts.semantic_context_refs,
            &mut next_contexts,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            &mut ctx.contexts.constraint_refs,
            &mut next_constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
    *active_contexts = next_contexts;
    *active_constraints = next_constraints;
}

#[allow(clippy::too_many_arguments)]
fn retain_qualification_correspondence(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    source_fact: facts::FactHandle,
    destination_fact: facts::FactHandle,
    source_place: PlaceHandle,
    source_occurrence_place: PlaceHandle,
    destination_place: PlaceHandle,
    formation: ProgramPoint,
    destination_payload: FactPayload,
    evidence: QualificationEvidence,
) {
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = formation
    else {
        return;
    };
    if evidence.origin != language_semantics::QualificationEvidenceOrigin::CheckedTransformation
        || !exact_evidence_source(program, evidence)
        || !exact_statement_owner(program, machine_symbol, state_symbol)
        || !exact_structural_symbol_place(
            program,
            semantic,
            source_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !exact_structural_symbol_place(
            program,
            semantic,
            source_occurrence_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !exact_structural_symbol_place(
            program,
            semantic,
            destination_place,
            machine_symbol,
            state_symbol,
            statement_index,
        )
        || !semantic.facts.is_valid(source_fact)
        || !semantic.facts.is_valid(destination_fact)
        || source_fact == destination_fact
        || source_fact.arena_index() >= destination_fact.arena_index()
        || source_place == destination_place
        || !semantic.places_equal(source_place, source_occurrence_place)
    {
        return;
    }
    let source = semantic.facts.get(source_fact);
    let destination = semantic.facts.get(destination_fact);
    if source.place != FactPlace::Place(source_place)
        || source.evidence != evidence
        || destination.place != FactPlace::Place(destination_place)
        || destination.point != formation
        || destination.origin != FactOrigin::StatementTransfer
        || destination.evidence != evidence
        || destination.payload != destination_payload
    {
        return;
    }
    let Some(payload) = QualificationPayloadIdentity::from_fact_payload(source.payload) else {
        return;
    };
    if QualificationPayloadIdentity::from_fact_payload(destination_payload) != Some(payload)
        || !exact_qualification_payload(program, payload)
    {
        return;
    }
    semantic.append_qualification_correspondence(QualificationCorrespondence {
        source_fact,
        destination_fact,
        source_occurrence_place,
        source_place,
        destination_place,
        formation,
        payload,
        evidence,
    });
}

fn exact_qualification_payload(
    program: &typed_trees::TypedTrees,
    payload: QualificationPayloadIdentity,
) -> bool {
    match payload {
        QualificationPayloadIdentity::DomainMembership {
            domain,
            domain_symbol,
            semantic_domain,
        } => {
            (!semantic_domain.is_valid()
                || program.semantic_domains.name(semantic_domain).is_some())
                && domain_symbol.is_valid()
                && program.symbols.get(domain_symbol).kind == symbols::SymbolKind::Domain
                && program.domain_path_members.span(domain).is_some()
        }
        QualificationPayloadIdentity::CarryPermission { .. }
        | QualificationPayloadIdentity::CarryOrigin => true,
    }
}

fn exact_evidence_source(
    program: &typed_trees::TypedTrees,
    evidence: QualificationEvidence,
) -> bool {
    evidence.source_symbol.is_valid()
        && evidence.requirement_symbol == SymbolHandle::invalid()
        && evidence.receipt_identity == 0
        && matches!(
            program.symbols.get(evidence.source_symbol).kind,
            symbols::SymbolKind::Machine | symbols::SymbolKind::Operator
        )
}

fn exact_structural_symbol_place(
    program: &typed_trees::TypedTrees,
    semantic: &FactPlan,
    handle: PlaceHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
) -> bool {
    if !semantic.places.is_valid(handle) {
        return false;
    }
    let place = semantic.places.get(handle);
    let facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    if !root.is_valid() {
        return false;
    }
    let Some(segments) = semantic.place_segments.span(place.segments) else {
        return false;
    };
    let Some(mut current) = correspondence_root_type_reference(
        program,
        machine_symbol,
        state_symbol,
        formation_statement_index,
        root,
    ) else {
        return false;
    };
    let mut selected_variant = None;
    for segment in segments {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                if !symbol.is_valid()
                    || program.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                {
                    return false;
                }
                let Some(data) = correspondence_data_type(program, current, machine_symbol) else {
                    return false;
                };
                let field = if let Some(variant_symbol) = selected_variant.take() {
                    program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == variant_symbol).then(|| {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == *symbol)
                        })?
                    })
                } else {
                    program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.symbol == *symbol).then_some(field)
                    })
                };
                let Some(field) = field else {
                    return false;
                };
                current = field.type_reference;
            }
            facts::PlaceSegment::Case { variant } => {
                if selected_variant.is_some()
                    || !variant.is_valid()
                    || program.symbols.get(*variant).kind != symbols::SymbolKind::Variant
                {
                    return false;
                }
                let Some(data) = correspondence_data_type(program, current, machine_symbol) else {
                    return false;
                };
                if !program.data_members(data).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Variant(candidate)
                        if candidate.symbol == *variant)
                }) {
                    return false;
                }
                selected_variant = Some(*variant);
            }
            facts::PlaceSegment::FixedIndex { index } => {
                if selected_variant.is_some() {
                    return false;
                }
                loop {
                    match program.type_reference_table.type_reference(current) {
                        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                        | typed_trees::types::TypeReferenceNode::Constrained {
                            base_type: referee,
                            ..
                        } => current = *referee,
                        typed_trees::types::TypeReferenceNode::FixedArray {
                            element_type,
                            length: typed_trees::types::FixedArrayLength::Literal(length),
                        } if *index < *length => {
                            current = *element_type;
                            break;
                        }
                        _ => return false,
                    }
                }
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::Index { .. } => {
                return false;
            }
        }
    }
    true
}

fn correspondence_root_type_reference(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
    root: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    match program.symbols.get(root).kind {
        symbols::SymbolKind::Parameter
            if matches!(
                program.symbols.get(root).parent,
                parent if parent == machine_symbol || parent == state_symbol
            ) =>
        {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == root)
                .map(|parameter| parameter.type_reference)
        }
        symbols::SymbolKind::Local
            if program.symbols.get(root).parent == state_symbol
                && formation_statement_index
                    < program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len() =>
        {
            let mut declarations = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(formation_statement_index)
                .filter_map(|statement| {
                    let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.symbol == root).then_some(local.type_reference)
                });
            let declared_type = declarations.next()?;
            declarations.next().is_none().then_some(declared_type)
        }
        _ => None,
    }
}

fn correspondence_data_type(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    machine_symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => correspondence_data_type(program, *referee, machine_symbol),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data =>
        {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol && definition.name == *name)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == machine_symbol && name.as_str() == "Self" =>
        {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            machine.attached_data_symbol.is_valid().then_some(())?;
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == machine.attached_data_symbol)
        }
        _ => None,
    }
}

fn exact_statement_owner(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    machine_symbol.is_valid()
        && state_symbol.is_valid()
        && program.symbols.get(machine_symbol).kind == symbols::SymbolKind::Machine
        && program.symbols.get(state_symbol).kind == symbols::SymbolKind::State
        && program.symbols.get(state_symbol).parent == machine_symbol
}

fn contextual_expression_place(
    program: &typed_trees::TypedTrees,
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

#[cfg(test)]
mod tests;

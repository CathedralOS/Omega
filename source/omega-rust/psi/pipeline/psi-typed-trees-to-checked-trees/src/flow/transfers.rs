use super::*;
use psi_facts::{PlaceHandle, QualificationCorrespondence, QualificationPayloadIdentity};

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
            .refs
            .span_or_empty(context.facts)
            .iter()
            .filter_map(|reference| {
                let fact = *semantic.facts.get(reference.fact);
                match fact.payload {
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
            if let Some((source_fact, source_fact_place)) = source {
                if let Some(source_occurrence_place) = source_place {
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

#[allow(clippy::too_many_arguments)]
fn retain_qualification_correspondence(
    program: &psi_typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    source_fact: psi_facts::FactHandle,
    destination_fact: psi_facts::FactHandle,
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
        ..
    } = formation
    else {
        return;
    };
    if evidence.origin != psi_language_semantics::QualificationEvidenceOrigin::CheckedTransformation
        || !exact_evidence_source(program, evidence)
        || !exact_statement_owner(program, machine_symbol, state_symbol)
        || !exact_structural_symbol_place(
            program,
            semantic,
            source_place,
            machine_symbol,
            state_symbol,
        )
        || !exact_structural_symbol_place(
            program,
            semantic,
            source_occurrence_place,
            machine_symbol,
            state_symbol,
        )
        || !exact_structural_symbol_place(
            program,
            semantic,
            destination_place,
            machine_symbol,
            state_symbol,
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
    program: &psi_typed_trees::TypedTrees,
    payload: QualificationPayloadIdentity,
) -> bool {
    match payload {
        QualificationPayloadIdentity::DomainMembership {
            domain,
            domain_symbol,
        } => {
            domain_symbol.is_valid()
                && program.symbols.get(domain_symbol).kind == psi_symbols::SymbolKind::Domain
                && program.domain_path_members.span(domain).is_some()
        }
        QualificationPayloadIdentity::CarryPermission { .. }
        | QualificationPayloadIdentity::CarryOrigin => true,
    }
}

fn exact_evidence_source(
    program: &psi_typed_trees::TypedTrees,
    evidence: QualificationEvidence,
) -> bool {
    evidence.source_symbol.is_valid()
        && evidence.requirement_symbol == SymbolHandle::invalid()
        && evidence.receipt_identity == 0
        && matches!(
            program.symbols.get(evidence.source_symbol).kind,
            psi_symbols::SymbolKind::Machine | psi_symbols::SymbolKind::Operator
        )
}

fn exact_structural_symbol_place(
    program: &psi_typed_trees::TypedTrees,
    semantic: &FactPlan,
    handle: PlaceHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    if !semantic.places.is_valid(handle) {
        return false;
    }
    let place = semantic.places.get(handle);
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    if !root.is_valid()
        || program.symbols.get(root).kind != psi_symbols::SymbolKind::Parameter
        || !matches!(
            program.symbols.get(root).parent,
            parent if parent == machine_symbol || parent == state_symbol
        )
    {
        return false;
    }
    let Some(segments) = semantic.place_segments.span(place.segments) else {
        return false;
    };
    segments.iter().all(|segment| match segment {
        psi_facts::PlaceSegment::Field { symbol } => {
            symbol.is_valid() && program.symbols.get(*symbol).kind == psi_symbols::SymbolKind::Field
        }
        psi_facts::PlaceSegment::Case { variant } => {
            variant.is_valid()
                && program.symbols.get(*variant).kind == psi_symbols::SymbolKind::Variant
        }
        psi_facts::PlaceSegment::FixedIndex { .. }
        | psi_facts::PlaceSegment::FixedRange { .. }
        | psi_facts::PlaceSegment::Index { .. } => false,
    })
}

fn exact_statement_owner(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> bool {
    machine_symbol.is_valid()
        && state_symbol.is_valid()
        && program.symbols.get(machine_symbol).kind == psi_symbols::SymbolKind::Machine
        && program.symbols.get(state_symbol).kind == psi_symbols::SymbolKind::State
        && program.symbols.get(state_symbol).parent == machine_symbol
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

#[cfg(test)]
mod tests {
    use super::*;
    use psi_arena::HandleSpan;
    use psi_facts::{Fact, FactOrigin, FactPlace, PlaceSegment};
    use psi_symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    struct CorrespondenceFixture {
        program: psi_typed_trees::TypedTrees,
        semantic: FactPlan,
        source_fact: psi_facts::FactHandle,
        destination_fact: psi_facts::FactHandle,
        source_place: PlaceHandle,
        source_occurrence_place: PlaceHandle,
        destination_place: PlaceHandle,
        formation: ProgramPoint,
        payload: FactPayload,
        evidence: QualificationEvidence,
        local: SymbolHandle,
        foreign_parameter: SymbolHandle,
        sibling_state_parameter: SymbolHandle,
    }

    fn correspondence_fixture() -> CorrespondenceFixture {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("transform")),
                (SymbolKind::Domain, SymbolNameRef::Static("Ready")),
                (SymbolKind::Field, SymbolNameRef::Static("source")),
                (SymbolKind::Field, SymbolNameRef::Static("destination")),
                (SymbolKind::Local, SymbolNameRef::Static("excluded_local")),
                (
                    SymbolKind::Machine,
                    SymbolNameRef::Static("foreign_transform"),
                ),
            ],
        );
        let declarations = SymbolTableBuilder::child_handles(declarations).collect::<Vec<_>>();
        let machine = declarations[0];
        let domain = declarations[1];
        let source_field = declarations[2];
        let destination_field = declarations[3];
        let local = declarations[4];
        let foreign_machine = declarations[5];
        let machine_members = symbols.insert_children(
            machine,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::Parameter, SymbolNameRef::Static("self")),
                (SymbolKind::State, SymbolNameRef::Static("sibling")),
            ],
        );
        let machine_members =
            SymbolTableBuilder::child_handles(machine_members).collect::<Vec<_>>();
        let state = machine_members[0];
        let parameter = machine_members[1];
        let sibling_state = machine_members[2];
        let sibling_state_parameter = SymbolTableBuilder::child_handles(symbols.insert_children(
            sibling_state,
            [(SymbolKind::Parameter, SymbolNameRef::Static("sibling_self"))],
        ))
        .next()
        .expect("sibling state parameter");
        let foreign_parameter = SymbolTableBuilder::child_handles(symbols.insert_children(
            foreign_machine,
            [(SymbolKind::Parameter, SymbolNameRef::Static("foreign_self"))],
        ))
        .next()
        .expect("foreign machine parameter");
        let program = psi_typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..psi_typed_trees::TypedTrees::default()
        };

        let mut semantic = FactPlan::default();
        let source_place = semantic.append_symbol_place(parameter);
        semantic.push_place_segment(
            source_place,
            PlaceSegment::Field {
                symbol: source_field,
            },
        );
        let destination_place = semantic.append_symbol_place(parameter);
        semantic.push_place_segment(
            destination_place,
            PlaceSegment::Field {
                symbol: destination_field,
            },
        );
        let payload = FactPayload::DomainMembership {
            value: ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: domain,
        };
        let evidence = QualificationEvidence::from_origin(
            psi_language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
            machine,
        );
        let source_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(source_place),
            point: ProgramPoint::CallEnsures {
                machine_symbol: machine,
                state_symbol: state,
                statement_index: 0,
                call_ordinal: 0,
            },
            origin: FactOrigin::CallEnsures,
            evidence,
            payload,
        });
        let formation = ProgramPoint::Statement {
            machine_symbol: machine,
            state_symbol: state,
            statement_index: 1,
        };
        let destination_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(destination_place),
            point: formation,
            origin: FactOrigin::StatementTransfer,
            evidence,
            payload,
        });
        CorrespondenceFixture {
            program,
            semantic,
            source_fact,
            destination_fact,
            source_place,
            source_occurrence_place: source_place,
            destination_place,
            formation,
            payload,
            evidence,
            local,
            foreign_parameter,
            sibling_state_parameter,
        }
    }

    fn retain(fixture: &mut CorrespondenceFixture) {
        retain_qualification_correspondence(
            &fixture.program,
            &mut fixture.semantic,
            fixture.source_fact,
            fixture.destination_fact,
            fixture.source_place,
            fixture.source_occurrence_place,
            fixture.destination_place,
            fixture.formation,
            fixture.payload,
            fixture.evidence,
        );
    }

    #[test]
    fn exact_parameter_field_correspondence_is_retained_once() {
        let mut fixture = correspondence_fixture();
        retain(&mut fixture);
        retain(&mut fixture);
        assert_eq!(fixture.semantic.qualification_correspondences.len(), 1);
        let (_, retained) = fixture
            .semantic
            .qualification_correspondences
            .iter()
            .next()
            .expect("exact retained correspondence");
        assert_eq!(retained.source_fact, fixture.source_fact);
        assert_eq!(retained.destination_fact, fixture.destination_fact);
        assert_eq!(retained.source_place, fixture.source_place);
        assert_eq!(
            retained.source_occurrence_place,
            fixture.source_occurrence_place
        );
        assert_eq!(retained.destination_place, fixture.destination_place);
        assert_eq!(retained.formation, fixture.formation);
        assert_eq!(retained.evidence, fixture.evidence);
    }

    #[test]
    fn local_or_indexed_correspondence_is_not_retained() {
        let mut local = correspondence_fixture();
        local.semantic.places.get_mut(local.source_place).root =
            psi_facts::PlaceRoot::Symbol(local.local);
        retain(&mut local);
        assert!(local.semantic.qualification_correspondences.is_empty());

        let mut indexed = correspondence_fixture();
        let root = indexed.semantic.places.get(indexed.source_place).root;
        let indexed_place = indexed.semantic.append_place(psi_facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        indexed
            .semantic
            .push_place_segment(indexed_place, PlaceSegment::FixedIndex { index: 0 });
        indexed.source_place = indexed_place;
        indexed.semantic.facts.get_mut(indexed.source_fact).place = FactPlace::Place(indexed_place);
        retain(&mut indexed);
        assert!(indexed.semantic.qualification_correspondences.is_empty());

        let mut mismatched_occurrence = correspondence_fixture();
        mismatched_occurrence.source_occurrence_place = mismatched_occurrence.destination_place;
        assert!(!mismatched_occurrence.semantic.places_equal(
            mismatched_occurrence.source_place,
            mismatched_occurrence.source_occurrence_place
        ));
        retain(&mut mismatched_occurrence);
        assert!(
            mismatched_occurrence
                .semantic
                .qualification_correspondences
                .is_empty()
        );
    }

    #[test]
    fn foreign_machine_or_sibling_state_parameter_correspondence_is_not_retained() {
        for sibling_state in [false, true] {
            for endpoint in 0..3 {
                let mut fixture = correspondence_fixture();
                let foreign_root = if sibling_state {
                    fixture.sibling_state_parameter
                } else {
                    fixture.foreign_parameter
                };
                match endpoint {
                    0 => {
                        fixture.semantic.places.get_mut(fixture.source_place).root =
                            psi_facts::PlaceRoot::Symbol(foreign_root);
                    }
                    1 => {
                        let occurrence_place = fixture.semantic.append_symbol_place(foreign_root);
                        fixture.source_occurrence_place = occurrence_place;
                    }
                    2 => {
                        fixture
                            .semantic
                            .places
                            .get_mut(fixture.destination_place)
                            .root = psi_facts::PlaceRoot::Symbol(foreign_root);
                    }
                    _ => unreachable!(),
                }
                retain(&mut fixture);
                assert!(fixture.semantic.qualification_correspondences.is_empty());
            }
        }
    }
}

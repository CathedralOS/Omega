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
        statement_index,
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
    formation_statement_index: usize,
) -> bool {
    if !semantic.places.is_valid(handle) {
        return false;
    }
    let place = semantic.places.get(handle);
    let psi_facts::PlaceRoot::Symbol(root) = place.root else {
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
            psi_facts::PlaceSegment::Field { symbol } => {
                if !symbol.is_valid()
                    || program.symbols.get(*symbol).kind != psi_symbols::SymbolKind::Field
                {
                    return false;
                }
                let Some(data) = correspondence_data_type(program, current, machine_symbol) else {
                    return false;
                };
                let field = if let Some(variant_symbol) = selected_variant.take() {
                    program.data_members(data).iter().find_map(|member| {
                        let psi_typed_trees::data::DataMember::Variant(variant) = member else {
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
                        let psi_typed_trees::data::DataMember::Field(field) = member else {
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
            psi_facts::PlaceSegment::Case { variant } => {
                if selected_variant.is_some()
                    || !variant.is_valid()
                    || program.symbols.get(*variant).kind != psi_symbols::SymbolKind::Variant
                {
                    return false;
                }
                let Some(data) = correspondence_data_type(program, current, machine_symbol) else {
                    return false;
                };
                if !program.data_members(data).iter().any(|member| {
                    matches!(member, psi_typed_trees::data::DataMember::Variant(candidate)
                        if candidate.symbol == *variant)
                }) {
                    return false;
                }
                selected_variant = Some(*variant);
            }
            psi_facts::PlaceSegment::FixedIndex { index } => {
                if selected_variant.is_some() {
                    return false;
                }
                loop {
                    match program.type_reference_table.type_reference(current) {
                        psi_typed_trees::types::TypeReferenceNode::Reference {
                            referee, ..
                        }
                        | psi_typed_trees::types::TypeReferenceNode::Constrained {
                            base_type: referee,
                            ..
                        } => current = *referee,
                        psi_typed_trees::types::TypeReferenceNode::FixedArray {
                            element_type,
                            length: psi_typed_trees::types::FixedArrayLength::Literal(length),
                        } if *index < *length => {
                            current = *element_type;
                            break;
                        }
                        _ => return false,
                    }
                }
            }
            psi_facts::PlaceSegment::FixedRange { .. } | psi_facts::PlaceSegment::Index { .. } => {
                return false;
            }
        }
    }
    true
}

fn correspondence_root_type_reference(
    program: &psi_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
    root: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    match program.symbols.get(root).kind {
        psi_symbols::SymbolKind::Parameter
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
        psi_symbols::SymbolKind::Local
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
                    let psi_typed_trees::statement::StatementNode::LocalData(local) = statement
                    else {
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
    program: &psi_typed_trees::TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
    machine_symbol: SymbolHandle,
) -> Option<&psi_typed_trees::data::DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | psi_typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => correspondence_data_type(program, *referee, machine_symbol),
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == psi_symbols::SymbolKind::Data =>
        {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol && definition.name == *name)
        }
        psi_typed_trees::types::TypeReferenceNode::Named { symbol, name }
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
        exact_local: SymbolHandle,
        data_symbol: SymbolHandle,
        wrong_data_symbol: SymbolHandle,
        foreign_parameter: SymbolHandle,
        sibling_state_parameter: SymbolHandle,
        foreign_local: SymbolHandle,
        sibling_state_local: SymbolHandle,
    }

    fn correspondence_fixture() -> CorrespondenceFixture {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("transform")),
                (SymbolKind::Domain, SymbolNameRef::Static("Ready")),
                (SymbolKind::Data, SymbolNameRef::Static("Pair")),
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
        let data_symbol = declarations[2];
        let local = declarations[3];
        let foreign_machine = declarations[4];
        let fields = SymbolTableBuilder::child_handles(symbols.insert_children(
            data_symbol,
            [
                (SymbolKind::Field, SymbolNameRef::Static("source")),
                (SymbolKind::Field, SymbolNameRef::Static("destination")),
            ],
        ))
        .collect::<Vec<_>>();
        let source_field = fields[0];
        let destination_field = fields[1];
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
        let sibling_members = SymbolTableBuilder::child_handles(symbols.insert_children(
            sibling_state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("sibling_self")),
                (SymbolKind::Local, SymbolNameRef::Static("sibling_local")),
            ],
        ))
        .collect::<Vec<_>>();
        let sibling_state_parameter = sibling_members[0];
        let sibling_state_local = sibling_members[1];
        let foreign_members = SymbolTableBuilder::child_handles(symbols.insert_children(
            foreign_machine,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("foreign_self")),
                (SymbolKind::State, SymbolNameRef::Static("foreign_entry")),
            ],
        ))
        .collect::<Vec<_>>();
        let foreign_parameter = foreign_members[0];
        let foreign_state = foreign_members[1];
        let exact_local = SymbolTableBuilder::child_handles(
            symbols.insert_children(state, [(SymbolKind::Local, SymbolNameRef::Static("local"))]),
        )
        .next()
        .expect("exact state local");
        let foreign_local = SymbolTableBuilder::child_handles(symbols.insert_children(
            foreign_state,
            [(SymbolKind::Local, SymbolNameRef::Static("foreign_local"))],
        ))
        .next()
        .expect("foreign state local");
        let mut program = psi_typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..psi_typed_trees::TypedTrees::default()
        };
        let unit = program
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Unit);
        let pair_type =
            program
                .type_reference_table
                .insert(psi_typed_trees::types::TypeReferenceNode::Named {
                    symbol: data_symbol,
                    name: psi_typed_trees::name::Identifier::generated("Pair"),
                });
        let mut data = psi_typed_trees::data::DataDefinition {
            symbol: data_symbol,
            name: psi_typed_trees::name::Identifier::generated("Pair"),
            ..Default::default()
        };
        for (symbol, name) in [(source_field, "source"), (destination_field, "destination")] {
            program.push_data_member(
                &mut data,
                psi_typed_trees::data::DataMember::Field(psi_typed_trees::data::DataField {
                    symbol,
                    name: psi_typed_trees::name::Identifier::generated(name),
                    type_reference: unit,
                    ..Default::default()
                }),
            );
        }
        program.push_data_definition(data);
        let mut state_node = psi_typed_trees::state::State {
            symbol: state,
            name: psi_typed_trees::name::Identifier::generated("entry"),
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state_node,
            psi_typed_trees::signature::StateParameter {
                symbol: parameter,
                name: psi_typed_trees::name::Identifier::generated("self"),
                type_reference: pair_type,
                is_self: true,
                ..Default::default()
            },
        );
        program.statement_table.push_statement(
            &mut state_node.statement_nodes,
            psi_typed_trees::statement::StatementNode::LocalData(
                psi_typed_trees::statement::TableLocalData {
                    symbol: exact_local,
                    name: psi_typed_trees::name::Identifier::generated("local"),
                    type_reference: pair_type,
                    initial_value: ExpressionHandle::invalid(),
                    is_mutable: true,
                },
            ),
        );
        program.statement_table.push_statement(
            &mut state_node.statement_nodes,
            psi_typed_trees::statement::StatementNode::Expression(ExpressionHandle::invalid()),
        );
        program.statement_table.push_statement(
            &mut state_node.statement_nodes,
            psi_typed_trees::statement::StatementNode::Expression(ExpressionHandle::invalid()),
        );
        let mut machine_node = psi_typed_trees::machine::Machine {
            symbol: machine,
            name: psi_typed_trees::name::Identifier::generated("transform"),
            ..Default::default()
        };
        program.push_machine_state(&mut machine_node, state_node);
        program.push_machine(machine_node);

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
            exact_local,
            data_symbol,
            wrong_data_symbol: foreign_machine,
            foreign_parameter,
            sibling_state_parameter,
            foreign_local,
            sibling_state_local,
        }
    }

    fn set_formation_parameter_type(
        fixture: &mut CorrespondenceFixture,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) {
        let ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        } = fixture.formation
        else {
            unreachable!("correspondence fixture formation")
        };
        let machine = fixture
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .expect("formation machine");
        let state = fixture
            .program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .expect("formation state");
        let parameter = state.parameters.start();
        fixture
            .program
            .state_parameters
            .get_mut(parameter)
            .type_reference = type_reference;
    }

    fn install_correspondence_paths(
        fixture: &mut CorrespondenceFixture,
        source: &[PlaceSegment],
        destination: &[PlaceSegment],
    ) {
        let root = fixture.semantic.places.get(fixture.source_place).root;
        let source_place = fixture.semantic.append_place(psi_facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        for segment in source {
            fixture.semantic.push_place_segment(source_place, *segment);
        }
        let source_occurrence_place = fixture.semantic.append_place(psi_facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        for segment in source {
            fixture
                .semantic
                .push_place_segment(source_occurrence_place, *segment);
        }
        let destination_place = fixture.semantic.append_place(psi_facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        for segment in destination {
            fixture
                .semantic
                .push_place_segment(destination_place, *segment);
        }
        fixture.source_place = source_place;
        fixture.source_occurrence_place = source_occurrence_place;
        fixture.destination_place = destination_place;
        fixture.semantic.facts.get_mut(fixture.source_fact).place = FactPlace::Place(source_place);
        fixture
            .semantic
            .facts
            .get_mut(fixture.destination_fact)
            .place = FactPlace::Place(destination_place);
    }

    fn set_correspondence_roots(
        fixture: &mut CorrespondenceFixture,
        source_root: SymbolHandle,
        destination_root: SymbolHandle,
    ) {
        fixture.semantic.places.get_mut(fixture.source_place).root =
            psi_facts::PlaceRoot::Symbol(source_root);
        fixture
            .semantic
            .places
            .get_mut(fixture.source_occurrence_place)
            .root = psi_facts::PlaceRoot::Symbol(source_root);
        fixture
            .semantic
            .places
            .get_mut(fixture.destination_place)
            .root = psi_facts::PlaceRoot::Symbol(destination_root);
    }

    fn set_pair_field_type(
        fixture: &mut CorrespondenceFixture,
        type_reference: psi_typed_trees::types::TypeReferenceHandle,
    ) {
        let members = fixture
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == fixture.data_symbol)
            .map(|data| data.members)
            .expect("Pair members");
        for member in fixture.program.data_members.span_mut_or_empty(members) {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                unreachable!("Pair record field")
            };
            field.type_reference = type_reference;
        }
    }

    fn nested_fixed_array_fixture() -> CorrespondenceFixture {
        let mut fixture = correspondence_fixture();
        let source_field = fixture
            .semantic
            .place_segments
            .span(fixture.semantic.places.get(fixture.source_place).segments)
            .and_then(|segments| segments.first())
            .copied()
            .expect("source field");
        let destination_field = fixture
            .semantic
            .place_segments
            .span(
                fixture
                    .semantic
                    .places
                    .get(fixture.destination_place)
                    .segments,
            )
            .and_then(|segments| segments.first())
            .copied()
            .expect("destination field");
        let unit = fixture
            .program
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Unit);
        let inner = fixture.program.type_reference_table.insert(
            psi_typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: unit,
                length: psi_typed_trees::types::FixedArrayLength::Literal(2),
            },
        );
        let outer = fixture.program.type_reference_table.insert(
            psi_typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: inner,
                length: psi_typed_trees::types::FixedArrayLength::Literal(2),
            },
        );
        set_pair_field_type(&mut fixture, outer);
        install_correspondence_paths(
            &mut fixture,
            &[
                source_field,
                PlaceSegment::FixedIndex { index: 0 },
                PlaceSegment::FixedIndex { index: 1 },
            ],
            &[
                destination_field,
                PlaceSegment::FixedIndex { index: 1 },
                PlaceSegment::FixedIndex { index: 0 },
            ],
        );
        fixture
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
    fn exact_prior_state_local_is_retained_as_either_endpoint() {
        let mut local_source = correspondence_fixture();
        let local = local_source.exact_local;
        let parameter = match local_source
            .semantic
            .places
            .get(local_source.destination_place)
            .root
        {
            psi_facts::PlaceRoot::Symbol(root) => root,
            _ => unreachable!("symbol root"),
        };
        set_correspondence_roots(&mut local_source, local, parameter);
        retain(&mut local_source);
        assert_eq!(local_source.semantic.qualification_correspondences.len(), 1);

        let mut local_destination = correspondence_fixture();
        let local = local_destination.exact_local;
        let parameter = match local_destination
            .semantic
            .places
            .get(local_destination.source_place)
            .root
        {
            psi_facts::PlaceRoot::Symbol(root) => root,
            _ => unreachable!("symbol root"),
        };
        set_correspondence_roots(&mut local_destination, parameter, local);
        retain(&mut local_destination);
        assert_eq!(
            local_destination
                .semantic
                .qualification_correspondences
                .len(),
            1
        );
    }

    #[test]
    fn state_local_at_or_after_formation_is_not_retained() {
        let mut fixture = correspondence_fixture();
        let local = fixture.exact_local;
        let parameter = match fixture.semantic.places.get(fixture.destination_place).root {
            psi_facts::PlaceRoot::Symbol(root) => root,
            _ => unreachable!("symbol root"),
        };
        set_correspondence_roots(&mut fixture, local, parameter);
        fixture.formation = ProgramPoint::Statement {
            machine_symbol: match fixture.formation {
                ProgramPoint::Statement { machine_symbol, .. } => machine_symbol,
                _ => unreachable!("statement formation"),
            },
            state_symbol: match fixture.formation {
                ProgramPoint::Statement { state_symbol, .. } => state_symbol,
                _ => unreachable!("statement formation"),
            },
            statement_index: 0,
        };
        fixture
            .semantic
            .facts
            .get_mut(fixture.destination_fact)
            .point = fixture.formation;
        retain(&mut fixture);
        assert!(fixture.semantic.qualification_correspondences.is_empty());
    }

    #[test]
    fn nested_literal_fixed_indexes_retain_exact_correspondence() {
        let mut fixture = nested_fixed_array_fixture();
        retain(&mut fixture);
        let retained = fixture
            .semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(_, correspondence)| correspondence)
            .expect("nested in-bounds correspondence");
        assert_eq!(retained.source_place, fixture.source_place);
        assert_eq!(
            retained.source_occurrence_place,
            fixture.source_occurrence_place
        );
        assert_eq!(retained.destination_place, fixture.destination_place);
        assert_eq!(retained.formation, fixture.formation);
        assert_eq!(
            retained.payload,
            QualificationPayloadIdentity::from_fact_payload(fixture.payload).expect("payload")
        );
        assert_eq!(retained.evidence, fixture.evidence);
    }

    #[test]
    fn nonliteral_out_of_bounds_runtime_and_wrong_type_indexes_are_not_retained() {
        let mut out_of_bounds = nested_fixed_array_fixture();
        let source_field = out_of_bounds
            .semantic
            .place_segments
            .span(
                out_of_bounds
                    .semantic
                    .places
                    .get(out_of_bounds.source_place)
                    .segments,
            )
            .and_then(|segments| segments.first())
            .copied()
            .expect("source field");
        let destination_field = out_of_bounds
            .semantic
            .place_segments
            .span(
                out_of_bounds
                    .semantic
                    .places
                    .get(out_of_bounds.destination_place)
                    .segments,
            )
            .and_then(|segments| segments.first())
            .copied()
            .expect("destination field");
        install_correspondence_paths(
            &mut out_of_bounds,
            &[source_field, PlaceSegment::FixedIndex { index: 2 }],
            &[destination_field, PlaceSegment::FixedIndex { index: 0 }],
        );
        retain(&mut out_of_bounds);
        assert!(
            out_of_bounds
                .semantic
                .qualification_correspondences
                .is_empty()
        );

        let mut runtime = nested_fixed_array_fixture();
        install_correspondence_paths(
            &mut runtime,
            &[
                source_field,
                PlaceSegment::Index {
                    expression: ExpressionHandle::invalid(),
                },
            ],
            &[destination_field, PlaceSegment::FixedIndex { index: 0 }],
        );
        retain(&mut runtime);
        assert!(runtime.semantic.qualification_correspondences.is_empty());

        let mut range = nested_fixed_array_fixture();
        install_correspondence_paths(
            &mut range,
            &[source_field, PlaceSegment::FixedRange { start: 0, end: 1 }],
            &[destination_field, PlaceSegment::FixedIndex { index: 0 }],
        );
        retain(&mut range);
        assert!(range.semantic.qualification_correspondences.is_empty());

        let mut nonliteral = nested_fixed_array_fixture();
        let unit = nonliteral
            .program
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Unit);
        let array = nonliteral.program.type_reference_table.insert(
            psi_typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: unit,
                length: psi_typed_trees::types::FixedArrayLength::ConstParameter {
                    symbol: nonliteral.local,
                    name: psi_typed_trees::name::Identifier::generated("N"),
                },
            },
        );
        set_pair_field_type(&mut nonliteral, array);
        retain(&mut nonliteral);
        assert!(nonliteral.semantic.qualification_correspondences.is_empty());

        let mut wrong_type = nested_fixed_array_fixture();
        let unit = wrong_type
            .program
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Unit);
        set_pair_field_type(&mut wrong_type, unit);
        retain(&mut wrong_type);
        assert!(wrong_type.semantic.qualification_correspondences.is_empty());
    }

    #[test]
    fn generic_or_label_only_data_traversal_is_not_retained() {
        let mut generic = correspondence_fixture();
        let arguments = generic
            .program
            .type_reference_table
            .insert_type_reference_handles([]);
        let generic_type = generic.program.type_reference_table.insert(
            psi_typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: generic.data_symbol,
                base_name: psi_typed_trees::name::Identifier::generated("Pair"),
                lifetime_arguments: Vec::new(),
                arguments,
            },
        );
        set_formation_parameter_type(&mut generic, generic_type);
        retain(&mut generic);
        assert!(generic.semantic.qualification_correspondences.is_empty());

        let mut label_only = correspondence_fixture();
        let wrong = label_only.program.type_reference_table.insert(
            psi_typed_trees::types::TypeReferenceNode::Named {
                symbol: label_only.wrong_data_symbol,
                name: psi_typed_trees::name::Identifier::generated("Pair"),
            },
        );
        set_formation_parameter_type(&mut label_only, wrong);
        retain(&mut label_only);
        assert!(label_only.semantic.qualification_correspondences.is_empty());
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

    #[test]
    fn foreign_machine_or_sibling_state_local_correspondence_is_not_retained() {
        for sibling_state in [false, true] {
            for endpoint in 0..2 {
                let mut fixture = correspondence_fixture();
                let excluded_root = if sibling_state {
                    fixture.sibling_state_local
                } else {
                    fixture.foreign_local
                };
                let parameter = match fixture.semantic.places.get(fixture.destination_place).root {
                    psi_facts::PlaceRoot::Symbol(root) => root,
                    _ => unreachable!("symbol root"),
                };
                if endpoint == 0 {
                    set_correspondence_roots(&mut fixture, excluded_root, parameter);
                } else {
                    set_correspondence_roots(&mut fixture, parameter, excluded_root);
                }
                retain(&mut fixture);
                assert!(fixture.semantic.qualification_correspondences.is_empty());
            }
        }
    }
}

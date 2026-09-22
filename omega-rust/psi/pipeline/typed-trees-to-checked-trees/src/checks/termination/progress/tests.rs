//! Checked progress tests.

use super::SymbolHandle;
use crate::checks::termination::progress::qualification_correspondences::{
    replay_root_type_reference, validate_qualification_correspondences,
};
use arena::HandleSpan;
use facts::{
    Fact, FactOrigin, QualificationCorrespondence, QualificationEvidence,
    QualificationPayloadIdentity,
};
use facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn correspondence_fixture() -> (
    typed_trees::TypedTrees,
    facts::FactPlan,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
    SymbolHandle,
) {
    let mut symbols = SymbolTableBuilder::new();
    let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let roots = symbols.insert_children(
        root,
        [
            (SymbolKind::Machine, SymbolNameRef::Static("worker")),
            (SymbolKind::Domain, SymbolNameRef::Static("Ready")),
            (SymbolKind::Data, SymbolNameRef::Static("Pair")),
            (SymbolKind::Local, SymbolNameRef::Static("excluded_local")),
            (
                SymbolKind::TypeParameter,
                SymbolNameRef::Static("ExcludedType"),
            ),
            (SymbolKind::Machine, SymbolNameRef::Static("foreign_worker")),
        ],
    );
    let roots = SymbolTableBuilder::child_handles(roots).collect::<Vec<_>>();
    let machine = roots[0];
    let domain = roots[1];
    let data_symbol = roots[2];
    let excluded_local = roots[3];
    let excluded_generic = roots[4];
    let foreign_machine = roots[5];
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
    let machine_members = SymbolTableBuilder::child_handles(machine_members).collect::<Vec<_>>();
    let state = machine_members[0];
    let self_parameter = machine_members[1];
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
    let mut program = typed_trees::TypedTrees {
        symbols: symbols.finish(),
        ..typed_trees::TypedTrees::default()
    };
    let unit = program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let pair_type =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: data_symbol,
                name: typed_trees::name::Identifier::generated("Pair"),
            });
    let mut data = typed_trees::data::DataDefinition {
        symbol: data_symbol,
        name: typed_trees::name::Identifier::generated("Pair"),
        ..Default::default()
    };
    for (symbol, name) in [(source_field, "source"), (destination_field, "destination")] {
        program.push_data_member(
            &mut data,
            typed_trees::data::DataMember::Field(typed_trees::data::DataField {
                symbol,
                name: typed_trees::name::Identifier::generated(name),
                type_reference: unit,
                ..Default::default()
            }),
        );
    }
    program.push_data_definition(data);
    let mut state_node = typed_trees::state::State {
        symbol: state,
        name: typed_trees::name::Identifier::generated("entry"),
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state_node,
        typed_trees::signature::StateParameter {
            symbol: self_parameter,
            name: typed_trees::name::Identifier::generated("self"),
            type_reference: pair_type,
            is_self: true,
            ..Default::default()
        },
    );
    program.statement_table.push_statement(
        &mut state_node.statement_nodes,
        typed_trees::statement::StatementNode::LocalData(typed_trees::statement::TableLocalData {
            symbol: exact_local,
            name: typed_trees::name::Identifier::generated("local"),
            type_reference: pair_type,
            initial_value: typed_trees::expression::ExpressionHandle::invalid(),
            is_mutable: true,
            type_is_inferred: false,
            relevance: language_core::BindingRelevance::Relevant,
        }),
    );
    program.statement_table.push_statement(
        &mut state_node.statement_nodes,
        typed_trees::statement::StatementNode::Expression(
            typed_trees::expression::ExpressionHandle::invalid(),
        ),
    );
    program.statement_table.push_statement(
        &mut state_node.statement_nodes,
        typed_trees::statement::StatementNode::Expression(
            typed_trees::expression::ExpressionHandle::invalid(),
        ),
    );
    let mut machine_node = typed_trees::machine::Machine {
        symbol: machine,
        name: typed_trees::name::Identifier::generated("worker"),
        ..Default::default()
    };
    program.push_machine_state(&mut machine_node, state_node);
    program.push_machine(machine_node);

    let mut semantic = facts::FactPlan::default();
    let source_place = semantic.append_symbol_place(self_parameter);
    semantic.push_place_segment(
        source_place,
        PlaceSegment::Field {
            symbol: source_field,
        },
    );
    let destination_place = semantic.append_symbol_place(self_parameter);
    semantic.push_place_segment(
        destination_place,
        PlaceSegment::Field {
            symbol: destination_field,
        },
    );
    let formation = facts::ProgramPoint::Statement {
        machine_symbol: machine,
        state_symbol: state,
        statement_index: 1,
    };
    let payload = FactPayload::DomainMembership {
        value: typed_trees::expression::ExpressionHandle::invalid(),
        domain: HandleSpan::empty(),
        domain_symbol: domain,
        semantic_domain: language_semantics::SemanticDomainId::NULL,
    };
    let evidence = QualificationEvidence::from_origin(
        language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
        machine,
    );
    let source_fact = semantic.append_fact(Fact {
        place: FactPlace::Place(source_place),
        point: facts::ProgramPoint::CallEnsures {
            machine_symbol: machine,
            state_symbol: state,
            statement_index: 0,
            call_ordinal: 0,
        },
        origin: FactOrigin::CallEnsures,
        evidence,
        payload,
    });
    let destination_fact = semantic.append_fact(Fact {
        place: FactPlace::Place(destination_place),
        point: formation,
        origin: FactOrigin::StatementTransfer,
        evidence,
        payload,
    });
    semantic.append_qualification_correspondence(QualificationCorrespondence {
        source_fact,
        destination_fact,
        source_occurrence_place: source_place,
        source_place,
        destination_place,
        formation,
        payload: QualificationPayloadIdentity::DomainMembership {
            domain: HandleSpan::empty(),
            domain_symbol: domain,
            semantic_domain: language_semantics::SemanticDomainId::NULL,
        },
        evidence,
    });
    (
        program,
        semantic,
        excluded_local,
        excluded_generic,
        foreign_parameter,
        sibling_state_parameter,
        exact_local,
        foreign_local,
        sibling_state_local,
    )
}

fn set_replay_parameter_type(
    program: &mut typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    type_reference: typed_trees::types::TypeReferenceHandle,
) {
    let correspondence = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(_, correspondence)| correspondence)
        .expect("correspondence");
    let facts::ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        ..
    } = correspondence.formation
    else {
        unreachable!("correspondence fixture formation")
    };
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("formation machine");
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .expect("formation state");
    let parameter = state.parameters.start();
    program.state_parameters.get_mut(parameter).type_reference = type_reference;
}

fn install_replay_paths(
    semantic: &mut facts::FactPlan,
    source: &[PlaceSegment],
    destination: &[PlaceSegment],
) {
    let row = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let retained = *semantic.qualification_correspondences.get(row);
    let root = semantic.places.get(retained.source_place).root;
    let source_place = semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in source {
        semantic.push_place_segment(source_place, *segment);
    }
    let source_occurrence_place = semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in source {
        semantic.push_place_segment(source_occurrence_place, *segment);
    }
    let destination_place = semantic.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in destination {
        semantic.push_place_segment(destination_place, *segment);
    }
    semantic.facts.get_mut(retained.source_fact).place = FactPlace::Place(source_place);
    semantic.facts.get_mut(retained.destination_fact).place = FactPlace::Place(destination_place);
    let retained = semantic.qualification_correspondences.get_mut(row);
    retained.source_place = source_place;
    retained.source_occurrence_place = source_occurrence_place;
    retained.destination_place = destination_place;
}

fn set_replay_roots(
    semantic: &mut facts::FactPlan,
    source_root: SymbolHandle,
    destination_root: SymbolHandle,
) {
    let row = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let retained = *semantic.qualification_correspondences.get(row);
    semantic.places.get_mut(retained.source_place).root = PlaceRoot::Symbol(source_root);
    semantic
        .places
        .get_mut(retained.source_occurrence_place)
        .root = PlaceRoot::Symbol(source_root);
    semantic.places.get_mut(retained.destination_place).root = PlaceRoot::Symbol(destination_root);
}

fn replay_state_statement_span(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
) -> HandleSpan<typed_trees::statement::StatementNode> {
    let formation = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(_, row)| row.formation)
        .expect("correspondence");
    let facts::ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        ..
    } = formation
    else {
        unreachable!("statement formation")
    };
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .expect("formation machine");
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.statement_nodes)
        .expect("formation state")
}

fn nested_fixed_array_correspondence_fixture() -> (typed_trees::TypedTrees, facts::FactPlan) {
    let (mut program, mut semantic, _, _, _, _, _, _, _) = correspondence_fixture();
    let unit = program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let inner =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: unit,
                length: typed_trees::types::FixedArrayLength::Literal(2),
            });
    let outer =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: inner,
                length: typed_trees::types::FixedArrayLength::Literal(2),
            });
    set_replay_parameter_type(&mut program, &semantic, outer);
    install_replay_paths(
        &mut semantic,
        &[
            PlaceSegment::FixedIndex { index: 0 },
            PlaceSegment::FixedIndex { index: 1 },
        ],
        &[
            PlaceSegment::FixedIndex { index: 1 },
            PlaceSegment::FixedIndex { index: 0 },
        ],
    );
    (program, semantic)
}

#[test]
fn checked_progress_replays_exact_qualification_correspondence() {
    let (program, semantic, _, _, _, _, _, _, _) = correspondence_fixture();
    assert!(validate_qualification_correspondences(&program, &semantic).is_empty());
}

#[test]
fn checked_progress_replays_exact_prior_state_local_as_either_endpoint() {
    let (program, mut local_source, _, _, _, _, exact_local, _, _) = correspondence_fixture();
    let parameter = match local_source
        .places
        .get(
            local_source
                .qualification_correspondences
                .iter()
                .next()
                .map(|(_, row)| row.destination_place)
                .expect("destination place"),
        )
        .root
    {
        PlaceRoot::Symbol(root) => root,
        _ => unreachable!("symbol root"),
    };
    set_replay_roots(&mut local_source, exact_local, parameter);
    assert!(validate_qualification_correspondences(&program, &local_source).is_empty());

    let mut local_destination = local_source.clone();
    set_replay_roots(&mut local_destination, parameter, exact_local);
    assert!(validate_qualification_correspondences(&program, &local_destination).is_empty());
}

#[test]
fn checked_progress_rejects_local_declaration_type_missing_duplicate_and_order_drift() {
    let (program, mut semantic, excluded_local, _, _, _, exact_local, _, _) =
        correspondence_fixture();
    let parameter = match semantic
        .places
        .get(
            semantic
                .qualification_correspondences
                .iter()
                .next()
                .map(|(_, row)| row.destination_place)
                .expect("destination place"),
        )
        .root
    {
        PlaceRoot::Symbol(root) => root,
        _ => unreachable!("symbol root"),
    };
    set_replay_roots(&mut semantic, exact_local, parameter);

    let mut wrong_type = program.clone();
    let unit = wrong_type
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let statements = replay_state_statement_span(&wrong_type, &semantic);
    let typed_trees::statement::StatementNode::LocalData(local) =
        &mut wrong_type.statement_table.statements_mut(statements)[0]
    else {
        unreachable!("local declaration")
    };
    local.type_reference = unit;
    assert!(
        validate_qualification_correspondences(&wrong_type, &semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut missing = program.clone();
    let statements = replay_state_statement_span(&missing, &semantic);
    let typed_trees::statement::StatementNode::LocalData(local) =
        &mut missing.statement_table.statements_mut(statements)[0]
    else {
        unreachable!("local declaration")
    };
    local.symbol = excluded_local;
    assert!(
        validate_qualification_correspondences(&missing, &semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("formation-owned"))
    );

    let mut duplicate = program.clone();
    let statements = replay_state_statement_span(&duplicate, &semantic);
    let duplicate_declaration = duplicate.statement_table.statements(statements)[0].clone();
    duplicate.statement_table.statements_mut(statements)[1] = duplicate_declaration;
    let mut duplicate_semantic = semantic.clone();
    let row = duplicate_semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let formation = match duplicate_semantic
        .qualification_correspondences
        .get(row)
        .formation
    {
        facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        } => facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index: 2,
        },
        _ => unreachable!("statement formation"),
    };
    duplicate_semantic
        .qualification_correspondences
        .get_mut(row)
        .formation = formation;
    let destination_fact = duplicate_semantic
        .qualification_correspondences
        .get(row)
        .destination_fact;
    duplicate_semantic.facts.get_mut(destination_fact).point = formation;
    assert!(
        validate_qualification_correspondences(&duplicate, &duplicate_semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("formation-owned"))
    );

    let mut reordered = semantic;
    let row = reordered
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let formation = match reordered.qualification_correspondences.get(row).formation {
        facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        } => facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index: 0,
        },
        _ => unreachable!("statement formation"),
    };
    reordered
        .qualification_correspondences
        .get_mut(row)
        .formation = formation;
    let destination_fact = reordered
        .qualification_correspondences
        .get(row)
        .destination_fact;
    reordered.facts.get_mut(destination_fact).point = formation;
    assert!(
        validate_qualification_correspondences(&program, &reordered)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("formation-owned"))
    );
}

#[test]
fn checked_progress_replays_nested_in_bounds_literal_fixed_indexes() {
    let (program, semantic) = nested_fixed_array_correspondence_fixture();
    assert!(validate_qualification_correspondences(&program, &semantic).is_empty());
}

#[test]
fn checked_progress_rejects_fixed_index_bounds_runtime_type_and_length_tamper() {
    let (program, semantic) = nested_fixed_array_correspondence_fixture();

    let mut out_of_bounds = semantic.clone();
    install_replay_paths(
        &mut out_of_bounds,
        &[PlaceSegment::FixedIndex { index: 2 }],
        &[PlaceSegment::FixedIndex { index: 0 }],
    );
    assert!(
        validate_qualification_correspondences(&program, &out_of_bounds)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut runtime = semantic.clone();
    install_replay_paths(
        &mut runtime,
        &[PlaceSegment::Index {
            expression: typed_trees::expression::ExpressionHandle::invalid(),
        }],
        &[PlaceSegment::FixedIndex { index: 0 }],
    );
    assert!(
        validate_qualification_correspondences(&program, &runtime)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut range = semantic.clone();
    install_replay_paths(
        &mut range,
        &[PlaceSegment::FixedRange { start: 0, end: 1 }],
        &[PlaceSegment::FixedIndex { index: 0 }],
    );
    assert!(
        validate_qualification_correspondences(&program, &range)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut wrong_type_program = program.clone();
    let unit = wrong_type_program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    set_replay_parameter_type(&mut wrong_type_program, &semantic, unit);
    assert!(
        validate_qualification_correspondences(&wrong_type_program, &semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut nonliteral_program = program;
    let unit = nonliteral_program
        .type_reference_table
        .insert(typed_trees::types::TypeReferenceNode::Unit);
    let formation = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(_, row)| row.formation)
        .expect("correspondence");
    let facts::ProgramPoint::Statement { machine_symbol, .. } = formation else {
        unreachable!("statement formation")
    };
    let nonliteral = nonliteral_program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::FixedArray {
            element_type: unit,
            length: typed_trees::types::FixedArrayLength::ConstParameter {
                symbol: machine_symbol,
                name: typed_trees::name::Identifier::generated("N"),
            },
        },
    );
    set_replay_parameter_type(&mut nonliteral_program, &semantic, nonliteral);
    assert!(
        validate_qualification_correspondences(&nonliteral_program, &semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );
}

#[test]
fn checked_progress_rejects_generic_and_label_only_data_traversal() {
    let (mut generic_program, semantic, _, _, _, _, _, _, _) = correspondence_fixture();
    let row = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(_, row)| row)
        .expect("correspondence");
    let (machine_symbol, state_symbol) = match row.formation {
        facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        } => (machine_symbol, state_symbol),
        _ => unreachable!("statement formation"),
    };
    let parameter_root = match semantic.places.get(row.source_place).root {
        PlaceRoot::Symbol(root) => root,
        _ => unreachable!("symbol root"),
    };
    let data_symbol = match generic_program.type_reference_table.type_reference(
        replay_root_type_reference(
            &generic_program,
            machine_symbol,
            state_symbol,
            1,
            parameter_root,
        )
        .expect("root type"),
    ) {
        typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
        _ => unreachable!("named fixture type"),
    };
    let arguments = generic_program
        .type_reference_table
        .insert_type_reference_handles([]);
    let generic = generic_program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Generic {
            base_symbol: data_symbol,
            base_name: typed_trees::name::Identifier::generated("Pair"),
            lifetime_arguments: Vec::new(),
            arguments,
        },
    );
    set_replay_parameter_type(&mut generic_program, &semantic, generic);
    assert!(
        validate_qualification_correspondences(&generic_program, &semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut label_only_program = generic_program;
    let wrong_symbol = match row.formation {
        facts::ProgramPoint::Statement { machine_symbol, .. } => machine_symbol,
        _ => unreachable!("statement formation"),
    };
    let wrong = label_only_program.type_reference_table.insert(
        typed_trees::types::TypeReferenceNode::Named {
            symbol: wrong_symbol,
            name: typed_trees::name::Identifier::generated("Pair"),
        },
    );
    set_replay_parameter_type(&mut label_only_program, &semantic, wrong);
    assert!(
        validate_qualification_correspondences(&label_only_program, &semantic)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );
}

#[test]
fn checked_progress_rejects_correspondence_payload_place_and_order_drift() {
    let (program, semantic, _, _, _, _, _, _, _) = correspondence_fixture();

    let mut payload = semantic.clone();
    let row = payload
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    payload.qualification_correspondences.get_mut(row).payload =
        QualificationPayloadIdentity::CarryOrigin;
    assert!(
        validate_qualification_correspondences(&program, &payload)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("payload or domain identity"))
    );

    let mut occurrence = semantic.clone();
    let row = occurrence
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let destination_place = occurrence
        .qualification_correspondences
        .get(row)
        .destination_place;
    occurrence
        .qualification_correspondences
        .get_mut(row)
        .source_occurrence_place = destination_place;
    assert!(
        validate_qualification_correspondences(&program, &occurrence)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("source occurrence drifted"))
    );

    let mut indexed = semantic.clone();
    let source_place = indexed
        .qualification_correspondences
        .iter()
        .next()
        .map(|(_, row)| row.source_place)
        .expect("source place");
    let root = indexed.places.get(source_place).root;
    let indexed_place = indexed.append_place(facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    indexed.push_place_segment(indexed_place, PlaceSegment::FixedIndex { index: 0 });
    let row = indexed
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    indexed
        .qualification_correspondences
        .get_mut(row)
        .source_place = indexed_place;
    let source_fact = indexed.qualification_correspondences.get(row).source_fact;
    indexed.facts.get_mut(source_fact).place = FactPlace::Place(indexed_place);
    assert!(
        validate_qualification_correspondences(&program, &indexed)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
    );

    let mut reversed = semantic;
    let row = reversed
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let retained = reversed.qualification_correspondences.get_mut(row);
    std::mem::swap(&mut retained.source_fact, &mut retained.destination_fact);
    assert!(
        validate_qualification_correspondences(&program, &reversed)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("construction order drifted"))
    );
}

#[test]
fn checked_progress_rejects_excluded_roots_and_malformed_formation() {
    let (
        program,
        semantic,
        excluded_local,
        excluded_generic,
        foreign_parameter,
        sibling_state_parameter,
        _,
        foreign_local,
        sibling_state_local,
    ) = correspondence_fixture();
    let row = semantic
        .qualification_correspondences
        .iter()
        .next()
        .map(|(handle, _)| handle)
        .expect("correspondence");
    let source_place = semantic.qualification_correspondences.get(row).source_place;

    for excluded_root in [
        PlaceRoot::Unknown,
        PlaceRoot::Expression(typed_trees::expression::ExpressionHandle::invalid()),
        PlaceRoot::TypeReference(typed_trees::types::TypeReferenceHandle::invalid()),
    ] {
        let mut drifted = semantic.clone();
        drifted.places.get_mut(source_place).root = excluded_root;
        assert!(
            validate_qualification_correspondences(&program, &drifted)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );
    }

    for excluded_symbol in [excluded_local, excluded_generic] {
        let mut drifted = semantic.clone();
        drifted.places.get_mut(source_place).root = PlaceRoot::Symbol(excluded_symbol);
        assert!(
            validate_qualification_correspondences(&program, &drifted)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );
    }

    for excluded_symbol in [foreign_parameter, sibling_state_parameter] {
        for destination in [false, true] {
            let mut drifted = semantic.clone();
            let place = if destination {
                drifted
                    .qualification_correspondences
                    .get(row)
                    .destination_place
            } else {
                source_place
            };
            drifted.places.get_mut(place).root = PlaceRoot::Symbol(excluded_symbol);
            assert!(
                validate_qualification_correspondences(&program, &drifted)
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("formation-owned"))
            );
        }

        let mut occurrence = semantic.clone();
        let occurrence_place = occurrence.append_symbol_place(excluded_symbol);
        occurrence
            .qualification_correspondences
            .get_mut(row)
            .source_occurrence_place = occurrence_place;
        assert!(
            validate_qualification_correspondences(&program, &occurrence)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("formation-owned"))
        );
    }

    for excluded_symbol in [foreign_local, sibling_state_local] {
        for destination in [false, true] {
            let mut drifted = semantic.clone();
            let place = if destination {
                drifted
                    .qualification_correspondences
                    .get(row)
                    .destination_place
            } else {
                source_place
            };
            drifted.places.get_mut(place).root = PlaceRoot::Symbol(excluded_symbol);
            assert!(
                validate_qualification_correspondences(&program, &drifted)
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("formation-owned"))
            );
        }
    }

    let mut formation = semantic;
    formation
        .qualification_correspondences
        .get_mut(row)
        .formation = facts::ProgramPoint::Statement {
        machine_symbol: SymbolHandle::invalid(),
        state_symbol: SymbolHandle::invalid(),
        statement_index: 1,
    };
    assert!(
        validate_qualification_correspondences(&program, &formation)
            .iter()
            .any(|diagnostic| diagnostic.message.contains("owner identity drifted"))
    );
}

/// End-to-end subject reconstruction through the real checked pipeline: a
/// premise demanded inside an inner state must name the exact caller place
/// even when the named-transition argument that binds it is a nested call
/// result rather than a spelled place. The lowerer materializes the
/// `TransitionTargetNode::Named` operand into a temporary, so the shared
/// origin replay walks the temporary's initializer back through the nested
/// callee's returned expression. Bound value-call arguments stay inline, but
/// result-realization validation still fences a machine call nested directly
/// in one, so that argument shape is exercised at the unit level in
/// `origins::tests` instead.
mod nested_call_arguments {
    use checked_trees::CheckedTrees;
    use diagnostics::Diagnostic;
    use language_semantics::TerminationGuarantee;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    /// The progress-profile context every fixture shares: a `WeakFair`
    /// admission granted through the boundary trait, so `requires ... in
    /// WeakFair` publishes an exact subject-bearing premise and a callee's
    /// declared `-> T in WeakFair` return discharges the requires check on a
    /// call-result argument.
    const PROFILE: &str = r#"
        pub data SchedulerHandle [copy] {}
        pub domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        pub boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        pub data Context { scheduler: SchedulerHandle; }
        pub machine consume(value: SchedulerHandle in WeakFair)
        requires value in WeakFair
        terminates;
        -> u64 { 0 }
        "#;

    fn diagnostics(source: &str) -> Vec<Diagnostic> {
        let source = format!("data Main {{}} machine Main::run(&mut self) {{}} {PROFILE} {source}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize nested-argument fixture");
        let syntax = parse_syntax_trees(&tokens).expect("parse nested-argument fixture");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("resolve nested-argument fixture");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type nested-argument fixture");
        match crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()) {
            Ok(_) => Vec::new(),
            Err(diagnostics) => diagnostics,
        }
    }

    fn checked(source: &str) -> CheckedTrees {
        let source = format!("data Main {{}} machine Main::run(&mut self) {{}} {PROFILE} {source}");
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize nested-argument fixture");
        let syntax = parse_syntax_trees(&tokens).expect("parse nested-argument fixture");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("resolve nested-argument fixture");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type nested-argument fixture");
        crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).unwrap_or_else(
            |diagnostics| {
                panic!("nested-argument fixture must reach checked trees: {diagnostics:#?}")
            },
        )
    }

    fn machine<'program>(
        program: &'program CheckedTrees,
        name: &str,
    ) -> &'program checked_trees::checked_trees::machine::Machine {
        program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine {name}"))
    }

    fn summary<'program>(
        program: &'program CheckedTrees,
        name: &str,
    ) -> &'program TerminationGuarantee {
        &program
            .facts
            .termination
            .for_machine(machine(program, name).symbol)
            .unwrap_or_else(|| panic!("checked termination plan for {name}"))
            .checked_summary
    }

    /// Assert `name`'s checked summary carries exactly one premise, rooted at
    /// its `parameter` entry parameter with the exact projection path
    /// `Owner::field[::Owner::field ...]`.
    fn assert_single_premise(
        program: &CheckedTrees,
        name: &str,
        parameter: &str,
        projection_path: &str,
    ) {
        let TerminationGuarantee::Terminates { premises } = summary(program, name) else {
            panic!(
                "{name} must retain checked termination: {:#?}",
                summary(program, name)
            )
        };
        let [premise] = premises.as_slice() else {
            panic!("{name} must carry exactly one premise: {premises:#?}")
        };
        let machine = machine(program, name);
        let entry = &program.machine_states(machine)[0];
        let entry_parameter = program
            .state_parameters(entry)
            .iter()
            .find(|candidate| candidate.name.as_str() == parameter)
            .unwrap_or_else(|| panic!("entry parameter {parameter}"));
        assert_eq!(
            premise.subject.root, entry_parameter.symbol,
            "premise must root at the exact entry parameter: {premise:#?}"
        );
        let projections: Vec<String> = premise
            .subject
            .projections
            .iter()
            .map(|projection| program.symbols.display_path(*projection, "::"))
            .collect();
        assert_eq!(
            projections.join("::"),
            projection_path,
            "premise must keep the exact declared-field path: {premise:#?}"
        );
    }

    #[test]
    fn transition_argument_call_result_derives_the_exact_entry_subject() {
        // `pick` publishes no premise of its own; it returns its domained
        // parameter exactly. Binding `waiting`'s `selected` to
        // `pick(context.scheduler)` makes `consume(selected)` demand
        // `context.scheduler` on the caller — provable only by tracing the
        // nested call result through `pick`'s returned expression, never
        // through a same-shaped root.
        let program = checked(
            r#"
            pub machine pick(handle: SchedulerHandle in WeakFair)
            terminates;
            -> SchedulerHandle in WeakFair { handle }
            pub machine process(context: &Context, ready: bool)
            requires context.scheduler in WeakFair
            terminates;
            -> u64 {
                transition ready {
                    true -> waiting(pick(context.scheduler))
                    false -> 0
                }
                state waiting(selected: SchedulerHandle in WeakFair) -> u64 { consume(selected) }
            }
            "#,
        );
        assert_single_premise(&program, "process", "context", "Context::scheduler");
    }

    #[test]
    fn transition_argument_nested_call_result_derives_the_exact_entry_subject() {
        // The argument is a call whose own argument is a call. The premise
        // still traces through both returned expressions to the caller's
        // exact entry subject, at the arm's authored evaluation point.
        let program = checked(
            r#"
            pub machine pick(handle: SchedulerHandle in WeakFair)
            terminates;
            -> SchedulerHandle in WeakFair { handle }
            pub machine process(context: &Context, ready: bool)
            requires context.scheduler in WeakFair
            terminates;
            -> u64 {
                transition ready {
                    true -> waiting(pick(pick(context.scheduler)))
                    false -> 0
                }
                state waiting(selected: SchedulerHandle in WeakFair) -> u64 { consume(selected) }
            }
            "#,
        );
        assert_single_premise(&program, "process", "context", "Context::scheduler");
    }

    #[test]
    fn may_write_helper_result_derives_the_replacement_input_premise() {
        // `stamp` writes the replacement input into `context` and returns the
        // input itself: the demanded premise is the replacement's exact
        // subject, never the written aggregate's root.
        let program = checked(
            r#"
            pub machine stamp(context: &mut Context, fresh: SchedulerHandle in WeakFair)
            terminates;
            -> SchedulerHandle in WeakFair { context.scheduler = fresh as SchedulerHandle; fresh }
            pub machine process(context: &mut Context, replacement: &Context, ready: bool)
            requires replacement.scheduler in WeakFair
            terminates;
            -> u64 {
                transition ready {
                    true -> waiting(stamp(context, replacement.scheduler))
                    false -> 0
                }
                state waiting(selected: SchedulerHandle in WeakFair) -> u64 { consume(selected) }
            }
            "#,
        );
        assert_single_premise(&program, "process", "replacement", "Context::scheduler");
    }

    #[test]
    fn may_write_helper_returning_the_written_projection_derives_the_replacement_input_premise() {
        // `stamp_read` returns the projection it just stored into: the
        // premise replays that exact store through the callee's own body and
        // lands on the replacement input's subject — the mutated aggregate's
        // root correspondence is not evidence for the field's value.
        let program = checked(
            r#"
            pub data FairContext { slot: SchedulerHandle in WeakFair; }
            pub machine stamp_read(context: &mut FairContext, fresh: SchedulerHandle in WeakFair)
            terminates;
            -> SchedulerHandle in WeakFair { context.slot = fresh; context.slot }
            pub machine process(context: &mut FairContext, replacement: &FairContext, ready: bool)
            requires replacement.slot in WeakFair
            terminates;
            -> u64 {
                transition ready {
                    true -> waiting(stamp_read(context, replacement.slot))
                    false -> 0
                }
                state waiting(selected: SchedulerHandle in WeakFair) -> u64 { consume(selected) }
            }
            "#,
        );
        assert_single_premise(&program, "process", "replacement", "FairContext::slot");
    }

    #[test]
    fn transition_argument_call_with_unresolved_route_stays_unproven() {
        // `choose` routes through a control-flow join; neither operand is the
        // exact origin, so the inner state's demanded premise cannot be
        // reconstructed onto a caller subject. `process` declares `terminates`
        // but its checked body stays unproven, so the route is rejected rather
        // than silently accepted against a same-shaped field.
        let diagnostics = diagnostics(
            r#"
            pub machine choose(
                first: SchedulerHandle in WeakFair,
                second: SchedulerHandle in WeakFair,
                pick_first: bool
            )
            terminates;
            -> SchedulerHandle in WeakFair {
                transition pick_first {
                    true -> first
                    false -> second
                }
            }
            machine process(context: &Context, spare: &Context, pick_first: bool)
            requires context.scheduler in WeakFair
            requires spare.scheduler in WeakFair
            terminates;
            -> u64 {
                transition pick_first {
                    true -> waiting(choose(context.scheduler, spare.scheduler, pick_first))
                    false -> 0
                }
                state waiting(selected: SchedulerHandle in WeakFair) -> u64 { consume(selected) }
            }
            "#,
        );
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove published termination for machine `process`")),
            "unresolved route must stay unproven: {diagnostics:#?}"
        );
    }
}

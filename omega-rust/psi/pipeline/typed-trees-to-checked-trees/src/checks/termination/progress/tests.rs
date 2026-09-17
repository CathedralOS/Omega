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

use crate::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint,
    QualificationCorrespondence, QualificationEvidence, QualificationPayloadIdentity,
};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{
    ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
};
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceHandle;

#[test]
fn qualification_correspondence_ledger_is_exact_and_idempotent() {
    let mut facts = FactPlan::default();
    let source_place = facts.append_symbol_place(SymbolHandle::from_arena_index(10));
    let destination_place = facts.append_symbol_place(SymbolHandle::from_arena_index(11));
    let source_fact = facts.append_fact(Fact::default());
    let destination_fact = facts.append_fact(Fact::default());
    let row = QualificationCorrespondence {
        source_fact,
        destination_fact,
        source_occurrence_place: source_place,
        source_place,
        destination_place,
        formation: ProgramPoint::Statement {
            machine_symbol: SymbolHandle::from_arena_index(12),
            state_symbol: SymbolHandle::from_arena_index(13),
            statement_index: 2,
        },
        payload: QualificationPayloadIdentity::CarryOrigin,
        evidence: QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
            SymbolHandle::from_arena_index(14),
        ),
    };

    let first = facts.append_qualification_correspondence(row);
    let second = facts.append_qualification_correspondence(row);
    assert_eq!(first, second);
    assert_eq!(facts.qualification_correspondences.len(), 1);
    assert_eq!(*facts.qualification_correspondences.get(first), row);
}

#[test]
fn expression_places_preserve_roots_and_segments() {
    let root_symbol = SymbolHandle::from_arena_index(30);
    let field_symbol = SymbolHandle::from_arena_index(31);
    let tail_symbol = SymbolHandle::from_arena_index(32);

    let mut program = TypedTrees::default();
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("root"));
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated("field"));
    let mut member_symbols = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, root_symbol);
    program
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, field_symbol);
    let name = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            head_symbol: root_symbol,
            symbol: field_symbol,
        }));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_value(0),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: name,
                index,
            }));
    let member = program
        .expression_table
        .insert(ExpressionNode::Member(TableMemberExpression {
            receiver: indexed,
            member_symbol: tail_symbol,
            member: Identifier::generated("tail"),
            case_variant: None,
        }));

    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(&program, member);
    let place = facts.places.get(place);
    let segments = facts.place_segments.span_or_empty(place.segments);

    assert_eq!(place.root, PlaceRoot::Symbol(root_symbol));
    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments[0],
        PlaceSegment::Field {
            symbol: field_symbol
        }
    );
    assert_eq!(segments[1], PlaceSegment::FixedIndex { index: 0 });
    assert_eq!(
        segments[2],
        PlaceSegment::Field {
            symbol: tail_symbol
        }
    );
}

#[test]
fn proves_domain_membership_for_structurally_equal_places() {
    let domain_symbol = SymbolHandle::from_arena_index(40);
    let value_symbol = SymbolHandle::from_arena_index(41);
    let field_symbol = SymbolHandle::from_arena_index(42);

    let mut facts = FactPlan::default();
    let left = facts.append_symbol_place(value_symbol);
    facts.push_place_segment(
        left,
        PlaceSegment::Field {
            symbol: field_symbol,
        },
    );

    let right = facts.append_symbol_place(value_symbol);
    facts.push_place_segment(
        right,
        PlaceSegment::Field {
            symbol: field_symbol,
        },
    );

    let fact = facts.append_fact(Fact {
        place: FactPlace::Place(left),
        point: ProgramPoint::Global,
        origin: FactOrigin::DomainDefinition { domain_symbol },
        evidence: Default::default(),
        payload: FactPayload::DomainMembership {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol,
        },
    });
    let mut refs = HandleSpan::empty();
    facts.append_ref(&mut refs, fact);
    let context = facts.append_context(ProgramPoint::Global, refs);

    assert!(facts.places_equal(left, right));
    assert!(
        facts
            .context_view(facts.contexts.get(context))
            .proves_place_domain_membership(right, domain_symbol)
    );
}

#[test]
fn expression_places_resolve_attached_data_members() {
    let machine_symbol = SymbolHandle::from_arena_index(50);
    let self_symbol = SymbolHandle::from_arena_index(51);
    let player_field_symbol = SymbolHandle::from_arena_index(52);
    let player_type_symbol = SymbolHandle::from_arena_index(53);
    let main_data_symbol = SymbolHandle::from_arena_index(54);

    let mut program = TypedTrees::default();
    program.push_data_definition(typed_trees::data::DataDefinition {
        symbol: player_type_symbol,
        name: Identifier::generated("Player"),
        is_public: false,
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        generic_instance: None,
        members: HandleSpan::empty(),
    });
    let mut main_data = typed_trees::data::DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        is_public: false,
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        generic_instance: None,
        members: HandleSpan::empty(),
    };
    program.push_data_member(
        &mut main_data,
        typed_trees::data::DataMember::Field(typed_trees::data::DataField {
            identity: None,
            symbol: player_field_symbol,
            name: Identifier::generated("player"),
            relevance: Default::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }),
    );
    program.push_data_definition(main_data);

    let mut machine = typed_trees::machine::Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks_keyword_source_spans: Vec::new(),
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        attached_data: Some(Identifier::generated("Main")),
        attached_data_symbol: main_data_symbol,
        is_public: false,
        owned_data: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        invokes: HandleSpan::empty(),
        suspends: false,
        blocks: false,
        contracts: HandleSpan::empty(),
        states: HandleSpan::empty(),
    };
    let mut state = typed_trees::state::State {
        symbol: SymbolHandle::from_arena_index(55),
        name: Identifier::generated("main"),
        parameters: HandleSpan::empty(),
        return_type: TypeReferenceHandle::invalid(),
        contracts: HandleSpan::empty(),
        statement_nodes: HandleSpan::empty(),
    };
    let self_type =
        program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Named {
                symbol: machine_symbol,
                name: Identifier::generated("Self"),
            });
    program.push_state_parameter(
        &mut state,
        typed_trees::signature::StateParameter {
            symbol: self_symbol,
            name: Identifier::generated("self"),
            type_reference: self_type,
            is_const: false,
            is_mutable: true,
            is_self: true,
        },
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let self_expression = program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members: HandleSpan::empty(),
            member_symbols: HandleSpan::empty(),
            head_symbol: self_symbol,
            symbol: self_symbol,
        }));
    let member_expression =
        program
            .expression_table
            .insert(ExpressionNode::Member(TableMemberExpression {
                receiver: self_expression,
                member_symbol: SymbolHandle::invalid(),
                member: Identifier::generated("player"),
                case_variant: None,
            }));

    let mut facts = FactPlan::default();
    let place = facts.append_place_from_expression(&program, member_expression);
    let place = facts.places.get(place);
    let segments = facts.place_segments.span_or_empty(place.segments);

    assert_eq!(place.root, PlaceRoot::Symbol(self_symbol));
    assert_eq!(segments.len(), 1);
    assert_eq!(
        segments[0],
        PlaceSegment::Field {
            symbol: player_field_symbol
        }
    );
}

#[test]
fn expression_places_resolve_exact_local_and_collection_element_types() {
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::statement::{StatementNode, TableLocalData};
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
    for (shape, indexed, malformed) in [
        ("record", false, "none"),
        ("array", true, "none"),
        ("slice", true, "none"),
        ("array", true, "stale"),
        ("record", false, "wrong_parent"),
    ] {
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = symbols.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("Outer")),
                (SymbolKind::Machine, SymbolNameRef::Static("main")),
            ],
        );
        let children: Vec<_> = SymbolTableBuilder::child_handles(children).collect();
        let data_symbol = children[0];
        let machine_symbol = children[1];
        let field_symbol = symbols
            .insert_children(
                data_symbol,
                [(SymbolKind::Field, SymbolNameRef::Static("inner"))],
            )
            .start();
        let states = symbols.insert_children(
            machine_symbol,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::State, SymbolNameRef::Static("foreign")),
            ],
        );
        let states: Vec<_> = SymbolTableBuilder::child_handles(states).collect();
        let local_symbol = symbols
            .insert_children(
                if malformed == "wrong_parent" {
                    states[1]
                } else {
                    states[0]
                },
                [(SymbolKind::Local, SymbolNameRef::Static("value"))],
            )
            .start();
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let record = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                name: Identifier::generated("Outer"),
                symbol: data_symbol,
            });
        let local_type = match shape {
            "array" => program
                .type_reference_table
                .insert(TypeReferenceNode::FixedArray {
                    element_type: record,
                    length: FixedArrayLength::Literal(2),
                }),
            "slice" => program
                .type_reference_table
                .insert(TypeReferenceNode::Slice {
                    element_type: record,
                }),
            _ => record,
        };
        let mut data = DataDefinition {
            symbol: data_symbol,
            name: Identifier::generated("Outer"),
            ..DataDefinition::default()
        };
        program.push_data_member(
            &mut data,
            DataMember::Field(DataField {
                identity: None,
                symbol: field_symbol,
                name: Identifier::generated("inner"),
                relevance: Default::default(),
                type_reference: TypeReferenceHandle::invalid(),
            }),
        );
        program.push_data_definition(data);
        let mut state = typed_trees::state::State {
            symbol: states[0],
            name: Identifier::generated("entry"),
            ..Default::default()
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: local_symbol,
                name: Identifier::generated("value"),
                type_reference: local_type,
                ..Default::default()
            }),
        );
        let mut machine = typed_trees::machine::Machine {
            symbol: machine_symbol,
            name: Identifier::generated("main"),
            ..Default::default()
        };
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);
        let root_symbol = if malformed == "stale" {
            SymbolHandle::from_parts(local_symbol.arena_index(), local_symbol.generation() + 1)
        } else {
            local_symbol
        };
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("value"));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, root_symbol);
        let mut receiver = program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath {
                members,
                member_symbols,
                head_symbol: root_symbol,
                symbol: root_symbol,
            }));
        if indexed {
            let index = program.expression_table.insert(ExpressionNode::Integer(
                numerics::literals::IntegerLiteral::from_value(0),
            ));
            receiver =
                program
                    .expression_table
                    .insert(ExpressionNode::Indexed(TableIndexedExpression {
                        collection: receiver,
                        index,
                    }));
        }
        let expression =
            program
                .expression_table
                .insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: SymbolHandle::invalid(),
                    member: Identifier::generated("inner"),
                    case_variant: None,
                }));
        let mut facts = FactPlan::default();
        let place = facts.append_place_from_expression(&program, expression);
        let place = facts.places.get(place);
        let segments = facts.place_segments.span_or_empty(place.segments);
        assert_eq!(place.root, PlaceRoot::Symbol(root_symbol));
        assert_eq!(segments.len(), if indexed { 2 } else { 1 });
        if indexed {
            assert_eq!(segments[0], PlaceSegment::FixedIndex { index: 0 });
        }
        assert_eq!(
            segments.last(),
            Some(&PlaceSegment::Field {
                symbol: if malformed == "none" {
                    field_symbol
                } else {
                    SymbolHandle::invalid()
                },
            }),
            "{shape} {malformed}"
        );
    }
}

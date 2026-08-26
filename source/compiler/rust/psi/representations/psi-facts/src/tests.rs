use super::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint,
    QualificationCorrespondence, QualificationEvidence, QualificationPayloadIdentity,
    build_definition_fact_plan,
};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataField, DataMember};
use psi_typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
use psi_typed_trees::expression::{
    ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
};
use psi_typed_trees::name::Identifier;
use psi_typed_trees::proposition::PropositionApplication;
use psi_typed_trees::types::TypeReferenceHandle;

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
            psi_language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
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
fn builds_definition_fact_plan_for_domains() {
    let valid_domain_symbol = SymbolHandle::from_arena_index(10);
    let alive_domain_symbol = SymbolHandle::from_arena_index(11);

    let mut program = TypedTrees::default();
    let expression = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(ProofFact::Expression(expression));
    let membership = program
        .proof_facts
        .append(ProofFact::Membership(ProofMembershipFact {
            value: expression,
            domain: HandleSpan::empty(),
            domain_symbol: valid_domain_symbol,
        }));
    assert_eq!(membership.arena_index(), fact.arena_index() + 1);
    program.push_domain_definition(DomainDefinition {
        symbol: alive_domain_symbol,
        name: Identifier::generated("Player::Alive"),
        type_parameters: HandleSpan::empty(),
        target_type: TypeReferenceHandle::invalid(),
        index_arguments: Vec::new(),
        is_public: false,
        alias: None,
        classification: None,
        predicate_body: psi_language_semantics::DomainPredicateBody::Present,
        semantic_id: psi_language_semantics::SemanticDomainId::NULL,
        semantic_roles: Default::default(),
        facts: HandleSpan::from_parts(fact, 2),
        operators: HandleSpan::empty(),
        semantic_clause_token_count: 2,
        establishment_routes: Vec::new(),
    });
    program.push_domain_definition(DomainDefinition {
        symbol: valid_domain_symbol,
        name: Identifier::generated("Player::Valid"),
        type_parameters: HandleSpan::empty(),
        target_type: TypeReferenceHandle::invalid(),
        index_arguments: Vec::new(),
        is_public: false,
        alias: None,
        classification: None,
        predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
        semantic_id: psi_language_semantics::SemanticDomainId::NULL,
        semantic_roles: Default::default(),
        facts: HandleSpan::empty(),
        operators: HandleSpan::empty(),
        semantic_clause_token_count: 0,
        establishment_routes: Vec::new(),
    });

    let facts = build_definition_fact_plan(&program);

    assert_eq!(facts.places.len(), 2);
    assert_eq!(facts.facts.len(), 2);
    assert_eq!(facts.domain_definition_facts.len(), 2);
    assert_eq!(facts.contexts.len(), 2);
    assert_eq!(facts.symbol_sets.len(), 2);
    assert_eq!(
        facts.boolean_facts_for_symbol(alive_domain_symbol).count(),
        1
    );
    assert!(facts.symbol_references_domain(alive_domain_symbol, valid_domain_symbol));
    assert_eq!(
        facts
            .facts_at_point(super::ProgramPoint::Definition {
                symbol: alive_domain_symbol,
            })
            .count(),
        2
    );
    let domain_context = facts
        .contexts_at_point(super::ProgramPoint::Definition {
            symbol: alive_domain_symbol,
        })
        .next()
        .expect("domain context");
    assert_eq!(domain_context.boolean_facts().count(), 1);
    assert!(domain_context.proves_domain_membership(expression, valid_domain_symbol));
    for (_, record) in facts.domain_definition_facts.iter() {
        assert_eq!(record.domain_symbol, alive_domain_symbol);
        assert!(record.fact == fact || record.fact == membership);
        assert!(
            facts
                .facts
                .iter()
                .any(|(handle, _)| handle == record.semantic_fact)
        );
        assert!(record.dependencies.is_empty());
    }
}

#[test]
fn builds_checked_ownership_for_every_data_where_fact_form() {
    let data_symbol = SymbolHandle::from_arena_index(60);
    let expression_field_symbol = SymbolHandle::from_arena_index(61);
    let membership_field_symbol = SymbolHandle::from_arena_index(62);
    let proposition_field_symbol = SymbolHandle::from_arena_index(63);
    let domain_symbol = SymbolHandle::from_arena_index(64);
    let proposition_symbol = SymbolHandle::from_arena_index(65);

    let mut program = TypedTrees::default();
    // The spelling intentionally differs: exact resolved identity must own the
    // dependency whenever it is available.
    let expression = append_bare_name_expression(
        &mut program,
        "not-the-expression-field",
        expression_field_symbol,
    );
    let membership_value =
        append_bare_name_expression(&mut program, "membership", membership_field_symbol);
    let proposition_argument = append_bare_name_expression(
        &mut program,
        "not-the-proposition-field",
        proposition_field_symbol,
    );
    let mut proposition_arguments = HandleSpan::empty();
    program
        .expression_table
        .push_expression_handle(&mut proposition_arguments, proposition_argument);

    let first_fact = program
        .proof_facts
        .append(ProofFact::Expression(expression));
    let membership_fact = program
        .proof_facts
        .append(ProofFact::Membership(ProofMembershipFact {
            value: membership_value,
            domain: HandleSpan::empty(),
            domain_symbol,
        }));
    let proposition_fact =
        program
            .proof_facts
            .append(ProofFact::Proposition(PropositionApplication {
                proposition: proposition_symbol,
                name: Identifier::generated("ChecksField"),
                binder_arguments: Vec::new().into_boxed_slice(),
                arguments: proposition_arguments,
            }));
    assert_eq!(membership_fact.arena_index(), first_fact.arena_index() + 1);
    assert_eq!(proposition_fact.arena_index(), first_fact.arena_index() + 2);

    let mut data = DataDefinition {
        symbol: data_symbol,
        name: Identifier::generated("Ledger"),
        where_facts: HandleSpan::from_parts(first_fact, 3),
        ..DataDefinition::default()
    };
    for (symbol, name) in [
        (expression_field_symbol, "expression"),
        (membership_field_symbol, "membership"),
        (proposition_field_symbol, "proposition"),
    ] {
        program.push_data_member(
            &mut data,
            DataMember::Field(DataField {
                symbol,
                name: Identifier::generated(name),
                ..DataField::default()
            }),
        );
    }
    program.push_data_definition(data);

    let facts = build_definition_fact_plan(&program);

    assert_eq!(facts.facts.len(), 3);
    assert_eq!(facts.data_definition_facts.len(), 3);
    assert_eq!(facts.contexts.len(), 1);
    assert_eq!(facts.symbol_sets.len(), 1);
    assert_eq!(
        facts
            .facts_at_point(ProgramPoint::Definition {
                symbol: data_symbol
            })
            .count(),
        3
    );

    let records = facts
        .data_definition_facts
        .iter()
        .map(|(_, record)| record)
        .collect::<Vec<_>>();
    for (record, typed_fact) in records
        .iter()
        .zip([first_fact, membership_fact, proposition_fact])
    {
        assert_eq!(record.data_symbol, data_symbol);
        assert_eq!(record.fact, typed_fact);
        assert_eq!(record.dependencies.len(), 1);
        let semantic = facts.facts.get(record.semantic_fact);
        assert_eq!(
            semantic.point,
            ProgramPoint::Definition {
                symbol: data_symbol
            }
        );
        assert_eq!(semantic.origin, FactOrigin::DataDefinition { data_symbol });
    }
    assert!(matches!(
        facts.facts.get(records[0].semantic_fact).payload,
        FactPayload::BooleanExpression(found) if found == expression
    ));
    assert!(matches!(
        facts.facts.get(records[1].semantic_fact).payload,
        FactPayload::DomainMembership {
            value,
            domain_symbol: found_domain,
            ..
        } if value == membership_value && found_domain == domain_symbol
    ));
    assert!(matches!(
        facts.facts.get(records[2].semantic_fact).payload,
        FactPayload::PropositionApplication {
            fact,
            proposition,
        } if fact == proposition_fact && proposition == proposition_symbol
    ));

    for (record, expected_expression, expected_field) in [
        (&records[0], expression, expression_field_symbol),
        (&records[1], membership_value, membership_field_symbol),
        (&records[2], proposition_argument, proposition_field_symbol),
    ] {
        let dependency = &record.dependencies[0];
        assert_eq!(dependency.expression, expected_expression);
        assert_data_field_place(&facts, dependency.place, data_symbol, expected_field);
    }
}

#[test]
fn resolved_non_field_identity_prevents_spelling_fallback_for_data_dependencies() {
    let data_symbol = SymbolHandle::from_arena_index(70);
    let field_symbol = SymbolHandle::from_arena_index(71);
    let foreign_symbol = SymbolHandle::from_arena_index(72);

    let mut program = TypedTrees::default();
    let expression = append_bare_name_expression(&mut program, "count", foreign_symbol);
    let fact = program
        .proof_facts
        .append(ProofFact::Expression(expression));
    let mut data = DataDefinition {
        symbol: data_symbol,
        name: Identifier::generated("Counter"),
        where_facts: HandleSpan::from_parts(fact, 1),
        ..DataDefinition::default()
    };
    program.push_data_member(
        &mut data,
        DataMember::Field(DataField {
            symbol: field_symbol,
            name: Identifier::generated("count"),
            ..DataField::default()
        }),
    );
    program.push_data_definition(data);

    let facts = build_definition_fact_plan(&program);
    let (_, record) = facts
        .data_definition_facts
        .iter()
        .next()
        .expect("one data-definition fact record");
    let dependency_place = facts.places.get(record.dependencies[0].place);

    assert_eq!(dependency_place.root, PlaceRoot::Symbol(foreign_symbol));
    assert!(dependency_place.segments.is_empty());
}

fn append_bare_name_expression(
    program: &mut TypedTrees,
    spelling: &str,
    symbol: SymbolHandle,
) -> psi_typed_trees::expression::ExpressionHandle {
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated(spelling));
    let mut member_symbols = HandleSpan::empty();
    if symbol.is_valid() {
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, symbol);
    }
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols,
            head_symbol: symbol,
            symbol,
        }))
}

fn assert_data_field_place(
    facts: &FactPlan,
    place: super::PlaceHandle,
    data_symbol: SymbolHandle,
    field_symbol: SymbolHandle,
) {
    let place = facts.places.get(place);
    assert_eq!(place.root, PlaceRoot::Symbol(data_symbol));
    assert_eq!(
        facts.place_segments.span_or_empty(place.segments),
        &[PlaceSegment::Field {
            symbol: field_symbol
        }]
    );
}

#[test]
fn domain_membership_queries_follow_domain_imports() {
    let valid_domain_symbol = SymbolHandle::from_arena_index(20);
    let alive_domain_symbol = SymbolHandle::from_arena_index(21);

    let mut program = TypedTrees::default();
    let expression = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let membership = program
        .proof_facts
        .append(ProofFact::Membership(ProofMembershipFact {
            value: expression,
            domain: HandleSpan::empty(),
            domain_symbol: valid_domain_symbol,
        }));
    program.push_domain_definition(DomainDefinition {
        symbol: alive_domain_symbol,
        name: Identifier::generated("Player::Alive"),
        type_parameters: HandleSpan::empty(),
        target_type: TypeReferenceHandle::invalid(),
        index_arguments: Vec::new(),
        is_public: false,
        alias: None,
        classification: None,
        predicate_body: psi_language_semantics::DomainPredicateBody::Present,
        semantic_id: psi_language_semantics::SemanticDomainId::NULL,
        semantic_roles: Default::default(),
        facts: HandleSpan::from_parts(membership, 1),
        operators: HandleSpan::empty(),
        semantic_clause_token_count: 1,
        establishment_routes: Vec::new(),
    });
    program.push_domain_definition(DomainDefinition {
        symbol: valid_domain_symbol,
        name: Identifier::generated("Player::Valid"),
        type_parameters: HandleSpan::empty(),
        target_type: TypeReferenceHandle::invalid(),
        index_arguments: Vec::new(),
        is_public: false,
        alias: None,
        classification: None,
        predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
        semantic_id: psi_language_semantics::SemanticDomainId::NULL,
        semantic_roles: Default::default(),
        facts: HandleSpan::empty(),
        operators: HandleSpan::empty(),
        semantic_clause_token_count: 0,
        establishment_routes: Vec::new(),
    });

    let mut facts = build_definition_fact_plan(&program);
    let place = facts.append_expression_place(expression);
    facts.append_fact_context(Fact {
        place: FactPlace::Place(place),
        point: ProgramPoint::Global,
        origin: FactOrigin::Unknown,
        evidence: Default::default(),
        payload: FactPayload::DomainMembership {
            value: expression,
            domain: HandleSpan::empty(),
            domain_symbol: alive_domain_symbol,
        },
    });

    assert!(facts.domain_implies(alive_domain_symbol, valid_domain_symbol));
    assert!(facts.proves_domain_membership_at_point(
        ProgramPoint::Global,
        expression,
        valid_domain_symbol
    ));
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
        psi_numerics::literals::IntegerLiteral::from_value(0),
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
            value: psi_typed_trees::expression::ExpressionHandle::invalid(),
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
    program.push_data_definition(psi_typed_trees::data::DataDefinition {
        symbol: player_type_symbol,
        name: Identifier::generated("Player"),
        is_public: false,
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: psi_typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        generic_instance: None,
        members: HandleSpan::empty(),
    });
    let mut main_data = psi_typed_trees::data::DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        is_public: false,
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: psi_typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        generic_instance: None,
        members: HandleSpan::empty(),
    };
    program.push_data_member(
        &mut main_data,
        psi_typed_trees::data::DataMember::Field(psi_typed_trees::data::DataField {
            identity: None,
            symbol: player_field_symbol,
            name: Identifier::generated("player"),
            relevance: Default::default(),
            type_reference: TypeReferenceHandle::invalid(),
        }),
    );
    program.push_data_definition(main_data);

    let mut machine = psi_typed_trees::machine::Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
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
    let mut state = psi_typed_trees::state::State {
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
            .insert(psi_typed_trees::types::TypeReferenceNode::Named {
                symbol: machine_symbol,
                name: Identifier::generated("Self"),
            });
    program.push_state_parameter(
        &mut state,
        psi_typed_trees::signature::StateParameter {
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

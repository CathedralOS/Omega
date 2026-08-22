use super::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint,
    build_definition_fact_plan,
};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::{DomainDefinition, ProofFact, ProofMembershipFact};
use psi_typed_trees::expression::{
    ExpressionNode, TableIndexedExpression, TableMemberExpression, TableNamePath,
};
use psi_typed_trees::invariant::InvariantDefinition;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

#[test]
fn builds_definition_fact_plan_for_domains_and_invariants() {
    let valid_domain_symbol = SymbolHandle::from_arena_index(10);
    let alive_domain_symbol = SymbolHandle::from_arena_index(11);
    let invariant_symbol = SymbolHandle::from_arena_index(12);

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
        predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
        semantic_id: psi_language_semantics::SemanticDomainId::NULL,
        semantic_roles: Default::default(),
        facts: HandleSpan::empty(),
        operators: HandleSpan::empty(),
        semantic_clause_token_count: 0,
        establishment_routes: Vec::new(),
    });

    let constraint = program
        .type_reference_table
        .insert_constraints([TypeConstraintNode::Named(Identifier::generated("finite"))]);
    program.push_invariant_definition(InvariantDefinition {
        symbol: invariant_symbol,
        name: Identifier::generated("Finite"),
        constraints: constraint,
    });

    let facts = build_definition_fact_plan(&program);

    assert_eq!(facts.places.len(), 3);
    assert_eq!(facts.facts.len(), 3);
    assert_eq!(facts.contexts.len(), 3);
    assert_eq!(facts.symbol_sets.len(), 3);
    assert_eq!(
        facts.boolean_facts_for_symbol(alive_domain_symbol).count(),
        1
    );
    assert!(facts.symbol_references_domain(alive_domain_symbol, valid_domain_symbol));
    assert_eq!(
        facts.type_constraints_for_symbol(invariant_symbol).count(),
        1
    );
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

    let invariant_context = facts
        .contexts_at_point(super::ProgramPoint::Definition {
            symbol: invariant_symbol,
        })
        .next()
        .expect("invariant context");
    assert_eq!(invariant_context.type_constraints().count(), 1);
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
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: psi_typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
        members: HandleSpan::empty(),
    });
    let mut main_data = psi_typed_trees::data::DataDefinition {
        symbol: main_data_symbol,
        name: Identifier::generated("Main"),
        supply_mode: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        properties: psi_typed_trees::data::DataProperties::default(),
        quotient: None,
        where_facts: Default::default(),
        zero_gated: false,
        retired_identities: Vec::new(),
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
        lifetime_parameters: Vec::new(),
        type_parameters: HandleSpan::empty(),
        attached_data: Some(Identifier::generated("Main")),
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

use super::*;

#[test]
fn instantiates_call_contract_places_onto_caller_arguments() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(1);
    let caller_state_symbol = SymbolHandle::from_arena_index(2);
    let callee_machine_symbol = SymbolHandle::from_arena_index(3);
    let callee_state_symbol = SymbolHandle::from_arena_index(4);
    let caller_argument_symbol = SymbolHandle::from_arena_index(5);
    let callee_parameter_symbol = SymbolHandle::from_arena_index(6);

    let mut program = psi_typed_trees::TypedTrees::default();
    let caller_argument_expression =
        program
            .expression_table
            .insert(psi_typed_trees::expression::ExpressionNode::Name(
                psi_typed_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: caller_argument_symbol,
                    symbol: caller_argument_symbol,
                },
            ));
    let callee_parameter_expression =
        program
            .expression_table
            .insert(psi_typed_trees::expression::ExpressionNode::Name(
                psi_typed_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: callee_parameter_symbol,
                    symbol: callee_parameter_symbol,
                },
            ));
    let callee_fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(
            callee_parameter_expression,
        ));

    let mut caller_arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut caller_arguments, caller_argument_expression);
    let caller_statement = program
        .statement_table
        .insert(StatementNode::Call(TableCall {
            source_span: psi_source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: callee_state_symbol,
            receiver: HandleSpan::empty(),
            target: Identifier::generated("run"),
            machine_arguments: Box::default(),
            arguments: caller_arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }));

    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: HandleSpan::empty(),
        return_type: Default::default(),
        contracts: Default::default(),
        statement_nodes: HandleSpan::from_parts(caller_statement, 1),
    };
    program.push_state_parameter(
        &mut caller_state,
        StateParameter {
            symbol: caller_argument_symbol,
            name: Identifier::generated("value"),
            type_reference: Default::default(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Caller"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let mut callee_state = State {
        symbol: callee_state_symbol,
        name: Identifier::generated("run"),
        parameters: HandleSpan::empty(),
        return_type: Default::default(),
        contracts: Default::default(),
        statement_nodes: HandleSpan::empty(),
    };
    program.push_state_parameter(
        &mut callee_state,
        StateParameter {
            symbol: callee_parameter_symbol,
            name: Identifier::generated("amount"),
            type_reference: Default::default(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        },
    );

    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: Identifier::generated("Worker"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine(callee_machine);

    let call = psi_checked_trees::ContractCallFact {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index: 0,
        call_ordinal: 0,
        target_machine_symbol: callee_machine_symbol,
        target_state_symbol: callee_state_symbol,
        requires: HandleSpan::empty(),
        ensures: HandleSpan::empty(),
        evidence_arguments: HandleSpan::empty(),
    };
    let contract = psi_checked_trees::ContractProofFact {
        kind: ContractProofFactKind::Requires,
        owner: ContractProofFactOwner::MachineState {
            machine_symbol: callee_machine_symbol,
            state_symbol: callee_state_symbol,
        },
        fact: callee_fact,
        evidence_term: None,
        qualification_authorization: None,
    };

    let mut semantic = psi_facts::FactPlan::default();
    let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
    let psi_facts::FactPlace::Place(place_handle) = place else {
        panic!("expected instantiated call place");
    };

    assert_eq!(
        semantic.places.get(place_handle).root,
        psi_facts::PlaceRoot::Symbol(caller_argument_symbol)
    );
}

#[test]
fn instantiates_call_contract_places_for_attached_data_arguments() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(1);
    let caller_state_symbol = SymbolHandle::from_arena_index(2);
    let callee_machine_symbol = SymbolHandle::from_arena_index(3);
    let callee_state_symbol = SymbolHandle::from_arena_index(4);
    let caller_player_symbol = SymbolHandle::from_arena_index(5);
    let callee_player_symbol = SymbolHandle::from_arena_index(6);

    let mut program = psi_typed_trees::TypedTrees::default();
    let player_fact_expression =
        program
            .expression_table
            .insert(psi_typed_trees::expression::ExpressionNode::Name(
                psi_checked_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: callee_player_symbol,
                    symbol: callee_player_symbol,
                },
            ));
    let callee_fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(
            player_fact_expression,
        ));

    let mut caller_arguments = HandleSpan::empty();
    let self_name = Expression::Name(NamePath::resolved(
        vec![Identifier::generated("self")],
        caller_machine_symbol,
        caller_machine_symbol,
    ));
    let player_member =
        Expression::Member(Box::new(psi_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: caller_player_symbol,
            member: Identifier::generated("player"),
            case_variant: None,
        }));
    let player_argument = mutable_borrow(player_member);
    let player_argument = program.expression_table.insert_tree(&player_argument);
    program
        .statement_table
        .push_expression_handle(&mut caller_arguments, player_argument);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            source_span: psi_source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: caller_machine_symbol,
            target_symbol: callee_state_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            machine_arguments: Box::default(),
            arguments: caller_arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: Identifier::generated("Game"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut callee_state = State {
        symbol: callee_state_symbol,
        name: Identifier::generated("heal"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut callee_state,
        StateParameter {
            symbol: callee_player_symbol,
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine(callee_machine);

    let call = psi_checked_trees::ContractCallFact {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index: 0,
        call_ordinal: 0,
        target_machine_symbol: callee_machine_symbol,
        target_state_symbol: callee_state_symbol,
        requires: HandleSpan::empty(),
        ensures: HandleSpan::empty(),
        evidence_arguments: HandleSpan::empty(),
    };
    let contract = psi_checked_trees::ContractProofFact {
        kind: ContractProofFactKind::Requires,
        owner: ContractProofFactOwner::MachineState {
            machine_symbol: callee_machine_symbol,
            state_symbol: callee_state_symbol,
        },
        fact: callee_fact,
        evidence_term: None,
        qualification_authorization: None,
    };

    let mut semantic = psi_facts::FactPlan::default();
    let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
    let psi_facts::FactPlace::Place(place_handle) = place else {
        panic!("expected instantiated call place");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    assert_eq!(
        place.root,
        psi_facts::PlaceRoot::Symbol(caller_machine_symbol)
    );
    assert_eq!(segments.len(), 1);
    assert_eq!(
        segments[0],
        psi_facts::PlaceSegment::Field {
            symbol: caller_player_symbol
        }
    );
}

#[test]
fn instantiates_call_contract_places_for_expression_statement_calls() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(1);
    let caller_state_symbol = SymbolHandle::from_arena_index(2);
    let callee_machine_symbol = SymbolHandle::from_arena_index(3);
    let callee_state_symbol = SymbolHandle::from_arena_index(4);
    let caller_player_symbol = SymbolHandle::from_arena_index(5);
    let callee_player_symbol = SymbolHandle::from_arena_index(6);

    let mut program = psi_typed_trees::TypedTrees::default();
    let player_fact_expression =
        program
            .expression_table
            .insert(psi_typed_trees::expression::ExpressionNode::Name(
                psi_checked_trees::expression::TableNamePath {
                    members: HandleSpan::empty(),
                    member_symbols: HandleSpan::empty(),
                    head_symbol: callee_player_symbol,
                    symbol: callee_player_symbol,
                },
            ));
    let callee_fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(
            player_fact_expression,
        ));

    let self_name = Expression::Name(NamePath::resolved(
        vec![Identifier::generated("self")],
        caller_machine_symbol,
        caller_machine_symbol,
    ));
    let player_member =
        Expression::Member(Box::new(psi_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: caller_player_symbol,
            member: Identifier::generated("player"),
            case_variant: None,
        }));
    let player_argument = mutable_borrow(player_member);
    let call_expression = Expression::Call(Box::new(CallExpression {
        receiver: Some(Box::new(Expression::Name(NamePath::resolved(
            vec![Identifier::generated("self")],
            caller_machine_symbol,
            caller_machine_symbol,
        )))),
        target_symbol: callee_state_symbol,
        target: Identifier::generated("heal"),
        arguments: Arc::from(vec![player_argument].into_boxed_slice()),
        evidence_arguments: Arc::default(),
        operational_acknowledgement: Default::default(),
    }));
    let call_expression = program.expression_table.insert_tree(&call_expression);

    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Expression(call_expression),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: Identifier::generated("Game"),
        attached_data: None,
        attached_data_symbol: psi_symbols::SymbolHandle::invalid(),
        is_public: false,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut callee_state = State {
        symbol: callee_state_symbol,
        name: Identifier::generated("heal"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut callee_state,
        StateParameter {
            symbol: callee_player_symbol,
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine(callee_machine);

    let call = psi_checked_trees::ContractCallFact {
        caller_machine_symbol,
        caller_state_symbol,
        statement_index: 0,
        call_ordinal: 0,
        target_machine_symbol: callee_machine_symbol,
        target_state_symbol: callee_state_symbol,
        requires: HandleSpan::empty(),
        ensures: HandleSpan::empty(),
        evidence_arguments: HandleSpan::empty(),
    };
    let contract = psi_checked_trees::ContractProofFact {
        kind: ContractProofFactKind::Requires,
        owner: ContractProofFactOwner::MachineState {
            machine_symbol: callee_machine_symbol,
            state_symbol: callee_state_symbol,
        },
        fact: callee_fact,
        evidence_term: None,
        qualification_authorization: None,
    };

    let mut semantic = psi_facts::FactPlan::default();
    let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
    let psi_facts::FactPlace::Place(place_handle) = place else {
        panic!("expected instantiated call place");
    };
    let place = semantic.places.get(place_handle);
    let segments = semantic.place_segments.span_or_empty(place.segments);

    assert_eq!(
        place.root,
        psi_facts::PlaceRoot::Symbol(caller_machine_symbol)
    );
    assert_eq!(segments.len(), 1);
    assert_eq!(
        segments[0],
        psi_facts::PlaceSegment::Field {
            symbol: caller_player_symbol
        }
    );
}

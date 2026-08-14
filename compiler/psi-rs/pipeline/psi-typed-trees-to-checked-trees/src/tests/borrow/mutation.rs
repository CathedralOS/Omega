use super::super::*;

#[test]
fn call_mutated_places_include_mutable_attached_data_arguments() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);

    let mut program = psi_typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![Identifier::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member =
        Expression::Member(Box::new(psi_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: player_symbol,
            member: Identifier::generated("player"),
            case_variant: None,
        }));
    let player_argument = Expression::Mutable(Box::new(player_member));
    let player_argument = program.expression_table.insert_tree(&player_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, player_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        blocks: false,
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            machine_arguments: Box::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut machine, state);
    let mut target_state = State {
        symbol: target_symbol,
        name: Identifier::generated("heal"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(5),
            name: Identifier::generated("self"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: true,
        },
    );
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(6),
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut machine, target_state);
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        machine_symbol,
        state_symbol,
        &facts,
        &call,
        &mut state_mutation_summary_cache,
    );

    assert!(places.iter().any(
        |place| place.root == psi_facts::PlaceRoot::Symbol(player_symbol)
            && place.segments.is_empty()
    ));
}

#[test]
fn call_mutated_places_include_mutable_local_arguments_from_unresolved_names() {
    let machine_symbol = SymbolHandle::from_arena_index(10);
    let state_symbol = SymbolHandle::from_arena_index(11);
    let target_symbol = SymbolHandle::from_arena_index(12);
    let local_symbol = SymbolHandle::from_arena_index(13);

    let mut program = psi_typed_trees::TypedTrees::default();
    let local_name = Expression::Name(NamePath::unresolved(vec![Identifier::generated("player")]));
    let local_argument = Expression::Mutable(Box::new(local_name));
    let local_argument = program.expression_table.insert_tree(&local_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, local_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main"),
        attached_data: None,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        conformance_bounds: Vec::new(),
        invokes: Default::default(),
        suspends: false,
        blocks: false,
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(psi_typed_trees::statement::TableLocalData {
            symbol: local_symbol,
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            initial_value: psi_typed_trees::expression::ExpressionHandle::invalid(),
            is_mutable: false,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: Identifier::generated("heal"),
            machine_arguments: Box::default(),
            arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut machine, state);
    let mut target_state = State {
        symbol: target_symbol,
        name: Identifier::generated("heal"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(14),
            name: Identifier::generated("player"),
            type_reference: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut machine, target_state);
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let places = call_mutated_places(
        &program,
        machine_symbol,
        state_symbol,
        &facts,
        &call,
        &mut state_mutation_summary_cache,
    );

    assert!(places.iter().any(
        |place| place.root == psi_facts::PlaceRoot::Symbol(local_symbol)
            && place.segments.is_empty()
    ));
}

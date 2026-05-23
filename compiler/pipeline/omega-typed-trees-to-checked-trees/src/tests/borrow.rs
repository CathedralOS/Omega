use super::*;

#[test]
fn collects_nested_state_call_ordinals_for_checked_borrow_facts() {
    let entry_symbol = SymbolHandle::from_arena_index(1);
    let outer_symbol = SymbolHandle::from_arena_index(2);
    let inner_symbol = SymbolHandle::from_arena_index(3);
    let item_symbol = SymbolHandle::from_arena_index(4);
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let item_argument = Expression::Mutable(Box::new(Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("item")],
        item_symbol,
        item_symbol,
    ))));

    let nested_call = Expression::Call(Box::new(CallExpression {
        receiver: None,
        target_symbol: inner_symbol,
        target: ProgramName::generated("inner"),
        arguments: Arc::from(vec![item_argument].into_boxed_slice()),
    }));

    let mut program = omega_typed_trees::TypedTrees::default();
    let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let nested_call = program.expression_table.insert_tree(&nested_call);
    let mut outer_arguments = Default::default();
    program
        .statement_table
        .push_expression_handle(&mut outer_arguments, nested_call);
    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Game"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut entry_state = State {
        symbol: entry_symbol,
        name: ProgramName::generated("entry"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut entry_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: outer_symbol,
            receiver: Default::default(),
            target: ProgramName::generated("outer"),
            arguments: outer_arguments,
        }),
    );
    program.push_state_parameter(
        &mut entry_state,
        StateParameter {
            symbol: item_symbol,
            name: ProgramName::generated("item"),
            type_reference: unit_type,
            is_const: false,
            is_mutable: true,
            is_self: false,
        },
    );
    program.push_machine_state(&mut machine, entry_state);
    program.push_machine_state(
        &mut machine,
        State {
            symbol: outer_symbol,
            name: ProgramName::generated("outer"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine_state(
        &mut machine,
        State {
            symbol: inner_symbol,
            name: ProgramName::generated("inner"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let calls = facts.calls.span(state.calls).unwrap();

    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].statement_index, 0);
    assert_eq!(calls[0].call_ordinal, 0);
    assert_eq!(calls[0].target_symbol, outer_symbol);
    assert_eq!(calls[1].statement_index, 0);
    assert_eq!(calls[1].call_ordinal, 1);
    assert_eq!(calls[1].target_symbol, inner_symbol);
}

#[test]
fn collects_mutable_attached_data_argument_access_roots() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);

    let mut program = omega_typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: player_symbol,
            member: ProgramName::generated("player"),
        },
    ));
    let player_argument = Expression::Mutable(Box::new(player_member));
    let player_argument = program.expression_table.insert_tree(&player_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, player_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: ProgramName::generated("heal"),
            arguments,
        }),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine_state(
        &mut machine,
        State {
            symbol: target_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(machine);

    let facts = build_borrow_facts(&program);
    let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
    let call = facts.calls.span(state.calls).unwrap()[0].clone();
    let accesses = facts.argument_accesses.span(call.accesses).unwrap();

    assert_eq!(accesses.len(), 1);
    assert_eq!(accesses[0].root_symbol, player_symbol);
    assert_eq!(accesses[0].kind, BorrowAccessKind::Mutable);
}

#[test]
fn call_mutated_places_include_mutable_attached_data_arguments() {
    let machine_symbol = SymbolHandle::from_arena_index(1);
    let state_symbol = SymbolHandle::from_arena_index(2);
    let target_symbol = SymbolHandle::from_arena_index(3);
    let player_symbol = SymbolHandle::from_arena_index(4);

    let mut program = omega_typed_trees::TypedTrees::default();
    let self_name = Expression::Name(NamePath::resolved(
        vec![ProgramName::generated("self")],
        machine_symbol,
        machine_symbol,
    ));
    let player_member = Expression::Member(Box::new(
        omega_checked_trees::expression::MemberExpression {
            receiver: self_name,
            member_symbol: player_symbol,
            member: ProgramName::generated("player"),
        },
    ));
    let player_argument = Expression::Mutable(Box::new(player_member));
    let player_argument = program.expression_table.insert_tree(&player_argument);

    let mut arguments = HandleSpan::empty();
    program
        .statement_table
        .push_expression_handle(&mut arguments, player_argument);

    let mut machine = Machine {
        symbol: machine_symbol,
        name: ProgramName::generated("Main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    let mut state = State {
        symbol: state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: machine_symbol,
            target_symbol,
            receiver: Default::default(),
            target: ProgramName::generated("heal"),
            arguments,
        }),
    );
    program.push_machine_state(&mut machine, state);
    let mut target_state = State {
        symbol: target_symbol,
        name: ProgramName::generated("heal"),
        parameters: Default::default(),
        return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
        statement_nodes: Default::default(),
    };
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(5),
            name: ProgramName::generated("self"),
            type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: true,
            is_self: true,
        },
    );
    program.push_state_parameter(
        &mut target_state,
        StateParameter {
            symbol: SymbolHandle::from_arena_index(6),
            name: ProgramName::generated("player"),
            type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
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

    assert!(places.iter().any(|place| place.root
        == omega_facts::PlaceRoot::Symbol(player_symbol)
        && place.segments.is_empty()));
}

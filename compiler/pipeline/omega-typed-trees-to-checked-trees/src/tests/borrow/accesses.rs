use super::super::*;

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

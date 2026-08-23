use super::super::*;

#[test]
fn collects_nested_state_call_ordinals_for_checked_borrow_facts() {
    let entry_symbol = SymbolHandle::from_arena_index(1);
    let outer_symbol = SymbolHandle::from_arena_index(2);
    let inner_symbol = SymbolHandle::from_arena_index(3);
    let item_symbol = SymbolHandle::from_arena_index(4);
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let item_argument = mutable_borrow(Expression::Name(NamePath::resolved(
        vec![Identifier::generated("item")],
        item_symbol,
        item_symbol,
    )));

    let nested_call = Expression::Call(Box::new(CallExpression {
        receiver: None,
        target_symbol: inner_symbol,
        target: Identifier::generated("inner"),
        arguments: Arc::from(vec![item_argument].into_boxed_slice()),
        evidence_arguments: Arc::default(),
        operational_acknowledgement: Default::default(),
    }));

    let mut program = psi_typed_trees::TypedTrees::default();
    let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let nested_call = program.expression_table.insert_tree(&nested_call);
    let mut outer_arguments = Default::default();
    program
        .statement_table
        .push_expression_handle(&mut outer_arguments, nested_call);
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Game"),
        attached_data: None,
        supply_mode: Default::default(),
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
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
    let mut entry_state = State {
        symbol: entry_symbol,
        name: Identifier::generated("entry"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    program.statement_table.push_statement(
        &mut entry_state.statement_nodes,
        StatementNode::Call(TableCall {
            receiver_symbol: SymbolHandle::invalid(),
            target_symbol: outer_symbol,
            receiver: Default::default(),
            target: Identifier::generated("outer"),
            machine_arguments: Box::default(),
            arguments: outer_arguments,
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_state_parameter(
        &mut entry_state,
        StateParameter {
            symbol: item_symbol,
            name: Identifier::generated("item"),
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
            name: Identifier::generated("outer"),
            parameters: Default::default(),
            return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            contracts: Default::default(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine_state(
        &mut machine,
        State {
            symbol: inner_symbol,
            name: Identifier::generated("inner"),
            parameters: Default::default(),
            return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            contracts: Default::default(),
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

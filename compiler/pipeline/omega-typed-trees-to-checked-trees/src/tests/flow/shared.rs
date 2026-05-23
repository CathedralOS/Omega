use super::*;

#[test]
fn builds_shared_flow_facts_for_state_and_call_sites() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(40);
    let caller_state_symbol = SymbolHandle::from_arena_index(41);
    let callee_machine_symbol = SymbolHandle::from_arena_index(42);
    let callee_state_symbol = SymbolHandle::from_arena_index(43);

    let mut program = omega_typed_trees::TypedTrees::default();
    let contract_expression = program
        .expression_table
        .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
    let contract_fact =
        program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                contract_expression,
            ));

    let callee_state = State {
        symbol: callee_state_symbol,
        name: ProgramName::generated("run"),
        parameters: Default::default(),
        return_type: Default::default(),
        statement_nodes: Default::default(),
    };
    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: ProgramName::generated("Worker::run"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine_contract(
        &mut callee_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: HandleSpan::from_parts(contract_fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(callee_machine);

    let call_arguments = HandleSpan::empty();
    let call_statement_receiver = HandleSpan::empty();
    let call_statement = StatementNode::Call(TableCall {
        receiver: call_statement_receiver,
        receiver_symbol: caller_machine_symbol,
        target: ProgramName::generated("run"),
        target_symbol: callee_state_symbol,
        arguments: call_arguments,
    });
    let caller_statement = program.statement_table.insert(call_statement);
    let caller_state = State {
        symbol: caller_state_symbol,
        name: ProgramName::generated("main"),
        parameters: Default::default(),
        return_type: Default::default(),
        statement_nodes: HandleSpan::from_parts(caller_statement, 1),
    };
    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: ProgramName::generated("Main::main"),
        attached_data: None,
        contains: Default::default(),
        owned_data: Default::default(),
        satisfies: Default::default(),
        effects: Default::default(),
        contracts: Default::default(),
        states: Default::default(),
    };
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    let effects = omega_effects::infer_effects(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(&program, &borrow, &proof, &semantic, &domains, &effects);

    let caller_flow = flow
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == caller_machine_symbol
                && state.state_symbol == caller_state_symbol)
                .then_some(state)
        })
        .expect("caller flow state");
    assert!(caller_flow.entry_semantic_contexts.is_empty());
    assert_eq!(flow.calls.span_or_empty(caller_flow.calls).len(), 1);

    let call_flow = flow.calls.span_or_empty(caller_flow.calls)[0].clone();
    assert_eq!(call_flow.statement_index, 0);
    assert_eq!(call_flow.call_ordinal, 0);
    assert_eq!(call_flow.target_symbol, callee_state_symbol);
    assert!(call_flow.entry_semantic_contexts.is_empty());
    assert!(!call_flow.requires_contexts.is_empty());
    assert!(call_flow.exit_semantic_contexts.is_empty());
    assert_eq!(
        proof
            .contract_fact_refs
            .span_or_empty(call_flow.requires)
            .len(),
        1
    );
}

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
        name: Identifier::generated("run"),
        parameters: Default::default(),
        return_type: Default::default(),
        statement_nodes: Default::default(),
    };
    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: Identifier::generated("Worker::run"),
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
        target: Identifier::generated("run"),
        target_symbol: callee_state_symbol,
        arguments: call_arguments,
    });
    let caller_statement = program.statement_table.insert(call_statement);
    let caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: Default::default(),
        statement_nodes: HandleSpan::from_parts(caller_statement, 1),
    };
    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Main::main"),
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

    let caller_borrow_state = borrow
        .states
        .iter()
        .find_map(|(handle, state)| {
            (state.machine_symbol == caller_machine_symbol
                && state.state_symbol == caller_state_symbol)
                .then_some(handle)
        })
        .expect("caller borrow state");
    let caller_borrow_call = borrow
        .calls
        .iter()
        .find_map(|(handle, call)| {
            (call.statement_index == 0
                && call.call_ordinal == 0
                && call.target_symbol == callee_state_symbol)
                .then_some(handle)
        })
        .expect("caller borrow call");
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
    assert_eq!(
        flow.borrow_state_constraints(caller_flow.entry_constraints)
            .collect::<Vec<_>>(),
        vec![caller_borrow_state]
    );
    assert_eq!(
        flow.borrow_writable_root_constraints(caller_flow.entry_constraints)
            .collect::<Vec<_>>(),
        Vec::<omega_core::arena::Handle<omega_checked_trees::BorrowWritableRootFact>>::new()
    );
    assert_eq!(flow.calls.span_or_empty(caller_flow.calls).len(), 1);

    let call_flow = flow.calls.span_or_empty(caller_flow.calls)[0].clone();
    assert_eq!(call_flow.statement_index, 0);
    assert_eq!(call_flow.call_ordinal, 0);
    assert_eq!(call_flow.target_symbol, callee_state_symbol);
    assert!(call_flow.entry_semantic_contexts.is_empty());
    assert_eq!(
        flow.borrow_state_constraints(call_flow.entry_constraints)
            .collect::<Vec<_>>(),
        vec![caller_borrow_state]
    );
    assert_eq!(
        flow.borrow_call_constraints(call_flow.entry_constraints)
            .collect::<Vec<_>>(),
        vec![caller_borrow_call]
    );
    assert_eq!(
        flow.borrow_access_constraints(call_flow.entry_constraints)
            .collect::<Vec<_>>()
            .len(),
        borrow
            .calls
            .get(caller_borrow_call)
            .accesses
            .count() as usize
    );
    assert!(!call_flow.requires_contexts.is_empty());
    assert_eq!(
        flow.semantic_constraint_contexts(call_flow.requires_constraints)
            .count(),
        1
    );
    assert!(call_flow.exit_semantic_contexts.is_empty());
    assert_eq!(
        proof
            .contract_fact_refs
            .span_or_empty(call_flow.requires)
            .len(),
        1
    );
}

#[test]
fn carries_local_borrow_loans_into_later_call_constraints() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.use_value(&mut self.value);
            self.write_alias(alias);
        }

        machine Main::use_value(&mut self, value: &mut i32) {}

        machine Main::write_alias(&mut self, value: &mut i32) {}
    "#;

    let tokens = omega_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);

    let state_flow = flow.states.iter().next().map(|(_, state)| state).unwrap();
    let call_flow = flow.calls.span_or_empty(state_flow.calls)[0].clone();
    let loans: Vec<_> = flow
        .borrow_loan_constraints(call_flow.entry_constraints)
        .collect();
    assert_eq!(loans.len(), 1);
    let loan = borrow.loans.get(loans[0]);
    assert!(loan.owner_symbol.is_valid());
    assert!(flow.borrow_weakenings.span_or_empty(state_flow.borrow_weakenings).is_empty());
}

#[test]
fn carries_helper_returned_loans_into_later_call_constraints() {
    let source = r#"
        data Exit {
            destination: i32;
        }

        data Room {
            exits: [Exit; 1];
        }

        data Main {
            room: Room;
        }

        machine Main::main(&mut self) {
            let alias: &mut [Exit] = self.room.exits.as_mut_slice();
            self.use_exit(&mut self.room.exits[0]);
            self.write_alias(alias);
        }

        machine Main::use_exit(&mut self, exit: &mut Exit) {}

        machine Main::write_alias(&mut self, exits: &mut [Exit]) {}
    "#;

    let tokens = omega_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);

    let state_flow = flow.states.iter().next().map(|(_, state)| state).unwrap();
    let call_flow = flow
        .calls
        .span_or_empty(state_flow.calls)
        .iter()
        .find(|call| call.statement_index == 1)
        .cloned()
        .expect("later call should be present");
    let loans: Vec<_> = flow
        .borrow_loan_constraints(call_flow.entry_constraints)
        .collect();
    assert_eq!(loans.len(), 1);
    let loan = borrow.loans.get(loans[0]);
    assert!(loan.owner_symbol.is_valid());
    assert!(flow.borrow_weakenings.span_or_empty(state_flow.borrow_weakenings).is_empty());
}

#[test]
fn drops_local_borrow_loans_after_last_use() {
    let source = r#"
        data Main {
            value: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            self.write_alias(alias);
            self.use_value(&mut self.value);
        }

        machine Main::write_alias(&mut self, value: &mut i32) {}

        machine Main::use_value(&mut self, value: &mut i32) {}
    "#;

    let tokens = omega_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = omega_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type");
    let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
    let effects = omega_effects::infer_effects(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &domains, &effects);

    let loan = borrow.loans.iter().next().map(|(_, loan)| loan).expect("loan");
    assert_eq!(loan.statement_index, 0);
    assert_eq!(loan.last_use_statement_index, 1);

    let state_flow = flow.states.iter().next().map(|(_, state)| state).unwrap();
    let call_flows = flow.calls.span_or_empty(state_flow.calls);
    assert_eq!(call_flows.len(), 2);

    let first_call_loans: Vec<_> = flow
        .borrow_loan_constraints(call_flows[0].entry_constraints)
        .collect();
    assert_eq!(first_call_loans.len(), 1);

    let second_call_loans: Vec<_> = flow
        .borrow_loan_constraints(call_flows[1].entry_constraints)
        .collect();
    assert!(second_call_loans.is_empty());
    let weakenings = flow.borrow_weakenings.span_or_empty(state_flow.borrow_weakenings);
    assert_eq!(weakenings.len(), 1);
    assert_eq!(weakenings[0].loan, first_call_loans[0]);
    assert_eq!(
        weakenings[0].reason,
        omega_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
    );
}

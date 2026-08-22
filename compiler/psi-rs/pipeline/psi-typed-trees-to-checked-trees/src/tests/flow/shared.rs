use super::*;

#[test]
fn builds_shared_flow_facts_for_state_and_call_sites() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(40);
    let caller_state_symbol = SymbolHandle::from_arena_index(41);
    let callee_machine_symbol = SymbolHandle::from_arena_index(42);
    let callee_state_symbol = SymbolHandle::from_arena_index(43);

    let mut program = psi_typed_trees::TypedTrees::default();
    let contract_expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let contract_fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(
            contract_expression,
        ));

    let callee_state = State {
        symbol: callee_state_symbol,
        name: Identifier::generated("run"),
        parameters: Default::default(),
        return_type: Default::default(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    let mut callee_machine = Machine {
        symbol: callee_machine_symbol,
        name: Identifier::generated("Worker::run"),
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
    program.push_machine_state(&mut callee_machine, callee_state);
    program.push_machine_contract(
        &mut callee_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            binding: None,
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
        machine_arguments: Box::default(),
        arguments: call_arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
        discards_result: false,
    });
    let caller_statement = program.statement_table.insert(call_statement);
    let caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: Default::default(),
        contracts: Default::default(),
        statement_nodes: HandleSpan::from_parts(caller_statement, 1),
    };
    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
        name: Identifier::generated("Main::main"),
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
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let operations = psi_effects::infer_operational_may(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(
        &program,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );

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
        .control
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
        Vec::<psi_arena::Handle<psi_checked_trees::BorrowWritableRootFact>>::new()
    );
    assert_eq!(flow.control.calls.span_or_empty(caller_flow.calls).len(), 1);

    let call_flow = flow.control.calls.span_or_empty(caller_flow.calls)[0].clone();
    let statement_flows = flow
        .control
        .statements
        .span_or_empty(caller_flow.statements);
    assert_eq!(statement_flows.len(), 1);
    assert_eq!(statement_flows[0].statement_index, 0);
    assert_eq!(
        flow.state_statement(caller_flow, 0)
            .expect("statement flow should be queryable")
            .statement_index,
        0
    );
    assert_eq!(
        flow.borrow_state_constraints(statement_flows[0].entry_constraints)
            .collect::<Vec<_>>(),
        vec![caller_borrow_state]
    );
    assert!(
        flow.borrow_lifetimes
            .activations
            .span_or_empty(caller_flow.borrow_activations)
            .is_empty()
    );
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
        borrow.calls.get(caller_borrow_call).accesses.count() as usize
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
fn records_checked_boundary_edges_for_boundary_trait_calls() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(60);
    let caller_state_symbol = SymbolHandle::from_arena_index(61);
    let target_machine_symbol = SymbolHandle::from_arena_index(62);
    let target_state_symbol = SymbolHandle::from_arena_index(63);
    let boundary_trait_symbol = SymbolHandle::from_arena_index(64);
    let boundary_signature_symbol = SymbolHandle::from_arena_index(65);

    let mut program = psi_typed_trees::TypedTrees::default();

    let mut boundary_trait = TraitDefinition {
        symbol: boundary_trait_symbol,
        is_boundary: true,
        name: Identifier::generated("Console"),
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        conformance_bounds: Vec::new(),
        invariants: Default::default(),
        requires: Default::default(),
        machines: Default::default(),
    };
    program.push_trait_machine_signature(
        &mut boundary_trait,
        StateSignature {
            symbol: boundary_signature_symbol,
            name: Identifier::generated("write_line"),
            spelling: None,
            type_parameters: Default::default(),
            is_default: false,
            parameters: Default::default(),
            return_type: Default::default(),
            invokes: Default::default(),
            service_reach_row: Default::default(),
            lifetime_parameters: Vec::new(),
            suspends: false,
            blocks: false,
            contracts: Default::default(),
            termination_guarantee: psi_language_semantics::TerminationGuarantee::NoGuarantee,
        },
    );
    program.push_trait_definition(boundary_trait);

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: Identifier::generated("ConsoleImpl"),
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
    program.push_machine_trait_conformance(
        &mut target_machine,
        TraitConformance {
            symbol: boundary_trait_symbol,
            name: Identifier::generated("Console"),
            requirement: None,
            alias: None,
            external_binding: None,
            ..Default::default()
        },
    );
    program.push_machine_state(
        &mut target_machine,
        State {
            symbol: target_state_symbol,
            name: Identifier::generated("write_line"),
            parameters: Default::default(),
            return_type: Default::default(),
            contracts: Default::default(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

    let call_statement = program
        .statement_table
        .insert(StatementNode::Call(TableCall {
            receiver: HandleSpan::empty(),
            receiver_symbol: target_machine_symbol,
            target: Identifier::generated("write_line"),
            target_symbol: target_state_symbol,
            machine_arguments: Box::default(),
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }));
    let mut caller_machine = Machine {
        symbol: caller_machine_symbol,
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
    program.push_machine_state(
        &mut caller_machine,
        State {
            symbol: caller_state_symbol,
            name: Identifier::generated("main"),
            parameters: Default::default(),
            return_type: Default::default(),
            contracts: Default::default(),
            statement_nodes: HandleSpan::from_parts(call_statement, 1),
        },
    );
    program.push_machine(caller_machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let operations = psi_effects::infer_operational_may(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(
        &program,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );

    let caller_flow = flow
        .control
        .states
        .iter()
        .find_map(|(_, state)| {
            (state.machine_symbol == caller_machine_symbol
                && state.state_symbol == caller_state_symbol)
                .then_some(state)
        })
        .expect("caller flow state");
    let call_flow = flow.control.calls.span_or_empty(caller_flow.calls)[0].clone();

    assert_eq!(flow.boundaries.edges.len(), 1);
    assert_eq!(caller_flow.boundary_edges.count(), 1);
    assert_eq!(call_flow.boundary_edges.count(), 1);
    let boundary = flow
        .boundaries
        .edges
        .span_or_empty(call_flow.boundary_edges)
        .first()
        .expect("boundary edge");
    assert_eq!(boundary.statement_index, 0);
    assert_eq!(boundary.call_ordinal, 0);
    assert_eq!(boundary.receiver_symbol, target_machine_symbol);
    assert_eq!(boundary.target_symbol, target_state_symbol);
    assert_eq!(boundary.boundary_trait_symbol, boundary_trait_symbol);
    assert_eq!(
        boundary.boundary_signature_symbol,
        boundary_signature_symbol
    );

    let service_reaches = psi_effects::infer_service_reaches(&program, &operations);
    let capabilities =
        crate::capabilities::build_capability_facts(&program, &service_reaches, &flow);
    assert_eq!(
        capabilities.count_by_kind(psi_effects::CapabilityFlowKind::Uses),
        1
    );
    let capability = capabilities.flows().next().expect("capability flow");
    assert_eq!(capability.kind, psi_effects::CapabilityFlowKind::Uses);
    assert_eq!(capability.capability_symbol, boundary_trait_symbol);
    assert_eq!(capability.machine_symbol, caller_machine_symbol);
    assert_eq!(capability.state_symbol, caller_state_symbol);
    assert_eq!(capability.statement_index, 0);
    assert_eq!(capability.call_ordinal, 0);
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

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );

    let state_flow = flow
        .control
        .states
        .iter()
        .next()
        .map(|(_, state)| state)
        .unwrap();
    let statement_flows = flow.control.statements.span_or_empty(state_flow.statements);
    assert_eq!(statement_flows.len(), 3);
    let statement_loans: Vec<_> = flow
        .borrow_loan_constraints(statement_flows[1].entry_constraints)
        .collect();
    assert_eq!(statement_loans.len(), 1);
    let call_flow = flow.control.calls.span_or_empty(state_flow.calls)[0].clone();
    let loans: Vec<_> = flow
        .borrow_loan_constraints(call_flow.entry_constraints)
        .collect();
    assert_eq!(loans.len(), 1);
    let activations = flow
        .borrow_lifetimes
        .activations
        .span_or_empty(state_flow.borrow_activations);
    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].loan, loans[0]);
    let loan = borrow.loans.get(loans[0]);
    assert!(loan.owner_symbol.is_valid());
    let weakenings = flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(state_flow.borrow_weakenings);
    assert_eq!(weakenings.len(), 1);
    assert_eq!(weakenings[0].loan, loans[0]);
    assert_eq!(
        weakenings[0].reason,
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit
    );
    assert_eq!(
        weakenings[0].source,
        psi_checked_trees::FlowInvalidationSource::Statement { statement_index: 3 }
    );
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

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );

    let state_flow = flow
        .control
        .states
        .iter()
        .next()
        .map(|(_, state)| state)
        .unwrap();
    let statement_flows = flow.control.statements.span_or_empty(state_flow.statements);
    assert_eq!(statement_flows.len(), 3);
    let statement_loans: Vec<_> = flow
        .borrow_loan_constraints(statement_flows[1].entry_constraints)
        .collect();
    assert_eq!(statement_loans.len(), 1);
    let call_flow = flow
        .control
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
    let activations = flow
        .borrow_lifetimes
        .activations
        .span_or_empty(state_flow.borrow_activations);
    assert_eq!(activations.len(), 1);
    assert_eq!(activations[0].loan, loans[0]);
    let loan = borrow.loans.get(loans[0]);
    assert!(loan.owner_symbol.is_valid());
    let weakenings = flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(state_flow.borrow_weakenings);
    assert_eq!(weakenings.len(), 1);
    assert_eq!(weakenings[0].loan, loans[0]);
    assert_eq!(
        weakenings[0].reason,
        psi_checked_trees::FlowBorrowWeakeningReason::StateExit
    );
    assert_eq!(
        weakenings[0].source,
        psi_checked_trees::FlowInvalidationSource::Statement { statement_index: 3 }
    );
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

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );

    let loan = borrow
        .loans
        .iter()
        .next()
        .map(|(_, loan)| loan)
        .expect("loan");
    assert_eq!(loan.statement_index, 0);
    assert_eq!(loan.last_use_statement_index, 1);

    let state_flow = flow
        .control
        .states
        .iter()
        .next()
        .map(|(_, state)| state)
        .unwrap();
    let call_flows = flow.control.calls.span_or_empty(state_flow.calls);
    assert_eq!(call_flows.len(), 2);

    let first_call_loans: Vec<_> = flow
        .borrow_loan_constraints(call_flows[0].entry_constraints)
        .collect();
    assert_eq!(first_call_loans.len(), 1);

    let second_call_loans: Vec<_> = flow
        .borrow_loan_constraints(call_flows[1].entry_constraints)
        .collect();
    assert!(second_call_loans.is_empty());
    let weakenings = flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(state_flow.borrow_weakenings);
    assert_eq!(weakenings.len(), 1);
    assert_eq!(weakenings[0].loan, first_call_loans[0]);
    assert_eq!(
        weakenings[0].reason,
        psi_checked_trees::FlowBorrowWeakeningReason::LastUseExpired
    );
}

#[test]
fn drops_local_borrow_loans_after_local_reassignment() {
    let source = r#"
        data Main {
            value: i32;
            other: i32;
        }

        machine Main::main(&mut self) {
            let alias: &mut i32 = &mut self.value;
            alias = &mut self.other;
            self.use_value(&mut self.value);
        }

        machine Main::use_value(&mut self, value: &mut i32) {}
    "#;

    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type");
    let proof_plan = psi_proof::obligations::build_proof_plan(&typed);
    let operations = psi_effects::infer_operational_may(&typed);
    let borrow = build_borrow_facts(&typed);
    let proof = build_proof_facts(&typed, &proof_plan, &borrow);
    let mut semantic = build_semantic_facts(&typed, &proof);
    let domains = build_domain_facts(&typed, &semantic);
    let flow = build_flow_facts(
        &typed,
        &borrow,
        &proof,
        &mut semantic,
        &domains,
        &operations,
    );

    let state_flow = flow
        .control
        .states
        .iter()
        .next()
        .map(|(_, state)| state)
        .unwrap();
    let call_flows = flow.control.calls.span_or_empty(state_flow.calls);
    assert_eq!(call_flows.len(), 1);
    let call_loans: Vec<_> = flow
        .borrow_loan_constraints(call_flows[0].entry_constraints)
        .collect();
    assert!(call_loans.is_empty());

    let weakenings = flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(state_flow.borrow_weakenings);
    // Reassignment now creates the replacement loan as well as ending the old
    // one. The unused replacement expires by last-use before the following
    // call; the old source still has the precise reassignment weakening.
    assert_eq!(weakenings.len(), 2);
    assert!(weakenings.iter().any(|weakening| {
        weakening.reason == psi_checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            && weakening.source
                == psi_checked_trees::FlowInvalidationSource::Statement { statement_index: 1 }
    }));
}

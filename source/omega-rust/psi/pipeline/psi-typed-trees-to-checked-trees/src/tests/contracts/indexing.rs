use super::*;

#[test]
fn carries_machine_contract_facts_into_checked_proof_facts() {
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let mut program = psi_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(expression));
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
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
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Crashes {
                cause: psi_typed_trees::signature::CrashCause::Abort,
            },
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 3,
        },
    );
    program.push_machine(machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_fact = facts
        .contract_facts
        .iter()
        .next()
        .map(|(_, fact)| fact)
        .expect("checked proof facts should include the machine contract");

    assert_eq!(
        facts.contract_facts.len(),
        1,
        "crash-route predicates are ceiling guards, not requires/ensures proof facts"
    );
    assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
    assert_eq!(contract_fact.fact, fact);
    assert_eq!(
        contract_fact.owner,
        ContractProofFactOwner::Machine { machine_symbol }
    );
}

#[test]
fn centralizes_contract_facts_in_semantic_fact_plan() {
    let machine_symbol = SymbolHandle::from_arena_index(5);

    let mut program = psi_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(expression));
    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
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
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let semantic = build_semantic_facts(&program, &proof);

    assert_eq!(semantic.facts.len(), 1);
    assert_eq!(semantic.contexts.len(), 1);
    assert_eq!(semantic.symbol_sets.len(), 0);

    let semantic_fact = semantic
        .facts
        .iter()
        .next()
        .map(|(_, fact)| fact)
        .expect("semantic contract fact");
    let psi_facts::FactPlace::Place(place) = semantic_fact.place else {
        panic!("expected canonical contract fact place");
    };
    assert_eq!(
        semantic.places.get(place).root,
        psi_facts::PlaceRoot::Expression(expression)
    );
    assert_eq!(
        semantic_fact.payload,
        psi_facts::FactPayload::ContractBooleanExpression {
            kind: psi_facts::ContractFactKind::Requires,
            fact,
            expression,
            instantiated: psi_arena::Handle::invalid(),
        }
    );
    let context = semantic
        .contexts_at_point(psi_facts::ProgramPoint::Machine { machine_symbol })
        .next()
        .expect("machine contract context");
    assert_eq!(context.boolean_facts().count(), 1);
}

#[test]
fn carries_trait_signature_contract_facts_into_checked_proof_facts() {
    let trait_symbol = SymbolHandle::from_arena_index(5);
    let signature_symbol = SymbolHandle::from_arena_index(6);

    let mut program = psi_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(expression));

    let mut trait_definition = TraitDefinition {
        is_public: false,
        symbol: trait_symbol,
        is_boundary: true,
        name: Identifier::generated("Console"),
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        conformance_bounds: Vec::new(),
        requires: Default::default(),
        machines: Default::default(),
    };
    let mut signature = StateSignature {
        symbol: signature_symbol,
        name: Identifier::generated("write_line"),
        spelling: None,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        is_default: false,
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        invokes: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        termination_guarantee: psi_language_semantics::TerminationGuarantee::NoGuarantee,
    };
    program.push_state_signature_contract(
        &mut signature,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_trait_machine_signature(&mut trait_definition, signature);
    program.push_trait_definition(trait_definition);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_fact = facts
        .contract_facts
        .iter()
        .next()
        .map(|(_, fact)| fact)
        .expect("checked proof facts should include the trait signature contract");

    assert_eq!(facts.contract_facts.len(), 1);
    assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
    assert_eq!(contract_fact.fact, fact);
    assert_eq!(
        contract_fact.owner,
        ContractProofFactOwner::StateSignature {
            owner_symbol: trait_symbol,
            state_symbol: signature_symbol,
        }
    );
}

#[test]
fn indexes_call_contract_facts_by_target_machine() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(5);
    let caller_state_symbol = SymbolHandle::from_arena_index(6);
    let target_machine_symbol = SymbolHandle::from_arena_index(7);
    let target_state_symbol = SymbolHandle::from_arena_index(8);

    let mut program = psi_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(expression));

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: Identifier::generated("Target"),
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
    program.push_machine_contract(
        &mut target_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_machine_state(
        &mut target_machine,
        State {
            symbol: target_state_symbol,
            name: Identifier::generated("run"),
            parameters: Default::default(),
            return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            contracts: Default::default(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

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
    let mut caller_state = State {
        symbol: caller_state_symbol,
        name: Identifier::generated("main"),
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, Identifier::generated("target"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            source_span: psi_source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: Identifier::generated("run"),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            arguments: Default::default(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_call = facts
        .contract_calls
        .iter()
        .next()
        .map(|(_, call)| call)
        .expect("checked proof facts should index the call contract");
    let requires = facts
        .contract_fact_refs
        .span_or_empty(contract_call.requires);

    assert_eq!(facts.contract_calls.len(), 1);
    assert_eq!(contract_call.caller_machine_symbol, caller_machine_symbol);
    assert_eq!(contract_call.caller_state_symbol, caller_state_symbol);
    assert_eq!(contract_call.statement_index, 0);
    assert_eq!(contract_call.call_ordinal, 0);
    assert_eq!(contract_call.target_machine_symbol, target_machine_symbol);
    assert_eq!(contract_call.target_state_symbol, target_state_symbol);
    assert_eq!(requires.len(), 1);
    assert_eq!(facts.contract_facts.get(requires[0].fact).fact, fact);
}

#[test]
fn indexes_inherited_trait_contracts_by_concrete_call_target() {
    let trait_symbol = SymbolHandle::from_arena_index(5);
    let signature_symbol = SymbolHandle::from_arena_index(6);
    let target_machine_symbol = SymbolHandle::from_arena_index(7);
    let target_state_symbol = SymbolHandle::from_arena_index(8);
    let caller_machine_symbol = SymbolHandle::from_arena_index(9);
    let caller_state_symbol = SymbolHandle::from_arena_index(10);

    let mut program = psi_typed_trees::TypedTrees::default();
    let expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(expression));

    let mut trait_definition = TraitDefinition {
        is_public: false,
        symbol: trait_symbol,
        is_boundary: true,
        name: Identifier::generated("Drawable"),
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        conformance_bounds: Vec::new(),
        requires: Default::default(),
        machines: Default::default(),
    };
    let mut signature = StateSignature {
        symbol: signature_symbol,
        name: Identifier::generated("draw"),
        spelling: None,
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        is_default: false,
        parameters: Default::default(),
        return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
        invokes: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        suspends: false,
        suspends_keyword_source_spans: Vec::new(),
        blocks: false,
        blocks_keyword_source_spans: Vec::new(),
        contracts: Default::default(),
        termination_guarantee: psi_language_semantics::TerminationGuarantee::NoGuarantee,
    };
    program.push_state_signature_contract(
        &mut signature,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );
    program.push_trait_machine_signature(&mut trait_definition, signature);
    program.push_trait_definition(trait_definition);

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: Identifier::generated("Sprite"),
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
    program.push_machine_trait_conformance(
        &mut target_machine,
        TraitConformance {
            symbol: trait_symbol,
            name: Identifier::generated("Drawable"),
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
            name: Identifier::generated("draw"),
            parameters: Default::default(),
            return_type: psi_typed_trees::types::TypeReferenceHandle::invalid(),
            contracts: Default::default(),
            statement_nodes: Default::default(),
        },
    );
    program.push_machine(target_machine);

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
    let mut receiver = HandleSpan::empty();
    program
        .statement_table
        .push_name_path_member(&mut receiver, Identifier::generated("sprite"));
    program.statement_table.push_statement(
        &mut caller_state.statement_nodes,
        StatementNode::Call(TableCall {
            source_span: psi_source::SourceSpan::default(),
            authored_call_selection: None,
            receiver_symbol: target_machine_symbol,
            target_symbol: target_state_symbol,
            receiver,
            target: Identifier::generated("draw"),
            static_requirement_dispatch: None,
            machine_arguments: Box::default(),
            arguments: Default::default(),
            evidence_arguments: Box::default(),
            operational_acknowledgement: Default::default(),
            discards_result: false,
        }),
    );
    program.push_machine_state(&mut caller_machine, caller_state);
    program.push_machine(caller_machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let contract_call = facts
        .contract_calls
        .iter()
        .next()
        .map(|(_, call)| call)
        .expect("checked proof facts should index inherited trait contracts");
    let requires = facts
        .contract_fact_refs
        .span_or_empty(contract_call.requires);

    assert_eq!(facts.contract_calls.len(), 1);
    assert_eq!(requires.len(), 1);
    let inherited_fact = facts.contract_facts.get(requires[0].fact);
    assert_eq!(inherited_fact.kind, ContractProofFactKind::Requires);
    assert_eq!(inherited_fact.fact, fact);
    assert_eq!(
        inherited_fact.owner,
        ContractProofFactOwner::MachineState {
            machine_symbol: target_machine_symbol,
            state_symbol: target_state_symbol,
        }
    );
}

#[test]
fn indexes_terminal_state_contract_ensures() {
    let machine_symbol = SymbolHandle::from_arena_index(5);
    let state_symbol = SymbolHandle::from_arena_index(6);

    let mut program = psi_typed_trees::TypedTrees::default();
    let fact_expression = program
        .expression_table
        .insert(psi_typed_trees::expression::ExpressionNode::Boolean(true));
    let fact = program
        .proof_facts
        .append(psi_typed_trees::domain::ProofFact::Expression(
            fact_expression,
        ));
    let return_expression =
        program
            .expression_table
            .insert(psi_typed_trees::expression::ExpressionNode::Integer(
                psi_numerics::literals::IntegerLiteral::from_value(0),
            ));

    let mut machine = Machine {
        symbol: machine_symbol,
        name: Identifier::generated("Main::main"),
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
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(fact, 1),
            token_count: 1,
        },
    );

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
        StatementNode::Expression(return_expression),
    );
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let proof_plan = psi_proof::obligations::build_proof_plan(&program);
    let borrow = build_borrow_facts(&program);
    let facts = build_proof_facts(&program, &proof_plan, &borrow);
    let exit = facts
        .contract_exits
        .iter()
        .next()
        .map(|(_, exit)| exit)
        .expect("checked proof facts should index the exit contract");
    let ensures = facts.contract_fact_refs.span_or_empty(exit.ensures);

    assert_eq!(facts.contract_exits.len(), 1);
    assert_eq!(exit.machine_symbol, machine_symbol);
    assert_eq!(exit.state_symbol, state_symbol);
    assert_eq!(exit.statement_index, 0);
    assert_eq!(ensures.len(), 1);
    assert_eq!(facts.contract_facts.get(ensures[0].fact).fact, fact);
}

#[test]
fn boundary_out_param_ensures_discharges_index_bounds() {
    // R4 witness mint, checker tier: `fw.get_size(&mut self.n)` with
    // `ensures size <= 8` proves `self.buf[self.n]` against length 12.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.buf[self.n] = 7;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("the ensures witness should discharge the index bound");
}

#[test]
fn boundary_out_param_without_ensures_keeps_index_refusal() {
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32);
        }
        data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.buf[self.n] = 7;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("without the ensures the index must stay unproven");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `self.n` is within length 12")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_out_param_ensures_bound_too_wide_keeps_index_refusal() {
    // `ensures size <= 12` admits index 12 into a length-12 buffer -- the
    // witness must not over-prove.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 12;
        }
        data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.buf[self.n] = 7;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a bound admitting the length itself must stay unproven");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `self.n` is within length 12")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_transport_through_transition_arguments() {
    // R4 slice 3: the ensures-bounded value passed as a transition argument
    // carries its bound into the target state's PARAM -- the own_machine
    // shape (map_size flows into the walk state).
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Firmware; buf: [u8; 12]; n: u32; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            transition { _ -> walk(self.n) }
            state walk(&mut self, off: u32) {
                self.buf[off] = 7;
            }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("the transported ensures bound should discharge the param index");
}

#[test]
fn boundary_ensures_transport_poisoned_by_unbounded_edge() {
    // A SECOND edge passing an unbounded value into the same state must
    // poison the merged bound -- the meet is max-over-edges, one unbounded
    // edge kills the fact.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Firmware; buf: [u8; 12]; n: u32; wild: u32; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            transition self.wild == 0 {
                true -> walk(self.n)
                _ -> walk(self.wild)
            }
            state walk(&mut self, off: u32) {
                self.buf[off] = 7;
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an unbounded sibling edge must poison the transported bound");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `off` is within length 12")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_transport_rebind_before_transition_kills_the_fact() {
    // Writing the place between the call and the transition stales the
    // bound; the transported fact must die with it.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Firmware; buf: [u8; 12]; n: u32; wild: u32; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.n = self.wild;
            transition { _ -> walk(self.n) }
            state walk(&mut self, off: u32) {
                self.buf[off] = 7;
            }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("the rebound place must lose the transported bound");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `off` is within length 12")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_discharges_bounded_assignment() {
    // R4 containment: `ensures size <= 8` refolds `self.n + 1` into
    // [1, 9], fitting the [0..=9] target with no guard.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Firmware; n: u32; m: u32 [0..=9]; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.m = self.n + 1;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("the ensures witness should discharge the bounded assignment");
}

#[test]
fn boundary_ensures_witness_wide_bounded_assignment_refuses() {
    // `self.n + 2` reaches 10 > 9 -- the witness must not over-prove.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
        }
        data Main { fw: Firmware; n: u32; m: u32 [0..=9]; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.m = self.n + 2;
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("a fold past the target must refuse");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("satisfies bounded target `self.m`")),
        "expected the containment refusal, got {diagnostics:#?}"
    );
}

#[test]
fn boundary_ensures_witness_survives_unrelated_later_call() {
    // The later resolved boundary call can mutate its receiver (`self.fw`),
    // but its may-write frame is disjoint from `self.n`, so the witness lives.
    let source = r#"
        boundary trait Firmware {
            machine get_size(size: &mut u32)
            ensures size <= 8;
            machine poke();
        }
        data Main { fw: Firmware; n: u32; m: u32 [0..=9]; }
        machine Main::main(&mut self) {
            self.fw.get_size(&mut self.n);
            self.fw.poke();
            self.m = self.n + 1;
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("a disjoint resolved boundary call must preserve the witness");
}

#[test]
fn value_vs_value_guard_transfers_the_range_endpoint() {
    // R1 endpoint mint: `i < k` with `k: u32 [0..=8]` proves `i < 8`.
    let source = r#"
        data Main { buf: [u8; 8]; i: u32; k: u32 [0..=8]; }
        machine Main::main(&mut self) {
            self.k = 4;
            self.i = 2;
            transition self.i < self.k { true -> put() _ -> done() }
            state put(&mut self) {
                self.buf[self.i] = 7;
            }
            state done(&mut self) { }
        }
    "#;
    lower_typed_trees(parse_typed_trees(source))
        .expect("the transferred endpoint should discharge the index");
}

#[test]
fn transitive_guard_survives_disjoint_pure_value_call_frame() {
    let source = r#"
        machine widen(value: u32) -> u64 {
            value as u64
        }

        data Main {
            buf: [u8; 4];
            i: u32;
            scratch: u64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition self.i < 4 { true -> prepare() _ -> done() }

            state prepare(&mut self) {
                self.scratch = widen(self.i);
                transition { _ -> put() }
            }

            state put(&mut self) {
                self.buf[self.i] = 7;
            }

            state done(&mut self) {}
        }
    "#;

    lower_typed_trees(parse_typed_trees(source))
        .expect("a disjoint pure value-call frame should preserve the transitive index guard");
}

#[test]
fn transitive_guard_dies_when_value_call_writes_guarded_place() {
    let source = r#"
        data Main {
            buf: [u8; 4];
            i: u32;
            scratch: u64;
        }

        machine Main::main(&mut self) {
            self.i = 0;
            transition self.i < 4 { true -> prepare() _ -> done() }

            state prepare(&mut self) {
                self.scratch = self.touch_i();
                transition { _ -> put() }
            }

            state touch_i(&mut self) -> u64 {
                self.i = 4;
                0
            }

            state put(&mut self) {
                self.buf[self.i] = 7;
            }

            state done(&mut self) {}
        }
    "#;

    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an overlapping value-call frame must invalidate the transitive guard");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `self.i` is within length 4")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

#[test]
fn value_vs_value_endpoint_one_past_the_region_refuses() {
    // `k: u32 [0..=9]` transfers i <= 8 -- index 8 into length 8 refuses.
    let source = r#"
        data Main { buf: [u8; 8]; i: u32; k: u32 [0..=9]; }
        machine Main::main(&mut self) {
            self.k = 4;
            self.i = 2;
            transition self.i < self.k { true -> put() _ -> done() }
            state put(&mut self) {
                self.buf[self.i] = 7;
            }
            state done(&mut self) { }
        }
    "#;
    let diagnostics = lower_typed_trees(parse_typed_trees(source))
        .expect_err("an endpoint reaching the length must refuse");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove index `self.i` is within length 8")),
        "expected the index refusal, got {diagnostics:#?}"
    );
}

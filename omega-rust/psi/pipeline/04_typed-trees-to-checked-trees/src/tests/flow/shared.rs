use super::super::{
    HandleSpan, Identifier, Machine, SignatureContract, SignatureContractKind, State,
    StateSignature, StatementNode, SymbolHandle, TableCall, TraitConformance, TraitDefinition,
};

use crate::borrow::build_borrow_facts;
use crate::flow::build_domain_facts;
use crate::flow::build_flow_facts;
use crate::proof::build_proof_facts;
use crate::semantic::facts::build_semantic_facts;

#[test]
fn builds_shared_flow_facts_for_state_and_call_sites() {
    let caller_machine_symbol = SymbolHandle::from_arena_index(40);
    let caller_state_symbol = SymbolHandle::from_arena_index(41);
    let callee_machine_symbol = SymbolHandle::from_arena_index(42);
    let callee_state_symbol = SymbolHandle::from_arena_index(43);

    let mut program = typed_trees::TypedTrees::default();
    let contract_expression = program
        .expression_table
        .insert(typed_trees::expression::ExpressionNode::Boolean(true));
    let contract_fact = program
        .proof_facts
        .append(typed_trees::domain::ProofFact::Expression(
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
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        attached_data_application: typed_trees::types::TypeReferenceHandle::invalid(),
        generic_data_template: symbols::SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        structural_type_equations_pending: false,
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
    program.push_machine_contract(
        &mut callee_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::from_parts(contract_fact, 1),
            token_count: 1,
        },
    );
    program.push_machine(callee_machine);

    let call_arguments = HandleSpan::empty();
    let call_statement_receiver = HandleSpan::empty();
    let call_statement = StatementNode::Call(TableCall {
        receiver_root_symbol: caller_machine_symbol,
        source_span: source::SourceSpan::default(),
        authored_call_selection: None,
        receiver: call_statement_receiver,
        receiver_symbol: caller_machine_symbol,
        target: Identifier::generated("run"),
        target_symbol: callee_state_symbol,
        static_machine_parameter: symbols::SymbolHandle::invalid(),
        static_requirement_dispatch: None,
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
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        attached_data_application: typed_trees::types::TypeReferenceHandle::invalid(),
        generic_data_template: symbols::SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        structural_type_equations_pending: false,
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

    let proof_plan = proof::obligations::build_proof_plan(&program);
    let operations = validation::infer_operational_may(&program);
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
        Vec::<arena::Handle<checked_trees::BorrowWritableRootFact>>::new()
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

    let mut program = typed_trees::TypedTrees::default();

    let mut boundary_trait = TraitDefinition {
        is_public: false,
        symbol: boundary_trait_symbol,
        is_boundary: true,
        name: Identifier::generated("Console"),
        lifetime_parameters: Vec::new(),
        type_parameters: Default::default(),
        conformance_bounds: Vec::new(),
        requires: Default::default(),
        machines: Default::default(),
        refines: None,
        refinement_clauses: Vec::new(),
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
            native_callback_parameters: Vec::new(),
            return_type: Default::default(),
            invokes: Default::default(),
            service_reach_row: Default::default(),
            service_reach_is_installation_bound: false,
            lifetime_parameters: Vec::new(),
            suspends: false,
            suspends_keyword_source_spans: Vec::new(),
            blocks: false,
            blocks_keyword_source_spans: Vec::new(),
            contracts: Default::default(),
            termination_guarantee: language_semantics::TerminationGuarantee::NoGuarantee,
            where_facts: Default::default(),
        },
    );
    program.push_trait_definition(boundary_trait);

    let mut target_machine = Machine {
        symbol: target_machine_symbol,
        name: Identifier::generated("ConsoleImpl"),
        attached_data: None,
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        attached_data_application: typed_trees::types::TypeReferenceHandle::invalid(),
        generic_data_template: symbols::SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        structural_type_equations_pending: false,
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
            receiver_root_symbol: SymbolHandle::invalid(),
            source_span: source::SourceSpan::default(),
            authored_call_selection: None,
            receiver: HandleSpan::empty(),
            receiver_symbol: target_machine_symbol,
            target: Identifier::generated("write_line"),
            target_symbol: target_state_symbol,
            static_machine_parameter: symbols::SymbolHandle::invalid(),
            static_requirement_dispatch: None,
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
        attached_data_symbol: symbols::SymbolHandle::invalid(),
        attached_data_application: typed_trees::types::TypeReferenceHandle::invalid(),
        generic_data_template: symbols::SymbolHandle::invalid(),
        spelling: None,
        is_public: false,
        supply_mode: Default::default(),
        target: None,
        termination_plan: Default::default(),
        service_reach_row: Default::default(),
        service_reach_is_installation_bound: false,
        body_is_present: true,
        structural_type_equations_pending: false,
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

    let proof_plan = proof::obligations::build_proof_plan(&program);
    let operations = validation::infer_operational_may(&program);
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

    let service_reaches = validation::infer_service_reaches(&program, &operations);
    let capabilities =
        crate::facts::capabilities::build_capability_facts(&program, &service_reaches, &flow);
    assert_eq!(
        capabilities.count_by_kind(flow_effects::CapabilityFlowKind::Uses),
        1
    );
    let capability = capabilities.flows().next().expect("capability flow");
    assert_eq!(capability.kind, flow_effects::CapabilityFlowKind::Uses);
    assert_eq!(capability.capability_symbol, boundary_trait_symbol);
    assert_eq!(capability.machine_symbol, caller_machine_symbol);
    assert_eq!(capability.state_symbol, caller_state_symbol);
    assert_eq!(capability.statement_index, 0);
    assert_eq!(capability.call_ordinal, 0);
}
